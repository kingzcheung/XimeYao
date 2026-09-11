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
插件框架复用 libximecore 的 `xime-plugin`（与 Xime plugin-core 的 Lua 契约对齐）：
宿主做重活（打包/监听/去重），插件只做协议传输。插件与配置都放 `%APPDATA%\Xime\plugins`。

### 下载数据目录映射（安卓 filesDir ↔ Windows %APPDATA%\Xime）
- 方案市场包 `files/market/{id}/` ↔ `market\<id>\`
- 插件 `files/plugins/{id}/` ↔ `plugins\<id>\`（注册表：安卓 plugins.xml / Windows registry.yaml）
- 模型 `files/models/{modelId}/` ↔ `models\<modelId>\`（独立于插件，用到才建，文件如实命名）
- 方案市场注册表在数据根（`.registry.json` / `.registry.yaml`），不放 market/ 内
- 下载临时文件用应用缓存（安卓 cache/ ↔ Windows %TEMP%\xime_plugin_*），即用即删
