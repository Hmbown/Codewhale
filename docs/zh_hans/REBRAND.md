# 更名：DeepSeek TUI → Codewhale

> 英文原文：[REBRAND.md](../REBRAND.md)。
> 最后与英文同步日期（last synced with English revision）：2026-09-26。

从 **v0.8.41** 起，本项目改用新名字发布：`codewhale`。

本文说明改了什么、没改什么，以及怎么迁移。DeepSeek 提供商（provider）集成没有任何变化，
变的只是本地 CLI / TUI 的品牌。

## 速览（TL;DR）

```bash
# 1. Uninstall the old wrapper or binaries.
npm uninstall -g deepseek-tui      # or:
cargo uninstall deepseek-tui-cli 2>/dev/null || true
cargo uninstall deepseek-tui 2>/dev/null || true
                                    # Homebrew:
                                    # brew upgrade codewhale

# 2. Install under the new name.
npm install -g codewhale            # or:
cargo install codewhale-cli --locked
                                    # Homebrew:
                                    # brew tap Hmbown/deepseek-tui
                                    # brew install codewhale

# 3. Run with the new command.
codewhale doctor
codewhale
```

你现有的 `~/.deepseek/config.toml`、`~/.deepseek/sessions/`、
`~/.deepseek/skills/`、`~/.deepseek/tasks/` 和 `~/.deepseek/mcp.json` 不会被删除。
新版 Codewhale 优先使用 `~/.codewhale/`；迁移期间，旧的 `~/.deepseek/` 状态
仍会被读取，用作回退。现有的 `DEEPSEEK_*` 环境变量继续有效。

## 改了名字的部分

| 对象 | 之前 | 之后 |
|---|---|---|
| 已安装命令 | `deepseek` / `deepseek-tui` | `codewhale` / `codew` |
| npm 包装器包 | `deepseek-tui` | `codewhale` |
| Crates.io crate | `deepseek-tui-cli` / `deepseek-tui` / `deepseek-*` | `codewhale-cli` / `codewhale-tui` / `codewhale-*` |
| 发布资产 | `deepseek-<platform>` / `deepseek-tui-<platform>` | `codewhale-<platform>` / `codew-<platform>`；`codewhale-tui-<platform>` 只作为兼容文件名保留 |
| 校验和清单 | `deepseek-artifacts-sha256.txt` | `codewhale-artifacts-sha256.txt` |

## 本地状态的变化

新安装会把产品自己的状态写到 `~/.codewhale/` 下。迁移期间，现有的 `~/.deepseek/`
配置、会话、技能、任务、MCP 配置、记忆和笔记，仍会被读取，用作旧版回退。
Codewhale 从不自动删除旧目录。

## 没有改的部分

凡是针对 DeepSeek 提供商 API 的东西，一律保持原样：

- **环境变量**：`DEEPSEEK_API_KEY`、`DEEPSEEK_BASE_URL`、`DEEPSEEK_MODEL`、
  `DEEPSEEK_PROVIDER`、`DEEPSEEK_PROFILE`、`DEEPSEEK_LOG_LEVEL`，以及现有的
  `DEEPSEEK_TUI_*` 运行时开关（`DEEPSEEK_TUI_BIN`、`DEEPSEEK_TUI_RELEASE_BASE_URL`
  等）。保留这些名字是为了向后兼容；一旦改名，全世界每个 shell rc 文件都会失效。
- **`DEEPSEEK_YOLO`**：现已弃用，但 0.9.x 及更早版本仍会把它当作 `CODEWHALE_YOLO`
  的别名来读取，现有脚本因此照常工作（两者都设置时，`CODEWHALE_YOLO` 生效）。
  它会在 0.10 中移除（#5443）；新脚本请用 `CODEWHALE_YOLO`。
- **模型 ID**：`deepseek-v4-pro`、`deepseek-v4-flash`，以及旧别名
  `deepseek-chat` 和 `deepseek-reasoner`。
- **主机**：`api.deepseek.com`（全球）。旧域名里的拼写错误 `api.deepseeki.com`
  不是 DeepSeek 的官方端点；它仍只在给现有配置做 URL 启发式判断时被接受，
  并不作为备用地址提供（#1079）。
- **GitHub 仓库地址**：`https://github.com/Hmbown/CodeWhale`。
  过渡期间，旧的 `Hmbown/DeepSeek-TUI` 地址会重定向到这里。
