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