# 编写工作流

> 英文原文：[WORKFLOW_AUTHORING.md](../WORKFLOW_AUTHORING.md)。
> 最后与英文同步日期（last synced with English revision）：2026-09-29。

> **普通的多智能体（agent）工作不需要看这份文档。** 在 Operate 里，正常发消息就行。
> 小活就直接做；多个委派步骤用一份紧凑的 Workflow 计划，带上依赖关系、
> 受限的范围和完成证据即可。Fleet 管理的是同一批子智能体（subagent）和角色。
> 一个受限且独立的任务可以直接用 agent，需要接着做时用 `followup`。
> Act/Agent 还可以选用软自动启动（soft-auto launch）。见
> [自动工作流（Automatic Workflows）](AUTOMATIC_WORKFLOWS.md)。

Workflow 只有一条运行时边界：编写好的源码先转换成类型化的 Rust `WorkflowSpec`，
再由 Rust 校验 IR，最后由调度器/无头 worker 运行时执行叶子节点。编写语言也拿不到
隐藏权限，无法支配文件、shell、网络、提供商（provider）、取消操作或 TUI 状态。

`workflow` 工具上保留的兼容启动方式：

| 输入 | 使用场景 |
|-------|-------------|
| `plan` | 结构化的 goal / phases / children（智能体的首选方式） |
| `script` | 由模型自己编写的短内联 JS |
| `source_path` | 工作区中已检入的 `.workflow.js` / `.workflow.ts` |

给子任务分配角色之前，先用 `agent(action="roster")` 查看已保存的 Fleet 模型和角色。
原生 plan 子任务可以用 `model` 指定已保存的候选模型，也可以用 `role`/`profile`
指定保存好的分配方案。具名的 Exact Fleet 会把成员的路由固定下来，并拒绝按步骤覆盖模型。
plan 子任务还接受 `cwd`，也就是相对于仓库的工作目录。多仓库工作区里 `cwd` 是
必需的，有了它，子智能体（以及 worktree 隔离）才能落到正确的仓库，行为与 `task({cwd})`
一致。

想看到从 Fleet 任务规格一路走到 Workflow 编写与监控的完整演练，见
[Fleet + Workflow 教程](FLEET_WORKFLOW_TUTORIAL.md)。


## 访问模型

Workflow 脚本**只是协调者**。它自己没有文件系统，也没有 shell。真正的活由脚本
启动的子智能体完成。

| 层 | 能访问什么 |
|-------|--------------------|
| Workflow 脚本（JS VM） | 脚本变量、分支/循环、`task()` / `parallel()` / `pipeline()`、`phase` / `log`、`budget` / `args`。**没有**直接的文件系统、shell、网络、env、import、时钟或随机数。 |
| Workflow 启动的子智能体 | 常规工具表面（读/搜索/编辑/写入、shell、web、MCP），受角色姿态（posture）、允许列表和父级策略约束。在 Workflow 下，可写角色的文件编辑会自动接受；shell / web / MCP 仍需要父级自动批准，否则按失败关闭（fail closed）处理。 |
| 父会话 | 工作目录、配置好的工具/MCP、权限模式、沙箱/网络规则。 |

### 规模

- 单次运行中最多 **16 个并发**活跃智能体（额外的启动请求会排队等空位）。
- 单次运行最多 **1_000 个智能体**（VM 生命周期内的启动上限）。
- 配置的 `max_children` 和 `max_concurrent` 可以进一步收紧这些上限。
- 自动启动由模型按范围判断；主机只强制 `max_children` / `max_depth` 这两个硬上限。
- 按工作需要规划规模，让主机去排队和收敛。
  这些上限是强制约束，不是提前压缩一份合法计划的理由。

主机侧按失败关闭（fail closed）的表面清单，见 Workflow 的 JS 沙箱测试。

## 语言选择

| 方案 | 优点 | 代价 | 定位 |
|---|---|---|---|
| YAML / JSON IR | 简单、可评审、不需要运行时 | 生成式工作流写起来啰嗦 | 保留为交换/调试格式 |
| JavaScript | 对象语法熟悉，智能体容易生成 | 当作通用运行时执行不安全 | 通过声明式、仅编译子集作为一等编写方式 |
| TypeScript | 工作流 SDK 的编辑器/类型体验最好 | 如果要支持完整 TS，需要剥离类型并做类型检查 | 目前用同一套仅编译子集；更丰富的 SDK 以后再上 |

默认的高能力路径是用 TypeScript/JavaScript 编写，但只当作一个编译步骤。
编译器接受 `.workflow.js` 或 `.workflow.ts` 里 `workflow({...})` 中与 JSON 兼容的
对象，把它降到 `WorkflowSpec`，然后跑 Rust 校验门禁。（Starlark 编写方式曾是
引导期的参考，现已移除；Workflow 编写只支持 JS。）

## 契约

可接受的源码形态：

