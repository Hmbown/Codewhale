import type { DocsGuideDict } from "../types";

/** 「开始第一个任务」页的简体中文词典；四个步骤见 `web/lib/content/getting-started.ts`。 */
export const docsGuide: DocsGuideDict = {
  metaTitle: "开始第一个任务 · Codewhale 文档",
  metaDescription:
    "安装 Codewhale，连接模型，然后在你的项目里交给它一项任务。之后如果需要多个模型和角色，再配置 Fleet。",
  bodyClassName: "text-ink-soft leading-[1.9] tracking-wide",
  overviewTitle: "开始第一个任务",
  overviewLead:
    "从什么都没装到完成第一个任务，只需四步。每一步都链接到详细说明页；配置 Fleet 这一步是可选的。",
  sessionTitle: "看一次真实会话",
  sessionLead: "查看一项任务从首次请求到完成的全过程。",
  nextTitle: "下一步",
  sourceNote:
    "来源文档：docs/GUIDE.md、docs/INSTALL.md、docs/KEYBINDINGS.md · 修改时同步更新 docs-map.ts。",
};
