// =====================================================================
// ximed WebDAV 剪贴板同步插件（QuickJS 脚本，xime 3.0 契约）
//
// 对接 ximed 的 WebDAV 直连适配器：通过 WebDAV 服务器上的单个 JSON 文件
// 实现剪贴板双向同步。
//
// 职责划分：
//   插件  = 协议逻辑（WebDAV PUT/GET/MKCOL、Basic Auth、ETag 条件拉取、Profile JSON）
//   宿主  = 同步引擎（轮询/去重/回声抑制）+ 通用原语：
//     host.http   请求原语（PUT/GET/PROPFIND/MKCOL，async await，域名经用户授权）
//     host.config 配置存储（含 ETag 缓存）
//
// WebDAV 交互模型（对齐 ximed WebDavTransport 的 JSON Profile 协议）：
//   文件   {davUrl}/{remotePath}/clipboard/current.json
//   附件   {davUrl}/{remotePath}/clipboard/blobs/{data_name}（图片原图）
//   push   PUT 文件，body = Profile JSON（Content-Type: application/json）；
//          有附件时先 PUT blob、再 PUT JSON
//   pull   GET 文件，If-None-Match（ETag 缓存）；304/404 → 无变更
//   兼容   远端若为旧版纯文本（无 JSON 结构），按纯文本 profile 处理
//   认证   Authorization: Basic base64(user:pass)
//
// JS 约定：JSON 用原生 JSON（宿主不提供 host.json）；body/base64 一律 Uint8Array。
// 源码取自 Xime 仓库 plugins/webdav-clipboard-sync（main.ts + libs/，
// rolldown 构建时内联为单文件）；此文件为等价的内联单文件形态。
// =====================================================================

// ------------------------------------------------------------------
// libs/dav-url（内联）
// ------------------------------------------------------------------

// 剪贴板文件名（相对 remotePath）
var CLIPBOARD_FILE = 'clipboard/current.json';

// 附件目录名（相对 clipboard 目录）
var BLOB_DIR = 'blobs';

// 附件名白名单：内容寻址名（hex + 扩展名）只含 [A-Za-z0-9._-]，顺带挡掉路径穿越
var BLOB_NAME_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._-]*$/;

// 去掉末尾斜杠（url 归一化）
function stripTrailingSlashes(value) {
  return value.replace(/\/+$/, '');
}

// 去掉首尾斜杠（remotePath 归一化）
function stripSlashes(value) {
  return value.replace(/^\/+/, '').replace(/\/+$/, '');
}

// 远端剪贴板文件 URL：{davUrl}/{remotePath}/clipboard/current.json
// davUrl 为空返回 null（未配置服务器地址）
function buildFileUrl(davUrl, remotePath) {
  var base = stripTrailingSlashes(davUrl);
  if (base === '') return null;
  var path = stripSlashes(remotePath);
  if (path !== '') base = base + '/' + path;
  return base + '/' + CLIPBOARD_FILE;
}

// 远端剪贴板目录 URL（{davUrl}/{remotePath}/clipboard，用于连接测试的 PROPFIND 探测）
function buildDirUrl(fileUrl) {
  if (fileUrl === null) return null;
  return fileUrl.replace(/\/[^/]+$/, '');
}

// 远端附件 URL：{davUrl}/{remotePath}/clipboard/blobs/{dataName}
// dataName 由宿主生成（`<sha256>.<ext>`），但它是远端/宿主传入的字符串——
// 一律先过白名单再拼路径（拒绝空名、路径分隔符与 `..`），绝不直接拼接。
function buildBlobUrl(davUrl, remotePath, dataName) {
  if (dataName === null || dataName === undefined) return null;
  var name = dataName.trim();
  if (!BLOB_NAME_PATTERN.test(name)) return null;
  var clipboardFile = buildFileUrl(davUrl, remotePath);
  if (clipboardFile === null) return null;
  // 复用 current.json 的目录，避免"目录怎么拼"的规则写两份
  var clipboardDir = clipboardFile.substring(0, clipboardFile.lastIndexOf('/'));
  return clipboardDir + '/' + BLOB_DIR + '/' + name;
}

