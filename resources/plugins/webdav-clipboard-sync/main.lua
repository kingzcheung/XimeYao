-- ximed WebDAV 剪贴板同步插件（Lua 脚本插件）
--
-- 对接 ximed 的 WebDAV 直连适配器（crates/client/src/direct/webdav.rs）：
-- 通过 WebDAV 服务器上的单个 JSON 文件实现剪贴板双向同步。
--
-- 职责划分：
--   Lua   = 协议逻辑（WebDAV PUT/GET/MKCOL、Basic Auth、ETag 条件拉取、Profile JSON）
--   宿主  = 同步引擎（轮询/去重/回声抑制）+ 通用原语：
--     host.http        请求原语（PUT/GET/PROPFIND/MKCOL，域名经用户授权）
--     host.json        JSON 编解码
--     host.config      配置存储（含 ETag 缓存）
--
-- WebDAV 交互模型（对齐 ximed WebDavTransport 的 JSON Profile 协议）：
--   文件   {davUrl}/{remotePath}/clipboard/current.json
--          davUrl = 服务器根（如 https://dav.jianguoyun.com/dav/）
--          remotePath = 远程目录（如 xime，留空为根目录）
--   push   PUT 文件，body = Profile JSON（Content-Type: application/json）
--          目录不存在（409；部分服务器如 Alist/Nextcloud 返回 404）
--          → 逐级 MKCOL 创建后重试
--   pull   GET 文件，If-None-Match（ETag 缓存，ximed 无此优化，插件增强保留），
--          304 → 无变更；404 → 远端尚无文件；200 → 解析 Profile JSON
--   兼容   远端若为旧版纯文本（无 JSON 结构），按纯文本 profile 处理
--   认证   Authorization: Basic base64(user:pass)

local plugin = {}

local KEY_DAV_URL = "davUrl"
local KEY_REMOTE_PATH = "remotePath"
local KEY_USERNAME = "username"
local KEY_PASSWORD = "password"
local KEY_LAST_ETAG = "lastEtag"

local CLIPBOARD_KEY = "clipboard/current.json"

-- ================= 工具函数 =================

local function basicAuthHeader(username, password)
    local cred = username .. ":" .. password
    return "Basic " .. host.crypto.base64(cred)
end

local function buildHeaders()
    local headers = {}
    local username = host.config.get(KEY_USERNAME) or ""
    local password = host.config.get(KEY_PASSWORD) or ""
    if username ~= "" then
        headers["Authorization"] = basicAuthHeader(username, password)
    end
    return headers
end

-- 远端剪贴板文件 URL：{davUrl}/{remotePath}/clipboard/current.json
local function fileUrl()
    local base = host.config.get(KEY_DAV_URL) or ""
    base = base:gsub("/+$", "")
    if base == "" then return nil end
    local path = host.config.get(KEY_REMOTE_PATH) or ""
    path = path:gsub("^/+", ""):gsub("/+$", "")
    if path ~= "" then base = base .. "/" .. path end
    return base .. "/" .. CLIPBOARD_KEY
end

-- 远端剪贴板目录 URL（{davUrl}/{remotePath}/clipboard，用于连接测试的 PROPFIND 探测）
local function remoteDirUrl()
    local url = fileUrl()
    if url == nil then return nil end
    return url:gsub("/[^/]+$", "")
end

