/**
 * getting-started.ts — the canonical new-user path for codewhale.net.
 *
 * Four steps, in order: install → provider connection → first task
 * → optional fleet setup. Both the homepage band and the /docs/guide page
 * render from this module, so the path reads identically everywhere.
 *
 * TRUTH CONTRACT:
 *   - Step copy must match documented behavior in docs/GUIDE.md, docs/MODES.md,
 *     docs/PROVIDERS.md, and docs/FLEET.md. The runtime launches without any
 *     API key (recommended working-agreement setup); model replies require a provider —
 *     hosted key or a keyless loopback route. Do not imply otherwise.
 *   - `href` values are locale-relative (no locale prefix); consumers render
 *     `/${locale}${href}` and the tests assert every target route exists.
 *
 * EXTENSION PATH FOR NEW LOCALES: add the locale key to each `{ en, zh }`
 * pair; commands stay locale-agnostic shell.
 */

import type { LocalizedText } from "./vocabulary";

export interface GuideStep {
  id: "install" | "first-session" | "connect-provider" | "fleet-workflow";
  title: LocalizedText;
  body: LocalizedText;
  /** Locale-agnostic shell commands shown for the step (may be empty). */
  commands: string[];
  /** Deeper-reading link; href is locale-relative. */
  link: { href: string; label: LocalizedText };
}

export const GETTING_STARTED_STEPS: GuideStep[] = [
  {
    id: "install",
    title: { en: "Install Codewhale", zh: "安装 Codewhale" },
    body: {
      en: "This command installs the latest release into ~/.local/bin on macOS or Linux. If that folder is not on your PATH, the installer prints the line to add for your shell; run it before the next step. For Windows, package managers, or source builds, use the install guide.",
      zh: "这条命令会在 macOS 或 Linux 上把最新发布版本安装到 ~/.local/bin。如果该目录不在 PATH 中，安装程序会打印需要为你的 shell 添加的那一行；请先运行它，再进行下一步。Windows、包管理器或源码构建，请参阅安装指南。",
    },
    commands: ["curl -fsSL https://codewhale.net/install.sh | sh"],
    link: {
      href: "/install",
      label: { en: "Full install guide", zh: "完整安装指南" },
    },
  },
  {
    id: "connect-provider",
    title: { en: "Connect your model", zh: "连接你的模型" },
    body: {
      en: "Codewhale needs a model to reply. Save your own provider key, as in the DeepSeek example below, or run a local model such as Ollama with no key. You pay the provider directly.",
      zh: "Codewhale 需要一个模型来回复。你可以像下方的 DeepSeek 示例那样保存自己的提供商密钥，也可以运行 Ollama 等本地模型，无需密钥。费用由你直接付给提供商。",
    },
    commands: ["codewhale auth set --provider deepseek"],
    link: {
      href: "/docs/auth",
      label: { en: "Connect a provider", zh: "连接模型提供商" },
    },
  },
  {
    id: "first-session",
    title: { en: "Run your first task", zh: "运行第一项任务" },
    body: {
      en: "Run codewhale in your project folder and ask for something concrete. (If the command is not found, finish the PATH line from step 1.) Start in /mode plan to have it explain the project without changing anything, then switch to /mode work for edits and commands. It shows each edit as a diff and asks before running a shell command.",
      zh: "在项目文件夹中运行 codewhale，交给它一件具体的事。（如果找不到该命令，请先完成第 1 步中的 PATH 设置。）可以先用 /mode plan 让它在不改动任何东西的前提下讲解项目，需要修改文件或运行命令时再切换到 /mode work。它会以 diff 展示每一处修改，并在运行 shell 命令前先征求你的同意。",
    },
    commands: ["codewhale"],
    link: {
      href: "/docs/modes",
      label: { en: "Set modes and approvals", zh: "设置模式与审批" },
    },
  },
  {
    id: "fleet-workflow",
    title: { en: "Add a Fleet when the work splits", zh: "任务可以拆分时添加 Fleet" },
    body: {
      en: "When a task needs several models and roles, run /fleet setup inside Codewhale to save each role and its model. From your shell, codewhale fleet status counts queued, running, and finished Fleet runs.",
      zh: "当任务需要多个模型和角色时，在 Codewhale 中运行 /fleet setup，保存每个角色及其使用的模型。在 shell 中，codewhale fleet status 会统计排队中、运行中和已结束的 Fleet 运行。",
    },
    commands: ["/fleet setup", "codewhale fleet status"],
    link: {
      href: "/docs/fleet",
      label: { en: "Set up a Fleet", zh: "配置 Fleet" },
    },
  },
];

/**
 * Where to go after the path — discovery links rendered at the end of the
 * /docs/guide page. Hooks are first-class here on purpose: they are the
 * supported extension point a new user should find without digging.
 */
export const GUIDE_NEXT_LINKS: { href: string; label: LocalizedText; note: LocalizedText }[] = [
  {
    href: "/docs/review",
    label: { en: "Review what changed", zh: "查看改动" },
    note: {
      en: "See every edit in a session, roll files back to an earlier turn, and get a code review before you push.",
      zh: "查看一次会话中的每处修改，把文件回滚到之前的回合，并在推送前做一次代码审查。",
    },
  },
  {
    href: "/docs/modes",
    label: { en: "Set modes and approvals", zh: "设置模式与审批" },
    note: {
      en: "Plan, Work, or Operate for the kind of work; Ask, Auto-Review, or Full Access for when it stops to ask you.",
      zh: "用 Plan、Work、Operate 选择工作类型，用 Ask、Auto-Review、Full Access 决定它什么时候停下来问你。",
    },
  },
  {
    href: "/docs/hooks",
    label: { en: "Run commands on events", zh: "在事件发生时运行命令" },
    note: {
      en: "Run your own scripts when a session starts, before a tool call, or when a turn ends. Use them to add context, enforce a rule, or send a notification.",
      zh: "在会话开始、工具调用之前或回合结束时运行你自己的脚本。可以用它们补充上下文、执行规则或发送通知。",
    },
  },
];
