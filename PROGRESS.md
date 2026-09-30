# 曦码·曜 (Xime Yao) 五笔输入法 - 进度跟踪

## 当前状态
- ✅ cargo build 零错误
- ✅ Debug/Release 双版本编译
- ✅ 候选栏 UI 正常显示
- ✅ 输入法可用（已添加到系统）
- ✅ MSI 安装包可用
- ✅ GitHub Actions 自动构建

## 已完成功能 (2026-05-08)

### 核心功能
- [x] librime 引擎集成
- [x] IPC 架构 (TSF DLL + Server)
- [x] 候选栏 Direct2D 渲染
- [x] 配置管理模块
- [x] 方向键导航修复
- [x] 候选栏坐标修复（在 ProcessKeyEvent 前同步获取坐标）
- [x] Shift 键切换中/英文
- [x] 系统托盘图标（嵌入 ICO 文件）
- [x] 托盘显示中/EN 状态图标
- [x] 托盘左键点击切换中/英
- [x] 托盘右键菜单（设置、退出）
- [x] 切换输入法时自动显示/隐藏托盘图标
- [x] 任务栏按钮点击切换中/英文
- [x] 状态同步：输入法启动/切换时正确显示当前中/英状态
- [x] **ITfCompartmentEventSink 实现（监听输入法切换，已打开应用立即生效）**

### 2026-05-09 新增
- [x] **修复候选栏背景截断问题**
  - 问题：候选词少于5个时背景右边被截断，无圆角
  - 原因：窗口大小预留空间固定为25像素，但阴影需要16*scale像素
  - 解决：添加 BLUR_RADIUS 常量，窗口大小改为 `(width + blur_radius * 2) * scale`

- [x] **ITfThreadMgrEventSink 实现（修复已打开应用切换输入法不生效问题）**
  - 问题：从其他输入法切换到当前输入法时，已打开的应用不触发 StartSession
  - 原因：缺少 `ITfThreadMgrEventSink::OnSetFocus` 接口实现
  - 解决：添加 `ITfThreadMgrEventSink` 接口，在文档焦点变化时触发 start_session

- [x] **架构重构：XimeTextService 直接实现 ITfKeyEventSink**
  - 参考 windows-chewing-tsf 项目架构
  - 移除独立的 KeyEventSink 结构
  - 在 Activate 时一次性注册，永不重新注册

- [x] **修复按键双重处理 bug (P0)**
  - 问题：OnTestKeyDown 和 OnKeyDown 都调用 process_key，按键被处理两次
  - 修复：OnTestKeyDown 只做 should_handle_key 检查，不调用 process_key

- [x] **修复 OnSetFocus 焦点处理 (P0)**
  - 问题：两个分支做相同事情，没有区分焦点丢失/获得
  - 修复：pdimfocus.is_null() → focus_out + 清除 composition；非 null → focus_in + start_session

- [x] **移除所有 unwrap() 调用**
  - winxime-tsf 和 winxime-core 已零 unwrap/expect
  - 改用 `lock().unwrap_or_else(|e| e.into_inner())` 容忍 mutex 中毒

## 已验证
- [x] 候选栏第一个字母位置正确

### 2026-08-15 品牌名更新
- [x] 品牌名改为「曦码·曜 (Xime Yao)」
  - [x] 文档标题（README/AGENTS/PROGRESS/DECISIONS）
  - [x] 调用 libximecore 的 metadata（`RimeEngine::new("Xime Yao")`、`resources/xime.yaml`）
  - [x] TSF 注册名 / 语言栏 / DLL 注册名（中文「曦码·曜」）
  - [x] 设置窗口标题、MSI/MSIX 安装包显示名、Release 标题

### 2026-08-15 按键处理对齐 weasel (librime)
- [x] **libximecore `crates/librime/src/key.rs` 键码映射修复**
  - [x] `vk_to_xk` 补充 OEM 标点键映射（VK_OEM_1→XK_SEMICOLON、VK_OEM_7→XK_APOSTROPHE、VK_OEM_4/6→XK_BRACKETLEFT/RIGHT、VK_OEM_MINUS/PLUS→XK_MINUS/EQUAL、VK_OEM_COMMA/PERIOD→XK_COMMA/PERIOD、VK_OEM_2/5/3→XK_SLASH/BACKSLASH/GRAVE、VK_CAPITAL→XK_CAPS_LOCK）
  - [x] 新增常量：K_LOCK_MASK、VK_CAPITAL/SHIFT/CONTROL/MENU、VK_OEM_*、XK_* 标点 keysym
  - [x] `get_key_modifiers(is_key_up: bool)` 补充 Caps Lock 检测（LOCK_MASK）与按键释放（RELEASE_MASK）
  - [x] 单元测试：test_vk_to_xk_oem_punctuation / test_vk_to_xk_letters_lowercase / test_vk_to_xk_misc（11 项全部通过）
- [x] **TSF 层移除硬编码选词/翻页拦截，改由 rime 配置驱动**
  - [x] `handle_key_event` 删除数字 1-9 选词、`;`/`'` 选词、`[`/`]`/`-`/`=`/Tab/Shift+Tab/PgUp/PgDn 翻页的直接 IPC 调用
  - [x] 这些键现在统一走 `process_key(xk, mods)`，由 rime 的 `key_binder`（default.custom.yaml: semicolon→2、bracketleft/right→Page_Up/Down、Tab→Page_Down 等）处理
  - [x] 新增 `handle_key_up_event`：非 Shift/Ctrl 的 KeyUp 转发给 rime（带 RELEASE_MASK），供 ascii_composer 使用
  - [x] `OnTestKeyUp` 同步返回 `should_handle_key` 结果，保证 `OnKeyUp` 能被 TSF 调用
  - [x] `get_key_modifiers` 仅调用一次（不再在按键时刻前后重复取异步状态）
- [x] `cargo check` 零错误；libximecore `cargo test -p librime key` 全部通过

### 2026-08-15 CI 构建修复 (librime-sys2 build.rs)
- [x] **修复 `vswhere failed: os error 123`（CI 源码构建路径）**
  - 问题：`find_vswhere` 用 `Command::new("where")` 输出 `.trim()` 直接作为路径，但 `where vswhere` 可能输出多行（PATH 多个匹配），中间换行符未去除 → `Command::new` 收到非法路径（ERROR_INVALID_NAME）
  - 解决：改为逐行解析 `where` 输出，取第一个 `exists()` 的文件路径；候选路径兜底不变
  - 额外：vswhere 返回的 VS installationPath 校验非空且目录存在，避免写入无效 `vcvars64.bat` 路径
  - 验证：`cargo check -p librime-sys2`（debug/release）零错误（本地因预编译 rime.dll 走跳过分支，CI 源码构建路径由修复逻辑覆盖）

### Server 后台运行
- [x] 单实例检测 + 自动停止旧进程
- [x] `/q` 命令停止
- [x] RegisterApplicationRestart (Windows 自动重启)
- [x] DPI 感知
- [x] Debug/Release 条件编译
- [x] UI 主线程创建（修复消息处理）

### 设置程序
- [x] winxime-setup (GPUI UI)
- [x] 基础设置界面
- [x] 状态管理模块 (Entity<SettingsState>)
- [x] 组件回调支持 (Switch/NumberInput/Button)
- [x] 关于页面 (版本、作者、仓库、许可)
- [x] 菜单图标 (SVG)
- [x] 标题栏左侧与侧边栏颜色一致
- [x] 菜单选中背景色改为主色

### 安装部署 (新增)
- [x] winxime-tsf-register 工具 (TSF 注册)
- [x] MSI 安装包 (WiX v3.14)
- [x] GitHub Actions CI/CD
- [x] SignPath 代码签名配置
- [x] package-release.ps1 打包脚本

## 架构

```
winxime-tsf.dll         → TSF 输入框架 (注册到系统)
winxime-server.exe      → 候选栏 + Rime引擎 (后台运行)
  - Debug: 有控制台窗口 (1.09 MB)
  - Release: 无控制台窗口 (447 KB)
winxime-setup.exe       → 设置界面
winxime-tsf-register.exe → TSF 注册工具 (MSI 安装用)
```

## GitHub Actions

- `.github/workflows/ci.yml` - 构建 MSI
- `.github/workflows/code-signing.yml` - SignPath 签名
- `.github/workflows/release.yml` - 发布流程

## 使用方式

### 开发调试
```powershell
cargo run                     # 启动 Server (有日志)
cargo run -p winxime-server -- /q  # 停止 Server
cargo wix --package winxime-server --bin-path "C:\Program Files (x86)\WiX Toolset v3.14\bin"  # 构建 MSI
```

### 本地安装
```powershell
# 方式1: MSI 安装 (需管理员)
msiexec /i target\wix\winxime-server-0.1.0-x86_64.msi

# 方式2: dist 目录安装
.\dist\install.bat  # 管理员运行
```

### SignPath 签名配置
1. 注册 SignPath.io 组织
2. 创建项目 `winxime`
3. 配置签名策略 `release-signing`
4. 添加 GitHub Secrets:
   - `SIGNPATH_API_TOKEN`
   - `SIGNPATH_ORGANIZATION_ID`

## 设计决策

### winxime-setup 配置交互方案 (2026-05-09)
参考项目分析：
- **weasel (小狼毫)**：`WeaselDeployer.exe` 通过 IPC + librime API 交互
  - `StartMaintenance()` → Server 暂停服务
  - 修改 Rime 配置文件
  - `rime->deploy()` → 重新部署
  - `EndMaintenance()` → 恢复服务
- **windows-chewing-tsf**：注册表 + 自动重载
  - 配置存储在 `HKCU\Software\ChewingTextService`
  - TSF DLL 通过 `reload_if_needed()` 检测变化

**最终方案**：采用 `xime.custom.yaml` 配置文件方式
- 配置路径：`%APPDATA%\Xime\xime.custom.yaml`
- winxime-setup 修改配置文件
- winxime-server 通过 librime API 加载，定期检测变化重载
- 交互方式（待定）：文件监听 或 IPC `ReloadConfig` 命令
- UI 设计要符合 fluent design

