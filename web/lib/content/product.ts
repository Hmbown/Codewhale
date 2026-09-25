/**
 * product.ts — the copy for /product, the "what is this and what do I get"
 * page behind the primary nav's first link.
 *
 * TRUTH CONTRACT: availability is stated per surface as it is today and
 * mirrors the homepage's availability chapter and docs/public-surface-facts.json.
 * Nothing here claims cloud execution; the web app is described as account
 * sign-in plus a development preview; desktop is a development build.
 * Counts (providers, tools, sandbox backends) come from the facts layer at
 * render time, never typed here.
 */

import type { LocalizedText } from "./vocabulary";

export interface ProductRow {
  title: LocalizedText;
  body: LocalizedText;
}

export interface ProductAvailabilityRow {
  surface: LocalizedText;
  status: LocalizedText;
  detail: LocalizedText;
  /** Locale-relative route with the full story, or null. */
  href: string | null;
  linkLabel: LocalizedText | null;
}

export const PRODUCT_COPY = {
  metadata: {
    title: { en: "Product · Codewhale", zh: "产品 · Codewhale" },
    description: {
      en: "Build projects, research questions, and automate tasks with agents that use your files, tools, and choice of models.",
      zh: "用你选择的模型，让智能体利用文件和工具，帮你构建项目、研究问题、自动化任务。",
    },
  },
  title: {
    en: "Build and automate with your choice of models.",
    zh: "用你选择的模型，创造与自动化。",
  },
  lede: {
    en: "Codewhale is an open-source agent that can read a project, edit files, run commands, and use connected tools. Direct the work from your terminal or local browser, choose the model, and keep the conversation and tool results in a saved session.",
    zh: "Codewhale 是一个开源智能体，能够读取项目、编辑文件、运行命令并使用连接的工具。在终端或本地浏览器中指挥任务，选择模型，并将对话和工具结果保存在会话中。",
  },

  gainHeading: { en: "Models, teamwork, and control", zh: "模型、协作与控制" },
  gain: [
    {
      title: { en: "Choose your models", zh: "选择你的模型" },
      body: {
        en: "Connect a provider with your own API key or a supported sign-in flow, use a gateway, or run a local model server. Choose the model for each session.",
        zh: "使用自己的 API 密钥或受支持的登录方式连接提供商，也可以使用网关或本地模型服务。为每个会话选择模型。",
      },
    },
    {
      title: { en: "Share the work", zh: "分工协作" },
      body: {
        en: "Give parts of a larger task to agents with different models and roles. Save the team as a Fleet to use again.",
        zh: "让不同模型和角色的智能体分担大型任务，将团队保存为 Fleet，方便下次使用。",
      },
    },
    {
      title: { en: "Set the permissions", zh: "设定权限" },
      body: {
        en: "Choose the session mode and approval settings. Inspect tool calls and results, review file changes, interrupt work, and continue from a saved session.",
        zh: "选择会话模式与审批设置，查看工具调用和结果、检查文件变更、随时中断，并从保存的会话继续工作。",
      },
    },
  ] satisfies ProductRow[],

  availabilityHeading: { en: "Where to use Codewhale", zh: "在哪里使用 Codewhale" },
  availabilityLede: {
    en: "The terminal and local browser client are available now. Desktop and hosted web apps are being developed around the same session model; their availability is separate. The terminal and local browser work without a Codewhale account. Hosted web and desktop access use an account, while the model connection remains your choice.",
    zh: "终端与本地浏览器客户端现已可用，无需 Codewhale 账户。桌面和托管网页应用正基于同一会话模型开发，开放状态分别说明；访问这些应用需要账户，模型连接仍由你选择。",
  },
  availability: [
    {
      surface: { en: "Terminal", zh: "终端" },
      status: { en: "Released", zh: "已发布" },
      detail: {
        en: "Install GitHub release binaries for Linux, macOS, and Windows. npm and Cargo are alternatives. Android on Termux is a preview. The interactive TUI and codewhale exec for scripts ship together.",
        zh: "优先使用 GitHub Releases 中适用于 Linux、macOS、Windows 的二进制；npm 和 Cargo 为其他安装方式。Android 上的 Termux 为预览。交互式 TUI 与用于脚本的 codewhale exec 一同发布。",
      },
      href: "/install",
      linkLabel: { en: "Install guide", zh: "安装指南" },
    },
    {
      surface: { en: "Local browser", zh: "本地浏览器" },
      status: { en: "Included with the terminal", zh: "随终端提供" },
      detail: {
        en: "Run codewhale web to open Codewhale in your browser. Read conversations and tool results, send a task, and respond to approvals on your machine.",
        zh: "运行 codewhale web，在浏览器中使用本机上的 Codewhale，查看对话与工具结果、发送任务并处理本机上的审批。",
      },
      href: "/docs/web",
      linkLabel: { en: "Local browser guide", zh: "本地浏览器指南" },
    },
    {
      surface: { en: "Hosted web app", zh: "托管网页应用" },
      status: { en: "Development preview", zh: "开发预览" },
      detail: {
        en: "Sign in with a Codewhale account, then type /rc in a running terminal session to continue it from the web app. Hosted task execution is still being qualified.",
        zh: "使用 Codewhale 账户登录后，在正在运行的终端会话中输入 /rc，即可在网页应用中继续。托管任务执行仍在验证中。",
      },
      href: "/signin",
      linkLabel: { en: "Sign in", zh: "登录" },
    },
    {
      surface: { en: "Desktop", zh: "桌面端" },
      status: { en: "Development build", zh: "开发版本" },
      detail: {
        en: "The macOS app brings folders, conversations, and model connections into a desktop window. A public download is coming later.",
        zh: "macOS 应用将文件夹、对话和模型连接整合在桌面窗口中，将来会提供公开下载。",
      },
      href: null,
      linkLabel: null,
    },
    {
      surface: { en: "Cloud computers", zh: "云端计算机" },
      status: { en: "In development", zh: "开发中" },
      detail: {
        en: "Hosted computers for running your tasks.",
        zh: "用于运行任务的托管计算机。",
      },
      href: null,
      linkLabel: null,
    },
  ] satisfies ProductAvailabilityRow[],

  controlHeading: { en: "Choose how your agents work", zh: "选择智能体的工作方式" },
  controlLede: {
    en: "Use Plan to explore, Work to make changes, and Operate to coordinate agents. Approval settings control when actions need review.",
    zh: "用 Plan 探索方案、Work 执行修改、Operate 协调智能体。审批设置决定哪些操作需要审核。",
  },
  modes: [
    { title: { en: "Plan", zh: "Plan" }, body: { en: "Blocks file mutation and shell execution. Permitted research may contact external services; session state can still be saved.", zh: "禁止文件修改与 shell 执行。获准的研究可访问外部服务；会话状态仍可保存。" } },
    { title: { en: "Work", zh: "Work" }, body: { en: "Edits files and runs commands within the permission you set.", zh: "在你设定的权限内修改文件、运行命令。" } },
    { title: { en: "Operate", zh: "Operate" }, body: { en: "Runs a fleet: several agents on one job, each in its role.", zh: "调度 fleet：多个智能体各司其职，处理同一件事。" } },
  ] satisfies ProductRow[],
  permissions: [
    { title: { en: "Ask", zh: "Ask" }, body: { en: "Prompts according to the active approval rules; saved permissions and hard policy boundaries still apply.", zh: "按当前审批规则询问；已保存的权限与强制策略边界仍然生效。" } },
    { title: { en: "Auto-Review", zh: "Auto-Review" }, body: { en: "Automatically reviews eligible actions and reports any action it cannot approve.", zh: "自动审核符合条件的操作，并报告无法批准的操作。" } },
    { title: { en: "Full Access", zh: "Full Access" }, body: { en: "Reduces approval prompts. It does not bypass hard policy boundaries or grant access outside the allowed scope.", zh: "减少审批提示，但不会绕过强制策略边界，也不会授予允许范围之外的访问权限。" } },
  ] satisfies ProductRow[],

  surfacesHeading: { en: "Use the tools that fit your work", zh: "选择适合工作的工具" },
  surfacesLede: {
    en: "Tools let the agent act on a task: edit files, run a command, or use a connected service. Codewhale runs the agent and its tools locally, keeping the conversation and tool results together. Plugins use the permissions you grant.",
    zh: "工具让智能体能够执行任务：编辑文件、运行命令或使用连接的服务。Codewhale 在本地运行智能体及其工具，将对话与工具结果保存在一起。插件在你授予的权限内工作。",
  },
  surfacesLink: { en: "Explore integrations", zh: "查看集成" },

  actions: {
    install: { en: "Get Codewhale", zh: "获取 Codewhale" },
    models: { en: "See every provider", zh: "查看所有提供商" },
    docs: { en: "Read the docs", zh: "阅读文档" },
  },
} as const;