// 从文件 URL 里解析出相对 davUrl 的目录层级（不含文件名），供逐级 MKCOL 创建。
// 从用户配置的 davUrl（通常已存在）之后开始创建，避免 MKCOL 服务器根/WebDAV 根被拒。
// 纯字符串前缀匹配，避免正则中 . - + 等特殊字符或前缀误匹配。
function relativeDirParts(baseUrl, fileUrl) {
  var basePath = stripTrailingSlashes(baseUrl.replace(/^https?:\/\/[^/]+/, ''));
  var dirPart = fileUrl.replace(/^https?:\/\/[^/]+/, '').replace(/\/[^/]+$/, '');
  if (basePath !== '' && dirPart.substring(0, basePath.length) === basePath) {
    dirPart = dirPart.substring(basePath.length);
  }
  return dirPart.replace(/^\/+/, '').split('/').filter(function (part) { return part !== ''; });
}

// ------------------------------------------------------------------
// 常量与工具函数
// ------------------------------------------------------------------

var KEY_DAV_URL = 'davUrl';
var KEY_REMOTE_PATH = 'remotePath';
var KEY_USERNAME = 'username';
var KEY_PASSWORD = 'password';
var KEY_LAST_ETAG = 'lastEtag';

// 503 限流退避（沙箱无时钟/定时器，用调用计数近似时间窗口）：
// 坚果云等 WebDAV 服务限流（免费版每 30 分钟限 600 次请求，超限返回 503）
// 后需等待解封，解封前继续请求会延长封禁窗口。
// pull 经宿主节流后约 30s 一次，跳 20 次 ≈ 10 分钟退避；
// push 由剪贴板变化驱动（频率不定），跳 5 次后放行一次探测，仍 503 再退。
var PULL_BACKOFF_SKIPS = 20;
var PUSH_BACKOFF_SKIPS = 5;
var pullSkipCount = 0;
var pushSkipCount = 0;

// 去除 \r\n：用户名/密码可能因配置或粘贴带入换行，若不清除会进入 base64 凭据，
// 导致部分不对换行做特殊处理的 WebDAV 服务器认证失败（典型：凭据末尾多一个 \n）。
function stripNewlines(value) {
  return value.replace(/[\r\n]/g, '');
}

function basicAuthHeader(username, password) {
  var credential = new TextEncoder().encode(stripNewlines(username) + ':' + stripNewlines(password));
  // base64 结果同样清理换行，保证 Authorization 请求头是单行值
  return 'Basic ' + stripNewlines(host.crypto.base64(credential));
}

function buildHeaders() {
  var headers = {};
  var username = host.config.get(KEY_USERNAME) || '';
  var password = host.config.get(KEY_PASSWORD) || '';
  if (username !== '') {
    headers['Authorization'] = basicAuthHeader(username, password);
  }
  return headers;
}

// 远端剪贴板文件 URL：{davUrl}/{remotePath}/clipboard/current.json
function fileUrl() {
  return buildFileUrl(host.config.get(KEY_DAV_URL) || '', host.config.get(KEY_REMOTE_PATH) || '');
}

// 远端剪贴板目录 URL（{davUrl}/{remotePath}/clipboard，用于连接测试的 PROPFIND 探测）
function remoteDirUrl() {
  return buildDirUrl(fileUrl());
}

