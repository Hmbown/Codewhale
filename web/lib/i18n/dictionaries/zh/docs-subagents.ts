import type { DocsSubagentsDict } from "../types";

/** 「并行运行 Agent」页的简体中文词典；与 `en/docs-subagents.ts` 逐段对应。 */
export const docsSubagents: DocsSubagentsDict = {
  metaTitle: "并行运行 Agent · Codewhale 文档",
  metaDescription:
    "让 Codewhale 把任务中彼此独立的部分交给子 Agent，为它们选择角色，把它们的修改放在各自的工作树里，并随时查看进度。",
  bodyClassName: "text-ink-soft leading-[1.9] tracking-wide",
  title: "并行运行 Agent",
  lede:
    "Codewhale 可以把一块聚焦的工作交给子 Agent，自己继续往下做。当一个任务能拆成彼此独立的几部分——梳理代码库、审查改动、跑测试——时，用这种方式让它们同时进行。",
  sections: [
    {
      id: "ask",
      title: "直接提出来",
      blocks: [
        {
          p: "启动子 Agent 不需要命令，用平常的话提出来就行。在 Operate 模式下，Codewhale 会自己把独立的步骤分派出去；在 Work 模式下，说清楚你想怎么拆分：",
        },
        {
          code: `Use three explore agents in parallel: one maps the auth code,
one maps the billing code, one lists the tests that cover both.
Then summarize what would break if we changed the session format.`,
          lang: "提示词",
        },
        {
          p: "每个子 Agent 都在后台启动，完成后回报结果。你的输入框始终可用，主回合会接着处理这些结果。",
        },
      ],
    },
    {
      id: "roles",
      title: "选择角色",
      blocks: [
        {
          p: "角色决定了子 Agent 以什么姿态做事。你可以在请求里点名，也可以让 Codewhale 自己选。子 Agent 拿到的权限永远不会超过你当前会话的权限。",
        },
        {
          rows: [
            ["general", "按任务说明做事，可以修改文件、运行命令。默认角色。"],
            ["explore", "只读。快速梳理相关代码——比如“找出这个函数的所有调用方”。"],
            ["planner", "设计方案，不做任何改动。"],
            ["reviewer", "阅读并评估一处改动，为每个发现标注严重程度。"],
            ["implement", "用最小的修改落实一项具体改动。"],
            ["test", "运行测试和检查，报告通过或失败。不修改代码。"],
            ["advisor", "针对需要判断的问题，给出简短而审慎的第二意见。不运行命令。"],
            ["custom", "只能使用你列出的工具，适合权限需要严格收紧的任务。"],
          ],
          codeTerms: true,
        },
        {
          p: "想让某个角色固定使用某个模型，用 `/fleet setup` 保存——见[运行 Workflow](/docs/fleet)。",
        },
      ],
    },
    {
      id: "worktrees",
      title: "让并行修改互不干扰",
      blocks: [
        {
          p: "如果两个子 Agent 会修改同一个仓库，请让它们各自在自己的工作树里工作。Codewhale 会在你的仓库旁边的 `.codewhale-worktrees/` 下，为该 Agent 创建新的 git 工作树和分支；在你合并之前，你的工作副本保持干净。",
        },
        {
          p: "工作树只是隔离，不代表授权：需要写入的 Agent 仍然要有可写的角色，以及允许修改的路径。两个 Agent 如果声明要改同一批文件，会在任何一方动手之前就被拦下。",
        },
      ],
    },
    {
      id: "watch",
      title: "查看进度并回应它们",
      blocks: [
        { code: "/subagents", lang: "Codewhale" },
        {
          p: "`/subagents`（与 `/fleet workers` 是同一个视图）会列出挂在本次会话上的 Agent，以及每个 Agent 正在做什么。选中一个即可阅读它的对话记录。",
        },
        {
          p: "子 Agent 遵循你的[审批设置](/docs/modes)。在 Ask 下，需要审批的调用会像普通提示一样出现在你的会话里，Agent 则在一旁等待；在 Auto-Review 下，每个被拦下的调用都会经过同样的独立审查，不会打扰你。凡是不是由你亲自做出的决定，都会写进该 Agent 的对话记录。",
        },
      ],
    },
    {
      id: "limits",
      title: "了解上限",
      blocks: [
        {
          rows: [
            ["同时运行", "默认 64 个。可在 `~/.codewhale/config.toml` 中设置 `max_subagents`（最多 128）。"],
            ["排队加运行", "最多 1,024 个；超出的启动请求会排队等待。"],
            ["嵌套", "子 Agent 也可以再启动子 Agent，默认最多三层，绝不超过八层。"],
          ],
        },
        {
          p: "这些只是上限，不是目标。少量边界清晰的 Agent 加一份清楚的汇总，胜过一大堆彼此重叠的 Agent。如果工作需要在重启或笔记本休眠后继续，请改用 [Fleet 运行](/docs/fleet)。",
        },
      ],
    },
  ],
  next: [
    {
      href: "/docs/fleet",
      label: "运行 Workflow",
      note: "把反复使用的计划写成 Workflow，分阶段执行，每次运行都有记录。",
    },
    {
      href: "/docs/work",
      label: "跟踪进度",
      note: "To-do 列表如何显示已完成、进行中和剩余的工作。",
    },
    {
      href: "/docs/review",
      label: "查看改动",
      note: "在保留之前，先检查这些 Agent 改了什么。",
    },
  ],
  sourceNote: "来源文档：docs/SUBAGENTS.md、docs/FLEET.md、docs/MODES.md · 修改时同步更新 docs-map.ts。",
};
