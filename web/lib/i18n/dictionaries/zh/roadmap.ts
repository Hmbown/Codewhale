import type { RoadmapDict } from "../types";

/** Chinese dictionary for `app/[locale]/roadmap/page.tsx`. */
export const roadmap: RoadmapDict = {
  metaTitle: "路线图 · Codewhale",
  metaDescription: "Codewhale 已完成、进行中、考虑中和明确不在范围内的工作。",
  eyebrow: "项目路线图",
  title: "路线图",
  introduction:
    "这里将已完成的仓库工作、正在推进的工作、仍在评估的方案和明确不在范围内的方向分开列出。路线图的“已完成”可包含已在源码候选版中实现的工作；安装页与首页另行标明最新已发布包。发布记录和 GitHub issues 会在可用时更新这些分类。",
  sectionTitle: "按状态查看工作",
  browseIssues: "浏览 open issues",
  trackCount: "{count} 项",
  trackCountOne: "{count} 项",
  contributeTitle: "路线图决策公开进行。",
  contributeBody:
    "Bug 和范围明确的功能请求请使用 issues；尚在形成中的想法可以先在 Discussions 讨论；已有具体实现时，欢迎发送带测试或文档的 pull request。来自不同语言、平台和提供商的验证结果都能帮助维护者判断优先级。",
  issuesDetail: "报告问题，或提出范围明确的工作。",
  discussionsDetail: "在开始实现前讨论尚未成熟的想法。",
  pullsDetail: "审查现有改动，或发送一个范围清楚的补丁。",
};
