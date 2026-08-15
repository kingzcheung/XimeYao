# 曦码·曜 (Xime Yao) 输入法 (Windows)

基于 RIME 引擎的 Windows 五笔输入法，使用 Rust + TSF 构建。

> ⚠️ 注意：本项目还在开发中，请勿用于生产环境。

## 快速开始

```powershell
# 开发构建（构建+注册+启动服务器）
.\rebuild.ps1

# 打包 MSI 安装包
.\msi-build.ps1

# 打包 MSIX 应用包
.\msix-bundle.ps1

# 打包并测试安装（无需签名）
.\msix-bundle.ps1 -InstallUnsigned
```

## 安装

### MSI 安装（管理员权限）

```powershell
msiexec /i target\wix\xime-{version}.msi
```

### MSIX 安装

```powershell
# 打包并直接安装（开发测试，无需签名）
.\msix-bundle.ps1 -InstallUnsigned

# 打包、签名并安装
.\msix-bundle.ps1 -Sign

# 生成未签名 MSIX（用于商店提交）
.\msix-bundle.ps1
```

安装后按 `Win+Space` 切换到曦码·曜（Xime Yao）输入法。

## 开发构建

### 前置要求

- Rust 工具链（nightly，项目自带 `rust-toolchain.toml`）
- Visual Studio 构建工具（C++ 支持）
- CMake
- WiX Toolset v3.14（用于 MSI 打包）
- Windows SDK（用于 MSIX 打包）

### 数据目录结构

```
Program Files\Xime\
├── winxime-server.exe        # IPC 服务器进程
├── winxime-setup.exe         # 设置界面
├── winxime_tsf.dll           # TSF 输入法 DLL
├── winxime-tsf-register.exe  # TSF 注册工具
├── rime.dll                  # RIME 引擎
├── data/                     # RIME 基础数据（key_bindings 等）
├── user-data/                # 方案文件（首次启动部署到 %APPDATA%）
└── resources/                # 应用资源

%APPDATA%\Xime\rime\          # 用户数据目录（方案文件、用户配置）
```

## 项目结构

| 目录 | 说明 |
|------|------|
| `crates/winxime-server/` | IPC 服务器（候选栏渲染 + RIME 引擎 + 托盘图标） |
| `crates/winxime-setup/` | 设置界面（GPUI） |
| `crates/winxime-tsf/` | TSF 输入法 DLL |
| `crates/winxime-tsf-register/` | TSF 注册/卸载工具 |
| `crates/winxime-ipc/` | 命名管道 IPC 协议 |
| `rime-wubi/` | 五笔方案文件（.schema.yaml, .dict.yaml 等） |

外部依赖：
| 仓库 | 用途 |
|------|------|
| `libximecore/` | RIME 引擎绑定 + 配置管理 + 共享库 |
| `librime/` | RIME 引擎 C 库（子模块，只读） |

## 打包

### MSI

```powershell
.\msi-build.ps1
# 输出: target\wix\xime-{version}.msi
```

### MSIX

```powershell
# 开发测试（直接注册）
.\msix-bundle.ps1 -Register

# 生成未签名包（商店提交）
.\msix-bundle.ps1

# 生成并安装未签名包
.\msix-bundle.ps1 -InstallUnsigned

# 签名包（自签名证书自动安装到受信任根）
.\msix-bundle.ps1 -Sign

# 输出: target\wix\xime-{version}-x86_64.msix
```

## 许可证

GPL-3.0-or-later

本项目链接 librime (BSD-3-Clause)、opencc (Apache-2.0)、yaml-cpp (MIT)、leveldb (BSD-3-Clause) 等库。