// 从文件 URL 里解析出目录层级（不含文件名），逐级 MKCOL 创建。
// 从用户配置的 davUrl（通常已存在）之后开始创建，避免 MKCOL 服务器根/WebDAV 根被拒。
// 已存在（405/201/重定向）视为成功，权限不足/网络失败返回 false。
async function ensureDirectories(url) {
  var base = stripTrailingSlashes(host.config.get(KEY_DAV_URL) || '');
  if (base === '') return false;
  var current = base;
  var parts = relativeDirParts(base, url);
  for (var i = 0; i < parts.length; i++) {
    current = current + '/' + parts[i];
    try {
      var resp = await host.http.request('MKCOL', current, buildHeaders(), null);
      var s = resp.status;
      if (s >= 200 && s < 300) {
        // 已创建（201）或已存在（200）
      } else if (s === 405 || s === 301 || s === 302 || s === 307 || s === 308) {
        // 已存在 / 重定向：视为目录可用
      } else {
        host.logError('MKCOL 失败: ' + current + ' -> HTTP ' + s);
        return false;
      }
    } catch (e) {
      host.logError('MKCOL 失败（网络/拒绝）: ' + current + ' ' + ((e && e.message) || ''));
      return false;
    }
  }
  return true;
}

function cacheEtag(resp) {
  if (resp === null || resp === undefined || !resp.headers) return;
  var etag = resp.headers['ETag'] || resp.headers['etag'];
  if (etag !== null && etag !== undefined && etag !== '') {
    host.config.set(KEY_LAST_ETAG, etag);
  }
}

// ------------------------------------------------------------------
// 附件（图片原图）
// ------------------------------------------------------------------

// 扩展名 → Content-Type（图片原图直传，不压缩；未知类型退化为二进制流）。
var EXT_CONTENT_TYPE = {
  png: 'image/png',
  jpg: 'image/jpeg',
  jpeg: 'image/jpeg',
  gif: 'image/gif',
  webp: 'image/webp',
  bmp: 'image/bmp',
  heic: 'image/heic',
  heif: 'image/heic',
  avif: 'image/avif',
};

function contentTypeOf(dataName) {
  if (dataName === null || dataName === undefined) return 'application/octet-stream';
  var dot = dataName.lastIndexOf('.');
  if (dot < 0) return 'application/octet-stream';
  return EXT_CONTENT_TYPE[dataName.substring(dot + 1).toLowerCase()] || 'application/octet-stream';
}

// 上传附件字节到 {clipboard}/blobs/{data_name}。
// blob 名即内容 hash ⇒ 重复 PUT 幂等，不做 HEAD 预检（省一次请求，避开 503 限流）。
// 返回 true 才允许写 current.json：绝不留下"JSON 指向不存在 blob"的悬空引用。
async function putBlob(profile) {
  var data = profile.data;
  if (data === null || data === undefined || data.length === 0) {
    host.logError('push failed: has_data 为真但没有附件字节');
    return false;
  }
  var url = buildBlobUrl(
    host.config.get(KEY_DAV_URL) || '',
    host.config.get(KEY_REMOTE_PATH) || '',
    profile.dataName
  );
  if (url === null) {
    host.logError('push failed: 附件名非法或未配置服务器地址');
    return false;
  }
  var headers = buildHeaders();
  headers['Content-Type'] = contentTypeOf(profile.dataName);
  var resp;
  try {
    resp = await host.http.request('PUT', url, headers, data);
  } catch (e) {
    host.logError('push failed: 附件上传请求失败 ' + ((e && e.message) || ''));
    return false;
  }
  if (resp.status === 503) {
    pushSkipCount = PUSH_BACKOFF_SKIPS;
    host.logError('push failed: 附件上传 HTTP 503 服务器限流，跳过接下来 ' + PUSH_BACKOFF_SKIPS + ' 次推送');
    return false;
  }
  if (resp.status >= 200 && resp.status < 300) {
    host.log('push ok: PUT blob ' + url + ' -> ' + resp.status + ' (' + data.length + ' bytes)');
    return true;
  }
  // 目录不存在（409 / 部分服务器 404）→ 逐级 MKCOL 后重试一次
  if (resp.status === 409 || resp.status === 404) {
    host.log('push: blob ' + resp.status + ' 目录不存在，尝试 MKCOL 创建');
    if (await ensureDirectories(url)) {
      var retry = await host.http.request('PUT', url, headers, data);
      if (retry.status >= 200 && retry.status < 300) {
        host.log('push ok: blob MKCOL 后重试成功 -> ' + retry.status);
        return true;
      }
      host.logError('push failed: blob MKCOL 后重试失败 HTTP ' + retry.status);
      return false;
    }
    host.logError('push failed: blob 目录创建失败');
    return false;
  }
  host.logError('push failed: PUT blob ' + url + ' -> HTTP ' + resp.status);
  return false;
}

