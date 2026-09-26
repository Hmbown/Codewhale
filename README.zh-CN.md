<!-- source: README.md sha256:bbb8bfb61e57 -->
# Codewhale

Codewhale 是一款开源智能体，可使用你选择的托管模型或本地模型读取项目、编辑文件、运行命令并检查自己的工作。从终端中的一项任务开始。对于较大的工作，可以将其中的部分任务交给使用不同模型、承担不同角色的智能体。

![Codewhale 在终端中运行](web/public/codewhale-tui-5765d80.png)

*终端预览截图来自 v0.10.0 的开发构建。*

[English](README.md) · [日本語](README.ja-JP.md) · [Tiếng Việt](README.vi.md) · [Bahasa Indonesia](README.id.md) · [한국어](README.ko-KR.md) · [Español](README.es-419.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md) · [Українська](README.uk.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [繁體中文](README.zh-TW.md) · [हिन्दी](README.hi.md) · [Türkçe](README.tr.md) · [Italiano](README.it.md) · [Polski](README.pl.md) · [العربية](README.ar.md) · [Català](README.ca.md)

[![CI](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml/badge.svg)](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/codewhale-cli?label=crates.io)](https://crates.io/crates/codewhale-cli)
[![npm](https://img.shields.io/npm/v/codewhale?label=npm)](https://www.npmjs.com/package/codewhale)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.gg/37gfS3ksug)

## 安装

macOS / Linux：推荐安装官方 GitHub Release。

```bash
curl -fsSL https://codewhale.net/install.sh | sh
"$HOME/.local/bin/codewhale"
```

安装器会选择最新的已发布版本。[更新日志](CHANGELOG.md)也描述了下一版本尚未发布的候选构建；只有在该版本正式发布后，已发布的下载包才会包含这些变更。

Windows 请使用 [GitHub Releases](https://github.com/Hmbown/CodeWhale/releases/latest)
中的安装器或压缩包。已有的直接安装使用 `codewhale update`；它会显示当前可执行文件路径，
并保留比已发布版本更新的构建。npm 和 Cargo 是次要打包选项。
迁移与 PATH 排查见[安装指南](docs/zh_hans/INSTALL.md)。

首次运行会直接打开输入框，不会引导你完成设置流程。要获得模型回复，必须连接托管模型或本地模型：在连接之前，启动界面会显示 "no model connected"。运行 `/provider`（或按 F3）即可添加托管服务密钥或选择本地运行时。如果 Ollama 已在运行且带有聊天模型，Codewhale 会自动切换到它。Codewhale 还支持 npm 和 Cargo 作为次要打包方式，以及 Docker、Nix、Scoop、Android/Termux 和可选的 CNB 镜像。对于现有的软件包管理器安装，系统会提供迁移说明。请参阅[安装与 PATH 帮助](docs/INSTALL.md)。

每种 shell 只需一条命令即可启用 Tab 补全——`codewhale completion bash|zsh|fish|powershell|elvish`。请参阅 [shell 补全](docs/INSTALL.md#8-shell-completions)。

## 使用

在项目文件夹中打开终端并运行 `codewhale`。使用 `/provider` 选择提供商，使用 `/model` 选择模型，然后描述一项具体任务：

```text
Fix the failing tests and explain what changed.
```

也可以不打开 TUI，直接运行任务：

```bash
codewhale exec "fix the failing tests and explain what changed"
```

Codewhale 可以读取你的代码仓库、编辑文件、运行命令、检查结果，并持续推进目标。使用 `/mode plan` 可以在不修改文件、不执行 shell 命令的情况下进行探索；希望它实施修改时，使用 `/mode work`。按 `Shift+Tab` 可选择 Ask、Auto-Review 或 Full Access；[模式与权限指南](docs/MODES.md)说明了各自允许的操作。

## 终端、应用与 Computer Use

终端和图形客户端连接到 Codewhale Runtime，由它运行智能体及其工具：

- **终端：** `codewhale` 打开交互界面；`codewhale exec` 可从脚本或 CI 作业中运行任务。
- **本地浏览器：** `codewhale web` 打开随附的[本地 Web 客户端](docs/WEB.md)，使用同一个 Runtime。
- **Codewhale 桌面应用（GPUI）：** 在独立仓库中开发的原生桌面应用是登录后产品客户端的方向。app.codewhale.net 的托管网页应用将按它的样子重建；营销站点、登录、计费、法律和下载页面保留在网页上。其可用情况见[产品页面](https://codewhale.net/en/product)。

**Computer Use 提供观察其他应用并与之交互的工具。** 当前源码已包含此插件。使用前请查看它请求的访问权限并启用它；仍须满足操作系统权限和平台要求。请参阅随附的 [Computer Use 指南](crates/tui/plugins/computer-use/README.md)和[插件设置](docs/PLUGINS.md)。

在 VS Code 中，社区维护的 CodeWhale 扩展通过侧边栏连接本地 Runtime。可从 [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode) 安装；源码见 [GitHub](https://github.com/HengQuWorld/CodeWhale-VSCode)。

## 为什么选择 Codewhale

- **选择你的模型。** 连接托管提供商，或通过 Ollama、vLLM、SGLang 使用本地模型。使用 `/provider` 切换提供商，使用 `/model` 选择模型。
- **掌控始终在你手中。** 检查拟执行的操作及其造成的文件变更。审批设置决定何时需要审查；Full Access 仍须遵守不可逾越的策略边界。`/undo` 和 `/restore` 可帮助恢复工作区变更。
- **让长时间任务井然有序。** 保存会话、设置持久的 `/goal`、在工作流运行前进行审查，并协调多个智能体，同时不让其内部指令混入你的对话记录。
- **扩展你已有的智能体。** 连接 MCP 服务器和技能、配置钩子，并将智能体角色作为可读文件保存在项目或个人设置中。

在 TUI 中运行 `/help` 可查看命令和键盘快捷键。

## 安全

Codewhale 在你的机器上运行，并仅拥有你授予的访问权限。审批模式和仓库规则会限制智能体的行为；在支持的平台上，可选的操作系统沙箱可提供更强的执行边界。未知的模型价格会保持显示为未知，而不会被误报为免费。

阅读[授权顺序](docs/AUTHORIZATION_ORDER.md)了解确切的策略层级，阅读[配置](docs/CONFIGURATION.md)了解本地设置。

## 文档

- [提供商和本地模型](docs/PROVIDERS.md)
- [智能体团队](docs/FLEET.md)
- [MCP](docs/MCP.md)、[钩子](docs/HOOKS.md)和[配置](docs/CONFIGURATION.md)
- [本地 Web 客户端](docs/WEB.md)
- [全部文档](docs/README.md)
- [仓库结构与贡献指南](CONTRIBUTING.md#project-structure)

## 加入社区

**欢迎提交错误报告、功能建议和 pull request**，无论你已使用 Codewhale 数月，还是刚刚开始尝试。如果缺少某个提供商、工作流体验不佳，或终端界面妨碍了你，请[提交 issue](https://github.com/Hmbown/CodeWhale/issues/new/choose) 或[提交 pull request](CONTRIBUTING.md)，一起改进。我们欢迎首次贡献，贡献者也会保留已合入工作的署名。

加入 [Discord](https://discord.gg/37gfS3ksug)，或在微信添加 Hunter（`hunterbown`）并申请加入 Whale Brothers 群。

## 项目历史

Codewhale 起初名为 `deepseek-tui`，至今仍保留与其配置和会话的兼容性。如今它已不偏向任何提供商，由社区独立维护，也不隶属于任何模型提供商。

感谢每一位贡献者，以及帮助项目成长的开源社区。请参阅[贡献者记录](docs/CONTRIBUTORS.md)。

## 许可证

[MIT](LICENSE)。从其他开源项目改编的部分记录在[第三方声明](docs/THIRD_PARTY_NOTICES.md)中。
