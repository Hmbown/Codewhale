# Codewhale

Codewhale is an open-source agent that reads your project, edits files, runs
commands, and checks its work using a hosted or local model you choose. Start
with one task in your terminal. For a larger job, give parts of the work to
agents with different models and roles.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="brand/wordmark-inverted.svg">
  <img src="brand/wordmark.svg" alt="Codewhale" width="360">
</picture>

[简体中文](README.zh-CN.md) · [日本語](README.ja-JP.md) · [Tiếng Việt](README.vi.md) · [Bahasa Indonesia](README.id.md) · [한국어](README.ko-KR.md) · [Español](README.es-419.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md) · [Українська](README.uk.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [繁體中文](README.zh-TW.md) · [हिन्दी](README.hi.md) · [Türkçe](README.tr.md) · [Italiano](README.it.md) · [Polski](README.pl.md) · [العربية](README.ar.md) · [Català](README.ca.md)

[![CI](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml/badge.svg)](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/codewhale-cli?label=crates.io)](https://crates.io/crates/codewhale-cli)
[![npm](https://img.shields.io/npm/v/codewhale?label=npm)](https://www.npmjs.com/package/codewhale)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.gg/37gfS3ksug)

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="web/public/codewhale-tui-171acee.png">
  <img src="web/public/codewhale-tui-171acee.png" alt="A Codewhale terminal session" width="720">
</picture>

*Terminal preview from a v0.9.12 development build.*

## Install

macOS / Linux — install the official GitHub release:

```bash
curl -fsSL https://codewhale.net/install.sh | sh
"$HOME/.local/bin/codewhale"
```

The installer selects the latest published release. The [changelog](CHANGELOG.md)
also describes the next release's unreleased candidate; those changes are not
included in published downloads until the release is available.

Windows: download the matching installer or archive from
[GitHub Releases](https://github.com/Hmbown/CodeWhale/releases/latest).
For an existing direct install, run `codewhale update` (or `codewhale update --check`
to inspect it). The updater prints the executable path and keeps newer builds.


The first run helps you connect a provider or configure Codewhale offline.
Model replies require a connected hosted or local model. Codewhale also
supports npm and Cargo as secondary packaging routes, plus Docker, Nix, Scoop,
Android/Termux, and an optional CNB mirror. Existing package-managed installs
receive migration instructions. See [installation and PATH help](docs/INSTALL.md).

Tab completion is one command per shell — `codewhale completion bash|zsh|fish|powershell|elvish`.
See [shell completions](docs/INSTALL.md#8-shell-completions).

## Use

Open a terminal in your project folder and run `codewhale`. Choose your provider
with `/provider` and your model with `/model`. Then describe a concrete task:

```text
Fix the failing tests and explain what changed.
```

Or run a task without opening the TUI:

```bash
codewhale exec "fix the failing tests and explain what changed"
```

Codewhale can read your repository, edit files, run commands, inspect results,
and keep working toward a goal. Use `/mode plan` to explore without file changes
or shell execution, and `/mode work` when you want it to make changes. Press
`Shift+Tab` to choose Ask, Auto-Review, or Full Access; the
[modes and permissions guide](docs/MODES.md) explains what each allows.

## Terminal, apps, and Computer Use

The terminal and graphical clients connect to the Codewhale Runtime, which runs
the agent and its tools:

- **Terminal:** `codewhale` opens the interactive interface; `codewhale exec`
  runs a task from a script or CI job.
- **Local browser:** `codewhale web` opens the bundled
  [local web client](docs/WEB.md) for the same runtime.
- **Codewhale web and desktop apps:** graphical workbenches in development.
  Their availability is listed on the [product page](https://codewhale.net/en/product).

**Computer Use adds tools for observing and interacting with other applications.**
The plugin is included in the current source.
Review its requested access and enable it before use; OS permissions and
platform requirements still apply. See the included
[Computer Use guide](crates/tui/plugins/computer-use/README.md) and
[plugin setup](docs/PLUGINS.md).

For VS Code, the community-maintained CodeWhale extension connects to the local
Runtime from a sidebar. Install it from the
[VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode);
source code is on [GitHub](https://github.com/HengQuWorld/CodeWhale-VSCode).

## Why Codewhale

- **Choose your models.** Connect hosted providers or local models through
  Ollama, vLLM, or SGLang. Use `/provider` to change providers and `/model` to
  choose a model.
- **Stay in control.** Inspect proposed actions and resulting file changes.
  Approval settings govern when review is needed; Full Access still respects
  hard policy boundaries. `/undo` and `/restore` help recover workspace changes.
- **Keep long work organized.** Save sessions, set a durable `/goal`, review
  workflows before they run, and coordinate agents without turning their
  internal instructions into your transcript.
- **Extend the agent you already have.** Connect MCP servers and skills,
  configure hooks, and keep agent roles as readable files in your project or
  personal settings.

Run `/help` in the TUI for commands and keyboard shortcuts.

## Safety

Codewhale runs on your machine with the access you grant it. Approval modes and
repository rules limit what the agent may do; optional OS sandboxing adds a
stronger execution boundary where supported. Unknown model prices stay unknown
instead of being reported as free.

Read [authorization order](docs/AUTHORIZATION_ORDER.md) for the exact policy
stack and [configuration](docs/CONFIGURATION.md) for local settings.

## Documentation

- [Providers and local models](docs/PROVIDERS.md)
- [Agent teams](docs/FLEET.md)
- [MCP](docs/MCP.md), [hooks](docs/HOOKS.md), and [configuration](docs/CONFIGURATION.md)
- [Local web client](docs/WEB.md)
- [All documentation](docs)
- [Repository layout and contribution guide](CONTRIBUTING.md#project-structure)

## Join the community

**Bug reports, feature ideas, and pull requests are welcome**, whether you have
used Codewhale for months or are trying it for the first time. If a provider is
missing, a workflow is awkward, or the terminal UI gets in your way,
[open an issue](https://github.com/Hmbown/CodeWhale/issues/new/choose) or
[send a pull request](CONTRIBUTING.md) so we can improve it together. We welcome
first contributions, and contributors keep credit for the work that lands.

Join the [Discord](https://discord.gg/37gfS3ksug), or add Hunter on WeChat
(`hunterbown`) and ask to join the Whale Brothers group.

## Project history

Codewhale began as `deepseek-tui` and still preserves that configuration and
session compatibility. It is now provider-neutral and independently maintained;
it is not affiliated with any model provider.

Thanks to every contributor and to the open source communities that helped the
project grow. See [the contributor record](docs/CONTRIBUTORS.md).

## License

[MIT](LICENSE). Portions adapted from other open-source projects are recorded
in [third-party notices](docs/THIRD_PARTY_NOTICES.md).
