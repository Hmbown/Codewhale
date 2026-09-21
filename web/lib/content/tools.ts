import type { LocalizedText } from "./vocabulary";

export const TOOLS_COPY = {
  metaTitle: { en: "Tools · Codewhale Docs", zh: "工具 · Codewhale 文档" },
  metaDescription: { en: "Core work tools, on-demand discovery, and exact replay compatibility.", zh: "核心工作工具、按需发现与精确回放兼容。" },
  title: { en: "Tools", zh: "工具" },
  lead: { en: "Core work controls are available without discovery; specialized tools are loaded when needed. Mode and permission rules still apply.", zh: "核心工作工具无需搜索即可使用，专用工具按需加载。模式与权限规则始终生效。" },
  reference: { en: "Tool contract and design rationale", zh: "工具约定与设计说明" },
  rows: [
    { name: "read", detail: { en: "path · offset? · limit?", zh: "path · offset? · limit?" } },
    { name: "write", detail: { en: "path · content", zh: "path · content" } },
    { name: "edit", detail: { en: "path · edits", zh: "path · edits" } },
    { name: "bash", detail: { en: "command · timeout?", zh: "command · timeout?" } },
    { name: "agent · workflow", detail: { en: "Loaded on demand via tool_search. Delegate work and coordinate dependent phases.", zh: "通过 tool_search 按需加载。委派工作并协调有依赖关系的阶段。" } },
    { name: "todo_write", detail: { en: "Replace the task list with content and status entries.", zh: "用包含内容与状态的条目替换任务列表。" } },
    { name: "create_goal · get_goal · update_goal", detail: { en: "create_goal is available without discovery; get_goal/update_goal load on demand while a goal is active.", zh: "create_goal 无需搜索即可使用；get_goal/update_goal 仅在目标激活时按需加载。" } },
    { name: "tool_search", detail: { en: "Discover policy-allowed tools. Each child has its own search and activation cache.", zh: "发现策略允许的工具。每个子 Agent 拥有独立的搜索与激活缓存。" } },
    { name: "Git · Run · tasks · remember · Web · MCP · plugins", detail: { en: "Deferred: loaded by tool_search only when policy permits.", zh: "延迟加载：仅在策略允许时由 tool_search 加载。" } },
    { name: "8 names / 24 KiB", detail: { en: "Conversation toolbox cache; independent per child and revalidated every turn.", zh: "会话工具箱缓存；每个子 Agent 独立，每轮重新校验。" } },
    { name: "Web search/fetch", detail: { en: "Scout and Reviewer children can discover these without gaining mutation or arbitrary network authority.", zh: "侦察与审查子 Agent 可发现这些工具，但不会因此获得写入或任意网络权限。" } },
    { name: "MCP", detail: { en: "mcp_<server>_<tool> — registered from ~/.codewhale/mcp.json.", zh: "mcp_<server>_<tool>——从 ~/.codewhale/mcp.json 注册。" } },
  ],
  compatibilityTitle: { en: "Replay compatibility", zh: "回放兼容" },
  compatibility: { en: "Legacy names remain only for saved transcripts and protocol clients. Exact old calls retain their handlers but stay out of new catalogs and tool_search. Unknown names are never guessed or rewritten.", zh: "旧名称仅为已保存的记录与协议客户端保留。精确旧调用仍使用原处理器，但不会出现在新目录或 tool_search 中。未知名称不会被猜测或改写。" },
} satisfies {
  metaTitle: LocalizedText;
  metaDescription: LocalizedText;
  title: LocalizedText;
  lead: LocalizedText;
  reference: LocalizedText;
  rows: { name: string; detail: LocalizedText }[];
  compatibilityTitle: LocalizedText;
  compatibility: LocalizedText;
};