## 下一步
 - [x] winxime-setup UI 完善进度
   - [x] 状态管理模块
   - [x] 基础组件回调
   - [x] 关于页面
   - [x] 菜单图标
   - [x] 实现配置持久化 (保存到 xime.custom.yaml)
   - [x] 配置项分组细化
   - [x] 标题栏全局部署按钮
 - [x] 实现 xime.custom.yaml 配置读写
   - [x] librime-sys levers API 绑定
   - [x] RimeConfigManager (UI 配置管理)
   - [x] SchemaManager (输入方案管理)
   - [x] deploy_all() (重新部署功能)
   - [x] 自动创建用户配置文件 (%APPDATA%\Rime)
 - [x] Server 配置加载
   - [x] winxime-server/config.rs 模块
   - [x] config_open("xime") 读取 build/xime.yaml
   - [x] 应用到 CandidateModel (字体、颜色)
 - [x] 部署功能优化
    - [x] 标题栏全局部署按钮
    - [x] 部署结果反馈（标题栏显示消息）
 - [x] Server 配置重载机制
    - [x] IPC ReloadConfig 命令 (winxime-ipc)
    - [x] ipc_server.rs 处理 ReloadConfig → eng.deploy()
    - [x] winxime-setup 部署后调用 IpcClient::reload_config()
- [x] **方案级详细设置 (2026-05-12)**
     - [x] SchemaConfigManager (rime_config.rs)
     - [x] 读取方案配置 (speller/translator/reverse_lookup/tradition)
     - [x] 保存方案配置到 schema.custom.yaml
     - [x] InputSchemaState 添加 schema_config 字段
     - [x] 输入方案页面展示选中方案的详细设置
     - [x] SettingsGroup 组件渲染方案配置分组
  - [x] **日志系统重构 (2026-05-15)**
     - [x] 使用 tracing 替换原来的 log crate
     - [x] winxime-core: init_logging() 支持组件名参数
     - [x] winxime-server: 使用 tracing + init_logging_with_console()
     - [x] winxime-tsf: 使用 tracing::debug!
     - [x] winxime-tsf/language_bar.rs: 使用 tracing
- [x] winxime-server/tray.rs, ui.rs, ipc_server.rs: 使用 tracing
   - [x] **按键绑定实现 (2026-05-16)**
      - [x] key_binder: 分号选词、方括号/Tab翻页
      - [x] ascii_composer: commit_code 行为 (切换时提交编码)
      - [x] switcher: IPC 命令 (GetSchemaList, SelectSchema)
    - - [ ] 下一步
      - [ ] switcher: Ctrl+0 弹出方案选择菜单 (需要 UI)
      - [ ] punctuator 标点符号映射（键码映射修复后已可命中，需验证标点上屏）
      - [ ] recognizer 英文识别模式
      - [ ] menu.page_size 配置读取

### 2026-07-12 修复
- [x] **修复焦点事件风暴导致无法输入中文 (P0)**
   - 问题：三个 TSF sink (`ITfKeyEventSink`、`ITfThreadFocusSink`、`ITfThreadMgrEventSink`) 在同一个焦点转换时分别独立触发 IPC 调用
   - 同步 IPC 在 STA 线程阻塞时引发消息泵送 → 重入的 FocusOut → `abort_composition()` 清除输入状态
   - 解决：
     - 合并 focus 处理到 `ITfThreadMgrEventSink::OnSetFocus`，其他两个 sink 改为 no-op
     - 添加 `processing_focus` 重入保护
     - `show_tray_icon`/`hide_tray_icon` 添加幂等保护（`tray_visible` 标志）
     - 移除 `activate_impl` 中的冗余 `start_session()` 调用

### 2026-06-14 新增
- [x] **引入 librime-octagram / librime-lua / librime-lua-deps 插件**
   - 添加 `plugins/librime-octagram` 和 `plugins/librime-lua` 为 git submodule
   - `build.rs` 构建前自动复制插件到 `librime/plugins/` 并安装 Lua 5.4 第三方依赖
   - CI workflow 同步更新：插件缓存及构建步骤
   - `find_vswhere()` 改为通过 PATH 或候选路径查找，不再硬编码

### 2026-09-11 候选栏菜单面板（参考 macOS 版 XimeYi）
- [x] **候选栏右侧 "⋮" 菜单按钮 + 下方可展开面板（横向布局）**
  - 菜单页：2 列 × 4 行功能卡片（📋剪切板 🚀快捷发送 🧮计算器 😀表情 🔣符号 🎙️语音输入 ⚙️设置）
    + 底部品牌栏「曦码·曜输入法」
  - 子页面 v1 为占位（粗体标题 + 「← 菜单」返回 + 「功能开发中」，与 macOS 版占位页一致）
  - 「设置」项已接通：启动 winxime-setup.exe（main.rs 抽取 `launch_setup()`，托盘菜单共用）
  - 交互：点击 ⋮ 展开/收起；卡片 hover 高亮 + 手型光标（TrackMouseEvent/WM_SETCURSOR）；
    输入新内容（WM_UPDATE_CANDIDATE）或候选栏隐藏时自动收起并复位到菜单页
  - 单元测试 6 项（菜单布局/行槽不压底部栏/返回按钮矩形/命中路由/坐标换算/页面 id 覆盖），全部通过
  - 备注：竖排布局无菜单入口（与 macOS 一致）；表情/符号/剪贴板等子页实际功能为后续功能点

### 2026-09-11 rebuild.ps1 改为「安装效果」测试流程
- [x] **rebuild.ps1 重写：测试路径 = 安装路径（参考 msix-bundle.ps1）**
  - 旧流程的问题：cargo run 从 target\debug 直接启动（Debug 版带控制台黑窗口），
    COM/Profile 注册指向 target\debug 路径，与真实安装（System32 DLL + 包目录）割裂
  - 新流程：release 构建（windows_subsystem=windows，无黑窗口）→ 复用 msix-bundle.ps1
    按安装布局暂存（binaries + rime.dll + data + user-data + resources + AppxManifest）→
    `Add-AppxPackage -Register` 松散文件开发注册（= 安装效果）→ 从注册的包目录
    target\msix-pkg 启动 server（等价于 MSI 的 StartServer 动作）
  - 非管理员运行时自动 UAC 提权重启（-File 重跑自身，新窗口 -NoExit 保留输出），
    重启后 Set-Location 锚定仓库根（提权进程 cwd 是 System32，cargo/msix-bundle 都按相对路径解析）
  - 修复提权后 cwd 分裂：Set-Location 只改 PowerShell 当前位置（cmdlet/外部 exe 用它），
    而 [System.IO.File] 等 .NET API 读进程级 cwd（提权进程默认 system32），导致 manifest
    写到 system32\target\msix-pkg 失败；msix-bundle.ps1 头部现在同时锚定两级目录到脚本根
  - %APPDATA%\Xime 用户数据跨重建保留；日志在 %TEMP%\winxime\*.log
- [x] **msix-bundle.ps1 -Register 分支修复**
  - 原来注册后删除 target\msix-pkg：松散文件注册的包内容就指向该目录，删除等于注册出空壳包
  - 现保留暂存目录（等价于真实安装的 WindowsApps 目录常驻磁盘），并增加注册失败检测（exit 1）
- [x] **winxime-tsf-register 定性（未废弃）**
  - MSI 安装/卸载仍依赖它：main.wxs 自定义动作 RegisterTSF（-copy-and-register，拷 DLL
    到 System32+注册）/ UnregisterTSF（-unregister-and-remove）/ StopServer
  - full-uninstall.ps1 依赖它；MSIX 流程不用它（server 的 register.rs::ensure_registered 自我注册），
    msix-bundle.ps1 打包时只是带着它的 exe 但从不调用（后续可从包清单中移除）

### 2026-09-11 重构：ui.rs 拆分为 ui/ 目录模块
- [x] 原 ui.rs（约 1720 行，职责混杂）按职责拆为 6 个文件，行为不变：
  - `ui/mod.rs`（约 200 行）：`CandidateWindow` 对外 API（show/hide/update/show_root/hide_root）、
    面板状态字段、对外消息常量、共享布局常量
  - `ui/model.rs`（约 160 行）：`CandidateModel`/`RootModel`/`RenderedMetrics` 数据模型
  - `ui/layout.rs`（约 240 行）：DirectWrite 文本测量与布局计算（自由函数，传入工厂而非 self）
  - `ui/paint.rs`（约 580 行）：候选栏/字根提示 D2D 绘制；
    顺带把候选栏与字根提示两处逐字重复的高斯模糊投影块合并为 `draw_drop_shadow()`
  - `ui/view.rs`（约 660 行）：窗口/交换链/合成设备创建 + 全部 `wnd_proc` 消息处理（含面板鼠标交互）
  - `ui/panel.rs`（约 710 行）：菜单面板（上一功能点已建，迁入 ui/ 目录，路径 `crate::ui::panel`）
  - 对外接口不变：`ipc_server.rs` 的 `crate::ui::CandidateWindow` 与 `main.rs` 的 `ui::panel::*` 照常工作
  - `cargo check` 零错误（仅剩拆分前就存在的旧代码 dead_code warning）；6 项单测全部通过
### 2026-09-11 数据目录修复（对齐 Xime 单目录模型）
- [x] **修复「MSIX 安装后方案丢失/数据目录不对/设置里方案列表为空」**
  - 根因 1：`xime_config::get_data_dirs()` 无人调用 `set_rime_paths()`，Windows 回退 Unix
    路径（HOME 未设 → `C:\.config\xime\rime`），设置程序的方案列表/打开数据目录/SchemaManager
    全部扫错目录
  - 根因 2：AppxManifest 缺 `unvirtualizedResources`，MSIX 把 `%APPDATA%\Xime` 虚拟化到包
    LocalCache，包外进程（资源管理器、宿主内 TSF DLL）看不到
  - 根因 3：旧 `ensure_user_config_files` 只要用户目录有任意 .yaml 就永久跳过方案部署
- [x] 修复内容：
  - xime-config `default_rime_paths()` 增加 Windows 分支：单目录模型 shared == user ==
    `%APPDATA%\<config_dir>\rime`（对齐 Xime 的 userDataDir == sharedDataDir）；Unix 分支不变
  - server `get_data_dirs()` release 分支改单目录模型；部署函数重写为 Xime 语义
    （`ensure_rime_data`：rime 目录无 *.schema.yaml 视为首装 → 全量复制安装目录 data/ +
    user-data/；升级 → 仅覆盖内容有变化且文件名不含 "custom" 的文件，保护用户定制）
  - AppxManifest 增加 `<rescap:Capability Name="unvirtualizedResources" />`
  - market_dir 改与真实用户目录同级（修复原先落到 `C:\.config\xime\market` 的错位）
  - xime-config 环境依赖的坏测试改为临时目录 fixture（密封测试）
