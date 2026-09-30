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

## 2026-09-30 设置程序「打开慢」的定位以实测为准：不是业务冻结

- **实测（读用户实机 `setup.log` 时间戳还原，不跑程序）**：托盘→设置的一次冷启动里，
  `SettingsState::new()`（librime 初始化、方案/配置/剪贴板/同步加载）只占 **2~5ms**；
  从 `run()` 到首帧 **约 1.12 秒**，几乎全部是 iced/wgpu 的图形初始化——
  建 Vulkan/DX12/**GL** 三套实例 232ms、枚举适配器 214ms、选定适配器 212ms、
  surface 配置 84+367ms、字体图集 60ms。
- **结论**：历史上那类「UI 冻结」（全量部署、daemon 重载跑在 UI 线程）已经不在
  启动路径上；再优化「打开慢」要动的是图形初始化（收敛 wgpu 后端 / 固定适配器）
  或让设置进程常驻（关窗不退出、托盘唤起即 ShowWindow），而不是业务加载。
  这两条都还没做——先按实测数据决定，不凭感觉改。
- **保持长期可见**：冷启动分段计时（server 记启动时间戳 + setup 记日志就绪 /
  state 初始化 / 首帧）与「单帧或后台轮询 > 50ms 就 warn」的埋点保留在代码里，
  以后任何一次「变慢了」都能在日志里定位，不再靠猜。
- **日志默认过滤**从 `debug` 收紧为「三方库 info、本项目 crate debug」：
  三方库（wgpu/winit/glyphon/fontdb/hyper）的 debug 日志实测 12.9MB/22.5 万行，
  且 tracing 的格式化发生在**写日志的线程**（设置程序里就是 UI 线程），
  属于白送的每帧开销；排障需要时 `RUST_LOG=debug` 覆盖即可。

## 2026-09-30 UI 冻结面审计（xime-setup / winxime-server / winxime-tsf / libximecore）

- **复核结论**（用户重启后的实测日志）：托盘→设置的开窗时间线是
  「日志就绪 +1ms → 回调注册 +2ms → `SettingsState::new()` 完成 **50ms** →
  首帧 `run()+1119ms`（本帧 view 133µs）」。**业务加载 50ms，开窗慢来自图形初始化**，
  这个判断已被第二次实测确认，不再需要讨论。
- **审计方法**：按「是否跑在 UI/消息线程」判定，分四路（setup state 层、setup
  pages/组件层、server 托盘+候选栏消息线程、TSF 热路径）读源码 + 追调用链
  （含 `xime_config`/`xime_plugin`/`librime` 与宿主 IPC 实现），全程只读、不跑程序。
- **核心判断：UI 冻结几乎全部来自「handler 里同步做重活」，而不是启动路径或页面
  view**。启动慢是图形初始化（保持现状或另议）；页面层唯一每帧磁盘 I/O 是
  `input_schema.rs:372 scan_market_dir()`（仅「已下载」tab）。
- **P0 四处（点击后秒级冻结）已定位并决定要修**，顺序：
  ① `save_schema()` 回退 `deploy_all_schemas()`（服务未运行**或引擎锁忙**时点方案行）；
  ② `RimeSyncNow` 同步 IPC（服务端 join 维护线程）；
  ③ 安装/还原冲突预检里的 `refresh_builtin_package()`（缺 `reload_schemas` 那样的
  「已有 builtin 就跳过」短路）；
  ④ 250ms 轮询里 `poll_market_task → reload_schemas()`。
  修法统一是既有模式：**丢后台线程 + 结果槽 + `poll_background` 回收**
  （`start_deploy()` 就是范本，其注释已写明「IPC 同步等待绝不能进 UI 线程」）。
- **IPC 的两条硬约束（新发现的系统性风险，不只是慢）**：
  ① `IpcClient::connect()` 用 `interprocess` 的默认 `ConnectWaitMode::Unbounded`，
  管道实例忙时 `WaitNamedPipeW(FOREVER)`，**连接没有超时**；应答读取的
  `READ_TIMEOUT_MS = 100` **实际上不生效**——它只在两次 `read` 之间检查，而底层
  `ReadFileEx` + `SleepEx(INFINITE)` 的单次调用永不超时（write/flush 同理，
  `FlushFileBuffers` 会阻塞到对端读完）→ 调 IPC 的线程（含 TSF 的宿主 UI 线程）
  **可以永久挂住**，不是「最多 100ms」；
  ② 服务端 `ipc_server.rs:174` 在请求开头取引擎锁并**整个请求都持锁**，
  网络（`FetchSchemaIndex`）、解压（`download_schema`）、维护（`SyncUserData`/
  `ReloadConfig`）、插件重载都在锁内 → 这些操作进行时，TSF 每次按键拿到
  `success:false`（**按键被丢**），托盘 `try_lock` 路径静默无操作。
  决定：IPC 连接改为 `ConnectWaitMode::Timeout(..)`、读写加总截止时间且超时只丢
  本次响应（保留连接，避免下次按键再赌一次无界连接）；引擎锁范围收敛到
  「只包引擎调用」。
- **TSF 侧最要命的一处不是「慢」而是「锁住宿主文档」**：
  `text_input_processor.rs:669`（`start_composition` 的 `DoEditSession`，
  会话按 `TF_ES_READWRITE` 申请）→ 587 `ipc.update_position(...)`——
  **IPC 往返发生在持有宿主文档写锁的编辑会话内**。服务端一慢，宿主程序直接
  无响应。决定：编辑会话内零 IPC（坐标改为会话外采样 + 后台发送 + 去重），
  这是 TSF 侧修复的第一优先级。
- **输入法激活路径有盘 IO 与最多约 1.1s 的等待**：`activate_impl` 每次都
  `init_logging("tsf")`（建目录/文件），并覆盖 `LOG_GUARD` 从而 drop 旧
  `WorkerGuard`——`tracing_appender` 的 drop 是 100ms + 1000ms 两个
  `send_timeout`（日志积压时最坏约 1.1s），跑在宿主 UI 线程上。
  决定：日志初始化只做一次（Once / 不覆盖 guard），并移出 Activate。
- **候选栏（打字链路）的每按键开销**：`ui/view.rs:258 → ui/model.rs:96
  XimeConfig::load()` 无缓存（每次按键重读+解析三份 yaml）、按键路径上 13 条
  `info!/debug!`、`ui/paint.rs:249-260` 每次绘制无条件 `ResizeBuffers`+重建位图、
  用 WARP 软件光栅、每帧重建文本格式/画刷。这些决定按 P1 处理（打字卡顿类）。
- **不做什么**：不为了「顺手」重构 `pages/`（除 `scan_market_dir` 缓存外页面层干净）；
  不改 `rfd` 原生模态对话框（设计如此）；不在本次审计里动 wgpu 后端收敛与进程常驻
  （那两条属「开窗慢」，等用户定优先级）。

## 2026-09-30 输入方案页的「进行中」状态由 `market_schema.installing` 驱动

- 方案包类的三个动作（安装 / 卸载 / 还原内置）**共用一个 `installing: Option<String>`**
  （值是方案包 id，还原时是 `builtin`）。页面不再自己维护 busy 标志，只按两个事实渲染：
  「`installing` 就是本卡片的包」→ 进行中文案 + 禁用；「有任务在跑」→ 保留原标签但禁用。
  这样以后再加方案包类操作，只要沿用这个字段就自动有 loading。
- 按钮文案/可用性/颜色收敛到一个**纯函数** `action_button(...) -> ActionButton`，
  用单测固定三种组合（空闲 / 本卡在忙 / 其它任务在忙），而不是散在 `view` 里的
  三元表达式——UI 结构没法测，纯函数可以。
- **进行中一律用禁用按钮 + 文案**（`安装中…`/`卸载中…`/`还原中…`），不加转圈动画：
  与扩展商店页既有做法一致；而且安装过程没有可上报的进度（`do_install` 没有进度回调），
  只有「开始/结束」，所以不给百分比，改在卡片副标题说明正在做什么（解压/部署、删除文件）。
- `install_message` 在**两个 tab 都渲染**：点「安装」的入口在「已下载」，错误提示不该只
  出现在「已安装」上。这是上一次审计里「失败看不见」的直接修补。
- `disabled_button` 从 `pages/store.rs` 提到 `components/widgets.rs`（公共
  `button_disabled`）：两个页面共用同一种禁用态按钮，避免复制粘贴出两套样式。
- **本次只做 UI 状态（用户明确选 B）**：预检短路（审计 P0 #3，让 `安装中…` 能立刻出现
  而不是等同步扫描跑完）**没有**一起做，留作下一个独立功能点——所以现在仍会有
  「点下去先卡一下、然后才显示安装中…」的中间态，这是已知且已被记录的边界。

## 2026-09-30 方案包「整包启用」；切换方案＝先落盘 → 通知 → 未部署才后台补部署

- **`schema_list` 是「启用」的唯一事实源，而 librime 只编译它（+ `dependencies`）**。
  由此推出两条硬规则：
  1. 安装 / 还原内置 / 卸载最后一个来源后的自动还原，都必须把该包的**全部**顶层方案
     写进列表（默认方案置顶），不能只写一个——只写一个的话包内其余方案永远拿不到
     `build/` 产物，用户切过去会被守卫判「未部署」。这就是四处写入收敛到同一个纯函数
     `package_schema_ids` 的原因（此前是「启用新方案的第一个」的单项语义）。
  2. 想让某个方案可切换，唯一的正路是**让它进列表并部署**；不能靠 librime 的
     `select_schema`（它对新旧 librime 版本会接受未部署方案，产出「所有按键不组词」的
     死会话——上一节已加守卫）。
- **切换方案的顺序固定为：先写启用列表 → 再通知宿主 `SelectSchema` → 失败才部署。**
  先通知是为了保住「已部署方案毫秒级切换、零部署」的快路径；先落盘是因为宿主的判据是
  `build/<id>.schema.yaml`，列表不落盘的话兜底部署也编不出目标方案。
- **未部署方案的兜底必须三步，少一步都是「部署了但没切过去」**：
  `deploy_all()`（产出产物）→ `notify_daemon_reload()`（宿主重建会话——注意宿主会优先
  恢复它记住的旧方案，所以这步还不够）→ 再 `notify_select_schema()`（补一次显式选中）。
- **整段兜底必须后台**（全量维护 + redeploy 数秒），结果写进 `deploy_result` 由
  `poll_deploy` 统一提示——顺手修掉审计 P0 #1 里「点方案行冻设置窗口」那一处；
  这条路径不再有 UI 线程上的同步部署。
- **设置页不自己触盘判断「有没有产物」**：`reload_schemas` 扫一次 `build/` 存进
  `deployed_schema_ids`，页面只读状态（保持「页面层零磁盘 IO」的既有约定）；
  没产物的方案标「未启用」badge 但**仍可点**——点它就是「启用并部署」。
- **保留「安装即切换」语义**：整包启用后仍把该包默认方案（内置包 = wubi86）置顶，
  宿主新建会话回落到列表第一位，用户装完即可用。
