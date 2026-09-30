# 曦码·曜 (Xime Yao) 五笔输入法 - 架构决策

## 2026-05-06 Workspace 架构决策

### 决策
采用 5 个 crate 的 workspace 架构

### Crate 分离方案
1. **winxime-tsf** - TSF (Text Services Framework) 集成
   - Windows TSF 接口实现
   - 输入法注册与管理
   - 与应用程序通信

2. **winxime-rime** - rime 引擎封装
   - librime FFI 绑定
   - rime API 封装
   - 方案管理

3. **winxime-ui** - GUI 渲染
   - winit 窗口管理
   - skia-safe 候选词渲染
   - 窗口样式与交互

4. **winxime-core** - 核心逻辑
   - 输入法状态机
   - 候选词处理
   - 配置管理

5. **winxime** - 主入口
   - 整合所有组件
   - 应用程序入口点

### 技术选型理由
- **TSF**: Windows 官方输入法框架，兼容性好
- **rime**: 开源输入法引擎，五笔支持完善
- **winit**: 跨平台窗口管理，Rust 生态成熟
- **skia-safe**: 高性能 2D 渲染，Skia 的 Rust 绑定

### 依赖关系
```
winxime (主入口)
├── winxime-core
├── winxime-tsf
├── winxime-rime
└── winxime-ui
    └── winxime-core (共享类型定义)
```
## 2026-09-11 Windows 端 rime 数据目录：单目录模型（对齐 Xime）

### 决策
Windows release 环境下 `user_data_dir == shared_data_dir == %APPDATA%\Xime\rime`，
方案部署采用 Xime 语义：首装（目录内无 *.schema.yaml）全量复制安装目录自带 data/ +
user-data/；升级仅覆盖内容有变化且文件名不含 "custom" 的文件。

### 理由
- Xime（Android）与 ximed 均已迁移到单目录模型（旧 shared/user 分离导致方案来源混乱）
- 旧双目录模型下 shared 是 MSIX 只读包目录，且 `ensure_user_config_files` 被"有任意 yaml
  即跳过"条件永久短路，导致升级/修复部署无从发生
- MSIX 同时声明 `unvirtualizedResources`，保证包内外进程看到同一份真实 %APPDATA%\Xime

### 插件
插件框架复用 libximecore 的 `xime-plugin`（2026-09 起为 QuickJS 运行时，契约对齐
xime 3.0 Android `JsScriptRuntime`：manifest.json / main.js / `globalThis.plugin`）。
宿主做重活（打包/监听/去重），插件只做协议传输。插件与配置都放 `%APPDATA%\Xime\plugins`。

### 插件运行时线程模型（2026-09-29，PluginRuntime 非 Send）
QuickJS 的 Context 持有裸指针，`PluginRuntime` 不再实现 `Send`，必须活在创建线程内：
- backup 类操作「一操作一实例」：调用线程内 load → call_on_load → 执行 → 即弃
  （onLoad 幂等；对齐 libximecore setup 侧模式）
- 剪贴板同步由专用工作线程独占运行时，宿主经 mpsc 投递命令，去重状态归线程所有

