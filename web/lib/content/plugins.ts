import type { LocalizedText } from "./vocabulary";

export const PLUGINS_COPY = {
  metaTitle: { en: "Plugins and marketplace · Codewhale", zh: "插件与市场 · Codewhale" },
  metaDescription: {
    en: "Computer Use puts the agent's eyes and hands on your real desktop — plus a plugin marketplace where any GitHub repository can become a reviewed, trusted capability.",
    zh: "Computer Use 让代理的眼睛和手触及你的真实桌面——插件市场中任何 GitHub 仓库都可以成为经审阅、受信任的能力。",
  },
  kicker: { en: "Plugins and marketplace", zh: "插件与市场" },
  title: { en: "Your agent, on your actual computer.", zh: "你的代理，就在你真实的电脑上。" },
  lead: {
    en: "Codewhale plugins add reviewed capabilities to the agent you already run — from the first-party Codewhale catalog, a GitHub repository, or a tarball. Nothing executes until you read the manifest and trust it yourself.",
    zh: "Codewhale 插件为你已在运行的代理添加经审阅的能力——来自官方 Codewhale 目录、GitHub 仓库或压缩包。在你亲自阅读清单并信任之前，不会执行任何内容。",
  },
  marketplaceCta: { en: "Browse the Codewhale catalog", zh: "浏览 Codewhale 目录" },
  docsCta: { en: "Plugin authoring guide", zh: "插件编写指南" },

  cuLabel: { en: "First-party plugin · source preview", zh: "官方插件 · 源码预览" },
  cuTitle: { en: "Computer Use: eyes and hands for the agent.", zh: "Computer Use：代理的眼睛与手。" },
  cuLead: {
    en: "The agent stops being blind to your desktop. It observes the apps you choose, reads their controls through the accessibility tree, types and clicks where you point it — then reports every action with a receipt. Launch qualification covers the local macOS candidate; Windows, Linux and HarmonyOS paths are implemented and still under device verification.",
    zh: "代理不再对你的桌面视而不见。它观察你选择的应用，通过无障碍树读取控件，在你指定的地方输入与点击——然后用回执报告每一步操作。发布认证目前覆盖本地 macOS 候选版本；Windows、Linux 与 HarmonyOS 路径已实现，仍在设备验证中。",
  },
  cuInstall: { en: "/plugin marketplace install codewhale computer-use", zh: "/plugin marketplace install codewhale computer-use" },
  cuFeatures: [
    {
      title: { en: "Works in the background", zh: "后台工作" },
      detail: {
        en: "On macOS the agent can drive a selected app without touching your pointer or stealing focus — process-directed input while you keep using the machine. Foreground control is a separate, explicit grant.",
        zh: "在 macOS 上，代理可以驱动指定应用而不移动你的指针或抢占焦点——你在前台继续工作，它在后台定向输入。前台控制是独立的显式授权。",
      },
    },
    {
      title: { en: "Reads the UI, not just pixels", zh: "读懂界面，而非只看像素" },
      detail: {
        en: "The accessibility tree (macOS AX, Windows UIA, Linux AT-SPI) returns controls, values, and advertised actions — a text-only model can drive a whole app without a single screenshot.",
        zh: "无障碍树（macOS AX、Windows UIA、Linux AT-SPI）返回控件、值与可执行动作——纯文本模型无需任何截图即可驱动整个应用。",
      },
    },
    {
      title: { en: "Sees when it has to", zh: "必要时才看" },
      detail: {
        en: "Screenshots and zoom feed compatible vision models; on macOS, local OCR reads window text on-device — no remote service, no vision model required.",
        zh: "截图与缩放供视觉模型使用；在 macOS 上，本地 OCR 直接在设备上读取窗口文字——无需远程服务，也无需视觉模型。",
      },
    },
    {
      title: { en: "One agent, many computers", zh: "一个代理，多台计算机" },
      detail: {
        en: "Control the local machine, registered SSH hosts (the remote agent installs itself), or HarmonyOS devices over hdc. Every receipt names the computer it happened on.",
        zh: "控制本机、已注册的 SSH 主机（远程代理自动安装）或通过 hdc 连接的 HarmonyOS 设备。每条回执都标明它发生在哪台计算机上。",
      },
    },
    {
      title: { en: "Permissions stay yours", zh: "权限仍归你所有" },
      detail: {
        en: "Accessibility and Screen Recording grants live in System Settings under your control. The agent asks once what it is missing — it never pops dialogs.",
        zh: "辅助功能与屏幕录制权限由你在系统设置中掌控。代理只会询问一次缺少什么——绝不弹出对话框。",
      },
    },
    {
      title: { en: "Fails closed, with receipts", zh: "失败即停，回执为证" },
      detail: {
        en: "Stale observations, unexpected foreground changes, and unavailable capabilities refuse with a receipt instead of guessing. Each task owns its session; stopping releases every held input.",
        zh: "过期的观察结果、意外的前台变化与不可用的能力都会携带回执拒绝执行，而非猜测。每个任务独占自己的会话；停止即释放所有按住的输入。",
      },
    },
  ],
  cuToolsNote: {
    en: "39 MCP tools: observation, actions, screenshots and zoom, keyboard and pointer, clipboard, recording, and computer switching.",
    zh: "39 个 MCP 工具：观察、操作、截图与缩放、键盘与指针、剪贴板、录制以及计算机切换。",
  },

  catalogLabel: { en: "First-party catalog", zh: "官方目录" },
  catalogTitle: { en: "The Codewhale marketplace.", zh: "Codewhale 市场。" },
  catalogLead: {
    en: "An offline snapshot of the Codewhale catalog ships with the terminal and the app, so browsing never fetches or executes anything. Install from it with one command.",
    zh: "终端与应用内置 Codewhale 目录的离线快照，浏览不会拉取或执行任何内容。一条命令即可安装。",
  },
  catalogBrowse: { en: "/plugin marketplace list", zh: "/plugin marketplace list" },
  catalogEntries: [
    {
      title: { en: "WhaleWiki", zh: "WhaleWiki" },
      detail: {
        en: "Repository and concept documentation the agent can consult while it works.",
        zh: "供代理在工作时查阅的仓库与概念文档。",
      },
      reference: "/plugin marketplace install codewhale whalewiki",
    },
    {
      title: { en: "Cloudflare Docs", zh: "Cloudflare 文档" },
      detail: {
        en: "Cloudflare documentation lookup inside a session.",
        zh: "在会话内查询 Cloudflare 文档。",
      },
      reference: "/plugin marketplace install codewhale cloudflare-docs",
    },
    {
      title: { en: "Skill bundle", zh: "技能包" },
      detail: {
        en: "The bundled Codewhale skills the terminal ships with, kept in sync with the catalog.",
        zh: "终端内置的 Codewhale 技能集，与目录保持同步。",
      },
      reference: "#path=skills",
    },
  ],
  catalogNote: {
    en: "Catalog source: github.com/Hmbown/codewhale-plugin-marketplace. A local catalog named codewhale takes precedence over the bundled snapshot.",
    zh: "目录来源：github.com/Hmbown/codewhale-plugin-marketplace。名为 codewhale 的本地目录优先于内置快照。",
  },

  sourcesLabel: { en: "Install sources", zh: "安装来源" },
  sourcesTitle: { en: "Any GitHub repository can be a plugin.", zh: "任何 GitHub 仓库都可以是插件。" },
  sourcesLead: {
    en: "Point /plugin install at a directory, a repository, or a tarball. The fetched tree must hold exactly one bundle root — a plugin.json, a compatible kimi.plugin.json or .claude-plugin/plugin.json, or a legacy plugin.toml.",
    zh: "/plugin install 可以指向目录、仓库或压缩包。获取的目录树必须恰好包含一个 bundle 根——plugin.json、兼容的 kimi.plugin.json 或 .claude-plugin/plugin.json，或旧版 plugin.toml。",
  },
  sources: [
    {
      title: { en: "GitHub repository", zh: "GitHub 仓库" },
      detail: {
        en: "Archive of the repository’s default branch — the marketplace catalog uses the same mechanism.",
        zh: "仓库默认分支的归档——市场目录使用同一机制。",
      },
      reference: "/plugin install github:owner/repo",
    },
    {
      title: { en: "Local directory", zh: "本地目录" },
      detail: {
        en: "Copied from disk. The fastest loop while authoring your own bundle.",
        zh: "从磁盘复制。编写自己的 bundle 时最快的迭代方式。",
      },
      reference: "/plugin install ./path/to/bundle",
    },
    {
      title: { en: "Tarball URL", zh: "压缩包 URL" },
      detail: {
        en: "Direct archive download, gated by the per-domain network policy — an unknown host is refused until you allow it.",
        zh: "直接下载归档，受按域网络策略约束——未知主机会被拒绝，直到你允许它。",
      },
      reference: "/plugin install https://example.com/x.tar.gz",
    },
  ],

  trustLabel: { en: "Trust lifecycle", zh: "信任生命周期" },
  trustTitle: { en: "Installed is not enabled.", zh: "已安装不等于已启用。" },
  trustLead: {
    en: "Every bundle lands disabled and untrusted. Review the manifest and declared capabilities, then trust and enable it. An update with changed bytes requires review again.",
    zh: "每个 bundle 安装后均为未启用、未信任状态。审阅清单与声明的能力后，再信任并启用。内容有变更的更新需要重新审阅。",
  },
  trustSteps: [
    {
      title: { en: "Review", zh: "审阅" },
      detail: {
        en: "/plugin show <name> prints the manifest, declared capabilities, and install receipt — including the source and bundle selector.",
        zh: "/plugin show <name> 打印清单、声明的能力与安装回执——包括来源与 bundle 选择器。",
      },
    },
    {
      title: { en: "Trust", zh: "信任" },
      detail: {
        en: "/plugin trust <name> marks your decision. Official catalog provenance grants no execution or network permission on its own.",
        zh: "/plugin trust <name> 记录你的决定。官方目录来源本身不授予任何执行或网络权限。",
      },
    },
    {
      title: { en: "Enable", zh: "启用" },
      detail: {
        en: "/plugin enable <name> activates the bundle. Plugin-contributed MCP servers run under namespaced <plugin>-<server> identities with the stricter boundary.",
        zh: "/plugin enable <name> 激活 bundle。插件提供的 MCP 服务以 <plugin>-<server> 命名空间身份运行，适用更严格的边界。",
      },
    },
  ],
  trustNote: {
    en: "Downloads are tarball-only with a size cap and no symlinks; traversal, ambiguous roots, and changed plugin identities are rejected. /plugin suggest ranks installed and catalog bundles against a task without installing anything.",
    zh: "下载仅限压缩包，有大小限制且不允许符号链接；路径穿越、模糊根目录与插件身份变更都会被拒绝。/plugin suggest 仅对已安装与目录中的 bundle 按任务排序，不会安装任何内容。",
  },

  commandsTitle: { en: "Command reference", zh: "命令参考" },
  commandsLead: {
    en: "The full lifecycle lives in the terminal. The documented commands:",
    zh: "完整生命周期在终端中进行。已记录的命令：",
  },
} satisfies Record<
  string,
  | LocalizedText
  | { title: LocalizedText; detail: LocalizedText; reference: string }[]
  | { title: LocalizedText; detail: LocalizedText }[]
>;

export const PLUGIN_COMMANDS = [
  "/plugin marketplace list",
  "/plugin marketplace show codewhale",
  "/plugin marketplace install codewhale <name>",
  "/plugin install github:owner/repo",
  "/plugin show <name>",
  "/plugin trust <name>",
  "/plugin enable <name>",
  "/plugin update <name>",
  "/plugin suggest <task>",
] as const;
