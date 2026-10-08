/**
 * product.ts — the copy for /product, the "what is this and what do I get"
 * page behind the primary nav's first link.
 *
 * TRUTH CONTRACT: availability is stated per surface as it is today. The
 * terminal, browser, hosted web, desktop, and cloud rows mirror the homepage's
 * availability chapter and docs/public-surface-facts.json; the GUI row states
 * the community graphical frontend — a separate project from this repository's
 * own VS Code extension — exactly as README.md links it.
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
  /** Locale-relative route with the full story, an absolute external URL, or null. */
  href: string | null;
  linkLabel: LocalizedText | null;
}

export const PRODUCT_COPY = {
  metadata: {
    title: { en: "Product · Codewhale", zh: "产品 · Codewhale" },
    description: {
      en: "Codewhale is an open-source agent that reads your project, edits files, runs commands, and uses connected tools with the model you choose.",
      zh: "Codewhale 是一个开源智能体，使用你选择的模型读取项目、编辑文件、运行命令并使用连接的工具。",
    },
  },
  title: {
    en: "Hand off the work. Keep the controls.",
    zh: "把工作交出去，控制权留在手里。",
  },
  lede: {
    en: "Codewhale is an open-source agent that reads a project, edits files, runs commands, and uses connected tools. Direct it from your terminal, the local browser, or the CodeWhale GUI in VS Code, and resume any saved session.",
    zh: "Codewhale 是一个开源智能体，能够读取项目、编辑文件、运行命令并使用连接的工具。在终端、本地浏览器或 VS Code 中的 CodeWhale GUI 里指挥它，并可继续任何已保存的会话。",
  },

  gainHeading: { en: "Choose the model, split the work, set limits", zh: "选择模型、分配工作、设定限制" },
  gain: [
    {
      title: { en: "Choose your models", zh: "选择你的模型" },
      body: {
        en: "Connect a provider with your API key or a supported sign-in, use a gateway, or run a local model server. Switch models per session.",
        zh: "使用你的 API 密钥或受支持的登录方式连接提供商，也可以使用网关或本地模型服务。按会话切换模型。",
      },
    },
    {
      title: { en: "Split the work", zh: "分工协作" },
      body: {
        en: "Assign parts of a larger task to agents with different models and roles. Save the team as a Fleet and reuse it.",
        zh: "把大型任务的各部分分配给使用不同模型和角色的智能体。将团队保存为 Fleet，之后重复使用。",
      },
    },
    {
      title: { en: "Set the permissions", zh: "设定权限" },
      body: {
        en: "Pick the mode and approval settings, then inspect each tool call and file change. Interrupt at any point and continue from the saved session.",
        zh: "选择模式与审批设置，然后查看每次工具调用和文件变更。随时中断，并从保存的会话继续。",
      },
    },
  ] satisfies ProductRow[],

  availabilityHeading: { en: "Use it in your terminal today", zh: "现在就在终端中使用" },
  availabilityLede: {
    en: "The terminal, local browser client, and community CodeWhale GUI are available now and need no Codewhale account. The desktop and hosted web apps are in development, share the same session model, and use an account; your model connection stays your choice.",
    zh: "终端、本地浏览器客户端与社区维护的 CodeWhale GUI 现已可用，无需 Codewhale 账户。桌面和托管网页应用正在开发，共用同一会话模型，需要使用账户；模型连接仍由你选择。",
  },
  availability: [
    {
      surface: { en: "Terminal", zh: "终端" },
      status: { en: "Released", zh: "已发布" },
      detail: {
        en: "Install release binaries for Linux, macOS, or Windows; npm and Cargo also work, and Android on Termux is a preview. The interactive TUI and codewhale exec for scripts ship together.",
        zh: "安装适用于 Linux、macOS 或 Windows 的发布版二进制；也可以用 npm 和 Cargo 安装，Android 上的 Termux 为预览。交互式 TUI 与用于脚本的 codewhale exec 一同发布。",
      },
      href: "/install",
      linkLabel: { en: "Install guide", zh: "安装指南" },
    },
    {
      surface: { en: "Local browser", zh: "本地浏览器" },
      status: { en: "Included with the terminal", zh: "随终端提供" },
      detail: {
        en: "Run codewhale web to open Codewhale in your browser. Read the conversation, send a task, and answer approvals on your machine.",
        zh: "运行 codewhale web，在浏览器中打开 Codewhale。在本机上查看对话、发送任务并回应审批。",
      },
      href: "/docs/web",
      linkLabel: { en: "Local browser guide", zh: "本地浏览器指南" },
    },
    {
      surface: { en: "CodeWhale GUI (VS Code)", zh: "CodeWhale GUI（VS Code）" },
      status: { en: "Available", zh: "可用" },
      detail: {
        en: "A separate community project: chat, threads, and file changes in a VS Code sidebar over the local Runtime. Install it from the VS Code Marketplace; the source is on GitHub.",
        zh: "独立的社区项目：在 VS Code 侧边栏中连接本地 Runtime，进行对话、管理线程并查看文件变更。可从 VS Code Marketplace 安装；源码见 GitHub。",
      },
      href: "https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode",
      linkLabel: { en: "VS Code Marketplace", zh: "VS Code Marketplace" },
    },
    {
      surface: { en: "Hosted web app", zh: "托管网页应用" },
      status: { en: "Development preview", zh: "开发预览" },
      detail: {
        en: "Being rebuilt to match the desktop app. Today you can sign in, then type /rc in a running terminal session to continue it on the web; hosted task execution is still being qualified.",
        zh: "正在重建，以与桌面应用保持一致。目前你可以登录，然后在正在运行的终端会话中输入 /rc，在网页上继续该会话；托管任务执行仍在验证中。",
      },
      href: "/signin",
      linkLabel: { en: "Sign in", zh: "登录" },
    },
    {
      surface: { en: "Desktop", zh: "桌面端" },
      status: { en: "Development build", zh: "开发版本" },
      detail: {
        en: "The native app becoming the main Codewhale client: folders, conversations, and model connections in one window. No public download yet.",
        zh: "正在成为 Codewhale 主要客户端的原生应用：文件夹、对话和模型连接集中在一个窗口中。暂无公开下载。",
      },
      href: null,
      linkLabel: null,
    },
    {
      surface: { en: "Cloud computers", zh: "云端计算机" },
      status: { en: "In development", zh: "开发中" },
      detail: {
        en: "Hosted computers that run your tasks.",
        zh: "为你运行任务的托管计算机。",
      },
      href: null,
      linkLabel: null,
    },
  ] satisfies ProductAvailabilityRow[],

  controlHeading: { en: "Set what the agent may do", zh: "设定智能体可以做什么" },
  controlLede: {
    en: "Use Plan to explore, Work to make changes, and Operate to coordinate agents. Approval settings decide which actions wait for you.",
    zh: "用 Plan 探索方案、Work 执行修改、Operate 协调智能体。审批设置决定哪些操作需要等你确认。",
  },
  modes: [
    { title: { en: "Plan", zh: "Plan" }, body: { en: "Blocks file mutation and shell execution. Permitted research may contact external services; session state can still be saved.", zh: "禁止文件修改与 shell 执行。获准的研究可访问外部服务；会话状态仍可保存。" } },
    { title: { en: "Work", zh: "Work" }, body: { en: "Edits files and runs commands within the permission you set.", zh: "在你设定的权限内修改文件、运行命令。" } },
    { title: { en: "Operate", zh: "Operate" }, body: { en: "Runs a Fleet: several agents on one job, each in its role.", zh: "运行 Fleet：多个智能体按各自角色处理同一项任务。" } },
  ] satisfies ProductRow[],
  permissions: [
    { title: { en: "Ask", zh: "Ask" }, body: { en: "Prompts according to the active approval rules; saved permissions and hard policy boundaries still apply.", zh: "按当前审批规则询问；已保存的权限与强制策略边界仍然生效。" } },
    { title: { en: "Auto-Review", zh: "Auto-Review" }, body: { en: "Automatically reviews eligible actions and reports any action it cannot approve.", zh: "自动审核符合条件的操作，并报告无法批准的操作。" } },
    { title: { en: "Full Access", zh: "Full Access" }, body: { en: "Reduces approval prompts. It does not bypass hard policy boundaries or grant access outside the allowed scope.", zh: "减少审批提示，但不会绕过强制策略边界，也不会授予允许范围之外的访问权限。" } },
  ] satisfies ProductRow[],

  surfacesHeading: { en: "Connect the tools your work needs", zh: "连接工作所需的工具" },
  surfacesLede: {
    en: "Tools let the agent act: edit files, run commands, or call a connected service. Codewhale runs the agent and its tools on your machine, and plugins get only the permissions you grant.",
    zh: "工具让智能体能够行动：编辑文件、运行命令或调用连接的服务。Codewhale 在你的电脑上运行智能体及其工具，插件只获得你授予的权限。",
  },
  surfacesLink: { en: "See all integrations", zh: "查看全部集成" },

  actions: {
    install: { en: "Install Codewhale", zh: "安装 Codewhale" },
    models: { en: "See every provider", zh: "查看所有提供商" },
    docs: { en: "Read the docs", zh: "阅读文档" },
  },
} as const;