- [x] 验证：xime-config 5/5、xime-plugin 27/27、winxime-server 6/6，release 构建零错误

### 2026-09-11 插件系统（plugins-core 宿主接入 + 云备份/剪贴板同步）
- [x] **背景**：libximecore 已有平台无关的 `xime-plugin` crate（mlua Lua54 沙箱运行时、
  manifest/capabilities 解析、PluginManager 安装/启停、host.http/crypto/json/config 等 host API），
  与 Xime（Android）的 plugin-core Lua 插件契约逐字对齐
- [x] **libximecore 扩展**：
  - `PluginRuntime` 补 backup 契约封装：`backup_push/pull/list/delete`（二进制备份包经
    Lua string 往返，与 Android LuaBackupPluginAdapter 一致），新增二进制往返单测
  - `PluginManager` 补 `install_from_dir`（安装随宿主分发的解压态内置插件，保留启用状态）
- [x] **winxime-server 宿主接线**（新模块 `plugins.rs` + `clipboard.rs`）：
  - 启动时安装 `resources/plugins/` 内置插件 → `%APPDATA%\Xime\plugins`，加载全部已启用插件
  - 内置插件：`webdav-backup`（云备份）与 `webdav-clipboard-sync`（剪贴板同步），源码取自
    Xime 仓库 plugins/ 目录，随 resources 打包进 MSIX/安装目录
  - 云备份：宿主打包 zip（rime 目录全部文件、跳过 build/，条目前缀 `rime/`，与 Xime 布局
    一致）→ 插件 WebDAV PUT；恢复：按条目写回 rime 目录（enclosed_name 防穿越，跳过
    `_xime_backup/` 元数据）；托盘菜单新增「立即云备份」
  - 剪贴板同步：消息窗口 `WM_CLIPBOARDUPDATE` 监听本地变化 + 30s 定时拉取；三通道去重
    （当前 hash / 上次推送 / 远端写回），写回走系统剪贴板从而进入输入法剪贴板历史；
    Profile JSON 与 ximed 同构（snake_case），阻塞 HTTP 全部派发到工作线程
- [x] 配置方式（v1，设置 UI 为后续功能点）：手工创建
  `%APPDATA%\Xime\plugins\config\<plugin-id>.yaml`，webdav-backup 键：url/username/
  password/remote_path；webdav-clipboard-sync 键：davUrl/remotePath/username/password
- [ ] 后续功能点：设置程序插件中心页（启停/配置表单 getSettingsSchema/备份列表）；
  IPC 插件命令；`_xime_backup/` 设置与插件配置恢复；候选栏剪贴板/备份入口卡片接线

