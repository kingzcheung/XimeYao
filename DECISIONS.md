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