```js
export default workflow({
  "id": "issue-audit-js",
  "goal": "Audit an issue fix with parallel agents",
  "nodes": [
    {
      "branch": {
        "id": "parallel-audit",
        "children": [
          { "agent": { "id": "code-audit", "prompt": "Review code", "agent_type": "review" } },
          { "agent": { "id": "test-audit", "prompt": "Review tests", "agent_type": "verifier" } }
        ]
      }
    },
    { "reduce": { "id": "summary", "inputs": ["code-audit", "test-audit"], "prompt": "Summarize" } }
  ]
});
```

支持的节点包装器类型：`agent`、`branch`、`sequence`、`reduce`、`teacher_review`、
`loop_until`、`cond` 和 `expand`。带 `kind` / `spec` 的原始
`WorkflowNode` JSON IR 同样有效。

`agent` 节点可以声明 `"profile": "reviewer"`，按某个具名的 Fleet 名册 profile
来运行。编译期会去掉这个名字的首尾空白并转成小写，而且它必须是单个 token
（不能有空白、引号或 `=`）；已保存的名册在派发时才解析，agent 节点上显式写出的字段
会覆盖 profile 的默认值。

运行时的 `task()` 也接受 `cwd`，指向一个已存在的、相对于仓库的工作目录。
工作流从多仓库工作区启动、子智能体又需要 shell 或文件访问时，就必须带上这个参数。
`cwd` 由主机校验，不授予写权限；子智能体需要独立检出时，应当同时设置
`worktree: true`。

编译器会拒绝 `import`、`require`、`fetch`、`process`、`Deno`、`Bun`、
`child_process`、文件读写、`eval`、`async`、`await` 这类带副作用的写法。
这是刻意比 JavaScript 更严格：工作流源码只是一种熟悉的声明格式，不是第二个执行
运行时。这些副作用只是不让写进脚本，整次运行并没有禁止它们——放进子 worker 里就行，
那里有完整的工具表面，脚本只管协调。

## 验证

- `cargo test -p codewhale-workflow --locked javascript`

当前示例：`workflows/issue_audit.workflow.js`。

## 由智能体编写的 Fleet 工作流

产品的主要流程不是"让用户去写脚本"。主智能体应当自己判断某个任务是否值得用工作流
编排，起草 Workflow 源码，按当前权限模式展示计划，然后交给运行时去编译和监控。

Workflow 负责计划本身：阶段、分支、循环、归约器和中间结果。Fleet 负责持久名册、
成员身份、语义角色，以及保存下来的提供商/模型绑定或继承关系。Runtime 负责
工具姿态（tool posture）、启动并发、租约、心跳、日志、回执，以及恢复/停止/重启
控制。换句话说，工作流负责挑选 Fleet 成员并监控它们在 Runtime 上的运行。
它本身不是执行者：脚本没有 shell，也没有文件系统，副作用都发生在 worker 里。

在 Workflow IR 被降到选定的 worker 之前，Workflow 到 Runtime 的启动校验会先套上
一个保守的默认形态：

- 单次 Workflow 运行最多 1,000 个 worker 智能体；
- 同时最多 16 个活跃 worker 智能体；数量更多时会在主机按运行设立的并发门禁上排队（阻塞），
  等空出位置后，再经 Fleet 选择、由 Runtime 执行；
- Workflow IR 的结构嵌套不超过 5 层；
- Runtime 的子级委派默认 3 层，另有可选择启用的 8 层硬上限；这份执行预算与
  Workflow IR 的形态无关；
- 循环必须提供 `max_iterations`；
- 动态 `expand` 节点必须提供 `max_children` 和一个模板。

这些限制区分了"总量"和"瞬时启动并发"。一份合法的 1,000 智能体 Workflow，
照样可以通过一个更小的 Runtime worker 池慢慢跑完。模型选择仍然按成员进行：
DeepSeek 预设可以为编排者推荐 `deepseek-v4-pro`，为就近的 worker 推荐
`deepseek-v4-flash`，但任务需要时，用户和智能体都可以覆盖任何一处。

## 实验性搜索是 Workflow 的一个选项

实验性搜索把现成的 best-of-N 做法推广开来，而不新增产品模式、调度器或子智能体 API。
提议中的搜索规格（search spec）会在准入前固定以下内容：
目标、基线、模型请求与解析后的版本、公开证据、评估器哈希、硬门禁、评分规则、预算、
写入范围、轮次，以及"仅评审"的集成策略；目前它是设计，不是已发布的代码。

当前的 JS 起步模板通过 `strategy: "search"` 支持结构化生成和只读评审。由 Runtime
掌管的命令门禁、隐藏评估、基准评分和干净基线重放，都是明确留待接入的主机接缝；
候选方案自己给出的评判，永远不能当成评估器的真相。见
[Workflow Experimental Search](../WORKFLOW_EXPERIMENTAL_SEARCH.md)。
