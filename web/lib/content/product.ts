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
    title: { en: "Build and automate · Codewhale", zh: "构建与自动化 · Codewhale" },
    description: {
      en: "Build apps, automate workflows and work across connected tools with an open-source agent. Bring your existing model APIs or local and self-hosted inference.",
      zh: "用开源智能体构建应用、自动处理工作，并连接你常用的工具。使用你已有的模型 API、本地模型或自托管推理服务。",
    },
  },
  title: {
    en: "Put your models to work.",
    zh: "让你的模型，真正做事。",
  },
  lede: {
    en: "Build an app, automate your reporting, or connect the tools behind a recurring workflow. Codewhale is an open-source agent that works across your files, terminal and connected services, using your existing model APIs or your own inference.",
    zh: "构建一个应用，让报表自动生成，或把重复工作涉及的工具连接起来。Codewhale 是一个开源智能体，使用你已有的模型 API 或自己的推理服务，在文件、终端和已连接的服务之间完成工作。",
  },

  gainHeading: { en: "Bring it a real task.", zh: "交给它一件实实在在的事。" },
  gain: [
    {
      title: { en: "Build apps and useful tools", zh: "构建应用和实用工具" },
      body: {
        en: "Create a new application, extend a project, turn data into a report, or write a script that saves you time. The agent can create files, run commands and test the result.",
        zh: "做一个新应用、扩展现有项目、将数据整理成报告，或写一个节省时间的脚本。智能体可以创建文件、运行命令并测试成果。",
      },
    },
    {
      title: { en: "Work across your apps", zh: "让应用一起参与工作" },
      body: {
        en: "Connect services such as Gmail and Slack through plugins, MCP servers or APIs. Bring what the task needs into the same session as your files and commands.",
        zh: "通过插件、MCP 服务或 API 连接 Gmail、Slack 等服务。让任务所需的工具与文件、命令在同一个会话中协作。",
      },
    },
    {
      title: { en: "Make a workflow you can reuse", zh: "把工作变成可复用的流程" },
      body: {
        en: "Run tasks from scripts or CI with codewhale exec. For larger jobs, save a Fleet of agents with different roles and models and bring the team back when you need it.",
        zh: "用 codewhale exec 从脚本或 CI 中运行任务。面对更大的工作，保存由不同角色、不同模型组成的 Fleet 智能体团队，需要时再一起上场。",
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

  controlHeading: { en: "Choose the access. Keep control.", zh: "访问范围你来定，控制权在你手里。" },
  controlLede: {
    en: "Use Plan to explore, Work to carry out a task, and Operate to coordinate larger jobs. Approval settings decide which actions wait for you.",
    zh: "用 Plan 探索方案、Work 执行任务、Operate 协调较大的工作。审批设置决定哪些操作需要等你确认。",
  },
  modes: [
    { title: { en: "Plan", zh: "Plan" }, body: { en: "Blocks file mutation and shell execution. Permitted research may contact external services; session state can still be saved.", zh: "禁止文件修改与 shell 执行。获准的研究可访问外部服务；会话状态仍可保存。" } },
    { title: { en: "Work", zh: "Work" }, body: { en: "Edits files and runs commands within the permission you set.", zh: "在你设定的权限内修改文件、运行命令。" } },
    { title: { en: "Operate", zh: "Operate" }, body: { en: "Coordinates larger jobs, delegating to other agents when useful. Uses the same tools and permission boundaries as Work.", zh: "协调较大的任务，按需委派其他智能体。使用与 Work 相同的工具，遵循相同的权限边界。" } },
  ] satisfies ProductRow[],
  permissions: [
    { title: { en: "Ask", zh: "Ask" }, body: { en: "Prompts according to the active approval rules; saved permissions and hard policy boundaries still apply.", zh: "按当前审批规则询问；已保存的权限与强制策略边界仍然生效。" } },
    { title: { en: "Auto-Review", zh: "Auto-Review" }, body: { en: "Automatically reviews eligible actions and reports any action it cannot approve.", zh: "自动审核符合条件的操作，并报告无法批准的操作。" } },
    { title: { en: "Full Access", zh: "Full Access" }, body: { en: "Reduces approval prompts. It does not bypass hard policy boundaries or grant access outside the allowed scope.", zh: "减少审批提示，但不会绕过强制策略边界，也不会授予允许范围之外的访问权限。" } },
  ] satisfies ProductRow[],

  surfacesHeading: { en: "Files, apps and agents in one task.", zh: "一项任务，连接文件、应用和智能体。" },
  surfacesLede: {
    en: "Work with your project, connected services, browser tools and agent teams from one session. Choose the model, add the connections you need, and return to the work without starting over.",
    zh: "在同一个会话中使用项目文件、已连接的服务、浏览器工具和智能体团队。选择模型，添加所需的连接，之后随时接着做，无需从头开始。",
  },
  surfacesLink: { en: "See all integrations", zh: "查看全部集成" },

  actions: {
    install: { en: "Install Codewhale", zh: "安装 Codewhale" },
    models: { en: "See every provider", zh: "查看所有提供商" },
    docs: { en: "Read the docs", zh: "阅读文档" },
  },
} as const;
