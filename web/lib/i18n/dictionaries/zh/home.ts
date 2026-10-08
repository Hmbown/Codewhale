import type { HomeDict } from "../types";

/**
 * Simplified Chinese home dictionary — native copy for the whale-road landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale：适用于任意模型的开源编程智能体",
  metaDescription:
    "Codewhale 是一个在终端中运行的开源编程智能体。它使用你选择的云端或本地模型，读取项目、编辑文件并运行测试。",
  heroTitle: "适用于任意模型的开源编程智能体",
  heroIntro:
    "{brand} 在终端中读取你的项目、编辑文件并运行测试。连接云端或本地模型，并决定哪些操作需要你批准。",
  getCodewhale: "安装 Codewhale",
  heroInstallAria: "安装命令",
  exploreProduct: "了解工作方式",
  shotPreview: "终端预览",
  shotBuild: "v{version} 预发布版本",
  screenshotAlt:
    "Codewhale v{version} 预发布版本：鲸鱼标志、新会话、消息输入区、Ask 权限、Work 模式和模型状态。由隔离终端会话的实际画面渲染。",
  latestRelease: "最新发布 {tag}",
  releaseUnavailable: "发布状态暂不可用",
  currentSource: "源码",
  sourceCandidate: "未发布",
  publishedRelease: "已发布",
  figcaptionSourceCandidate: "未发布",
  chapterTerminal: "你的终端",
  chapterTerminalTitle: "实时查看每一次编辑和命令",
  gainHeading: "交出任务，保留控制权",
  gainLede: "直接说出想要的结果：修复错误、解释某个模块，或自动完成重复任务。先用一个智能体，任务变大时再增加。",
  gain: [
    [
      "修改代码并验证",
      "智能体查看你的项目、编辑文件并运行测试。你可以在它工作时查看每一次编辑和命令结果。"
    ],
    [
      "自动完成重复工作",
      "在脚本和 CI 中运行 codewhale exec。使用 Fleet 把大型任务分给多个智能体。"
    ],
    [
      "掌握执行过程",
      "开始前设定权限，回应审批请求，随时停止任务。运行 /receipts 可列出会话中的每个文件、命令和审批。"
    ]
  ],
  chapterModels: "你的模型",
  modelsHeading: "为每项任务选择模型",
  modelsBody:
    "为每个会话选择内置提供商、任意 OpenAI 兼容端点或本地模型。你的模型连接与 Codewhale 账户相互独立。",
  modelsFacts: [
    ["托管", "你自己的 API 密钥，用 codewhale auth set --provider <id> 保存"],
    ["网关", "一个端点接多个模型；提供商仍由你选择"],
    ["本地", "localhost 上的 vLLM、SGLang 或 Ollama，通常无需密钥"],
  ],
  modelsLink: "浏览模型与提供商",
  startHeading: "安装、连接模型、运行任务",
  startLede: "在项目文件夹中用三个步骤运行第一项任务。如果工作需要多个智能体，之后再添加 Fleet。",
  startGuideLink: "按照新手指引操作",
  startVocabularyLink: "查名词",
  chapterAvailability: "在哪里运行",
  availabilityHeading: "现在就在终端中使用",
  availabilityLede: "终端、本地浏览器客户端和社区维护的 CodeWhale GUI 现已可用。桌面应用和重建中的托管网页应用正在开发，两者共用同一会话模型。",
  availability: [
    [
      "终端与本地浏览器",
      "已发布",
      "在 Linux、macOS 或 Windows 上安装，然后运行 codewhale，或运行 codewhale web 打开本地浏览器客户端。也可以用 npm 或 Cargo 安装；Android 上的 Termux 版本为预览版。"
    ],
    [
      "CodeWhale GUI（VS Code）",
      "可用",
      "由社区维护的独立项目：在 VS Code 侧边栏中连接同一个 Codewhale Runtime，进行对话、管理线程并查看文件变更。可从 VS Code Marketplace 安装。",
      "https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode"
    ],
    [
      "托管网页应用",
      "开发预览",
      "正在重建，以与桌面应用保持一致。目前你可以登录，然后在正在运行的终端会话中输入 /rc，在网页上继续该会话；托管任务执行仍在验证中。"
    ],
    [
      "桌面端",
      "开发版本",
      "正在成为 Codewhale 主要客户端的原生应用：文件夹、对话和模型连接集中在一个窗口中。暂无公开下载。"
    ],
    [
      "云端计算机",
      "开发中",
      "为你运行任务的托管计算机。"
    ]
  ],
  availabilityNote: "终端、本地浏览器和 GUI 无需 Codewhale 账户。托管网页和桌面端使用账户，但账户不能代替模型连接；使用你自己的密钥产生的用量由提供商计费。",
  accountLink: "创建账户",
  surfacesHeading: "扩展智能体能接触的范围",
  surfaces: [
    ["文件与命令", "在你设定的权限内读取项目、编辑文件、运行测试并查看输出。"],
    ["插件与 MCP", "连接更多工具和服务。每个插件在你审核并启用之前都保持关闭。"],
    ["Computer Use · 预览", "让智能体查看并操作其他应用的插件。由你启用它，并授予它请求的系统权限。"],
    ["保存的会话", "将对话和工具结果保存在一起，可以接着之前的工作继续，无需从头开始。本地浏览器会打开你电脑上的同一个会话。"],
    ["Fleet", "把任务的各部分分配给使用不同模型和角色的智能体，然后跟踪它们的进度。"],
  ],
  runtimeLink: "查看全部集成",
  installBandHeading: "在 macOS 或 Linux 上安装",
  copy: "复制",
  copied: "已复制 ✓",
  binaries: "预编译包",
  chinaMirrors: "中国镜像",
  installGuideLink: "阅读安装指南",
  communityHeading: "和我们一起构建 Codewhale",
  communityBody: "在 GitHub 上报告 bug、提出功能建议，或提交你的第一个 pull request。欢迎小而经过测试的修复。",
  communityLinksAria: "社区链接",
  contribute: "提交 pull request",
};
