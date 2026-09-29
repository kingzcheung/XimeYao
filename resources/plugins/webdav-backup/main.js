// =====================================================================
// WebDAV 云备份插件（QuickJS 脚本，xime 3.0 契约）
//
// 备份包的生成与恢复由宿主完成，本插件只承载 WebDAV 传输协议：
// PUT / GET / PROPFIND / DELETE / MKCOL，认证用 Basic Auth
// （host.crypto.base64），配置经 host.config 存取。
//
// 备份条目 id = 服务器上的绝对路径（已解码），pull/delete 时原样回传。
//
// JS 约定（对齐 QuickJS 沙箱）：
//   - 二进制一律 Uint8Array；host.crypto.base64 / host.http.request 的 body
//     只接受字节，字符串先经 TextEncoder 转 UTF-8。
//   - JSON 用原生 JSON（宿主不提供 host.json）。
//   - 源码取自 Xime 仓库 plugins/webdav-backup（main.ts + libs/，
//     rolldown 构建时内联为单文件）；此文件为等价的内联单文件形态。
// =====================================================================

// ------------------------------------------------------------------
// libs/url-codec（内联）
// ------------------------------------------------------------------

// 路径编码：逐字节循环（保留字母数字与 -._~:/，其余转 %XX）。
// 中文等多字节字符必须逐 UTF-8 字节转义（逐字符编码会漏首字节导致 400）。
function encodePath(path) {
  var bytes = new TextEncoder().encode(path);
  var out = '';
  for (var i = 0; i < bytes.length; i++) {
    var b = bytes[i];
    if ((b >= 48 && b <= 57) || (b >= 65 && b <= 90) || (b >= 97 && b <= 122)
      || b === 45 || b === 46 || b === 95 || b === 126 || b === 47 || b === 58) {
      out += String.fromCharCode(b);
    } else {
      out += '%' + (b < 16 ? '0' : '') + b.toString(16).toUpperCase();
    }
  }
  return out;
}

function isHexDigit(ch) {
  return (ch >= '0' && ch <= '9') || (ch >= 'a' && ch <= 'f') || (ch >= 'A' && ch <= 'F');
}

// %XX 解码：只解码合法十六进制对，其余字符原样按 UTF-8 字节拼回
// （非法转义不抛异常）。
function decodeSegment(segment) {
  var bytes = [];
  for (var i = 0; i < segment.length; i++) {
    var c = segment.charCodeAt(i);
    if (c === 0x25 && i + 2 < segment.length
      && isHexDigit(segment.charAt(i + 1)) && isHexDigit(segment.charAt(i + 2))) {
      bytes.push(parseInt(segment.slice(i + 1, i + 3), 16));
      i += 2;
    } else {
      var encoded = new TextEncoder().encode(segment.charAt(i));
      for (var j = 0; j < encoded.length; j++) bytes.push(encoded[j]);
    }
  }
  return new TextDecoder().decode(new Uint8Array(bytes));
}

// href / id → 服务器绝对路径（解码后的）
function hrefToPath(href) {
  if (!href) return null;
  var m = href.match(/^https?:\/\/[^/]+(\/.*)$/);
  var p = m ? m[1] : href;
  if (p.charAt(0) !== '/') p = '/' + p;
  var decoded = [];
  var segs = p.split('/');
  for (var i = 0; i < segs.length; i++) {
    if (segs[i] === '') continue;
    decoded.push(decodeSegment(segs[i]));
  }
  return '/' + decoded.join('/');
}

// ------------------------------------------------------------------
// libs/webdav-xml（内联）
// ------------------------------------------------------------------

var MONTHS = {
  Jan: 1, Feb: 2, Mar: 3, Apr: 4, May: 5, Jun: 6,
  Jul: 7, Aug: 8, Sep: 9, Oct: 10, Nov: 11, Dec: 12,
};

