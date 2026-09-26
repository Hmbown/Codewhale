# HarmonyOS 与 OpenHarmony

> 英文原文：[HarmonyOS.md](../HarmonyOS.md)。
> 最后与英文同步日期（last synced with English revision）：2026-09-26。

本文讲 Codewhale 在 HarmonyOS PC 上运行，以及 OpenHarmony 的交叉编译环境。

## 支持层级

| 目标 | Codewhale 层级 | CI 覆盖 | 分发方式 |
| --- | --- | --- | --- |
| 用户空间兼容 glibc 的 HarmonyOS PC | Tier 1 Linux ARM64 运行时 | 由 Linux ARM64 发布构建覆盖 | GitHub 发布二进制文件；npm 为次要途径 |
| `aarch64-unknown-linux-ohos`（OpenHarmony） | Tier 2 交叉编译目标 | `codewhale-tui` 会用真实的 OpenHarmony 原生 SDK/sysroot 检查 | 从源码构建；没有预编译发布产物 |

Tier 2 的意思是：每一处相关的源码改动都会过一遍编译检查，但维护者不承诺提供发布二进制文件，
也不承诺做完整的设备级运行时测试。CI 任务使用已发布的 OpenHarmony 6.1 原生 SDK；
如果 SDK、Clang 或 sysroot 不可用，它会刻意失败，
而不是改用宿主头文件，或者拿一个可能谎报成功的桩顶上。

## 在 HarmonyOS PC 上运行

用户空间兼容时，HarmonyOS PC 可以直接用 Linux ARM64 发布版。
在 Linux 环境里做全新安装，用官方 GitHub 安装脚本：

```bash
curl -fsSL https://codewhale.net/install.sh | sh
"$HOME/.local/bin/codewhale" --version
```