- **Homebrew tap 与 formula**：formula 名为 `codewhale`。tap 的 GitHub 仓库
  在改名之前仍是 `Hmbown/homebrew-deepseek-tui`；当前的安装路径是
  `brew tap Hmbown/deepseek-tui && brew install codewhale`。旧的 `deepseek-tui`
  formula 在这一个重叠版本里仍是已弃用的别名。
- **Docker 镜像**：`ghcr.io/hmbown/codewhale`。

## 弃用 shim（v0.9.0 中移除）

为了让现有的 shell 别名、脚本和 CI 在改名期间继续可用，v0.8.41 及之后的
v0.8.x 版本都提供了**弃用 shim**：

- 一个 `deepseek` 二进制文件：向 stderr 打印一行警告，然后把 argv 转发给
  `codewhale`。
- 一个 `deepseek-tui` 二进制文件：对 `codewhale-tui` 做同样的事。
- 旧的 `deepseek-tui` npm 包已弃用，不再接收新版本。请改用 `codewhale` npm 包。

这些二进制 shim 会在 **v0.9.0** 中移除。DeepSeek 提供商支持、模型 ID、
`DEEPSEEK_*` 环境变量，以及旧的 `~/.deepseek/` 状态回退，都仍然受支持。

## 实际迁移

### npm

```bash
npm uninstall -g deepseek-tui
npm install -g codewhale
```

### Cargo

```bash
cargo uninstall deepseek-tui-cli 2>/dev/null || true
cargo uninstall deepseek-tui 2>/dev/null || true
cargo install codewhale-cli --locked
```

或者在源码检出目录里：

```bash
cargo install --path crates/cli --locked --force
```

Cargo 装出来的是正式命令 `codewhale`。发布包、npm 和 Homebrew 安装器还会提供
字节完全一致的短名 `codew`；Cargo 用户如果需要，可以在 `codewhale` 旁边自己加
一个 `codew` 符号链接。

### 旧的 `deepseek update`

当前的 v0.8.x 兼容二进制文件能识别出自己正以旧的 `deepseek` 或 `deepseek-tui`
文件名运行。这种情况下，`deepseek update` 或 `deepseek-tui update` 会下载正式的
Codewhale 发布资产。只要安装目录可写，这两条命令就会把这些资产以 `codewhale` 和
`codewhale-tui` 的名字装到旧二进制文件旁边。这里描述的是历史上的 v0.8 兼容更新器，
不是当前的安装入口；升级之后请用 `codewhale` 或 `codew`。

如果这条更新路径无法写入安装目录，请用上面的 npm、Cargo、Homebrew 或手动重装
命令。旧的 npm 包 `deepseek-tui` 仍是弃用状态，不会重新发布；npm 用户应改用
`npm install -g codewhale`。

### Homebrew

**当前发布状态（v0.9.13，发布于 2026-09-14；工作区源码版本 0.9.13）：**
formula 名为 `codewhale`。全新安装：

```bash
brew tap Hmbown/deepseek-tui
brew install codewhale
brew upgrade codewhale
```

tap 的 GitHub 仓库在改名为 `Hmbown/homebrew-codewhale` 之前仍是
`Hmbown/homebrew-deepseek-tui`（改名后 `brew tap Hmbown/codewhale` 即可用；
旧 tap 名靠 GitHub 的重定向继续工作）。旧的 `deepseek-tui` formula 在这个重叠
版本中仍是已弃用的别名，现有的 `brew upgrade deepseek-tui` 定时任务因此照常运行。

**后续推进：**

1. 在添加 `HOMEBREW_TAP_PAT` 时把 tap 仓库改名为
   `Hmbown/homebrew-codewhale`，然后通知 Codewhalebot。
2. 再发一个次版本之后，移除 `deepseek-tui` 别名。

### 手动安装 / GitHub Releases

`v0.8.41` 到 `v0.8.x` 的 Releases 附带正式的 `codewhale-*` / `codewhale-tui-*`
资产（从 v0.8.66 起还有 `codew-*`），以及仅作兼容用的 `deepseek-*` /
`deepseek-tui-*` shim 资产。从 v0.9.0 起，Releases 附带当前的 `codewhale-*` /
`codew-*` 资产、`codewhale-artifacts-sha256.txt` 校验和清单，以及字节完全一致的
`codewhale-tui-*` 兼容文件名——这是旧更新客户端所必需的。这些兼容文件名不是第三个
已安装命令。迁移到 v0.9.0 之前，请通过 `codewhale` 安装或更新。

### 会话、技能与手动指定的工作区

改个二进制文件名，不需要从头再来：

