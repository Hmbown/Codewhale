# 特定环境的注意事项

> 英文原文：[ENVIRONMENTS.md](../ENVIRONMENTS.md)。
> 最后与英文同步日期（last synced with English revision）：2026-09-29。

标准的构建/测试/运行命令放在 `AGENTS.md` 和 `CONTRIBUTING.md`。本文件只记录
特定环境里那些不明显的怪癖，免得在永远碰不到它们的机器上占用上下文。

## Cursor Cloud 虚拟机

- **系统构建依赖：** 构建需要 `libdbus-1-dev`（由 `crates/secrets` 为 OS
  密钥环引入）。它由启动更新脚本安装；如果 `cargo build` 报 `dbus`/`pkg-config`
  错误，就是缺这个依赖。
- **必须设置 `rustup default`：** 有些测试和运行时（runtime）路径会在本检出
  目录*之外*的临时目录里拉起 shell（例如 `run_verifiers_background_*`、
  子智能体（subagent）工作树）。这些被拉起的 shell 只有在 `/workspace` 内部
  才能看到仓库的 `rust-toolchain.toml` 覆盖设置，所以没有全局默认值时，
  它们会以“rustup could not choose a version of rustc to run”失败。
  更新脚本会运行 `rustup default stable` 来修复这一点。
- **`/workspace` 上已知的环境相关测试失败（不是代码缺陷）：** 因为检出目录
  直接位于 `/` 之下，有两个 `codewhale-tui` 子智能体测试会在这里失败——
  `git_repo_root_reports_attempted_paths_when_no_repo_found`（无法在不可写的
  父目录 `/` 里创建临时目录）和
  `create_isolated_worktree_reports_friendly_error_when_no_repo_found`
  （向上遍历到 `/` 时会把 `/workspace` 本身当成仓库）。当仓库检出到一个正常
  可写的父目录下时，这两个测试都会通过。

## 在没有提供商 API key 的情况下运行智能体

通过免密钥的 `vllm`/`ollama`/`sglang` 提供商（provider），把 Codewhale
指向任意本地 OpenAI 兼容端点：

```sh
CODEWHALE_PROVIDER=vllm VLLM_BASE_URL=http://127.0.0.1:8000/v1 VLLM_MODEL=<id> \
  codewhale exec --auto "..."
```

`codewhale exec`（加上 `--auto` 可启用工具调用）是跑通完整智能体循环的非交互路径。

## 在回合期间让主机保持唤醒

交互式 TUI 回合（turn）进行中时，Codewhale 会持有平台的空闲休眠断言，
这样无人值守的机器不会在回合中途空闲入睡而丢掉工作：

- macOS：`caffeinate -i`
- Linux：`systemd-inhibit --what=idle --why="Codewhale turn in flight" --mode=block cat`，
  其中 `cat` 读取的管道由 Codewhale 在回合期间持有

断言在回合结束的那一刻释放——在 Linux 上通过关闭那条管道实现，于是 `cat`
退出、`systemd-inhibit` 随之结束，不会留下残留进程——而且它只覆盖*空闲*休眠：
显式执行 `sleep` / `pmset sleepnow`、合上盖子或电量过低仍会让机器挂起。
无头主机——`exec`、app-server、CI——从不持有它，所以共享 runner 的电源策略
不受影响。Windows 未实现：`SetThreadExecutionState` 是线程亲和的，需要一个
固定线程的持有者，所以这个缺口是有意为之，而不是被悄悄忽略。

如果回合还是被挂起了，引擎会在唤醒时察觉——墙上时钟耗时与单调时钟耗时之差
超过挂起阈值——上报 `System sleep detected; connection lost — retrying request`，
并重新发起请求，而不是让回合失败（#2990）。

## Windows PowerShell 执行策略

shell 工具以 `-ExecutionPolicy Bypass` 运行 PowerShell。它只设置所启动子进程
自己的策略：不会持久化，不需要管理员权限，你自己打开的 PowerShell 窗口仍保持
原有策略。没有它时，本地策略为 `Restricted`（Windows 客户端的默认值）或
`AllSigned` 的机器，会拒绝运行 Codewhale 为多行命令写入的临时 `.ps1` 脚本
（#6745）。

由组策略设置的策略（`Get-ExecutionPolicy -List` 中 `MachinePolicy` 或
`UserPolicy` 行）优先级高于进程范围。在这样的机器上，多行命令仍会被拒绝，
PowerShell 的拒绝信息会作为该命令的错误返回；Codewhale 不会绕过管理员强制的
策略。单行命令通过 `-Command` 运行，不受执行策略约束。命令自身调用的脚本也在
同一进程范围内运行；决定什么可以运行的是 shell 工具的审批与沙箱设置，而不是
执行策略。

若希望改由机器或用户策略生效，请在启动 Codewhale 之前设置
`CODEWHALE_POWERSHELL_EXECUTION_POLICY=inherit`：此时 shell 工具会完全省略
`-ExecutionPolicy`，因此策略为 `Restricted` 或 `AllSigned` 时，需要临时 `.ps1`
脚本的多行命令会被拒绝。未设置、设置为 `bypass` 或任何其他值时，仍保持默认的
`Bypass`。

## 统一的运行时命令

当前的 `codewhale` 二进制在进程内运行 TUI。发布安装器会把同样的字节复制到
可选的 `codew` 短命令；不需要另外的 `codewhale-tui` 可执行文件。
`DEEPSEEK_TUI_BIN` 仍是遗留的回放/迁移设置，不是当前安装所必需的。