// Howard Hinnant days_from_civil（公历日期 → 自 1970-01-01 的天数）
function epochFromParts(y, m, d, hh, mm, ss) {
  if (!y || !m || !d) return 0;
  var yy = y;
  if (m <= 2) yy = yy - 1;
  var era = Math.floor(yy / 400);
  var yoe = yy - era * 400;
  var mp = m + (m > 2 ? -3 : 9);
  var doy = Math.floor((153 * mp + 2) / 5) + d - 1;
  var doe = yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy;
  var days = era * 146097 + doe - 719468;
  return days * 86400 + (hh || 0) * 3600 + (mm || 0) * 60 + (ss || 0);
}

// 优先 creationdate（ISO 8601），其次 getlastmodified（RFC 1123）
function parseTime(block) {
  var m = block.match(/<[dD]:?creationdate>\s*(\d{4})-(\d{2})-(\d{2})T(\d+):(\d+):(\d+)/);
  if (m) {
    return epochFromParts(
      parseInt(m[1], 10), parseInt(m[2], 10), parseInt(m[3], 10),
      parseInt(m[4], 10), parseInt(m[5], 10), parseInt(m[6], 10)
    );
  }
  var g = block.match(
    /<[dD]:?getlastmodified>\s*\w+,\s*(\d+)\s+(\w+)\s+(\d+)\s+(\d+):(\d+):(\d+)/);
  if (g && MONTHS[g[2]]) {
    return epochFromParts(
      parseInt(g[3], 10), MONTHS[g[2]], parseInt(g[1], 10),
      parseInt(g[4], 10), parseInt(g[5], 10), parseInt(g[6], 10)
    );
  }
  return 0;
}

// 大小写不敏感地取子标签文本（命名空间前缀 d:/D:/无 均可；开标签允许携带属性）
function tagValue(block, tag) {
  var re = new RegExp('<[dD]:?' + tag + '[^>]*>\\s*([\\s\\S]*?)\\s*</[dD]:?' + tag + '>');
  var m = block.match(re);
  return m ? m[1] : null;
}

// 对齐 `tonumber(x) or -1`：非法/缺失内容统一为 -1
function parseLength(raw) {
  if (raw === null || raw.trim() === '') return -1;
  var n = Number(raw.trim());
  return Number.isFinite(n) ? n : -1;
}

// 解析 PROPFIND Depth:1 响应，返回 basePath 下的文件条目（不含目录），
// 按 createdAt 降序（最新在前）。无法解析的响应返回空数组。
function parseBackupList(xml, basePath) {
  var basePrefix = basePath + '/';
  var items = [];
  var blockRe = /<[dD]:?response[^>]*>([\s\S]*?)<\/[dD]:?response>/g;
  var bm;
  while ((bm = blockRe.exec(xml)) !== null) {
    var block = bm[1];
    var p = hrefToPath(tagValue(block, 'href'));
    if (p === null) continue;
    var isDir = p.slice(-1) === '/' || /<d?:?collection/.test(block.toLowerCase());
    if (isDir) continue;
    if (p.substring(0, basePrefix.length) !== basePrefix) continue;
    var name = p.substring(basePrefix.length);
    if (name === '' || name.indexOf('/') !== -1) continue;
    items.push({
      id: p,
      name: name,
      createdAt: parseTime(block),
      size: parseLength(tagValue(block, 'getcontentlength')),
    });
  }
  items.sort(function (a, b) { return b.createdAt - a.createdAt; });
  return items;
}

// ------------------------------------------------------------------
// 配置
// ------------------------------------------------------------------

function getConfig() {
  return {
    url: host.config.get('url') || '',
    username: host.config.get('username') || '',
    password: host.config.get('password') || '',
    remotePath: host.config.get('remote_path') || '/xime_backup',
  };
}