// 下载附件字节。必须用 resp.body：resp.text 会把二进制按 UTF-8 解码，
// 图片字节会被替换字符破坏（且不可逆）。
async function fetchBlob(profile) {
  var url = buildBlobUrl(
    host.config.get(KEY_DAV_URL) || '',
    host.config.get(KEY_REMOTE_PATH) || '',
    profile.dataName
  );
  if (url === null) {
    host.logError('pull failed: 附件名非法或未配置服务器地址');
    return null;
  }
  var resp;
  try {
    resp = await host.http.request('GET', url, buildHeaders(), null);
  } catch (e) {
    host.logError('pull failed: 附件下载请求失败 ' + ((e && e.message) || ''));
    return null;
  }
  if (resp.status === 503) {
    pullSkipCount = PULL_BACKOFF_SKIPS;
    host.logError('pull failed: 附件下载 HTTP 503 服务器限流，跳过接下来 ' + PULL_BACKOFF_SKIPS + ' 次拉取');
    return null;
  }
  if (resp.status < 200 || resp.status >= 300) {
    host.logError('pull failed: GET blob ' + url + ' -> HTTP ' + resp.status);
    return null;
  }
  var body = resp.body;
  if (body === null || body === undefined || body.length === 0) {
    host.logError('pull failed: 远端附件为空');
    return null;
  }
  return body;
}

// ------------------------------------------------------------------
// 插件本体
// ------------------------------------------------------------------

