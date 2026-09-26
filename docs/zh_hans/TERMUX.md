# Termux / Android arm64 支持

> 英文原文：[TERMUX.md](../TERMUX.md)。
> 最后与英文同步日期（last synced with English revision）：2026-09-26。

Codewhale 为 [Termux](https://termux.dev) 提供 Android arm64 的构建与发布归档。
在 #4236 和 #4242 跟踪的真机运行时 QA 完成之前，Termux 支持都算是预览。
本文讲安装路径，以及你需要了解的平台特有行为差异。

## 安装

请使用 Android 专用的 GitHub 发布归档。
[v0.9.11 发行版](https://github.com/Hmbown/CodeWhale/releases/tag/v0.9.11)
包含 `codewhale-android-arm64.tar.gz`；设备支持仍为**预览**。
按 [Android / Termux 安装步骤](./INSTALL.md#android--termux-arm64) 用配套的
`codewhale-bundles-sha256.txt` 校验归档包，再用 `PREFIX="$PREFIX"` 运行包内安装器，
让命令装进 `$PREFIX/bin`。已经直接安装过的，用 `codewhale update` 更新；
由包管理器安装的文件，仍归包管理器管。

如果某个发行版没有兼容的 Android 归档，或者你想验证自己从源码构建的产物，
Termux 里还有 Cargo 这个预览期备选方案：

```sh
pkg install -y rust clang pkg-config make git
cargo install codewhale-cli --locked
```

macOS/Linux 通用的 Web 安装脚本不是 Android 的安装途径。
不要在 Termux 里安装 `codewhale-linux-arm64`：Android 用的是 Bionic libc，
构建目标也是另一套。Linux 的发布文件不是 Android 二进制。

## Android 上的平台行为

在 Android 上，Codewhale 的安全模型分三层：

1. **Android 应用沙箱**——Android 给 Termux 分配独立的应用 UID，并施加平台自身的
   SELinux 与 seccomp 保护。由 Codewhale 启动的命令继承这个应用边界，
   也继承用户授予 Termux 的存储及其他权限。参见
   [Android 应用沙箱](https://source.android.com/docs/security/app-sandbox)
   与 [Termux 文件系统布局](https://github.com/termux/termux-packages/wiki/Termux-file-system-layout)。
2. **Codewhale 的逐命令沙箱后端**——Seatbelt（macOS）或需要手动开启的
   bubblewrap 封装（Linux）能进一步限制子命令可访问的范围。
   Android 上 Codewhale 目前不提供这一层。
3. **Codewhale 自身的门禁**——工作区信任、审批提示、`allow_shell`/`disallowed-tools`，
   以及文件工具的权限系统。这些都跑在跨平台的应用代码路径上；
   它们在 Android 上的行为仍待下文跟踪的真机 QA 验证。

### Codewhale 沙箱后端：无

Codewhale 现有的 Seatbelt 与 Linux bubblewrap 集成都不面向 Android。
因此在 Android 上，`codewhale doctor --json` 报告的沙箱状态是
`{"available": false, "kind": null}`。这个状态说的是 Codewhale 那层额外的子进程沙箱不存在，
并不代表 Android 或 Termux 没有操作系统层面的隔离。

- Android 上 `get_platform_sandbox()` 返回 `None`。
- Android 构建里没有编入任何 Linux 专用的 bubblewrap 封装——它受
  `#[cfg(target_os = "linux")]` 约束，而 Rust 把 `android` 视作与 `linux` 不同的目标。
- Shell 命令仍在 Termux 的 Android 应用边界内，但不会受到 Codewhale 特有的
  文件系统收窄限制。凡是 Termux 能访问的位置——包括用户授予的共享存储——
  都要当作你批准的命令也可能访问到的地方。

### 审批：依然生效

Codewhale 的审批系统（针对高风险操作的交互式提示、`allow_shell`、
`--disallowed-tools`）实现在应用层，与操作系统沙箱无关。Android 代码路径已经存在，
但它的交互行为仍待 #4242 跟踪的真机 QA 验证。

### 密钥存储：基于文件

Codewhale 的 Termux/原生构建没有受支持的操作系统密钥环后端
（桌面端的 Secret Service/dbus 集成在这里不可用，Codewhale 也尚未接入
[Android Keystore](https://developer.android.com/privacy-and-security/keystore)）。
所以它回退到**基于文件的密钥存储**：`~/.codewhale/secrets/`（Termux 主目录）下的
明文 JSON 文件，只靠 `0600` 文件权限保护——它们**落盘时不加密**。
在单用户 Termux 上，这与 `~/.ssh` 私钥的 Unix 权限模式相同；落盘同样不加密。

- 通过设置向导、`/provider` 或 `codewhale auth set` 保存的密钥会写进
  `~/.codewhale/config.toml`，并镜像到 `~/.codewhale/secrets/secrets.json`。
  这两个文件都要当作明文敏感文件。
- `codewhale auth status --provider <id>` 会报告某个提供商（provider）当前用的是哪个密钥后端。

### 自更新

Android 上，`codewhale update` 索取的是 `codewhale-android-arm64` 发布文件，
绝不会去取 Linux arm64 的文件。GNU libc（glibc）兼容性预检只针对 Linux，
在 Android（Bionic libc）上完全跳过。

## 已知限制（首个 Termux 版本）

| 功能 | 状态 | 说明 |
|---------|--------|-------|
| Android 应用沙箱 | ✅ 继承 | 每个应用独立 UID，外加 Android 平台自身的保护 |
| Codewhale 命令沙箱 | ❌ 不可用 | Android 上没有 bubblewrap/Seatbelt 后端 |
| Codewhale 密钥环后端 | ❌ 不可用 | 回退到基于文件的密钥存储 |
| 审批 / 门禁 | ⚠️ 已实现 | 待真机 QA |
| 文件工具 | ⚠️ 已实现 | 待真机 QA |
| 自更新 | ⚠️ 已实现发布文件选择 | 发布文件与真机 QA 都待验证 |
| Shell 执行 | ⚠️ 仅有应用边界 | 没有 Codewhale 特有的收窄；待运行时 QA |

## 相关 issue

- #4236 —— Epic：官方支持 Termux / Android arm64
- #4238 —— 明确 Android 沙箱与密钥存储的行为
- #4240 —— 构建并打包 Android arm64 发布文件
- #4241 —— 让更新器在 Termux 上选择 Android 发布文件
- #4242 —— 运行 Termux 运行时 QA