// 归一化：url 去尾斜杠，remotePath 去首尾斜杠。
// 返回：
//   base     完整 URL（origin + url 路径前缀 + remotePath），PROPFIND/PUT 用
//   basePath 服务器绝对路径（如坚果云为 /dav/xime_backup），条目 id / 列表过滤用
//   davRoot  origin + url 路径前缀（如 https://dav.jianguoyun.com/dav），MKCOL 起点用
//   origin   scheme + host（pull/delete 用：条目 id 已是含路径前缀的绝对路径）
//   headers  认证头
function resolveBase() {
  var cfg = getConfig();
  var result = {
    origin: '', base: '', basePath: '', davRoot: '', headers: {}, cfg: cfg, error: null,
  };
  if (cfg.url === '') { result.error = '请先填写服务器地址'; return result; }
  if (cfg.username === '') { result.error = '请先填写账号'; return result; }
  var url = cfg.url;
  if (!/^https?:\/\//.test(url)) url = 'https://' + url;
  url = url.replace(/\/+$/, '');
  var m = url.match(/^https?:\/\/[^/]+/);
  var origin = m ? m[0] : url;
  var urlPath = url.substring(origin.length); // DAV 根的路径前缀（坚果云为 /dav）
  var remote = cfg.remotePath.replace(/^\/+/, '').replace(/\/+$/, '');
  result.origin = origin;
  result.base = origin + urlPath + '/' + remote;
  result.basePath = urlPath + '/' + remote;
  result.davRoot = origin + urlPath;
  if (cfg.password !== '') {
    var b64 = host.crypto.base64(new TextEncoder().encode(cfg.username + ':' + cfg.password));
    if (!b64) { result.error = 'Basic 认证编码失败'; return result; }
    result.headers['Authorization'] = 'Basic ' + b64;
  }
  return result;
}

// ------------------------------------------------------------------
// WebDAV 操作
// ------------------------------------------------------------------

// 逐级 MKCOL 创建远端目录（起点为 DAV 根，即 url 的路径前缀；已存在 405 视为成功）
async function ensureCollection(cfg, headers) {
  var davRoot = resolveBase().davRoot;
  if (!davRoot) return false;
  var remote = cfg.remotePath.replace(/^\/+/, '').replace(/\/+$/, '');
  var cur = davRoot;
  var segs = remote.split('/').filter(function (s) { return s !== ''; });
  for (var i = 0; i < segs.length; i++) {
    cur = cur + '/' + encodePath(segs[i]);
    try {
      var res = await host.http.request('MKCOL', cur, headers, null);
      if (res.status !== 201 && res.status !== 405 && res.status !== 200 && res.status !== 301) {
        return false;
      }
    } catch (e) {
      host.logError('MKCOL 失败: ' + cur + ' ' + ((e && e.message) || ''));
      return false;
    }
  }
  return true;
}

// 从失败响应提取可读原因（去 XML 标签、截断），拼进错误消息给设置页展示
function statusDetail(res) {
  var body = res.text || '';
  body = body.replace(/<[^>]*>/g, ' ').replace(/\s+/g, ' ').trim();
  if (body === '') return '';
  return ': ' + body.substring(0, 160);
}

// 仅供宿主回归测试使用；非插件契约字段，经 spread 并入
var testHooks = { _encodePath: encodePath };

var plugin = definePlugin({
  // ------------------------------------------------------------------
  // backup 扩展点（宿主调用路径 backup.*）
  // ------------------------------------------------------------------

  backup: {
    // 连接测试；返回错误消息，null 表示成功
    async test() {
      var r = resolveBase();
      var base = r.base, headers = r.headers, cfg = r.cfg, error = r.error;
      if (error) return error;
      headers['Depth'] = '0';
      try {
        var res = await host.http.request('PROPFIND', encodePath(base), headers, null);
        if (res.status === 207 || res.status === 200) return null;
        if (res.status === 401) return '认证失败（401），请检查账号与密码';
        if (res.status === 404) {
          // 目录不存在不算失败（首次备份会自动创建）
          if (await ensureCollection(cfg, headers)) return null;
          return '远端目录不存在且自动创建失败';
        }
        return '服务器返回 HTTP ' + res.status + statusDetail(res);
      } catch (e) {
        return (e && e.message) || '请求失败';
      }
    },

    // 上传备份包（args = {name, archive: Uint8Array}）
    async push(args) {
      var name = (args && args.name) || '';
      var archive = args && args.archive;
      if (name === '') return { ok: false, message: '备份包名为空' };
      if (!archive || archive.length === 0) {
        return { ok: false, message: '备份包为空' };
      }

      var r = resolveBase();
      var base = r.base, basePath = r.basePath, headers = r.headers, cfg = r.cfg, error = r.error;
      if (error) return { ok: false, message: error };

      headers['Content-Type'] = 'application/octet-stream';
      headers['Overwrite'] = 'T';
      var url = encodePath(base) + '/' + encodePath(name);

      try {
        var res = await host.http.request('PUT', url, headers, archive);
        if (res.status === 409 || res.status === 404) {
          // 父目录不存在：逐级创建后重试一次
          // （标准 DAV 报 409 Conflict；Alist/Nextcloud 等报 404）
          if (!(await ensureCollection(cfg, headers))) {
            return { ok: false, message: '创建远端目录失败' };
          }
          res = await host.http.request('PUT', url, headers, archive);
        }
        if (res.status === 200 || res.status === 201 || res.status === 204) {
          return { ok: true, id: basePath + '/' + name };
        }
        if (res.status === 401) return { ok: false, message: '认证失败（401），请检查账号与密码' };
        return { ok: false, message: '上传失败（HTTP ' + res.status + '）' + statusDetail(res) };
      } catch (e) {
        return { ok: false, message: (e && e.message) || '上传请求失败' };
      }
    },

    // 下载备份包（id 为 list 返回的远端条目 id）
    async pull(id) {
      if (!id || id === '') return null;
      var r = resolveBase();
      if (r.error) return null;
      try {
        var res = await host.http.request('GET', r.origin + encodePath(id), r.headers, null);
        if (res.status !== 200) return null;
        return res.body;
      } catch (e) {
        host.logError('pull 失败: ' + ((e && e.message) || ''));
        return null;
      }
    },

    // 列出远端备份
    async list() {
      var r = resolveBase();
      if (r.error) return null;
      r.headers['Depth'] = '1';
      try {
        var res = await host.http.request('PROPFIND', encodePath(r.base), r.headers, null);
        if (res.status !== 207 && res.status !== 200) return null;
        return parseBackupList(res.text || '', r.basePath);
      } catch (e) {
        host.logError('list 失败: ' + ((e && e.message) || ''));
        return null;
      }
    },

    // 删除远端备份
    async remove(id) {
      if (!id || id === '') return false;
      var r = resolveBase();
      if (r.error) return false;
      try {
        var res = await host.http.request('DELETE', r.origin + encodePath(id), r.headers, null);
        // 404：远端已不存在，视为删除成功
        return res.status === 204 || res.status === 200 || res.status === 404;
      } catch (e) {
        host.logError('remove 失败: ' + ((e && e.message) || ''));
        return false;
      }
    },
  },

  // ------------------------------------------------------------------
  // 配置表单（UiNode 契约）
  // ------------------------------------------------------------------

  settings: {
    schema() {
      return [
        {
          type: 'text', key: 'url', label: '服务器地址', required: true,
          placeholder: 'https://dav.jianguoyun.com/dav/',
          helpText: 'WebDAV 根地址；坚果云为 https://dav.jianguoyun.com/dav/',
        },
        { type: 'text', key: 'username', label: '账号', required: true },
        {
          type: 'secret', key: 'password', label: '密码 / 应用密码', required: true,
          helpText: '坚果云请到网页端「安全选项」生成应用密码',
        },
        {
          type: 'text', key: 'remote_path', label: '备份目录',
          defaultValue: '/xime_backup',
          helpText: '远端目录，不存在时自动创建',
        },
        { type: 'button', key: 'testConnection', label: '测试连接' },
      ];
    },
  },

});

// 导出（IIFE 挂到 globalThis.plugin，契约约定）
globalThis.plugin = plugin;