-- 从文件 URL 里解析出目录层级（不含文件名），逐级 MKCOL 创建。
-- 从用户配置的 davUrl（通常已存在）之后开始创建，避免 MKCOL 服务器根/WebDAV 根被拒。
-- 已存在（405/201/重定向）视为成功，权限不足/网络失败返回 false。
local function ensureDirectories(fileUrl)
    local base = host.config.get(KEY_DAV_URL) or ""
    base = base:gsub("/+$", "")
    if base == "" then return false end
    local dirPart = fileUrl:gsub("^https?://[^/]+", "")
    dirPart = dirPart:gsub("/[^/]+$", "") or ""
    local basePath = base:gsub("^https?://[^/]+", ""):gsub("/+$", "")
    -- 去掉 davUrl 已有路径前缀，只创建剩余层级
    -- （纯字符串前缀匹配，避免 gsub pattern 中 . - + 等特殊字符或前缀误匹配）
    if basePath ~= "" and dirPart:sub(1, #basePath) == basePath then
        dirPart = dirPart:sub(#basePath + 1)
    end
    dirPart = dirPart:gsub("^/+", "")
    local parts = {}
    for part in dirPart:gmatch("([^/]+)") do
        table.insert(parts, part)
    end
    local current = base
    for _, part in ipairs(parts) do
        current = current .. "/" .. part
        local resp = host.http.request("MKCOL", current, buildHeaders(), nil)
        if resp == nil then
            host.logError("MKCOL 失败（网络/拒绝）: " .. current .. " " .. (host.http.lastError() or ""))
            return false
        end
        local s = resp.status
        if s >= 200 and s < 300 then
            -- 已创建（201）或已存在（200）
        elseif s == 405 or s == 301 or s == 302 or s == 307 or s == 308 then
            -- 已存在 / 重定向：视为目录可用
        else
            host.logError("MKCOL 失败: " .. current .. " -> HTTP " .. s)
            return false
        end
    end
    return true
end

local function cacheEtag(resp)
    if resp == nil or resp.headers == nil then return end
    local etag = resp.headers["ETag"]
    if etag == nil then etag = resp.headers["etag"] end
    if etag ~= nil and etag ~= "" then
        host.config.set(KEY_LAST_ETAG, etag)
    end
end

-- ================= 配置 schema（与 manifest 一致） =================

function plugin.getSettingsSchema()
    return {
        {
            key = KEY_DAV_URL,
            label = "WebDAV 服务器",
            type = "text",
            placeholder = "https://host:port/dav/",
            helpText = "WebDAV 服务地址（形如 https://192.168.1.50:8080/dav/）",
        },
        {
            key = KEY_REMOTE_PATH,
            label = "远程目录",
            type = "text",
            placeholder = "xime",
            required = false,
            helpText = "剪贴板文件所在目录（留空为根目录，如 xime → dav/xime/clipboard/current.json）",
        },
        {
            key = KEY_USERNAME,
            label = "用户名",
            type = "text",
            required = false,
        },
        {
            key = KEY_PASSWORD,
            label = "密码",
            type = "secret",
            required = false,
        },
        {
            key = "testConnection",
            label = "测试连接",
            type = "button",
            required = false,
        },
    }
end

-- ================= 生命周期 =================

function plugin.onLoad()
    return true
end

function plugin.onUnload()
    return true
end

-- ================= 同步接口 =================

-- 推送本地 profile 到远端 WebDAV 文件（宿主剪贴板变化时调用）
function plugin.push(profile)
    local url = fileUrl()
    if url == nil then
        host.logError("push failed: 未配置服务器地址")
        return false
    end
    local headers = buildHeaders()
    headers["Content-Type"] = "application/json"
    local body = host.json.encode(profile)
    if body == nil then
        host.logError("push failed: Profile JSON 序列化失败")
        return false
    end
    local resp = host.http.request("PUT", url, headers, body)
    if resp == nil then
        host.logError("push failed: 请求失败 " .. (host.http.lastError() or ""))
        return false
    end
    if resp.status >= 200 and resp.status < 300 then
        cacheEtag(resp)
        host.log("push ok: PUT " .. url .. " -> " .. resp.status)
        return true
    end
    -- 409 Conflict / 404 Not Found：目录不存在（部分服务器对 PUT 缺失父目录返回 404），
    -- 逐级 MKCOL 后重试一次
    if resp.status == 409 or resp.status == 404 then
        host.log("push: " .. resp.status .. " 目录不存在，尝试 MKCOL 创建")
        if ensureDirectories(url) then
            local retry = host.http.request("PUT", url, headers, body)
            if retry ~= nil and retry.status >= 200 and retry.status < 300 then
                cacheEtag(retry)
                host.log("push ok: MKCOL 后重试成功 " .. url .. " -> " .. retry.status)
                return true
            end
            host.logError("push failed: MKCOL 后重试失败 " .. (retry ~= nil and retry.status or "无响应"))
        else
            host.logError("push failed: MKCOL 创建目录失败")
        end
        return false
    end
    host.logError("push failed: PUT " .. url .. " -> HTTP " .. resp.status)
    return false
end

-- 拉取远端 profile（宿主轮询调用）；无变更/无文件返回 nil
function plugin.pull()
    local url = fileUrl()
    if url == nil then
        host.log("pull: 未配置服务器地址")
        return nil
    end
    local headers = buildHeaders()
    local etag = host.config.get(KEY_LAST_ETAG)
    if etag ~= nil and etag ~= "" then
        headers["If-None-Match"] = etag
    end
    local resp = host.http.request("GET", url, headers, nil)
    if resp == nil then
        host.logError("pull failed: 请求失败 " .. (host.http.lastError() or ""))
        return nil
    end
    if resp.status == 304 then
        -- Not Modified，无变更
        return nil
    end
    if resp.status == 404 then
        -- 远端尚无文件，视为无变更
        return nil
    end
    if resp.status >= 200 and resp.status < 300 then
        cacheEtag(resp)
        local text = resp.text
        if text == nil or text == "" then
            host.log("pull: 远端文件为空")
            return nil
        end
        local decoded = host.json.decode(text)
        if decoded ~= nil and type(decoded) == "table" and decoded.text ~= nil then
            -- 新版 JSON Profile（与 ximed Profile 同构）
            return decoded
        end
        -- 兼容旧版纯文本文件：按纯文本构造 profile，hash 留空让宿主计算
        host.log("pull: 远端非 JSON，按纯文本兼容处理")
        return {
            type = "text",
            hash = "",
            text = text,
            has_data = false,
            data_name = nil,
            size = #text,
            source = nil,
        }
    end
    host.logError("pull failed: GET " .. url .. " -> HTTP " .. resp.status)
    return nil
end

-- 校验配置可用性（连接测试）；返回错误消息，nil 表示成功
function plugin.testConnection()
    local url = remoteDirUrl()
    if url == nil then return "未配置服务器地址" end
    -- 用 PROPFIND（Depth: 0）探测目录而非 HEAD：部分 WebDAV 服务（如坚果云）对 HEAD 返回 503
    local headers = buildHeaders()
    headers["Depth"] = "0"
    local resp = host.http.request("PROPFIND", url, headers, nil)
    if resp == nil then
        return host.http.lastError() or "连接失败"
    end
    if resp.status == 401 or resp.status == 403 then
        return "认证失败（HTTP " .. resp.status .. "）"
    end
    -- 207 Multi-Status / 2xx / 404（目录尚不存在，可创建）均视为连接成功
    if resp.status >= 200 and resp.status < 300 or resp.status == 404 or resp.status == 405 then
        return nil
    end
    return "连接失败（HTTP " .. resp.status .. "）"
end

return plugin
