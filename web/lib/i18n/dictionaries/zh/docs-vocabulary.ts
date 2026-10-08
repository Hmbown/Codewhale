import type { DocsVocabularyDict } from "../types";

/** Chinese dictionary for `app/[locale]/docs/vocabulary/page.tsx`. */
export const docsVocabulary: DocsVocabularyDict = {
  metaTitle: "产品名词 · Codewhale 文档",
  metaDescription:
    "确切的产品名词：Fleet、Workflow、Lane、Runtime、Advisor，Plan / Work / Operate 与权限级别，以及请求→实际思考强度、路由来源与测量原则。",
  bodyClassName: "text-ink-soft leading-[1.9] tracking-wide",
  title: "产品名词",
  lead: "这些名词在全站、TUI 和回执里含义完全一致。名词本身不翻译；每条定义都与仓库中的公共事实矩阵逐字对应。",
  executionHeading: "执行名词",
  controlHeading: "模式与权限级别",
  controlLead:
    "模式（Tab 循环，输入框空闲时）决定可见的交互方式；权限级别（Shift+Tab 循环）决定工具执行前询问的频繁程度。两者正交。",
  routeHeading: "路由身份：提供商 · 模型 · 请求→实际思考强度 · 来源",
  advisoryHeading: "咨询角色",
  measurementHeading: "测量原则",
  leaderboardNote: "基准排行榜：本站没有，也不会在没有路由身份与测量工具链的情况下出现。",
  sourceNote:
    "来源文档：docs/FLEET.md、docs/MODES.md、docs/public-surface-facts.json · 名词文案来自 web/lib/content/vocabulary.ts；更新时请同步修改 docs-map.ts。",
};