### 下载数据目录映射（安卓 filesDir ↔ Windows %APPDATA%\Xime）
- 方案市场包 `files/market/{id}/` ↔ `market\<id>\`
- 插件 `files/plugins/{id}/` ↔ `plugins\<id>\`（注册表：安卓 plugins.xml / Windows registry.yaml）
- 模型 `files/models/{modelId}/` ↔ `models\<modelId>\`（独立于插件，用到才建，文件如实命名）
- 方案市场注册表在数据根（`.registry.json` / `.registry.yaml`），不放 market/ 内
- 下载临时文件用应用缓存（安卓 cache/ ↔ Windows %TEMP%\xime_plugin_*），即用即删

## 2026-09-30 输入方案安装隔离：内置方案包（builtin）+ 一次只装一个方案包

### 决策
方案包的**文件归属**由数据根 `.registry.yaml` 一处权威管理（`xime_config::schema_manifest`，
Windows 侧把安卓的 `.manifests/<pkg>.json` + `.registry.json` 收敛成一份），语义逐条对齐
安卓 `SchemaManifestManager`：

1. **builtin 包**：rime 目录里不属于任何市场包的方案文件自动归入 `builtin`（内置方案包），
   并备份到 `market/builtin/`；注册表记录每个文件的 sha256。
2. **安装隔离（一次只存在一个方案包）**：安装市场包前，若 rime 目录已有其他方案包
   （含 builtin），先弹「文件冲突：需要先卸载冲突方案，是否继续？」，确认后逐个精确卸载再安装
   （对齐 `SchemaLocalViewModel.installPackage` + `confirmInstallWithUninstall`）。
   第三方方案的文件因此不会与内置方案文件混在同一份 rime 根目录里。
3. **冲突检测**：目标文件已被其他包声明且内容不同 → 拒绝（同内容视为共享依赖放行；
   同包重装/升级放行），对齐 `detectConflicts`。
4. **卸载按 claimedBy**：仍有其他包声明该文件则只撤声明不删文件（对齐 `uninstallWithManifest`），
   并清理 `<id>.custom.yaml` / `<id>_merged.dict.yaml` / `build/<id>.*`。
5. **用户数据与宿主自有配置永不入清单**：`*.custom.yaml`、`*.userdb/`、`build/`、`themes/`、
   `sync/`、`installation.yaml`、`custom_phrase.txt`、`default.yaml`、`xime.yaml`、其他平台
   前端配置（squirrel/weasel）——卸载不会碰用户输入记录与配置。
6. **可还原**：内置方案包被卸载后，设置程序「已安装」列表提供「还原内置方案」（从
   `market/builtin/` 恢复）；`ensure_rime_data` 在 rime 目录无任何 `*.schema.yaml` 时仍会全量重部署。

### 理由
- 旧实现只有「按包文件列表」，内置方案文件无人认领：第三方包可静默覆盖内置文件
  （本机历史数据：rime-ice 覆盖了 `custom_phrase.txt`、`lua/date_translator.lua`），
  卸载又连带删除；且「已安装」列表用磁盘上全部 `*.schema.yaml`，装完第三方方案后
  内置 wubi86/wubi98/pinyin_simp 与第三方方案同屏混排。
- 安卓已是这套模型；Windows 与安卓共用同一份注册表语义（字段、备份目录、确认流程），
  两端才能共享方案包与排障结论。

### 遗留
- server 侧旧 `schema_manager.rs`（IPC `InstallSchema`/`UninstallSchema`，`packages:` 包裹的
  旧注册表格式、只拷 `.schema.yaml`）与设置程序实现重复，当前无 UI 调用入口；待统一到
  `xime_config::schema_manifest` 或删除。

## 2026-09-30 选中方案的唯一记录：rime 的 user.yaml

- 选中方案**不另存文件**：librime 已在用户目录 `rime/user.yaml` 记
  `var/previously_selected_schema`（每次选择走 `RimeSelectSchema` → `Engine::ApplySchema`
  → `Switcher::SetActiveSchema` 即写盘，`Switcher::CreateSchema` 建会话时读回）。
  Windows 侧删除历史数据根 `selected_schema.txt`，启动恢复改为读该字段。
- **暂时保留**启动时按 id 的显式恢复：rime 建会话只从 `schema_list` 内恢复，而设置程序允许
  选中未启用（不在 `schema_list`）的方案——`RimeSelectSchema` 按 id 直选不受列表限制。
  若将来「选中方案即写入 schema_list」，这个显式恢复可一并删除。

## 2026-09-30 方案来源互斥是不变量，不只是安装时的一次确认

- **不变量**：rime 目录同一时刻只允许存在一个方案来源（内置方案包 **或** 某一个第三方包）。
  据此三条路径都必须收敛：①装第三方包（预检 → 确认 → 卸掉其余来源再装，已有）；
  ②还原内置包（同样预检 → 确认 → 先卸第三方再还原，本次补上）；
  ③历史混装数据（`schema_sources` 多于一个 → 「已安装」页告警卡片 + 「卸载其余，只保留 X」）。
- **不做混装提示界面**（曾实现「混装告警卡片 + 一键只保留某来源」，已按要求删除）：
  安卓只在安装那一刻提示一次，没有常驻的混装告警页；不变量由动作保证——
  装包与还原内置都会先卸掉其余来源，卸载市场包时又会把无主方案文件登记回内置包，
  于是任何一次安装/卸载之后 rime 目录里都只剩一个来源。
  代价：已经存在的历史混装不会主动提示，只在下一次安装/卸载/还原时收敛。
- **卸载语义严格对齐安卓**（曾走弯路，记录以正视听）：安卓 `uninstallWithManifest`
  （`SchemaManifestManager.kt` 276-377 行）删的是「清单文件（按 claimedBy 判共享）
  + 该方案的 `<id>.custom.yaml`、`<id>_merged.dict.yaml`、方案短语表、`build/<id>.*`」，
  并且短语表名要在删文件**之前**读好。Windows 侧一开始把未登记的 `<id>.custom.yaml`
  当用户数据保留，导致切到第三方方案后 rime 目录里还留着上一个方案的补丁——与
  「只保留当前方案包内容」冲突，已改回删。
  真正保留的只有 `*.userdb/` 输入记录、`default.yaml`/`xime.yaml`/`themes/` 等宿主文件。
- 安卓 `cleanRimeDir`（清空 rime/ 只留 default.yaml/xime.yaml/保护项/themes）只在其
  `uninstall()` 且「卸载后没有任何方案包」时触发，**安装流程不走它**；Windows 侧不做
  无条件清空，所有删除都由清单驱动（避免误删未入清单的文件）。
- 历史混装的来源归属无法追溯：修复前被第三方包覆盖的内置文件内容已丢失，
  builtin 备份要重新/修复安装后才完整（同上一条遗留）。
