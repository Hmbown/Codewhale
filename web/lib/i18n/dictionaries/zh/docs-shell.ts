import type { DocsShellDict } from "../types";

/**
 * Simplified-Chinese dictionary for the docs shell: portal hero, hub
 * metadata, task/topic search, sidebar and breadcrumb chrome, the
 * release-truth band, the contextual help band on every docs page, and the
 * shared page-body chrome.
 */
export const docsShell: DocsShellDict = {
  metaTitle: "文档 · Codewhale",
  metaDescription:
    "安装 Codewhale，连接模型提供商，然后把事情做完：模式与审批、查看改动、Workflow、子 Agent、MCP 工具、钩子、Runtime API 以及问题排查。",
  portalMark: "Codewhale 文档",
  heroTitle: "用 Codewhale 把事情做完。",
  heroLead: "从你想做的事开始。每一页都会说明你需要准备什么，给出今天就能运行的命令，并指向下一步。",
  installCta: "安装 Codewhale",

  releaseLabel: "版本",
  releasePublished: "最新发布 {tag} · {date}",
  releaseCandidate: "本站文档描述的是尚未发布的 {version} 源码候选版。",
  releaseMatches: "本站文档描述的是已发布的 {tag}。",
  releaseChangelog: "更新日志 →",

  searchLabel: "搜索文档",
  searchPlaceholder: "按任务或主题搜索…（按 / 快速聚焦）",
  searchClear: "清除",
  searchMatches: "{matched} / {total} 条匹配 “{query}”",
  searchNoMatches: "没有条目匹配 “{query}”",
  tasksHeading: "按任务",
  tasksLead: "从你想完成的事情开始。",
  topicsHeading: "按主题",
  webGuideTag: "网页",
  sourceDocTag: "源文档",
  emptyTitle: "没有匹配的条目",
  emptyBody: "换一个关键词试试——中英文都可以搜索——或浏览 GitHub 上的完整文档目录。",
  emptyCta: "GitHub 文档目录 ↗",
  indexNote: "“网页”条目在 codewhale.net 上打开；“源文档”条目会打开 GitHub 仓库中的完整参考资料。",

  sidebarHeading: "文档目录",
  sidebarAria: "文档目录",
  breadcrumbAria: "面包屑导航",
  breadcrumbHome: "首页",
  breadcrumbDocs: "文档",

  helpTitle: "这一页还不够？",
  helpLead: "每份指南都对照仓库中的一份文档核实过。如果它写错了或缺了什么，请在维护者能看到的地方告诉我们。",
  helpSource: "来源：{name}",
  helpTroubleshooting: "排查问题",
  helpFaq: "常见问题",
  helpDiscord: "到 Discord 提问 ↗",
  helpIssue: "报告文档问题 ↗",

  nextHeading: "下一步",
  noteLabel: "注意：",
  onThisPage: "本页内容",
};
