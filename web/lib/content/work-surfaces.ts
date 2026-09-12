/** Public entry points over the existing Codewhale runtime. */
export const WORK_SURFACES = [
  {
    id: "terminal",
    label: { en: "Terminal", zh: "终端" },
    title: { en: "Start where your project lives.", zh: "在项目所在的地方开始。" },
    body: { en: "Open a project folder, choose your model, and work with an agent that can read, edit, run commands, and check the result.", zh: "打开项目文件夹，选择模型，让智能体读取文件、编辑代码、运行命令并检查结果。" },
    command: "codewhale",
    href: "/docs/guide",
    link: { en: "Start your first session", zh: "开始第一个会话" },
  },
  {
    id: "browser",
    label: { en: "Local browser", zh: "本地浏览器" },
    title: { en: "The same work. A little more room.", zh: "同一项工作，更宽阔的视野。" },
    body: { en: "Open your local workspace in a browser. Find recent threads, preview saved sessions, follow tool results, and respond to approvals.", zh: "在浏览器中打开本地工作空间。查找最近的对话，预览已保存会话，查看工具结果并处理审批。" },
    command: "codewhale web",
    href: "/docs/web",
    link: { en: "Explore the browser client", zh: "了解浏览器客户端" },
  },
  {
    id: "automation",
    label: { en: "Scripts & CI", zh: "脚本与 CI" },
    title: { en: "Make useful work repeatable.", zh: "让有用的工作可以重复执行。" },
    body: { en: "Run a task from a script or CI job. Use the same tools and permission controls, with output you can inspect when the task finishes.", zh: "从脚本或 CI 任务运行工作，使用相同的工具与权限控制，并在任务完成后检查输出。" },
    command: 'codewhale exec "Explain the structure of this project"',
    href: "/docs/runtime-api",
    link: { en: "Read the automation guide", zh: "阅读自动化指南" },
  },
] as const;

export const WORK_SURFACES_COPY = {
  label: { en: "Choose how you work", zh: "选择工作方式" },
  prerequisite: { en: "After installation, run this in your project folder.", zh: "安装后，在项目文件夹中运行。" },
  install: { en: "Need to install?", zh: "还没安装？" },
};