var plugin = definePlugin({
  // ================= 生命周期 =================

  onLoad() {
  },

  onUnload() {
  },

  // ================= 配置 schema（与 manifest 一致） =================

  settings: {
    schema() {
      return [
        {
          key: KEY_DAV_URL,
          label: 'WebDAV 服务器',
          type: 'text',
          placeholder: 'https://host:port/dav/',
          helpText: 'WebDAV 服务地址（形如 https://192.168.1.50:8080/dav/）',
        },
        {
          key: KEY_REMOTE_PATH,
          label: '远程目录',
          type: 'text',
          placeholder: 'xime',
          required: false,
          helpText: '剪贴板文件所在目录（留空为根目录，如 xime 对应 dav/xime/clipboard/current.json）',
        },
        {
          key: KEY_USERNAME,
          label: '用户名',
          type: 'text',
          required: false,
        },
        {
          key: KEY_PASSWORD,
          label: '密码',
          type: 'secret',
          required: false,
        },
        {
          // 拉取间隔由宿主引擎消费（轮询节流），插件自身不读取
          key: 'pull_interval_seconds',
          label: '拉取最小间隔（秒）',
          type: 'number',
          defaultValue: '30',
          placeholder: '30',
          helpText: '宿主轮询远端的最小间隔，范围 1~600，默认 30。' +
            '坚果云免费版每 30 分钟限 600 次请求，调小易触发 503 限流',
          required: false,
        },
        {
          key: 'testConnection',
          label: '测试连接',
          type: 'button',
          required: false,
        },
      ];
    },
  },

  // ================= 同步接口（宿主调用路径 clipboardSync.*） =================

  clipboardSync: {
    // 推送本地 profile 到远端 WebDAV 文件（宿主剪贴板变化时调用）
    async push(profile) {
      if (pushSkipCount > 0) {
        pushSkipCount = pushSkipCount - 1;
        host.log('push: 限流退避中，跳过（剩余 ' + pushSkipCount + ' 次）');
        return false;
      }
      var url = fileUrl();
      if (url === null) {
        host.logError('push failed: 未配置服务器地址');
        return false;
      }
      // 附件先行：blob 没写成功就绝不写 JSON（否则对端会拉到"有元数据、无图"的空条目）
      if (profile.hasData) {
        var uploaded = await putBlob(profile);
        if (!uploaded) {
          host.logError('push failed: 附件未上传成功，跳过 Profile JSON');
          return false;
        }
      }
      var headers = buildHeaders();
      headers['Content-Type'] = 'application/json';
      var body;
      try {
        // wire 协议为 snake_case（与 ximed Profile 同构），SDK profile 为 camelCase：显式映射
        body = JSON.stringify({
          type: profile.type,
          hash: profile.hash,
          text: profile.text,
          has_data: profile.hasData,
          data_name: profile.dataName,
          size: profile.size,
          source: profile.source,
        });
      } catch (e) {
        body = undefined;
      }
      if (body === undefined || body === null) {
        host.logError('push failed: Profile JSON 序列化失败');
        return false;
      }
      var payload = new TextEncoder().encode(body);
      try {
        var resp = await host.http.request('PUT', url, headers, payload);
        if (resp.status === 503) {
          // 服务器限流：进入退避，解封前不再发请求（避免延长封禁）
          pushSkipCount = PUSH_BACKOFF_SKIPS;
          host.logError('push failed: HTTP 503 服务器限流，跳过接下来 ' + PUSH_BACKOFF_SKIPS + ' 次推送');
          return false;
        }
        if (resp.status >= 200 && resp.status < 300) {
          cacheEtag(resp);
          host.log('push ok: PUT ' + url + ' -> ' + resp.status);
          return true;
        }
        // 409 Conflict / 404 Not Found：目录不存在（部分服务器对 PUT 缺失父目录返回 404），
        // 逐级 MKCOL 后重试一次
        if (resp.status === 409 || resp.status === 404) {
          host.log('push: ' + resp.status + ' 目录不存在，尝试 MKCOL 创建');
          if (await ensureDirectories(url)) {
            var retry = await host.http.request('PUT', url, headers, payload);
            if (retry.status >= 200 && retry.status < 300) {
              cacheEtag(retry);
              host.log('push ok: MKCOL 后重试成功 ' + url + ' -> ' + retry.status);
              return true;
            }
            host.logError('push failed: MKCOL 后重试失败 HTTP ' + retry.status);
          } else {
            host.logError('push failed: MKCOL 创建目录失败');
          }
          return false;
        }
        host.logError('push failed: PUT ' + url + ' -> HTTP ' + resp.status);
        return false;
      } catch (e) {
        host.logError('push failed: 请求失败 ' + ((e && e.message) || ''));
        return false;
      }
    },

    // 拉取远端 profile（宿主轮询调用）；无变更/无文件返回 null
    async pull() {
      if (pullSkipCount > 0) {
        pullSkipCount = pullSkipCount - 1;
        host.log('pull: 限流退避中，跳过（剩余 ' + pullSkipCount + ' 次）');
        return null;
      }
      var url = fileUrl();
      if (url === null) {
        host.log('pull: 未配置服务器地址');
        return null;
      }
      var headers = buildHeaders();
      var etag = host.config.get(KEY_LAST_ETAG);
      if (etag !== null && etag !== '') {
        headers['If-None-Match'] = etag;
      }
      var resp;
      try {
        resp = await host.http.request('GET', url, headers, null);
      } catch (e) {
        host.logError('pull failed: 请求失败 ' + ((e && e.message) || ''));
        return null;
      }
      if (resp.status === 304) {
        // Not Modified，无变更
        return null;
      }
      if (resp.status === 404) {
        // 远端尚无文件，视为无变更
        return null;
      }
      if (resp.status === 503) {
        // 服务器限流：进入退避，解封前不再发请求（避免延长封禁）
        pullSkipCount = PULL_BACKOFF_SKIPS;
        host.logError('pull failed: HTTP 503 服务器限流，跳过接下来 ' + PULL_BACKOFF_SKIPS + ' 次拉取');
        return null;
      }
      if (resp.status >= 200 && resp.status < 300) {
        cacheEtag(resp);
        var text = resp.text;
        if (text === null || text === undefined || text === '') {
          host.log('pull: 远端文件为空');
          return null;
        }
        var decoded = null;
        try {
          decoded = JSON.parse(text);
        } catch (e) {
          decoded = null;
        }
        if (decoded !== null && decoded !== undefined && typeof decoded === 'object'
          && !Array.isArray(decoded)) {
          var w = decoded;
          var hasData = w.has_data === true;
          var hasTextKey = w.text !== null && w.text !== undefined;
          // 图片记录没有 text 键（只有 has_data）：必须一起判，
          // 否则会掉进下面的纯文本兼容分支，把整段 JSON 当剪贴板文本写回本地。
          if (hasData || hasTextKey) {
            // 新版 JSON Profile（wire 为 snake_case，与 ximed Profile 同构）→ SDK profile（camelCase）
            var profile = {
              type: w.type === undefined || w.type === null ? 'text' : String(w.type),
              hash: w.hash === undefined || w.hash === null ? '' : String(w.hash),
              text: w.text === undefined || w.text === null ? '' : String(w.text),
              hasData: hasData,
              dataName: w.data_name === undefined || w.data_name === null ? null : String(w.data_name),
              size: typeof w.size === 'number' ? w.size : 0,
              source: w.source === undefined || w.source === null ? null : String(w.source),
            };
            if (hasData) {
              var blob = await fetchBlob(profile);
              if (blob === null) {
                // 附件还没传上来（push 先 blob 后 json 的中间态）或下载失败：
                // 清掉 ETag，下一轮重新拉 JSON 再试；否则 304 会让这次失败变成永久跳过
                host.config.remove(KEY_LAST_ETAG);
                return null;
              }
              profile.data = blob;
              host.log('pull ok: 附件 ' + (profile.dataName || '') + ' (' + blob.length + ' bytes)');
            }
            return profile;
          }
        }
        // 兼容旧版纯文本文件：按纯文本构造 profile，hash 留空让宿主计算
        host.log('pull: 远端非 JSON，按纯文本兼容处理');
        return {
          type: 'text',
          hash: '',
          text: text,
          hasData: false,
          dataName: null,
          size: new TextEncoder().encode(text).length,
          source: null,
        };
      }
      host.logError('pull failed: GET ' + url + ' -> HTTP ' + resp.status);
      return null;
    },

    // 校验配置可用性（连接测试）；返回错误消息，null 表示成功
    async test() {
      var url = remoteDirUrl();
      if (url === null) return '未配置服务器地址';
      // 用 PROPFIND（Depth: 0）探测目录而非 HEAD：部分 WebDAV 服务（如坚果云）对 HEAD 返回 503
      var headers = buildHeaders();
      headers['Depth'] = '0';
      var resp;
      try {
        resp = await host.http.request('PROPFIND', url, headers, null);
      } catch (e) {
        return (e && e.message) || '连接失败';
      }
      if (resp.status === 401 || resp.status === 403) {
        return '认证失败（HTTP ' + resp.status + '）';
      }
      // 503 通常是限流而非配置错误：明确告知用户原因与等待建议，避免误改配置
      if (resp.status === 503) {
        return '服务器限流（HTTP 503）：请求过于频繁。坚果云免费版每 30 分钟限 600 次请求，'
          + '请等待几分钟后再试';
      }
      // 207 Multi-Status / 2xx / 404（目录尚不存在，可创建）均视为连接成功
      if ((resp.status >= 200 && resp.status < 300) || resp.status === 404 || resp.status === 405) {
        return null;
      }
      return '连接失败（HTTP ' + resp.status + '）';
    },
  },
});

// 导出（IIFE 挂到 globalThis.plugin，契约约定）
globalThis.plugin = plugin;
