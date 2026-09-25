import type { HomeDict } from "../types";

/**
 * Simplified Chinese home dictionary — native copy for the Tidal Folio landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale — 用你选择的模型，完成开发和自动化任务",
  metaDescription:
    "借助开源智能体和你选择的云端或本地 AI 模型，开发软件、处理文件，并自动完成日常任务。",
  heroTitle: "用你选择的模型，完成开发和自动化任务",
  heroIntro:
    "{brand} 是一个开源智能体，能够读取文件、编辑代码、运行命令，并可运行测试来检查结果。在终端或本地浏览器中使用它，连接你选择的云端或本地模型。工具和权限由你决定；对话与工具执行结果保存在会话中。",
  getCodewhale: "获取 Codewhale",
  heroInstallAria: "安装命令",
  exploreProduct: "了解产品",
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
  gainHeading: "你可以用 Codewhale 做什么",
  gainLede: "从具体目标开始：修复错误、理解项目，或把重复任务变成工作流程。先用一个智能体，需要时再拆分大型任务。",
  gain: [
    [
      "开发项目并验证结果",
      "让智能体查看项目、完成修改，再运行测试。你可以在工作过程中查看文件变更和命令结果。"
    ],
    [
      "复用重复的工作",
      "把重复任务变成脚本或保存的工作流程。在脚本和 CI 中使用 codewhale exec，也可以让多个智能体分担大型任务。"
    ],
    [
      "掌握执行过程",
      "开始前设定权限，处理审批请求，随时中断正在运行的任务。查看对话与工具结果，再决定如何继续。"
    ]
  ],
  modelsHeading: "为每项任务选择合适的模型",
  modelsBody:
    "为每个会话选择提供商和模型：使用 API 密钥、受支持的提供商登录方式，或本地模型。Codewhale 账户与模型连接各有用途。",
  modelsFacts: [
    ["托管", "你自己的 API 密钥，用 codewhale auth set --provider <id> 保存"],
    ["网关", "一个端点接多个模型，提供商仍由你选"],
    ["本地", "localhost 上的 vLLM、SGLang、Ollama——通常无需密钥"],
  ],
  modelsLink: "了解模型与提供商",
  startHeading: "开始使用 Codewhale",
  startLede: "安装已发布版本，连接模型，然后在项目文件夹中尝试一项任务。智能体团队是可选的；先用一个智能体，等任务适合拆分时再增加。",
  startGuideLink: "阅读新手指引",
  startVocabularyLink: "查名词",
  availabilityHeading: "你可以在哪里使用 Codewhale",
  availabilityLede: "终端与本地浏览器客户端现已可用。桌面和托管网页应用正基于同一会话模型开发；各端的开放状态分别列在下方。",
  availability: [
    [
      "终端与本地浏览器",
      "已发布",
      "支持 Linux、macOS 和 Windows。运行 codewhale 使用终端，或运行 codewhale web 打开本地浏览器客户端。也可通过 npm 或 Cargo 安装；Android 上的 Termux 版本为预览版。"
    ],
    [
      "托管网页应用",
      "开发预览",
      "使用 Codewhale 账户登录后，在正在运行的终端会话中输入 /rc，即可在网页应用中继续。托管任务执行仍在验证中。"
    ],
    [
      "桌面端",
      "开发版本",
      "macOS 应用将文件夹、对话和模型连接整合在桌面窗口中，稍后将提供公开下载。"
    ],
    [
      "云端计算机",
      "开发中",
      "用于运行任务的托管计算机。"
    ]
  ],
  availabilityNote: "终端和本地浏览器不需要 Codewhale 账户。账户用于托管网页和桌面端访问，不能代替模型连接。使用自己的密钥调用云端模型时，费用由该提供商收取。",
  accountLink: "创建账户",
  surfacesHeading: "工具、应用连接与保存的工作",
  surfaces: [
    ["文件与命令", "在你设定的权限内读取项目、编辑文件、运行测试并查看命令输出。"],
    ["插件与 MCP", "连接更多工具和服务。智能体使用插件前，需由你审核并启用。"],
    ["Computer Use · 预览", "让智能体查看并操作其他应用的插件，需主动启用并授予它请求的系统权限。"],
    ["保存的会话", "将对话和工具结果保存在一起。本地浏览器连接到你电脑上的同一个 Codewhale 会话；继续保存的工作，无需从头开始。"],
    ["Fleet", "把任务分配给不同模型和角色的智能体，并跟踪它们的进度。"],
  ],
  runtimeLink: "了解集成",
  installBandHeading: "在 macOS 或 Linux 上安装 Codewhale",
  copy: "复制",
  copied: "已复制 ✓",
  binaries: "预编译包",
  chinaMirrors: "中国镜像",
  installGuideLink: "阅读安装指南",
  communityHeading: "一起让 Codewhale 变得更好",
  communityBody: "无论你是发现了错误、有功能方面的想法，还是准备提交第一个 pull request，我们都希望听到你的意见，与你一起推进接下来的工作。",
  communityLinksAria: "社区链接",
  contribute: "提交 pull request",
};
