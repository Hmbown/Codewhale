import type { DocsFleetDict } from "../types";

/** 「运行 Workflow」页的简体中文词典；与 `en/docs-fleet.ts` 逐段对应。 */
export const docsFleet: DocsFleetDict = {
  metaTitle: "运行 Workflow · Codewhale 文档",
  metaDescription:
    "在 Fleet 中保存角色和模型，编写可重复的 Workflow，把它作为可查看、可停止的 Lane 运行，并用持久的 worker 批量执行任务。",
  bodyClassName: "text-ink-soft leading-[1.9] tracking-wide",
  title: "运行 Workflow",
  lede:
    "大多数多步骤工作，直接提出来就行：在 Operate 模式下，Codewhale 会自己规划步骤，并把彼此独立的步骤并行执行。如果你希望每次都按同一套有序计划来做——分阶段、并行分支、最后汇总——并且每次运行都留有记录，就写一个 Workflow。",
  sections: [
    {
      id: "fleet",
      title: "在 Fleet 中保存角色",
      blocks: [
        {
          p: "Fleet 是 Codewhale 可以分派工作的角色列表，以及每个角色使用的模型。在会话中配置一次即可：",
        },
        { code: "/fleet setup\n/fleet\n/fleet saved", lang: "Codewhale" },
        {
          p: "`/fleet setup` 会带你选定一个角色、它使用的模型（或“沿用会话的模型”），以及保存位置：当前项目，或者对所有仓库都生效的个人配置。写入之前你会看到完整的文件内容。`/fleet` 显示当前所选 Fleet 的成员，`/fleet saved` 用于在已命名的 Fleet 之间切换。",
        },
        {
          p: "Fleet 只决定由谁来做。一个 worker 能读、能写、能运行什么，仍然取决于你的工作区信任、[审批设置](/docs/modes)和沙箱。",
        },
      ],
    },
    {
      id: "write",
      title: "编写 Workflow",
      blocks: [
        {
          p: "Workflow 是放在仓库 `workflows/` 文件夹中的一个 JavaScript 文件。它只描述步骤，本身不干活。下面这个例子先并行审查两个部分，再汇总结论。把它保存为 `workflows/docs_readiness.workflow.js`：",
        },
        {
          code: `export default workflow({
  "id": "docs-readiness",
  "goal": "Review the docs and code for gaps, then summarize the next edit",
  "nodes": [
    {
      "branch": {
        "id": "parallel-review",
        "parallel": true,
        "children": [
          { "agent": { "id": "code-review", "prompt": "Inspect src/ for undocumented behavior.",
                       "agent_type": "review", "mode": "read_only", "file_scope": ["src"] } },
          { "agent": { "id": "docs-review", "prompt": "Inspect docs/ for stale or missing steps.",
                       "agent_type": "review", "mode": "read_only", "file_scope": ["docs"] } }
        ]
      }
    },
    {
      "reduce": {
        "id": "summary",
        "inputs": ["code-review", "docs-review"],
        "prompt": "Combine the findings into the safest next edit."
      }
    }
  ]
});`,
          lang: "workflows/docs_readiness.workflow.js",
        },
        {
          p: "可用的步骤类型有 `agent`、`branch`、`sequence`、`reduce`、`loop_until`、`cond`、`expand` 和 `teacher_review`。Workflow 文件本身不能访问文件、shell 或网络，`import`、`fetch`、`eval` 和 `async` 都会被拒绝。真正干活的是它启动的 Agent，它们按你平常的权限运行。",
        },
        {
          note: "一次运行最多可启动 1,000 个 Agent，同时工作的最多 16 个，其余排队等待。循环必须声明 `max_iterations`。",
        },
      ],
    },
    {
      id: "run",
      title: "运行",
      blocks: [
        {
          code: `codewhale workflow run docs-readiness --runtime inline
codewhale workflow run docs-readiness --goal "prepare the 1.2 release" --verify`,
          lang: "终端",
        },
        {
          p: "Codewhale 会根据名字找到 `workflows/docs_readiness.workflow.js`，检查后启动。`--runtime inline` 在当前终端中运行；默认的 `tmux` 则在一个独立的 tmux 会话中运行，关闭终端后也会继续。`--verify` 会在成功完成后运行验证关卡，`--fleet <名称>` 则使用指定的 Fleet 而不是内置角色。",
        },
        {
          p: "如果不想动到当前的工作副本，加上 `--worktree-repo . --branch <名称>`：这次运行会拥有自己的 git 工作树和分支。",
        },
        {
          p: "在会话中，`/workflow` 用于启动 Workflow，`/workflows` 用于列出或取消本会话中的运行。",
        },
      ],
    },
    {
      id: "watch",
      title: "查看和停止运行",
      blocks: [
        { p: "每次运行都是一个 Lane。Lane 会保存到磁盘上，所以你可以在任何终端里查看：" },
        {
          code: `codewhale lane list
codewhale lane status <lane-id>
codewhale lane logs <lane-id>
codewhale lane attach <lane-id>
codewhale lane interrupt <lane-id>`,
          lang: "终端",
        },
        {
          p: "`lane list`、`lane status` 和 `lane interrupt` 都支持 `--json`，会输出一份机器可读的收据。在会话中，`/lane` 提供同样的操作，结果也完全一致。",
        },
      ],
    },
    {
      id: "batch",
      title: "批量运行任务",
      blocks: [
        {
          p: "如果你手上是一串彼此独立的任务，而不是一套计划，就把它们写成任务文件，作为一次 Fleet 运行来执行。每个任务写明目标、角色和允许写入的路径。完整的 `tasks.json` 示例见[教程](https://github.com/Hmbown/CodeWhale/blob/main/docs/FLEET_WORKFLOW_TUTORIAL.md)。",
        },
        {
          code: `codewhale fleet init
codewhale fleet run tasks.json --max-workers 4
codewhale fleet status
codewhale fleet logs <worker-id>
codewhale fleet resume <run-id>
codewhale fleet stop --all`,
          lang: "终端",
        },
        {
          p: "`fleet status` 会根据当前工作区的运行记录，统计排队中、运行中、已完成和失败的工作。笔记本休眠或管理进程退出后，`fleet resume` 能接着原来的运行继续，而不会新开一次。如果只想看挂在当前会话上的 Agent，用 `/fleet workers`（或 `/subagents`）。",
        },
      ],
    },
  ],
  next: [
    {
      href: "/docs/subagents",
      label: "并行运行 Agent",
      note: "不写 Workflow，也能把一个任务中彼此独立的部分交给子 Agent。",
    },
    {
      href: "/docs/review",
      label: "查看改动",
      note: "检查一次运行产生的 diff，并在推送前做一次审查。",
    },
    {
      href: "/docs/vocabulary",
      label: "产品名词",
      note: "用一句话分别说明 Fleet、Workflow、Lane 和 Runtime。",
    },
  ],
  sourceNote:
    "来源文档：docs/FLEET.md、docs/FLEET_WORKFLOW_TUTORIAL.md、docs/WORKFLOW_AUTHORING.md · 修改时同步更新 docs-map.ts。",
};