[已发布的 v0.9.11 版本](https://github.com/Hmbown/CodeWhale/releases/tag/v0.9.11)
包含 `codewhale-linux-arm64` 和 `codew-linux-arm64`；有产物存在，并不代表它兼容每一台
HarmonyOS 设备。各版本的具体要求和 Cargo 兜底方案见
[Linux ARM64 可移植性](./INSTALL.md#linux-arm64-可移植性)。
已经直接安装过的，用 `codewhale update`。目录已被占用，或者由包管理器管理的安装，
见[迁移到全新目录](../INSTALL.md#migrating-from-npm-cargo-or-another-installation)。
npm 仍是次要的打包途径。`codewhale-tui-linux-arm64` 这个文件名只为兼容旧版更新器而保留，
并不是第三条命令。

## 交叉编译到 OpenHarmony

仓库不把机器相关的 SDK 路径纳入版本控制。把 `OHOS_NATIVE_SDK` 设成 OpenHarmony
原生 SDK 目录，也就是包含 `llvm/bin`、`sysroot` 和
`build/cmake/ohos.toolchain.cmake` 的那个目录。

Windows PowerShell：

```powershell
$env:OHOS_NATIVE_SDK="<path-to-openharmony-native-sdk>"
. .\scripts\ohos-env.ps1
rustup target add aarch64-unknown-linux-ohos
cargo build --target aarch64-unknown-linux-ohos -p codewhale-cli
```

Linux 或 macOS：

```bash
export OHOS_NATIVE_SDK=/path/to/openharmony/native
. ./scripts/ohos-env.sh
rustup target add aarch64-unknown-linux-ohos
cargo build --target aarch64-unknown-linux-ohos -p codewhale-cli
```

这些环境准备脚本会为 `aarch64-unknown-linux-ohos` 导出 Cargo 的目标专用
`linker`、`AR`、`CC`、`CXX`、`CFLAGS`、`CXXFLAGS`、`CARGO_ENCODED_RUSTFLAGS`、
`CC_SHELL_ESCAPED_FLAGS`，以及 CMake 工具链变量。它们还会把 `bindgen` 指向 SDK 的
`libclang` 和 sysroot，好让 `rquickjs-sys` 生成 OpenHarmony 绑定；
`rquickjs-sys` 本身并不附带这些绑定的预生成版本。

在 Windows 上，`ohos-env.ps1` 让 Cargo 使用仓库里的 `ohos-clang.cmd` 启动器。
该启动器再委托给 `ohos-clang.ps1`，所以不只 C/C++ 编译和
bindgen，最终那次 Rust 链接也总会带上 `-target aarch64-linux-ohos`、
SDK sysroot 和 `-D__MUSL__`，同时保留 Cargo 的链接器参数和退出状态。启动器转发前会重新给每个参数加引号，
所以就算 SDK 路径里带空格（比如默认的 `D:\DevEco Studio\...` 安装），
到最终链接时 `--sysroot` 依然完好。

## 编译器包装脚本

临时手动调用编译器时，用 `scripts/ohos/` 里的包装脚本。它们读取同一个
`OHOS_NATIVE_SDK` 变量，脚本里不含本机路径。

Windows PowerShell：

```powershell
.\scripts\ohos\ohos-clang.ps1 --version
.\scripts\ohos\ohos-clangxx.ps1 --version
```

Linux 或 macOS：

```bash
sh ./scripts/ohos/ohos-clang.sh --version
sh ./scripts/ohos/ohos-clangxx.sh --version
```

如果你想直接用 `./scripts/ohos/ohos-clang.sh` 这样运行 POSIX 包装脚本，
先给它们加上可执行权限：

```bash
chmod +x ./scripts/ohos/ohos-clang.sh ./scripts/ohos/ohos-clangxx.sh
```

## 链接器与工具链路径

仓库不把 Cargo 链接器路径或 CMake 工具链路径纳入版本控制。Cargo 无法展开 `linker`
值或 CMake 工具链路径值里的环境变量，所以这些值改由 `scripts/ohos-env.ps1` 和
`scripts/ohos-env.sh` 导出。

## 依赖守卫

发布准备阶段会跑一次不依赖 SDK 的依赖检查：

```bash
./scripts/release/check-ohos-deps.sh
```

这个守卫会校验 Windows 最终链接包装脚本的契约：证明 OHOS 会启用 `rquickjs-sys` 的
bindgen feature，解析 `codewhale-tui` 在 `aarch64-unknown-linux-ohos` 下的依赖图，
并且在不受支持的宿主/UI crate 重新进入该依赖图时失败：`nix` 0.28/0.29、
`portable-pty`、`starlark`、`arboard` 或 `keyring`。这项无 SDK 检查不能代替真实的
SDK/sysroot 构建，但能在发布前抓住已知的链接器、bindgen、
`starlark -> rustyline -> nix` 以及 PTY/keyring 回归。

因为 OpenHarmony 的依赖图有意不含 `portable-pty`，常驻的 `terminal/*` PTY 工具
在该目标上不会注册。普通的 `exec_shell` 工具仍然可用，走的是非 PTY 的进程实现。

仅 Linux 的沙箱实现（bubblewrap、seccomp 和 `prctl` 进程加固）只为
`all(target_os = "linux", not(target_env = "ohos"))` 编译。因此 OpenHarmony
会报告“没有本地操作系统沙箱”，而不是去探测自己并不支持的 Linux 内核路径或系统调用。
配置好之后，外部 OpenSandbox 执行仍可单独使用。

原生桌面剪贴板库和 Wayland 辅助库也不进入 OpenHarmony 的依赖图。
文本复制降级为终端客户端路径（OSC 52；在 tmux 里则是 tmux 的 `load-buffer -w`）；
粘贴由终端提供，走普通输入或 bracketed 输入。图像剪贴板读取在该目标上不可用。
如果终端无法接受 OSC 52，复制会返回一条明确的“Clipboard unavailable”错误，
而不是 panic，也不会谎称成功。