- **配置**：首次启动时，如果 `~/.codewhale/config.toml` 还不存在，Codewhale 会把
  `~/.deepseek/config.toml` 复制过去。它绝不会覆盖较新的 Codewhale 配置。
  你可以用 `codewhale doctor` 查看当前生效的路径。
- **会话与任务**：受管状态优先从 `~/.codewhale/...` 读取；只有旧目录存在时，
  才用 `~/.deepseek/...` 作为旧版回退。已保存的会话仍会出现在
  `codewhale sessions` 和 TUI 的恢复选择器里。
- **技能**：Codewhale 先发现工作区技能，再发现全局技能；全局技能的范围既有
  `~/.codewhale/skills`，也有旧的 `~/.deepseek/skills`。已含 `SKILL.md` 的
  现有技能目录不用重写。
- **MCP 配置**：默认路径是 `~/.codewhale/mcp.json`。如果该文件不存在，
  Codewhale 仍会读取旧的 `~/.deepseek/mcp.json`。要使用自定义 MCP 配置文件，
  请在 `config.toml` 里设置 `mcp_config_path`，或设置 `DEEPSEEK_MCP_CONFIG`。
- **手动安装二进制文件**：把当前的两个命令文件一起放进 `PATH`：`codewhale` 和
  `codew`。Windows 上推荐放在用户本地目录
  `%LOCALAPPDATA%\Programs\CodeWhale\bin`。类 Unix 系统上，任何用户可写的
  `PATH` 目录都可以，只要两个命令都在。不要把仅作兼容用的 `codewhale-tui-*`
  发布文件名装成第三个命令。
- **指定的工作目录**：在项目目录里运行 `codewhale`，或用特定工作区路径启动它，
  都不会移动项目文件。Codewhale 先读 `<workspace>/.codewhale/config.toml`，
  新路径不存在时回退到旧的 `<workspace>/.deepseek/config.toml`。

如果 `~/.codewhale/...` 和 `~/.deepseek/...` 两份都存在，以 Codewhale 路径为准。
先保留旧目录，直到你确认 `codewhale doctor`、`codewhale sessions` 和你预期的技能
显示的都是同一份状态。

### 升级后会话看起来不见了

先运行 `codewhale doctor`，再去复制或删除任何东西。doctor 只在
`~/.deepseek/sessions/` 和 `~/.codewhale/sessions/` 之间比较顶层会话 JSON 的
**文件名和文件系统元数据**。它不读取聊天内容，不遍历 `checkpoints/`，
也不修改这两个目录。JSON 输出在 `legacy_state.session_recovery`
里给出同样的结果。

如果 doctor 列出了可恢复的文件名：

1. 备份两个会话目录（如果存在），并关闭其他 Codewhale 进程。
2. 运行 `codewhale sessions`。这会调用现有的增量迁移：只补上目标端缺的文件，
   绝不覆盖 `~/.codewhale/sessions/` 下已存在的文件；会跳过 checkpoint 内部文件；
   所有旧文件原样留在原地。
3. 重新运行 `codewhale doctor`，再用 `codewhale sessions` 确认会话已经出现。
   如果列表里仍有文件名，请保留两份备份，并报告列出的源文件名/目标文件名，
   但不要分享聊天内容。

显式设置 `CODEWHALE_HOME` 会有意隔离该 home，并让环境里的默认
`~/.deepseek` 回退失效。这种模式下，doctor 不会去检查那个默认的旧 home。
如果想诊断默认 home，又不想动隔离出来的那个 home，请另开一个 shell，
确保 `CODEWHALE_HOME` 未设置，再运行 `codewhale doctor`。

## 为什么改名

Codewhale 是这个终端编码代理（agent）的新名字：更短，也更贴合终端习惯。它同时指向
更长期的产品方向——一个面向开源与开放权重编码模型的智能体终端；项目起步时的提供商
DeepSeek，与其他每个提供商一样仍是一等公民。项目名、命令名、包名、发布资产、
Docker 镜像和 CNB 镜像都改成了 Codewhale；官方的 DeepSeek 提供商、模型 ID、
环境变量和 `~/.deepseek/` 配置表面仍是一等公民。

## 报告改名相关的问题

如果你的安装在迁移期间坏了，请在
<https://github.com/Hmbown/CodeWhale/issues> 开一个 issue，并附上：

- `codewhale --version` 的输出（如果你还在用 shim，就是 `deepseek --version`）。
- 你用的安装方式（npm、cargo、brew、手动）。
- 你运行的确切命令，以及完整的错误输出。

迁移引发的回归问题我们会优先处理。
