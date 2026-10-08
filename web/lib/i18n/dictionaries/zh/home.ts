import type { HomeDict } from "../types";

/** Simplified Chinese home copy: useful work with chosen models and connected tools. */

export const home: HomeDict = {
  metaTitle: "Codewhale：用自己的模型和工具，把想法做出来",
  metaDescription:
    "用 Codewhale 构建应用、自动处理工作，并连接你常用的工具。开源，支持你已有的模型 API、本地模型和自托管推理服务。",
  heroTitle: "把你想做的，做出来。",
  heroIntro:
    "做一个应用，让一项工作自动完成，或把一堆研究资料变成有用的成果。{brand} 连接你已有的模型 API、自己的推理服务，以及你选择的工具。",
  getCodewhale: "安装 Codewhale",
  heroInstallAria: "安装命令",
  exploreProduct: "探索 Codewhale",
  shotPreview: "终端预览",
  screenshotAlt:
    "Codewhale v{version} 的终端实录画面，包含对话、消息输入框与会话控制。",
  latestRelease: "最新发布 {tag}",
  releaseUnavailable: "发布状态暂不可用",
  currentSource: "源码",
  sourceCandidate: "未发布",
  publishedRelease: "已发布",
  gainHeading: "做出真正有用的东西。",
  gainLede: "从你想要的结果开始。Codewhale 使用文件、命令和已连接的工具完成工作，由你决定访问范围和审批方式。",
  gain: [
    [
      "构建应用和工具",
      "把一个想法做成能运行的应用、实用脚本，或现有项目中的新功能。让智能体和你一起编写、运行并测试。"
    ],
    [
      "让重复工作自动完成",
      "把反复要做的事变成工作流，从终端、脚本或 CI 中运行。需要并行推进时，让 Fleet 智能体团队分工完成。"
    ],
    [
      "连接你常用的工具",
      "通过插件、MCP 服务或 API 连接 Gmail、Slack 等工具。让这些服务与文件和命令一起参与同一项任务。"
    ]
  ],
  exampleTasks: [
    "做一个能在线预约的应用。",
    "把销售 CSV 做成每周可重复生成的报告。",
    "把已连接邮箱中的邮件整理成行动清单。",
  ],
  // A static example report built from local sample orders.
  reportTitle: "周销售报告",
  reportSampleLabel: "示例报告 · 样本数据",
  reportDescription: "让 Codewhale 按周汇总订单，并把处理步骤保存下来，供下一份 CSV 使用。",
  reportSourceLabel: "输入数据：",
  reportColumns: ["周起始日","订单数","销售额（美元）"],
  reportTotalLabel: "合计",
  reportTrend: "首周到末周的销售额变化：{change}。",
  reportDownloadLabel: "下载报告 CSV",
  chapterModels: "你的模型",
  modelsHeading: "继续用你选择的模型。",
  modelsBody:
    "连接你已经在用的模型 API，使用兼容网关，或在自己的硬件上运行推理。为每个会话选择模型，也能为 Fleet 中的不同智能体分配不同模型。",
  modelsFacts: [
    [
      "你的 API 账户",
      "使用自己的密钥连接 OpenAI、Anthropic、Google、DeepSeek 等提供商。"
    ],
    [
      "你的网关",
      "连接 OpenAI 兼容端点，选择它提供的模型。"
    ],
    [
      "你的推理服务",
      "通过 Ollama、vLLM 或 SGLang 运行本地或自托管模型。"
    ]
  ],
  modelsLink: "浏览模型与提供商",
  startHeading: "带上任务，开始动手。",
  startLede: "安装 Codewhale，连接模型，交给它一件值得做的事。先从一个智能体开始，需要时再添加工具或团队。",
  startGuideLink: "按照新手指引操作",
  startVocabularyLink: "查名词",
  chapterAvailability: "在哪里运行",
  availabilityHeading: "从终端开始。",
  availabilityLede: "终端和本地浏览器客户端现已可用。原生桌面应用与重建中的托管网页应用正在开发。",
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
  availabilityNote: "终端、本地浏览器和 GUI 无需 Codewhale 账户。托管网页和桌面端使用账户。使用你自己的提供商密钥时，相应用量由该提供商计费。",
  accountLink: "创建账户",
  surfacesHeading: "一项任务，连接文件、应用和智能体。",
  surfaces: [
    [
      "文件与终端",
      "创建文件、运行命令、分析数据，并测试你构建的成果。工作目录和权限由你设定。"
    ],
    [
      "插件与已连接的应用",
      "通过插件和 MCP 添加技能与工具。审核并启用你希望智能体使用的连接。"
    ],
    [
      "浏览器与电脑操作 · 预览",
      "使用浏览器工具和 Computer Use 插件，在你授予的访问范围内操作应用与网站。"
    ],
    [
      "随时接着做的会话",
      "将对话、工具结果和工作记录保存在一起。在终端或本地浏览器客户端中继续之前的任务。"
    ],
    [
      "智能体团队",
      "用 Fleet 将大任务分配给不同角色和模型的智能体，在一处查看它们的进度。"
    ]
  ],
  runtimeLink: "查看全部集成",
  installBandHeading: "在 macOS 或 Linux 上安装",
  copy: "复制",
  copied: "已复制 ✓",
  binaries: "预编译包",
  chinaMirrors: "中国镜像",
  installGuideLink: "阅读安装指南",
  communityHeading: "把 Codewhale 变成你想要的样子。",
  communityBody: "Codewhale 是开源的。读源码、做插件、分享工作流，或一起把下一个版本做得更好。",
  communityLinksAria: "社区链接",
  contribute: "在 GitHub 上参与贡献",
};