### 2026-09-12 下载数据目录对齐安卓 Xime（插件/方案市场/模型）
- [x] **目录映射总表**（安卓 filesDir ↔ Windows %APPDATA%\Xime，见 DECISIONS.md）：
  - 方案市场包：`files/market/{id}/` ↔ `%APPDATA%\Xime\market\<id>\`（P0 已对齐）
  - 插件：`files/plugins/{id}/` ↔ `%APPDATA%\Xime\plugins\<id>\`（已对齐，注册表格式为
    Rust 平台实现 registry.yaml，目录布局一致）
  - 模型：`files/models/{modelId}/` ↔ `%APPDATA%\Xime\models\<modelId>\`（新增 `models.rs`
    固化约定：models_root/model_dir/ensure_model_dir/is_model_downloaded/delete_model，
    对齐安卓 ModelStorage/ModelManager 语义——文件如实命名、存在且非空才算已下载、
    用到才建目录、模型独立于插件管理；模型下载功能本身待 ASR/联想后端接入）
  - 市场注册表：安卓在数据根（files/.registry.json），Windows 从 market/.registry.yaml
    移到 `%APPDATA%\Xime\.registry.yaml`（market/ 只存下载包）
  - 下载临时文件：安卓约定 cache/xime_plugin_{id}_{fileName} 即用即删，Windows 对齐为
    `%TEMP%\xime_plugin_{id}_{fileName}`（plugins.rs `plugin_download_temp_path`，
    供后续插件市场下载使用）
- [x] 验证：winxime-server 测试 8/8（含 models 目录 2 项），release 构建零错误

### 2026-09-12 盘根残留目录清理（C:\.config\xime）
- [x] 旧 bug 残留的 `C:\.config\xime`（HOME 未设时 Unix 回退路径拼到盘根产生）已清理：
  - 其中 `models/ochwpro`（6.8MB 手写模型，模型中心经旧路径下载的真实数据）已迁移到
    `%APPDATA%\Xime\models\ochwpro\`，与联想模型 predictive-text-small 并列
  - 其余（rime/build 部署产物、installation.yaml、空 market）为可再生垃圾，随目录删除
- [x] 代码层面确认：全部路径经 `xime_config::get_data_dirs()`（Windows 分支 → %APPDATA%\xime），
  Unix 回退已 cfg(not(windows)) 隔离，setup 的模型/市场/插件目录不会再写盘根；
  设置程序（xime-setup-lib）cargo check 通过

### 2026-09-12 修复 MSIX 开发注册同版本重复注册失败（0x80073CFB）
- [x] 现象：第二次 `rebuild.ps1` 起必现「提供的程序包已安装，且禁止重新安装」
  （Add-AppxPackage -Register 拒绝同 Identity+Version 的重复注册）
- [x] 修复：`msix-bundle.ps1 -Register` 注册前按 manifest 的 Identity.Name 移除旧的开发注册
  （Get-AppxPackage → Remove-AppxPackage）再重新注册；独立调用时先停包内进程
  （winxime-server/winxime-setup）避免移除被文件占用阻塞

- [x] **修复：剪贴板历史不记录（未启用同步插件时）**——历史逻辑原先在
  剪贴板同步工作线程里，而该线程只在同步插件启用时启动。重构为
  **worker 常驻**：历史始终记录（SQLite），同步插件运行时按选型可选加载
  （worker 持 `Option<PluginRuntime>`，推送/拉取按需执行）；选型变更仍
  停旧起新（`worker_started` AtomicBool + 插件 id 比对）。补回归测试
  clipboard_worker_records_history_without_sync_plugin（无插件 LocalChanged
  仍落库）+ 恢复误删的 clipboard_selection_follows_clipboard_sync_toml；
  14/14

### 2026-09-29 插件配置值加密（对齐 Android SecureValueCipher）
- [x] **背景**：安卓端插件配置全值加密（Keystore AES-GCM，`enc:` 前缀 +
  base64(iv+密文+tag)，认证失败视为无效，明文兼容回退）；Windows 端
  host.config 值为明文 YAML，WebDAV 密码同机任意程序可读（%APPDATA% 按
  用户划界不按应用划界，已核对本机 ACL）
- [x] **libximecore 新增 `xime-plugin/src/cipher`**（同算法同密文格式）：
  - AES-256-GCM；密钥 32 字节随机生成，经 **DPAPI(CryptProtectData)**
    加密存于数据目录 `secret.key`（DPAPI = Windows 对应 Keystore 的角色，
    按用户绑定、文件离机不可解）；密钥文件损坏不覆盖（避免误清密文）
  - `encrypt_with_key_path / decrypt_with_key_path`；密钥路径由配置文件
    路径推导（plugins/config/<id>.yaml → 数据目录/secret.key）
  - 非 Windows：恒等实现（行为与旧版一致，Linux daemon 不受影响）
  - 单测 3 项（往返+明文兼容、篡改密文认证失败、密钥文件非裸密钥）
- [x] **两端接线**：xime-plugin runtime 的 load_config/save_config（host.config
  读写层）与 xime-setup 的 read/write_plugin_config + start_schema_load
  值读取全部走加解密；**存量明文配置在首次保存时随全量写入自动升级密文**
  （读取侧明文兼容，无迁移动作也不会丢数据）
- [x] 验证：cipher 3/3、winxime-server 14/14、debug/release 零错误

### 2026-09-30 安装对齐 Android：真实方案 id 发现 + 已安装列表即时刷新
- [x] **包 id ≠ 方案 id**：此前把包 id（如 rime-ice）直接塞进 schema_list，
  rime 不认识 → 方案装了却不生效。对齐 Android `installPackageFromMarketDir`：
  从释放的顶层 `*.schema.yaml` 提取真实方案 id（优先取与包 id 规范化后
  同名者），校验前置（无 .schema.yaml 拒装），安装即切换（启用列表替换为
  新方案，对齐 Android switchEnabled 语义）
- [x] **已安装列表不刷新**：`available_schemas` 只在启动时 load 一次且
  `schemas_loaded` 幂等挡板拦住重载；InstallDone/UninstallDone 只更新商店
  installed_ids。新增 `reload_schemas()`（无挡板强制重载），安装/卸载
  完成即刷新列表
- [x] 验证：构建零错误、libximecore 测试全绿

### 2026-09-30 停止分发 librime minimal 示例数据（用户 rime 目录污染源）
- [x] **根因**（用户对比 rime-wubi 发现多余文件、删除重装仍在）：
  `msix-bundle.ps1` / `msi-build.ps1` 把 `libximecore/librime/data/minimal`
  （librime 自带示例：cangjie5 / luna_pinyin / essay.txt / default.yaml /
  symbols.yaml）当"rime base data"打进安装包，`ensure_rime_data` 首装
  全量拷入用户 rime 目录；rime 目录里另有 market 安装的 rime-ice 全套
  （build/、cn_dicts/ 等属其正常内容，但 default.yaml 覆盖与 build/ 释放
  已由安装过滤器修复）
- [x] 修复：两个打包脚本移除 minimal 拷贝并清空暂存 data/（防历史残留）；
  rime-wubi（user-data/）自包含 default.yaml / symbols.yaml，无功能依赖；
  msi-build 顺带移除孤儿 `Find-LibrimeRoot`
- [x] 另：do_install 增加 `is_protected_release_path` 过滤——市场包不得
  释放 default.yaml 等宿主/引擎自有文件、`build/` 部署产物、`*.userdb/`
- [x] 验证：构建零错误、libximecore 测试全绿；PS1 语法校验通过
- [x] 用户操作：`.\rebuild.ps1` 重打包后删一次 `%APPDATA%\xime\rime`
  再启动，目录即只剩 rime-wubi 内容 + 运行产物

### 2026-09-30 系统通知改为 server 代弹（IPC ShowToast）
- [x] **setup 进程无包身份**：MSIX 清单只声明 `winxime-server.exe` 一个
  应用入口；设置程序直跑（非 server 派生）时 `GetCurrentPackageFullName`
  拿不到身份 → toast 无从归属被系统拒绝，且 setup.log 无失败日志（静默）
- [x] 方案（用户直觉验证成立：要走 IPC）：新增 `ShowToast` IPC 命令
  （ToastMessage{title,body}）+ `IpcClient::show_toast`；server 持有
  toast.rs（WinRT 实现，有包身份）代为弹出，失败记 server.log
- [x] setup 的 `toast::show_toast` 改为 **IPC 优先**，失败回退本进程直弹
  （server 派生启动时有身份的场景）；server windows crate 增
  Data_Xml_Dom / UI_Notifications / Win32_Storage_Packaging_Appx /
  Foundation features
- [x] 验证：构建零错误、winxime-server 18/18

### 2026-09-30 修复「部署失败：deploy returned 0」——deploy 语义误读
- [x] 根因（librime 源码 rime_api_impl.h 确认）：`api->deploy` 是
  `RimeStartMaintenanceOnWorkspaceChange`——`installation_update` /
  `detect_modifications` 判定**无变化时返回 0，是"无需维护"不是失败**；
  levers 的 `deploy_all_with_config` 把 0 当错误抛出
- [x] 修复：`xime_config::deploy_all` 改为显式全量维护（对齐 weasel
  「重新部署」）：`start_maintenance(full)` + `join_maintenance_thread`，
  之后补跑 `deploy_config_file(xime.yaml)`（幂等）；不再使用 OnWorkspaceChange
  语义的 api->deploy 做成败判定
- [x] 日志佐证：setup.log 无部署条目（错误在 UI 层产生），server.log 的
  启动 "deployment failed" 是另一处 DeployResult 通知未捕获的存量问题，
  不影响功能，后续单独处理
- [x] 验证：构建零错误、libximecore 测试全绿

### 2026-09-30 修复「部署方案」完全没有反馈
- [x] **toast 从未弹出的真凶**：`package_aumid()` 把 Win32 两段式调用的
  第一段（空缓冲取长度，正常返回 `ERROR_INSUFFICIENT_BUFFER`）误判为
  「非打包环境」直接返回 None——toast 永远静默跳过。修正：仅
  `APPMODEL_ERROR_NO_PACKAGE` 视为非打包
- [x] **页内消息从未显示**：`show_message` 只发宿主回调，而 winxime-setup
  从未注册 `set_notify_message`——「正在部署…」「部署成功」等全部落空。
  修复：show_message 写入 `ui_message`（Instant 时间戳），app 视图顶部
  渲染全局消息条（主色底、5 秒经 BackgroundPoll 过期）
- [x] 设置进程接日志：`init_logging_with_console("setup")` →
  `logs\setup.log`（toast 失败等此前 eprintln 进黑洞的诊断信息可见）
- [x] 验证：构建零错误、winxime-server 18/18、libximecore 全绿

### 2026-09-30 选中方案记忆 + 启动不再覆盖用户弃用的 builtin 方案
- [x] **打字时自动切回第一个方案**：engine 的会话选中完全没持久化——
  redeploy/deploy 重建会话、server 重启都回落 schema_list 第一个。修复：
  - `RimeEngine` 记住 `selected_schema`，`redeploy()`/`deploy()` 重建会话后
    自动重新选择
  - server：SelectSchema 成功后写数据根 `selected_schema.txt`；启动时
    deploy 完读回并恢复（重启也不丢）
- [x] **启动覆盖用户方案**：`ensure_rime_data` 升级路径此前强更所有非
  custom 文件。修复：读用户启用列表（default.custom.yaml 的 `- schema:`
  行），**未启用的 builtin 方案文件**（`<id>.schema.yaml/.dict.yaml`，
  id 属于安装目录 builtin 集合）不再强更——不覆盖用户自己的方案；
  共享资产（essay/symbols/lua/default.yaml）照常更新；首装全量不变
- [x] **卸载压扁启用列表**：`do_uninstall` 此前把启用列表写成只剩第一个
  剩余方案。修复：保留全部剩余启用方案（get_schema_list_ids 过滤），
  全空才回退首个现存方案
- [x] 验证：构建零错误、winxime-server 18/18、libximecore 全绿

### 2026-09-30 方案安装隔离（对齐 Android installPackageFromMarketDir）+ 部署按钮 toast
- [x] **部署按钮无通知**：「部署方案」（DeploySchemas）此前只更新页面底部
  消息；现成败两路接 `notify_deploy_toast`（与安装/卸载一致）
- [x] **安装隔离（此前致命缺口：只拷 .schema.yaml，词典/lua 全丢，多方案
  文件混在 rime 根目录、卸载删不掉）**。`do_install` 重写为对齐 Android：
  1. **全量释放**——归档内容全部进 rime 目录（保留相对路径，含词典/lua），
     损坏包解压失败即弃（对齐 validateArchive）
  2. **冲突检测**——目标文件已被其他包占用 → 拒绝安装并报冲突来源
     （同包重装允许覆盖；对齐 detectConflicts）
  3. **安装清单**——按包写数据根 `.registry.yaml`（`<pkg>: files: [...]`，
     与 server SchemaManager 同格式）；卸载侧（上一轮已接数据根）据此
     精确删除本包文件，跨包不再互相污染
- [x] 注意：修复前已混装的旧方案没有清单，仍卸载不干净（历史数据无法
  追溯归属），重新安装一次即可获得清单
- [x] 验证：构建零错误、libximecore 测试全绿

### 2026-09-30 修复设置程序三处 UI 冻结（启动 / 部署按钮）
- [x] **启动卡死**：`SettingsState::new()` → `load_schemas()` →
  `SchemaManager::new()` → `init_rime_deployer()` 内置
  `start_maintenance(true)+join`（全量部署，rime-ice 数秒）跑在 UI 线程。
  修复：初始化只做 setup+initialize+create_session（毫秒级），部署一律走
  显式 `deploy_all()`（调用方已后台线程）；server 启动时本就维护 build/
- [x] **部署按钮卡死**（输入方案「部署方案」/ 快捷键「重新部署」同一条
  `DeploySchemas` 路径）：`poll_deploy` 里的 `notify_daemon_reload()` 是
  同步 IPC，server `eng.redeploy()` 数秒期间 UI 冻结。修复：daemon 重载
  挪进 `start_deploy` 的后台线程（部署→重载→组合文案一并返回），
  `poll_deploy` 只展示结果字符串（DEPLOY_RESULT 类型改为 Result<String,String>）
- [x] 验证：构建零错误、libximecore 测试全绿

### 2026-09-30 日志目录收敛到数据根（%APPDATA%\<name>\logs）
- [x] `get_log_dir()` Windows 分支原为 `%TEMP%\<name>\`（TEMP 清理会丢日志、
  排障时也想不到去那找），改为数据根 `logs\` 子目录（与用户数据同处）；
  TEMP 兜底保留；Unix 分支不动
- [x] 说明：`clipboard_sync.toml` 是同步选型文件（开关写入、server 30s
  轮询读取），非垃圾——删除等于关掉剪贴板同步
- [x] 数据根全景盘点确认已聚合：*.toml/*.db/*.key 平铺 + market/models/
  plugins/rime/logs 子目录；唯一约定性例外是 %TEMP% 下载缓存（即用即删，
  对齐 Android cache/，DECISIONS 已记录）
- [x] 验证：构建零错误

### 2026-09-30 方案部署结果系统通知（WinRT toast）
- [x] **架构结论**：部署发生在 setup 进程（`init_rime_deployer` 在调用进程
  初始化 librime），结果就在 setup 手里；server 热载的成败也在 IPC 应答
  现场——**不需要新增 IPC**。toast 是 Windows 专属，不能进 libximecore
  （跨平台库），落在 winxime-setup 宿主
- [x] libximecore：`set_notify_deploy_toast(f: fn(&str, &str))` 平台无关
  钩子；安装/卸载线程的成败两路触发（含失败原因）
- [x] winxime-setup：`toast.rs`——WinRT ToastNotification（ToastGeneric 模板，
  XML 转义），AUMID 动态取 `GetCurrentPackageFullName()!XimeServer`；
  非打包环境（开发直跑）静默跳过；后台线程弹（先 CoInitializeEx MTA）
- [x] windows crate 增 features：Data_Xml_Dom / UI_Notifications /
  Win32_Storage_Packaging_Appx / Win32_System_Com
- [x] 验证：构建零错误、libximecore 测试全绿

### 2026-09-30 修复输入方案「已下载」列表为空
- [x] 根因：`input_schema.rs::scan_market_dir()` 是第三套路径——release 扫
  **exe 同级目录** `market`（MSIX 安装目录，不存在）、debug 扫仓库
  `target\debug\market`，而商店下载落在数据根 `market\`（上一条修复后）
- [x] 修复：改为复用 `state::market_dir()`（开为 pub(crate)），目录常量
  至此唯一；实机验证数据根下已有 `market\rime-ice` 包
- [x] 验证：构建零错误、libximecore 测试全绿

### 2026-09-30 修复方案市场路径/注册表与 DECISIONS 声明的漂移
- [x] setup 侧 `markets_dir()`（复数 `markets\`）改为 `market_dir()`
  （单数 `market\`），与 server SchemaManager 及 DECISIONS「下载数据目录
  映射」收敛为同一目录；5 处调用点（下载包目录/已装列表/安装/缓存清理）
  一并生效；过时注释（`~/.config/xime/markets/`）修正
- [x] setup 卸载的注册表从 `markets\.registry.yaml`（无人写入的孤儿文件）
  改为数据根 `.registry.yaml`（server 安装时写入的位置）——修复卸载
  找不到已装文件清单、根注册表条目残留的问题
- [x] 实机无历史数据（两条路径均未安装过），无迁移成本
- [x] 验证：构建零错误、libximecore 全部套件通过

### 2026-09-30 词典管理（对齐 weasel DictManagementDialog）
- [x] **librime 封装**：用户词典函数在 **levers API**（非主 API），levers.rs
  新增 list_user_dicts / backup_user_dict / restore_user_dict /
  export_user_dict / import_user_dict；lib.rs 补 get_user_data_sync_dir
- [x] **IPC**：ListUserDicts / BackupUserDict / RestoreUserDict /
  ExportUserDict / ImportUserDict 五命令 + DictResponse（dicts/count/
  sync_dir）挂 IpcResponse.dict_response；server handler 调 librime
- [x] **设置词典页**（原占位页重写）：用户词典列表（每项 备份/导出/导入）+
  恢复快照（rfd 原生文件对话框，对齐 weasel 恢复流程）+ 快照目录展示 +
  刷新；操作结果（含导出/导入条数）经后台线程 + BackgroundPoll 回显；
  回调注册 set_notify_dict_*（host 包 IpcClient）
- [x] 依赖：libximecore workspace 加 rfd = "0.15"（Windows 原生文件对话框）
- [x] 验证：构建零错误、winxime-server 18/18

### 2026-09-30 修复托盘菜单「第一下无效」
- [x] 根因：TrackPopupMenu 后未补 `PostMessage(WM_NULL)`（KB135788），菜单
  跟踪未正确结束，下一次点击被当作取消吞掉；对齐 weasel SystemTraySDK
  的三件套（SetForegroundWindow → TrackPopupMenu → WM_NULL）
- [x] 验证：构建零错误、winxime-server 18/18

### 2026-09-30 托盘菜单渲染当前方案 switches（对齐 Android menubar）
- [x] **解析**：`schema_switches.rs` 读 `<rime>/<当前方案>.schema.yaml` 的
  switches 块（根目录优先、build/ 产物兜底），结构对齐 Android
  SchemaSwitch——布尔开关（name + states 两态标签）/ 多选一开关（options
  轮转 + states）；字符串简写条目跳过（与 Android 一致）；4 个单测
- [x] **托盘**：菜单改为弹出时全量重建（`build_menu`），「用户资料同步」与
  「关于」之间插入 switches 分组——布尔开关显示当前态标签 + 勾选，多选一
  显示激活标签（点击轮转）；ascii_mode 跳过（与顶部「切换中/英」重复）；
  无 switches 时不渲染该组
- [x] **切换**：`TrayAction::ToggleSwitch{name, options}` → engine 取反 /
  options 循环 setOption（对齐 Android toggleSchemaSwitch；未做 user.yaml
  持久化，后续可接 librime levers）
- [x] 验证：构建零错误（新代码无告警）、winxime-server 18/18

### 2026-09-30 托盘移除「立即云备份」
- [x] 托盘菜单删「立即云备份」项（TrayAction::BackupNow / MENU_ID_BACKUP /
  main.rs 分支一并移除）；备份功能保留在设置 → 同步与备份页，
  `PluginHost::backup_now` 公共 API 与单测不动（后续 IPC/插件中心可用）
- [x] 验证：构建零错误、winxime-server 14/14

### 2026-09-30 语音转文本设置页（v1：Windows WinRT 听写）
- [x] **架构对齐 Android xime speech 模块**：`RecognitionState` 状态机
  （Idle/Listening/Processing/Error）+ 后台 worker 独占引擎 + 共享结果槽
  （Android 回调 → Rust UI 250ms 轮询 `SpeechSink`）；页面在「智能」组
- [x] **v1 后端选型**：sherpa-rs 仅离线封装无流式识别器，故 v1 接 Windows
  自带 `SpeechRecognizer` 连续听写（零新模型/依赖，麦克风系统托管）；
  后端抽象保留，后续接与 Android 同款的本地 zipformer（sherpa-onnx sys）
- [x] 实现：`speech.rs`（worker 线程 MTA + 听写约束 Dictation + 约束编译
  一次复用 + ResultGenerated 逐短语追加 + 轮询等待异步，windows-future 0.3
  无阻塞 get）+ `SpeechState`（toggle/poll/clear/copy_text）+ 3 消息
  （SpeechToggle/SpeechClear/SpeechCopy）+ `pages/voice.rs`（状态行/结果面板/
  复制到剪贴板 arboard/清空）+ mic.svg 图标
- [x] 接线：`voice-page` feature（winxime-setup 启用）；MSIX 清单加
  `microphone` DeviceCapability；VoiceHandle Drop 时停会话收尾
- [x] 验证：构建零错误（新代码无告警）；libximecore 全部套件通过
- [ ] 后续：本地离线模型后端（sherpa-onnx zipformer，与 Android 同模型源）；
  IME 面板语音按钮直通

### 2026-09-30 rime 用户资料同步（对齐 weasel「用户资料同步」）
- [x] **定位重整**：原「云备份」是整包快照（tar.gz 覆盖式恢复），rime
  `sync_user_data` 是词库快照导出+多端合并（sync/<installation_id>/），
  两者正交。设置导航「云备份」→「同步与备份」，页首新增用户资料同步卡片
- [x] IPC：`SyncUserData` 命令（winxime-ipc）→ server 调
  `librime::sync_user_data()` + `join_maintenance_thread()`（对齐 weasel
  Configurator::SyncUserData；server 同进程免维护模式切换）
- [x] 托盘菜单「用户资料同步」（MENU_ID_SYNC，走 IPC 回环与设置同路径）
- [x] 设置页：用户资料同步卡片（本机标识 / 快照目录 / 上次同步相对时间 /
  立即同步）；`RimeSyncState` 解析 installation.yaml + sync 目录设备列表
- [x] 回调：`set_notify_sync_user_data` 注册到 `IpcClient::sync_user_data`
- [x] 验证：构建零错误；libximecore 全部套件通过；winxime-server 14/14
- [ ] 后续：sync/ 目录上云（WebDAV 插件承载，与 Android 同目录约定）

### 2026-09-29 快捷发送卡片对齐历史卡 + 两列表翻页
- [x] **样式统一**：提取 `card_button` 共用组件（点击选中 / 主色边框 / hover），
  快捷发送卡与历史卡完全同款——删除按钮仅选中时出现（此前常驻右上）
- [x] **翻页**：两列表每页 8 条（2 列 × 4 行），页脚分页条（上一页 / 第 x / y 页 /
  下一页，首末页置灰禁用；单页不显示）；state 加 `page` + total_pages/clamp/
  prev/next（列表缩减后自动夹回），4 个翻页消息；单测 `list_pagination_pages_and_clamps`
- [x] 验证：构建零错误、xime-setup 12/12

### 2026-09-29 剪贴板同步「配了但不推送」修复
- [x] **根因**：`clipboard_sync.toml` 不存在——「启用剪贴板同步」开关从未打开
  （填插件配置表单只写 plugins/config/<id>.yaml，不写选型文件）；服务端日志
  全程无 ReloadPlugins、worker 显示「同步插件=未启用」，推送静默跳过
- [x] 服务端自愈：`clipboard_poll_remote`（30s Tick）先 `sync_clipboard_worker`
  对齐选型，开关打开后无需重启 IME 即生效（与 IPC ReloadPlugins 互为兜底）
- [x] 推送失败补告警日志（此前插件返回 false 静默）
- [x] 设置页：同步未启用时保存配置即提示「请先打开启用开关」
- [x] 验证：构建零错误、winxime-server 14/14

### 2026-09-29 操作按钮并入 Tab 栏（space-between 页头）
- [x] 历史页（刷新/清空历史）与快捷发送页（添加）的操作按钮从列表前的
  「操作」行上移到**页头 Tab 栏右侧**：`clipboard_header` = Tab 栏 +
  `Space(Fill)` + 操作行，同一行 space-between 布局（同步页无操作按钮）
- [x] 顺带更新空态文案（「刷新」「添加」位置改为右上角）
- [x] 验证：构建零错误

### 2026-09-29 快捷发送卡片样式与历史卡统一（选中交互）
- [x] 快捷发送卡片改为与剪贴板历史同款：**点击选中**（主色 1.5px 边框 +
  背景加深 + hover 反馈），「删除」按钮仅在选中时出现（此前常驻卡片右上）；
  常显的删除钮移除后卡片内容为标题（文本前缀）+ 内容摘要（含编码标记）
- [x] 结构：`QuickSendState.selected: Option<i64>` + `select`；
  `QuickSendSelected(i64)` 消息 + 分发
- [x] **操作工具栏移至表头**：历史（刷新/清空历史）与快捷发送（添加）的
  操作按钮从列表尾部的「操作」行改为列表上方的工具栏行；顺带修正快捷发送
  分组描述（存储已是 clipboard.db，不再是 quick_send.yaml）
- [x] 验证：构建零错误、14/14

### 2026-09-29 剪贴板历史卡片：选中交互 + 操作按钮
- [x] **交互**：点击历史卡片选中（主色边框高亮 + 背景加深），再点取消；
  选中时卡内出现「添加到快捷发送」「删除」两个按钮
- [x] **实现**：
  - store：`ClipboardHistoryItem` 增加 `id` 列（list 查询带 id），新增
    `remove_history_item`（按 id 删单条）
  - state：`ClipboardHistoryState.selected: Option<i64>` + `select`（点击
    切换）/`remove`（删库 + 刷新 + 清选中）；`QuickSendState.add_from_text`
    （历史文本直接加为快捷发送，无触发编码）
  - 消息 3 个：ClipboardHistorySelected / ClipboardHistoryRemove /
    QuickSendFromHistory；卡片改为 button（点击 + hover 反馈），选中态
    主色 1.5px 边框
- [x] 验证：构建零错误、14/14

### 2026-09-29 修复（二次）：OpenClipboard 传监听窗口句柄而非 NULL
- [x] **日志实锤的新矛盾**：`OpenClipboard(None)` 返回成功（无重试耗尽警告），
  但紧随的 `GetClipboardData` 报 `ERROR_CLIPBOARD_NOT_OPEN`（1418「线程没有
  打开的剪贴板」）且 `EnumClipboardFormats` 为空——打开状态在两调用之间
  无效，指向 `OpenClipboard(NULL)` 在窗口消息循环线程上的关联不可靠
- [x] **修复**：新增 `LISTENER_HWND` 静态句柄（start_listener 创建监听窗口
  后记录），`open_clipboard_with_retry` 改传 `OpenClipboard(Some(监听窗口))`
  ——**传真实窗口句柄是剪贴板管理器的常规做法**（NULL 关联在消息循环
  上下文中的行为 quirk 规避）；重试逻辑保留
- [x] 验证：构建零错误、14/14；效果待 rebuild 后复制确认（诊断日志仍保留：
  若仍有问题，warn 会给出错误码与实际格式枚举）

### 2026-09-29 修复：剪贴板历史仍为空（链路断点=OpenClipboard 竞态）
- [x] **诊断**（链路足迹日志实锤）：复制时「剪贴板事件触发: Changed」有日志、
  「剪贴板变化进入宿主」无——断在 `read_text()`：WM_CLIPBOARDUPDATE 到达时
  来源应用可能仍持有剪贴板锁，`OpenClipboard` 一次失败即放弃 → 事件静默丢弃
- [x] **修复**：`read_text`/`write_text` 的 `OpenClipboard` 加 5 次 × 10ms 重试
  （`open_clipboard_with_retry`，Windows 剪贴板读取的常规做法），重试耗尽打
  warn 日志
- [x] 顺带发现：worker 日志中 db 路径为小写 `xime`（与实际目录 `Xime` 大小写
  不一致；NTFS 不区分大小写，功能无影响，属命名不一致待统一）
- [x] 验证：构建零错误、14/14；效果待 rebuild 后复制确认（链路日志全量
  足迹：事件触发 → 进入宿主 → worker 收到 → 历史已记录）

### 2026-09-29 剪贴板存储迁移 JSON/YAML → SQLite（对齐 Android clipboard.db）
- [x] **背景**：安卓端剪贴板历史与快捷发送共用 SQLite（Room clipboard.db
  v4，表 clipboard_entries，快捷发送即 isQuickSend=1 子集，含触发编码
  code 列）；Windows 端此前用 clipboard_history.json / quick_send.yaml，
  与安卓 schema 不通
- [x] **libximecore 新增 `xime-config/src/clipboard_store`**（rusqlite
  bundled，工作区已声明 0.32）：
  - 建表语句与 Android Room 实体逐列对齐（id/text/code/timestamp/
    isPinned/isQuickShare/isQuickSend/consumed/type/imagePath/imageHash/
    mimeType/sizeBytes/width/height + text/imageHash 索引），库名同为
    clipboard.db（%APPDATA%\xime\），WAL 多进程安全
  - API：append_history（同文本去重移前 + 容量裁剪，快捷发送条目不受
    历史裁剪波及）/ list_history / clear_history / list_quick_send /
    add_quick_send / remove_quick_send / migrate_legacy
  - **旧 JSON/YAML 自动迁移**：server 与设置程序任一首次打开时幂等迁移
    （导入后改名 *.migrated）
  - 单测 3 项（去重移前+容量裁剪、快捷发送增删+清空保留、旧文件迁移）
- [x] **两端接线**：server worker 历史写入改走 store（截断 2000 字符）；
  设置程序历史/快捷发送状态全部改走 store（删除 JSON/YAML 读写）；
  **对话框从「名称/内容」改为「内容/触发编码」**（对齐安卓 QuickSendItem
  {id,text,code,timestamp,isPinned}——无独立名称列，列表标题取文本前缀，
  触发编码是安卓的真实功能：输入编码前缀条目进入候选栏）
- [x] 验证：xime-config 8/8、winxime-server 12/12、debug/release 零错误

### 2026-09-29 剪贴板页改版：Tabs 结构（历史 / 快捷发送 / 同步）
- [x] 页面重构为三个 Tab（样式对齐扩展商店页 tab_bar）：
  「剪贴板历史」（默认）、「快捷发送」、「剪贴板同步」；页标题改「剪贴板」
  （与侧栏菜单项一致）
- [x] 结构：view() 拆为 tab 栏 + 三个内容源（history_groups /
  quick_send_groups / sync_groups），`SettingsState.clipboard_tab` 记忆当前
  Tab，`Message::ClipboardTab(usize)` 切换
- [x] **添加快捷发送改为弹窗**（iced 0.14 无内置 Modal，widgets.rs 新增
  `modal_dialog` 通用组件：stack + 半透明遮罩 + 居中卡片，项目内可复用）：
  列表页只留「添加」按钮 → `QuickSendOpen` 弹出对话框（名称/内容 +
  取消/添加），确认成功自动关闭（内容为空保持打开），取消清空草稿
- [x] 验证：debug/release 构建零错误、14/14
- [x] **历史/快捷发送列表改 2 列卡片网格**（iced 0.14 Grid：columns(2) +
  height(Shrink)）：新增 `list_card` 通用单元样式（浅前景底色圆角卡）；
  历史格 = 截断文本；快捷发送格 = 名称行（semibold + 撑开 + 删除按钮）
  + 内容摘要；「刷新/清空/添加」操作项保持全宽

### 2026-09-29 设置页：剪贴板历史 + 快捷发送展示与编辑
- [x] **剪贴板历史**（文件契约，零 IPC 改动，与 clipboard_sync.toml 同模式）：
  - server 剪贴板工作线程每次本地复制/远端写回时持久化
    `%APPDATA%\Xime\clipboard_history.json`（读改写：内容去重移到最前
    （对齐 Windows 历史语义）、容量 50 条、单条截断 2000 字符；文件为
    唯一事实源，设置页清空 = 写空文件，server 下次追加自然接续）
  - 设置页「剪贴板历史」分组：最近 8 条（截断 60 字符展示）+ 刷新/清空按钮
    （接通上游预留的 `Message::ClearClipboardHistory`）
- [x] **快捷发送**：`%APPDATA%\Xime\quick_send.yaml`
  （`items: [{name, content}]`）——设置页「快捷发送」分组：列表（名称+内容
  摘要+删除）+ 新增草稿（名称留空取内容前 12 字符）；后续输入法候选栏
  「快捷发送」面板消费同一文件（host.quickSend 上游暂为占位）
- [x] 结构：xime-setup 新增 `ClipboardHistoryState/QuickSendState`
  （cfg clipboard-page 门控）+ 5 个 Message 变体 + update 分发；
  server 新增 `record_clipboard_history`（worker 线程内调用）
- [x] 测试 14/14（新增 clipboard_history_persists_and_dedups：追加去重
  移前 + 清空接续）；debug/release 构建零错误

### 2026-09-29 设置程序 ASCII 符号乱码修复（用户截图实锤定位）
- [x] **现象**：剪贴板页所有含 ASCII 的文字渲染成错误符号
  （"Web"→"Ⓐ−▼"、"Android"→"A■_↓X"、"30"→"←¯"），中文全部正常；
  字符数一一对应（非缺字形豆腐块），per-char 映射基本确定
- [x] **根因**：设置程序未指定具体字体，text 控件用 iced 通用族
  （Sans Serif）交给 fontdb 在系统字体中解析；该机器上通用族解析命中
  **图标字体**（Segoe Fluent Icons 类，也被归类为 sans-serif）——图标
  字体把 ASCII 码位映射成符号字形；CJK 不被图标字体覆盖、回退雅黑，
  所以只有拉丁/数字乱码
- [x] **修复**（libximecore xime-setup）：`components/widgets.rs` 新增
  `UI_FONT = Font::with_name("Microsoft YaHei UI")`（Windows 全版本自带、
  拉丁+中文覆盖完整），medium()/semibold() 改为基于 UI_FONT；
  app.rs `run()` 加 `.default_font(UI_FONT)`——所有未显式设字体的
  text 控件（含 pick_list/输入框/button）统一走雅黑 UI，通用族解析
  彻底不再参与
- [x] 验证：构建零错误、13/13；效果需 rebuild.ps1 后打开设置确认

### 2026-09-29 候选栏菜单面板留白修复
- [x] **问题**：菜单面板上下留白特别多——菜单页顶部预留了 36px 标题栏 + 8px
  间距但菜单页不画标题（「← 菜单」是子页面才有）→ 顶部空 44px；底部品牌栏
  32px 只有一行小字。内容卡片仅占 232px 面板中的 140px（60% 是留白）
- [x] **修复**（ui/panel.rs）：
  - 菜单页行槽改 `panel_menu_row_y`：从 `PANEL_MENU_TOP(10)` 起，不再预留标题栏
  - 面板高度 232 → 198（10 + 4 行卡片 140 + 间距 8 + 品牌栏 32 + 底边距 8），
    窗口高度经 `panel_extra_height` 自动跟随
  - 子页面占位文本改在「标题栏底 ↔ 品牌栏顶」间居中（原先引用菜单行几何）
  - 测试同步（行槽断言改新函数 + 新增首行紧贴顶部断言）；顺手清掉
    libximecore clipboard.rs 在 Windows 下的两个 unused import 警告
  - 验证：构建零错误、winxime-server 13/13

### 2026-09-29 修复：中文态 Shift+符号键无法上屏（如打「问题」后 Shift+/ 出不来 ？）
- [x] **根因**：`vk_to_xk(vk)` 无 shift 概念，Shift+/ 发给 rime 的是
  `XK_SLASH + SHIFT`；而 X11/weasel 语义是上报**移位后的字符 keysym**
  （`XK_question`），librime 的 punctuator/key_binder 按 '?'、'(' 等字符
  keysym 登记 → 永远匹配不上 → 按键被吞、无输出。数字行同理（Shift+9 的
  （、Shift+1 的！等全部失效）
- [x] **修复**（libximecore crates/librime/src/key.rs）：
  - `vk_to_xk(vk, shift: bool)`：shift=true 时数字行（0-9）与 OEM 键
    （;=,-./[\]' 共 11 键）返回移位字符 keysym（新增 21 个 XK_* 常量，
    ASCII 可见字符 keysym == ASCII 码）；无移位字符的键忽略 shift
  - 字母键不变：基键保持小写，大小写语义由 SHIFT 修饰位承载（X11 语义）
  - 新增单测 `test_vk_to_xk_shifted_printables`（22 项断言）；librime key
    测试 12/12 通过
- [x] **TSF 调用点**（text_input_processor.rs）：`handle_key_event` /
  `handle_key_up_event` 传入 `mods & K_SHIFT_MASK as i32 != 0`，
  key-up 与 key-down 同规则（配对一致）
- [x] **修复：Shift+符号键误触发中英切换**：OnKeyUp 对 VK_SHIFT 无条件
  toggle_ascii_mode，Shift+/ 松开 Shift 即切换。加 `shift_solo` 单按判定：
  Shift 按下置位，期间任何其他键按下/抬起（含 VK_CONTROL 早退路径之前）即
  清除；Shift 抬起时仅在仍置位时才切换（对齐 weasel「空按 Shift」语义）
- [x] **修复：中文态 Ctrl/Alt 组合键失效**（如 Ctrl+A/C/V/F5，英文态正常）：
  `should_handle_key` 不看修饰键，中文态对字母/数字/符号键一概认领 →
  应用跳过自身加速器路径 → rime 不处理（无 Ctrl/Alt 绑定）→ 按键丢失。
  修复：Ctrl 或 Alt 按住时不认领任何非修饰键（修饰键本身豁免——组合中
  Ctrl 按下仍需进入 OnKeyDown 触发字根提示 show_root）
- [x] **修复：中文态回车失效**（server 日志实锤：不组词时 rime 对回车
  `handled:false`）：同一类病——`should_handle_key` 不组词也认领回车/
  退格/Esc/Tab/空格/数字/翻页/方向键，但这些键只在组词中被 rime 消费
  （上屏原始码/删码/选候选/翻页/移光标）。修复：这些键不组词时不再认领
  （交应用原生处理）；字母（起始组词）与标点键（punctuator 上全角）保持
  认领。**认领原则沉淀：只在 rime 会处理的键上认领，认领 = 承诺消费**
- [x] **按键层重构：对齐 weasel KeyHandler 架构，根除认领启发式**
  （对比 weasel-0.17.4 WeaselTSF/KeyEventSink.cpp）：
  - weasel 无任何认领猜测——OnTestKeyDown 即完成整个按键处理（IPC 询问
    rime），rime 说吃才吃；OnKeyDown 只重放结果；`_fTestKeyDownPending/
    _fTestKeyUpPending` 应对怪异应用（多次 TestKeyDown / 只调 KeyDown，
    如 QQ、Word）
  - 本层同构实现：`process_key_event(context, vk, is_up)` 统一处理
    （合并原 handle_key_event/handle_key_up_event），四个 On* 入口全部
    改为 pending 重放模式；**删除整个 should_handle_key 启发式**——
    此前回车/Ctrl+A 两类丢键 bug 的根源（认领 = 猜测，猜错即丢键）从
    结构上消除，rime 拒绝的键天然交还应用
  - 保留的本地决策：修饰键（Shift/Ctrl/Alt 单按）不经 rime——Shift 中英
    切换走本层 shift_solo、Ctrl 走 show_root；英文态本地短路不询问 rime
    （对齐 weasel keyboard-open 检查）；shift 移位 keysym 转换与 weasel
    ToUnicodeEx 语义一致（移位后字符作 keysym）
  - 与 weasel 的已知差异（后续对齐项）：Caps Lock 事件不转发 rime
    （weasel 转 Caps_Lock 给 ascii_composer，含双按还原 SendInput 逻辑）；
    小键盘 VK_NUMPAD 映射 ASCII 数字而非 KP_*；VK→字符转换用静态美式
    布局表而非 ToUnicodeEx（非美式键盘布局移位字符可能不准）
- [x] **修复 CI 构建**：CI 用 git 依赖 libximecore a06f864（.cargo/config.toml
  本地 patch 不入库），其 vk_to_xk 是单参——移位映射从上游 key.rs 挪回
  winxime-tsf 本地（vk_to_xk_shifted/vk_to_xk_with_shift 包装 +
  VK_OEM_* 常量），libximecore 本地 key.rs 改动已回退（上游推送移位支持
  前本地/CI 编译路径一致）；代码只依赖上游已发布 API
- [x] 教训：严禁用 PowerShell Get-Content/Set-Content 改 UTF-8 源码
  （PS5.1 按 GBK 读写，中文注释全部乱码且可能吞换行）；改源码一律用
  Edit/Write 工具

### 2026-09-29 适配 libximecore：插件运行时 mlua(Lua) → quickjs-rusty(JS)
- [x] **libximecore 拉取**（a2853e6 → a06f864），关键变化：
  - 插件运行时迁移 QuickJS，契约对齐 xime 3.0 Android `JsScriptRuntime`
    （manifest.json 优先兼容 yaml、入口 main.js、`globalThis.plugin` 分组命名空间、
    契约调用硬超时 + 超时熔断、网络门禁 fail-closed）
  - backup 契约改名并类型化：`backup_push/list/pull/delete` →
    `push_backup → BackupUploadResult{ok,id,message}`、`list_backups → Vec<RemoteBackupEntry>`、
    `pull_backup → Option<Vec<u8>>`、`delete_backup → bool`；
    clipboard `push/pull` 返回值由 `Option<..>` 扁平化
  - **PluginRuntime 不再是 Send**（QuickJS 裸指针）：运行时必须活在创建线程内
  - host.http 底层 ureq → reqwest blocking（支持 PROPFIND/MKCOL，非 2xx 也返回响应对象）
- [x] **winxime-server 宿主线程模型重构**（plugins.rs）：
  - backup 类操作改为「一操作一实例」：调用线程内 `PluginRuntime::load` + `call_on_load`
    + 执行 + 即弃（对齐 libximecore setup 侧 `load_plugin_runtime` 模式）
  - 剪贴板同步改为**专用工作线程**独占持有运行时，宿主经 mpsc 投递
    `LocalChanged/PollTick` 命令；三通道去重状态（当前/上次推送/自写回显）归线程所有，
    不再加锁；远端拉取命中后由工作线程直接写回系统剪贴板
  - main.rs 剪贴板回调从「每事件 spawn 线程」简化为「非阻塞投递命令」
- [x] **内置插件迁移 Lua → JS**（源码取自 Xime 仓库 plugins/ 的 xime 3.0 版，
  libs 内联为单文件）：
  - `resources/plugins/webdav-backup`：manifest.json（id 不变，配置无缝延续；
    platforms + windows）+ main.js（backup.test/push/pull/list/remove + settings.schema）
  - `resources/plugins/webdav-clipboard-sync`：manifest.json + main.js
    （clipboardSync.push/pull/test + ETag 条件拉取 + 503 限流退避 + 附件 blobs 契约
    （桌面端休眠））；旧 main.lua/manifest.yaml 已删除（force 安装会清空旧目录完成迁移）
- [x] **构建环境**：quickjs 的 `libquickjs-ng-sys` 需要 bindgen(libclang) + clang 编译 C 源码，
  本机原先无 LLVM → scoop 用户级安装 llvm 23.1.2；`.cargo/config.toml`（未跟踪）
  增加 `[env] LIBCLANG_PATH / TARGET_CC` 指向 scoop LLVM
- [x] 测试：winxime-server 12/12（新增 4 项：backup_now 走真实 QuickJS 运行时跑通
  契约调用、无插件时报错、剪贴板线程启动门禁、clipboard_sync.toml 选型）；
  libximecore xime-plugin 43/43（1 忽略）
- [x] **剪贴板功能接入设置程序（对齐上游 7d22192「选中即启用并通知 daemon」）**：
  - winxime-setup 启用 xime-setup-lib 的 `clipboard-page` + `backup-page` feature
    （此前未启用，设置程序里看不到剪贴板/云备份页——「怎么没有剪切板功能」的根因）
  - winxime-ipc 新增 `ReloadPlugins` 命令 + `IpcClient::reload_plugins()`
  - winxime-server ipc_server 处理 ReloadPlugins → `PluginHost::reload()`
    （重扫启用清单 + 按 `clipboard_sync.toml` 选型对齐剪贴板工作线程：
    应启未启→启动、选型变更→停旧起新、应停→Shutdown）
  - 宿主遵守 `clipboard_sync.toml`（setup 写入，与 Android daemon 契约共享）：
    enabled + plugin_id 精确选中同步插件；无 toml 时不启用（明确 opt-in）
  - main.rs 插件宿主创建提前到 IPC 线程启动之前（IPC 需引用）
- [x] libximecore 小改：剪贴板页「同步服务器（xime-sync-server）」分组
  cfg(target_os="linux") 门控（Windows 端不分发该服务，隐藏以免误导；
  server_groups 抽成函数解决非 Linux 下闭包类型推断失败）
- [x] **「奇怪符号」真正根因：候选栏 ⋮ 菜单面板的 emoji 图标渲染成方框**
  （用户口中的"剪切板页面"即面板第一张卡片"📋 剪切板"）：panel.rs 用用户候选
  字体渲染 emoji 字符，中文字体无 emoji 字形 → 方框。修复：图标改用系统
  "Segoe UI Emoji" 字体 + `D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT`
  （Win10+ 彩色字形），标签文字仍用候选字体
- [x] 次要修复：剪贴板设置页 helpText 中的 `→`（U+2192）改为纯中文表述
  （iced 字体回退同样可能缺字形；运行时 dump schema 全字段排查，其余字符串
  均干净；manifest 的 ☁️ icon 不被设置 UI 渲染。iced 无法渲染彩色 emoji，
  第三方插件 schema 文案含 emoji 会显示方框——上游待办）
- [x] **msix-bundle.ps1 加固：被占用旧包目录改名让位**——rebuild 时旧
  winxime_tsf.dll 常被仍开着输入法的宿主进程映射（不允许删除/覆盖，
  允许改名）：Remove-Item 失败时整目录改名 `msix-pkg.old-<时间戳>` 让位
  再重建，历史让位目录下次构建自动清理；避免半删除半复制的脏暂存状态
- [ ] 后续功能点：设置程序插件中心页对接新契约（settings.schema/启停通知）；
  插件市场下载（install_from_zip + plugin_download_temp_path 已就绪）；
  候选栏剪贴板/备份入口卡片接线

### 2026-09-30 输入方案安装隔离闭环（builtin 方案包 + 冲突确认 + 精确卸载 + 备份还原）
- [x] **问题**：设置程序「输入方案 → 已下载 → 安装」把第三方方案包全量释放进 rime 根目录，
  第三方方案文件与内置方案（rime-wubi：wubi86*/pinyin_simp/symbols/lua…）混装；内置方案文件在
  注册表里无人认领 → 第三方包可**静默覆盖**它们（本机历史数据里 rime-ice 就覆盖了
  `custom_phrase.txt`、`lua/date_translator.lua`），卸载时又把这些内置文件一并删掉
- [x] **安卓参照**（Xime `SchemaManifestManager` + `SchemaLocalViewModel`）：untracked 文件归
  `builtin` 包 + `market/builtin/` 备份（`ensureBuiltinBackup`/`refreshBuiltinManifest`）、
  `detectConflicts`（claimedBy + sha256，含 builtin）、`uninstallWithManifest`（claimedBy 共享保护）、
  安装前「rime 目录已有其他方案包」→ 弹确认「需要先卸载冲突方案」→ `confirmInstallWithUninstall`
- [x] **libximecore `xime-config::schema_manifest`（新模块，唯一权威实现）**：
  - 数据根 `.registry.yaml`：`<pkg>: {files: [...], sha256: {rel: hex}}`（旧 `files:` 列表向后兼容）
  - `refresh_builtin_package`：无主文件 → builtin 包 + 备份 `market/builtin/`（含 sha256；
    已消失文件撤声明，避免幽灵条目）
  - `detect_conflicts`：异内容冲突 / 同内容视为共享依赖放行 / 同包重装升级放行
  - `uninstall_package`：claimedBy 保护（其他包仍声明的共享文件保留）+ 衍生产物清理
    （`<id>.custom.yaml`、`<id>_merged.dict.yaml`、`build/<id>.*`）+ 空目录剪枝
  - `restore_builtin_package`：从 `market/builtin/` 备份还原内置方案包
  - 路径规则：用户数据（`*.custom.yaml`/`*.userdb/`/`build/`/`themes/`/`sync/`/`installation.yaml`/
    `custom_phrase.txt`）与宿主自有配置（`default.yaml`/`xime.yaml`/`user.yaml`/`squirrel*`/`weasel*`/
    `.registry*`）永不入清单、卸载不删
  - 单测 7 项：登记与跳过用户数据、冲突/共享/同包重装、卸载共享保护与衍生清理、旧注册表兼容、
    备份还原、归属优先第三方包、路径规则
- [x] **xime-setup（设置程序）**：
  - `do_install` 全走清单：refresh builtin → 全量释放（过滤受保护路径）→ sha256 冲突检测 →
    释放 → 写包清单；`do_uninstall(pkg, deploy)` 同样走清单，启用列表按「包名下全部方案 id」移除
    （此前只按包 id 过滤，rime-ice ↔ rime_ice 这类不同名根本删不掉）
  - `install_market_schema` 冲突预检 → 弹窗（`ConfirmSchemaInstall` / `CancelSchemaInstall`）→
    `confirm_schema_install` 先逐个精确卸载冲突包（`deploy=false`，不重复数秒级部署）再安装
  - 「已安装」列表**按方案包分组**（内置方案包在前，标注 内置/第三方 + 每行来源），
    内置方案包被卸载后出现「还原内置方案」卡片（`RestoreBuiltinSchema`）
  - 已安装包列表改以**注册表**为准（此前用磁盘上全部 `*.schema.yaml`，package id ≠ schema id 时判定恒错）
  - 冲突弹窗在 app.rs 全局渲染：输入方案页与扩展商店两个安装入口共用同一隔离流程
- [x] **winxime-server**：启动部署内置数据后 `register_builtin_schema_package()`
  （`ensureBuiltinBackup` + `refreshBuiltinManifest` 的 Windows 对应物），内置方案文件启动即登记+备份，
  安装/卸载都在同一注册表事实上工作
- [x] 验证：`cargo build --quiet` 零错误；xime-config 15/15、xime-setup-lib 13/13
- [ ] 遗留：①server 侧旧 `schema_manager.rs`（IPC `InstallSchema`/`UninstallSchema`）仍是
  「只拷 .schema.yaml」且用 `packages:` 包裹的旧注册表格式，与设置程序实现重复且格式不兼容——
  当前 UI 无调用入口（死路径），待统一到 `xime_config::schema_manifest` 或删除；
  ②历史混装数据无法追溯归属（修复前已被第三方包覆盖的内置文件丢失原内容），
  重新/修复安装后 builtin 备份才完整

### 2026-09-30 选中方案改用 rime 自己的记录（删除数据根 selected_schema.txt）
- [x] **事实**：选中方案本就由 librime 自己持久化——`RimeSelectSchema` → `Engine::ApplySchema`
  → `Switcher::SetActiveSchema` 把 `var/previously_selected_schema`（+ `schema_access_time`）
  写进**用户目录 `rime/user.yaml`**，`Switcher::CreateSchema` 建会话时读回。
  数据根 `selected_schema.txt` 是重复的第二份记录（两处必然不同步）
- [x] **改动**：server 删除 `persist_selected_schema`（不再写 txt）；启动恢复改为
  `load_rime_selected_schema()` 读 `rime/user.yaml` 的 `var/previously_selected_schema`
  （纯函数 `parse_rime_selected_schema`，2 项单测：真实 user.yaml 形态 / 无记录与非法内容返回 None）
- [x] 显式恢复保留的原因：rime 建会话只在 **schema_list 之内**按该字段恢复，而设置程序允许
  选中未启用（不在 schema_list）的方案（`RimeSelectSchema` 按 id 直选不受列表限制）；
  待「选中即写入 schema_list」后这个显式恢复即可一起删掉
- [x] 已清理本机历史残留 `%APPDATA%\Xime\selected_schema.txt`（数据根现只剩
  `.registry.yaml` + 剪贴板/配对/密钥等真实数据文件）
- [x] 验证：`cargo build --quiet` 零错误；winxime-server 新增 2 项单测通过
- 注：本机 DSH 沙箱下 `%TEMP%` 建目录被拒（OS error 5），models/schema_switches/plugins
  共 7 项既有测试失败；已用 `git stash` 对照确认与本次改动无关（改动前同样 7 项失败）

### 2026-09-30 方案来源互斥（rime 目录同一时刻只允许一个来源）
- [x] **要求**：rime 目录里不允许出现多个方案来源（内置方案包 + 第三方包）混装——
  与安卓一致：装第三方方案时先把内置方案包卸掉，而不是两者共存
- [x] **安装路径**（已有）：`install_market_schema` 预检 → 冲突弹窗 → 确认后逐个精确卸载
  其余方案包（`deploy=false`）再安装目标包（只部署一次）= 装完只剩一个来源
- [x] **还原路径**（本次补）：`restore_builtin_schema` 同样走冲突预检
  （新增 `schema_restore_conflict()`：注册表里除 builtin 外的全部方案包），
  确认后先卸载第三方包再还原内置包 → 不允许还原成混装；
  弹窗文案按目标区分（`SchemaInstallConflict::is_restore_builtin()`）
- [x] **历史混装**：**不做界面提示**（对齐安卓——安卓只在安装时提示一次，没有常驻的
  混装告警）。历史混装靠动作自然收敛：装包（预检 → 确认 → 卸掉其余来源再装）、
  还原内置（同样先卸第三方）、卸载（卸掉市场包时 `refresh_builtin_package` 会把无主
  方案文件登记回内置方案包 → 目录里只剩内置一个来源）。
  曾实现「混装告警卡片 + 一键只保留某来源」（`schema_sources` / `KeepOnlySchemaSource` /
  `mixed_sources_card` / `keep_only_schema_source`），按要求**已删除**
- [x] **用户数据安全**：~~不删未登记的 `<id>.custom.yaml`~~ → **改为对齐安卓**：
  `uninstall_package` 按 `uninstallWithManifest`（安卓 333-361 行）删除该方案的
  `<id>.custom.yaml`、`<id>_merged.dict.yaml`、**方案短语表**（`custom_phrase.txt`，
  或 `custom_phrase.user_dict` 声明的 `<名>.txt`）、`build/<id>.*`；
  保留 `*.userdb/` 输入记录与受保护文件（`default.yaml`/`xime.yaml`/`themes/`）。
  短语表名必须在删文件**之前**解析（安卓 305-308 行注释点明：删掉
  `<id>.schema.yaml`/`<id>.custom.yaml` 后就解析不出 `user_dict` 了）——
  第一版顺序写反，被单测抓出
- [x] 新增回归单测 `mixed_sources_converge_to_single_source`：复现本机真实状态
  （内置 wubi86 系列 + 第三方包声明共享 `symbols.yaml`）→ 卸载内置后
  方案文件与 `<id>.custom.yaml` 删除、共享文件/受保护文件/输入记录保留、
  注册表只剩一个来源；再卸载第三方 → 从 `market/builtin/` 还原内置，仍是单来源
- [x] 新增单测 `custom_phrase_dict_name_follows_user_dict_declaration`：块式/行内式/
  补丁式（`custom_phrase/user_dict:`，安卓不认、我们多认）三种 `user_dict` 写法
- [x] 验证：`cargo build --quiet` 零错误；xime-config schema_manifest 9/9 通过
  （xime-setup-lib 3 项 `%TEMP%` 权限类失败是本机沙箱既有问题，与本次改动无关）

### 2026-09-30 打包产物改名 xime → ximeyao（对齐项目名曦码·曜）
- [x] 问题：项目叫 XimeYao（曦码·曜），但打包产物文件名还是 `xime-*`
- [x] 统一命名：`ximeyao-{version}-x86_64.msi` / `ximeyao-{version}-x86_64.msix`
  （MSI 顺手补上缺失的 `-x86_64`，修复 AGENTS.md 文档与实际产物名的漂移）
- [x] `msix-bundle.ps1`：输出路径 + 构建横幅（Building XimeYao (曦码·曜) MSIX）
- [x] `msi-build.ps1`：light -out 路径 + 结果检查路径 + 构建横幅
- [x] `install-msi.ps1`：去掉硬编码 `xime-0.1.0.msi`，改为自动从 Cargo.toml 读版本
  （与其他脚本一致，否则改名后必坏）
- [x] `ci.yml`：light -out、MakeAppx /p、两个 upload-artifact 的 path 通配
- [x] `code-signing.yml`：签名产物改名 `winxime.msi` → `ximeyao.msi`，
  mv 源从硬编码 `winxime-server-0.1.0-x86_64.msi` 改为 `ximeyao-*.msi` 通配
  （原硬编码名与实际产物从来对不上，全靠 `|| true` 掩盖）
- [x] 文档同步：AGENTS.md「MSI 构建」、README 安装命令与输出路径
- [x] 不改的部分（有意）：二进制名 winxime-server.exe 等（涉及 crate/IPC 管道/TSF
  注册，牵一发动全身）；安装目录 `Program Files\Xime`、注册表 `Software\Xime`、
  数据目录 `%APPDATA%\Xime`（改动会孤立既有安装与用户数据）；WiX 产品名本就是
  「曦码·曜」无需动
- [x] 验证：`cargo build --quiet` 零错误；三个 ps1 通过 PowerShell 语法解析检查；
  全仓 grep 无旧产物名残留（release.yml/AppPackageAutoUpdate.yml 用 `*.msi/*.msix`
  通配，不受影响）
