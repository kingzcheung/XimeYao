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
