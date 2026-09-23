# Fleet 与子代理

> 本文基于英文修订 `66816e6cc`（2026-08-16）的翻译。2026-09-13 仅核对并更新了角色名称、Workflow/Fleet 关系、生命周期和调用控制（状态、跟进、作用域与预算）；其余内容未全面重新同步。当前英文说明见 [SUBAGENTS.md](../SUBAGENTS.md)。

Fleet 管理同一批子代理的保存模型和角色分配。单项委派使用 `agent`；包含阶段、依赖和完成检查的多项工作使用原生 `workflow` 的 `plan`。先用 `agent(action="roster")` 查看已保存且可用的 Fleet 模型与角色：计划中的子任务可用 `model` 指定候选列表中的 `provider/model`，或用 `role` / `profile` 选择保存的分配。命名的 Exact Fleet 固定成员路由，拒绝任务级模型覆盖。它们都通过现有 worker 运行时执行；参见 [Workflow 编写指南](../WORKFLOW_AUTHORING.md)。

Fleet 的八个规范角色名是 `general`、`explore`、`planner`、`reviewer`、`implement`、`test`、`advisor` 和 `custom`。父代理通过 `agent` 启动任务，默认收到包含 `agent_id`、声明的交付文件和有效限制的紧凑回执；需要转录句柄或账本时，按 ID 请求详情。内部运行时类型仍为 `FleetRole`（以前叫 `SubAgentType`）。`worker`、`scout`、`builder`、`verifier`、`consultant` 等旧拼写仅在解析或反序列化边界兼容接受；新的提示、配置和回执使用规范名。

从架构上讲，子代理不应成为第二种执行基质。持久化原语是 [`AGENT_RUNTIME.md`](AGENT_RUNTIME.md) 中描述的 fleet 支撑的 worker 运行：重试、终态、回执、工件引用、检查与重启行为都归属于那里。面向模型的启动器是单一的 `agent` 工具，detached 工作应收敛到与 Agent Fleet 相同的生命周期。

当前 `agent` 实现在此切换完成期间委托给持久化子代理运行时。它对于会话内的短期委派仍然有用。瞬态的 provider header/stream/超时失败会在子运行时内部先以退避方式重试，然后才把 worker 标记为 interrupted；如果重试预算耗尽，Codewhale 会保留一个检查点并返回延续句柄，而不是让父代理去猜测发生了什么。对于必须跨进程重启、休眠或远程执行的工作，优先选择 Fleet 或 Workflow 支撑的 fleet 运行。

子代理继承父代理获准使用的工具，包括 `agent` 协调工具。生成后代遵守同一个绝对深度上限：根为 0，直接子代理为 1，到达 `max_spawn_depth` 的子代理不能再生成后代。操作者默认上限为 3，硬上限为 8；角色、保存配置或兼容输入只能收窄该上限。恢复和转录分叉保留源任务的深度及限制，不会多获得一代。已移除的 `agent_open` / `agent_eval` / `agent_close` 不在任何工具注册表中。

父代理正常结束一次响应后，健康的子代理继续运行；完成消息通过同一个 Engine 收件箱返回，并可唤醒父代理进入新的正常回合。显式中断和取消仍然有效。`detached: true` 额外使该子树不受父回合取消影响，但不解除子任务预算或无界面主机的截止时间。

本文档涵盖角色和单个 worker 的控制。多项委派由 `workflow` 通过同一个运行时协调；另见 `crates/tui/src/prompts/text.rs`（`AGENT_MODE`）中的子代理指南和工具描述。

## 角色分类

`agent` 上的 `type` 字段为子代理选择一种 Fleet 姿态（`agent_type` 作为兼容别名被接受）。每个角色都是对工作的一种独特立场——不只是标签不同。

## 维护者姿态

子代理帮助 Codewhale 更快地前进，但父代理仍然拥有维护者的决策权。使用子代理收集证据、审查补丁、运行验证，同时保持 [`AGENT_ETHOS.md`](../AGENT_ETHOS.md) 中的社区姿态：issue 是开放的接收入口，PR 门禁是审查负载控制，收割的工作需要明确的贡献者署名。

当子代理审查社区工作时，父代理在合并、收割、关闭或推迟之前，仍应检查 PR diff、关联 issue、测试和 CI。子代理的结果是一个工作集，而不是管家责任的替代品。

| 角色 | 姿态 | 可写？ | 联网？ | Shell 姿态 | 典型用途 |
|---|---|---|---|---|---|
| `general` | 灵活；父代理说什么就做什么 | 是 | 是 | 是 | 默认角色；多步任务 |
| `explore` | 只读；快速摸清相关代码 | 否 | 是 | 只读（网络 + 有界验证） | "找到 `Foo` 的每个调用点；用 gh 检查这个 PR" |
| `planner` | 分析并产出策略 | 否 | 是 | 只读探针 | "设计迁移方案；不要执行" |
| `reviewer` | 带严重度评分的阅读与评审 | 否 | 是 | 只读（网络 + 有界验证） | "审计这个 PR 的 bug" |
| `implement` | 以最小改动落地某个具体变更 | 是 | 是 | 是 | "把 `bar.rs::Foo::bar` 重写为做 X" |
| `test` | 运行测试/验证并报告结果 | 否 | 是 | 测试导向 | "运行 cargo test --workspace，并报告" |
| `advisor` | 短期、高推理密度的咨询 | 否 | 是 | 无 | "这个设计我们漏掉了什么？" |
| `custom` | 显式的窄工具 allowlist | 继承 | 继承 | 继承 | 在父代理姿态上精选的工具 |

角色的默认值就是该角色*想要*的姿态，而父代理的有效姿态永远是天花板（子代理绝不会扩得比父代理更宽）。只读角色按意图扣留**工作区写入**；默认不拿走任何其他东西——每个角色都保留网络读取，`custom` 继承父代理的写入/网络/shell 姿态，并且只被它的显式工具列表或发起调用收窄。被聚焦的 worker 的头部会依据运行时自身的权限快照声明有效姿态（`explore · read-only · network · read-only shell`）。

**委派移动的是工作，绝不是权限。** 只读父代理可以委派给 `implement`，但子代理的写入、网络、shell 和工具权限仍受父代理实时权限限制。检查角色可以使用分类的只读 shell；平台提供原生强制隔离时，也可以使用显式的只读分析模式。角色名称或 `read_only` 标志都不能授予调用方原本没有的 shell 权限。`fleet/exact.rs` 中的 `ChildAuthority::clamp` 对每个权限字段取较窄的值，并合并拒绝列表；`inherit_disallowed_tools: false` 不能删除操作者或祖先的拒绝规则。恢复保存的 worker 时，还会再次与当前调用方权限求交。测试 `a_read_only_parents_delegation_never_widens_authority` 验证此边界。

进程内部，解析后的权限只有一个对象——`crates/tui/src/worker_profile.rs` 中的 `ChildGrant`：`files`（none/read/write）、`shell`（none/inspect/verify/full）、`network`、`desktop`（子代理一律不授予）、命名工具 `surface`、调用方的显式 `scope`，以及剩余 `spawn` 深度。角色只是该对象上的预设（`ChildGrant::for_role`）；`ChildGrant::resolve` 将它与父代理派生的配置求交。子代理的工具目录、分发拒绝和能力包络读取同一份字段——可见即可调，被拒即不可见。

会话的**权限姿态**在每个子代理内部的应用方式与父代理回合完全一致：在 Auto-Review 下，同一个确定性底线和一次性模型守护者决定 worker 的被扣留调用（绝不是提示词；守护者不可用时拒绝，fail closed）；在 Ask 下，角色无法委派的被扣留调用会作为审批提示在父代理的 UI 中弹出，worker 可见地等待（`waiting for user`），或者在无法提示的主机上带着原因被拒绝；Full Access 仍然在不可绕过的安全底线上 fail closed。每一次没有人被提示的决策都是该 worker 转录中的一行备注（聚焦时可见）和一条审计日志记录。参见 `docs/MODES.md`。

每个角色的完整系统提示词位于 `crates/tui/src/tools/subagent/mod.rs`（搜索 `*_AGENT_INTRO`）。提示词前缀在子代理启动时自动加载；父代理的委派提示词成为第一个回合的用户消息。

## 上下文分叉

`agent` 默认全新启动：子代理拿到它的角色提示词加上你传入的任务。当子代理应从父代理当前的请求前缀继续时，使用 `fork_context: true`。（`fork_context` 不在对外公布的 v0.9.9 schema 中——它对兼容调用方保持解析接受，只读角色的自动分叉继续不变。）在分叉模式下，运行时会尽可能保持父代理 prefill/提示词前缀逐字节一致，追加一个结构化的状态快照，然后在尾部加上子代理角色指令和任务。这样既保留了 DeepSeek 前缀缓存的复用，又给了子代理做延续、审查、总结或压缩工作所需的上下文。

独立探索使用全新会话。当任务依赖父代理转录中已有的决策、文件、todo 或 plan 状态时，使用分叉会话。

分叉状态显示父代理的 To-do 快照——由 `todo_write` 写入的唯一的 Work 表面。子代理的 `<codewhale:fork_state>` 块携带由 `crates/tui/src/todo_snapshot.rs` 渲染的有界体，因此分叉是从父代理真实的进度位置继续，而不是一段转述。该 To-do 段在发起时解析，所以父代理回合内更早的 `todo_write` 会被包含进来。

**该列表只在发起那一刻显示一次，之后绝不会重新发送。** 没有哪个子代理请求会重新陈述 To-do 列表，父代理请求也不会。每个代理保留它自己的私有列表（#4810）；它对列表的了解来自自己 `todo_write` 调用返回的工具结果，这些结果是它自己转录中的普通消息。因此 worker 无法读取或写入父代理或兄弟代理的列表，分叉的子代理也无法修改它被交到手里的快照，或持续读取之后父代理的变化。

同一个私有列表就是子代理转录内卡片所显示的。一张委派卡片渲染**它自己**代理的 To-do 的有界投影——settled/total 计数、始终包含进行中的条目、最多三行，当界限省略其余部分时带一个显式的 `… +N more`——由 `card_todo_projection` 用与面向模型的主体相同的快照、优先级顺序和清洗器构建。卡片只消费 `agent_id` 与它匹配的信封，所以父代理的列表绝不会出现在子代理名下，也没有兄弟代理的列表会出现在另一个名下。没有声明任何工作的代理完全不显示 To-do 行，而不是显示占位任务；终态卡片保留它自己的代理实际发布的最后一个快照。扇出卡片保持圆点网格，不显示子代理 To-do：一张卡片后面有多个 worker 时，没有可以如实挂起单一列表的位置。只有当运行时已经把该子代理表示为它自己的委派卡片时，子代理 To-do 才会出现。

持久化的 task/Fleet 账本仍然拥有生命周期状态。`update_plan` 不再被模型触达：`model_visible()` 返回 `false`（`crates/tui/src/tools/plan.rs:408-413`），因此它从 API 工具列表中过滤掉，绝不会出现在子代理面前。它只用于重放更早的转录。以前放在那里的策略现在放进响应体，生命周期状态放进 `todo_write`。

## Worktree 隔离

对于并行的编辑通道，用 `worktree: true` 发起子代理。Codewhale 为那个子代理创建一个全新的 git worktree 和分支，从隔离的检出中运行子代理，并在返回的会话投影和 worker 记录中报告得到的 workspace/分支。默认分支是 `codex/agent-<name>-<id>`，检出位于父仓库旁边、`.codewhale-worktrees/` 之下，因此父检出保持干净。

隔离本身不授予写入权限。没有指定角色、配置或写入声明的纯提示词启动保持只读。显式选择 `general` / `implement` 等可写角色时，子代理继承父代理的写入上限，未收窄时默认作用域为工作区（`write_roots: ["."]`）。并行编辑应优先声明互不重叠的 `exact_files` 或 `write_roots`；`coordination_contracts` 用于具名共享契约。若写入作用域只由 `deliverables` 提供，则这些文件成为精确文件作用域。

`write_authority` 是可选的类型化收窄：`read_only` 不接受写入作用域，`workspace_write` 使用共享检出，`worktree_write` 要求实际的 worktree 隔离。`custom` 必须显式声明可写权限才能申领写入，否则保持只读。冲突的角色/作用域声明、活跃的重叠共享声明会在变更前被拒绝；真正隔离的 worktree 可以并行进行。

可选字段：

- `worktree_branch`：要创建的确切分支。
- `worktree_base`：要从中开分支的 git ref；默认为 `HEAD`。
- `worktree_path`：确切的检出路径。相对路径留在默认的兄弟目录 `.codewhale-worktrees/` 根下。

`cwd` 可与 `worktree` 组合：所请求的目录成为仓库根（及新检出）解析所用的发现锚点（`prepare_child_workspace`）。没有 `worktree` 时，`cwd` 仍是针对父工作区内已经存在的目录的手动逃生舱。

## 委派简报

父代理应该传递一份紧凑的简报，而不是一段松散的文字。使用结构化的 `dependencies` 和 `acceptance` 数组承载有界的前提事实与可观察检查；把聚焦的目标放在 `prompt` 里。不要复制原始父代理推理或无界的转录。

```
QUESTION:
SCOPE:
ALREADY_KNOWN:
EFFORT: quick | medium | thorough
STOP_CONDITION:
OUTPUT: VERDICT, EVIDENCE, GAPS, NEXT
```

`explore` 简报默认为快速的只读调查（不写，但网络触达和有界验证面可用于真正的侦察）。约 3-5 次工具调用足以完成快速探索：定位、搜索、读取决定性代码行，然后返回。除非证据与之矛盾，否则不要重复 `ALREADY_KNOWN` 的工作。reviewer 和 test 简报可以花更多调用，但应在拿到决定性证据后停止。implement 和修复型简报应在扩展示范围之前或反复失败之后设置检查点，而不是设置一个很小的调用上限。

好的委派提示词示例：

```text
QUESTION: PR #3124 是否在 provider 路由周围引入了发布风险行为？
SCOPE: PR #3124 的 diff、关联 issue、provider 路由测试、docs/PROVIDERS.md。
ALREADY_KNOWN: 分支是 hunter/0.8.62-glm-subagents；workspace 版本保持 0.8.61。
EFFORT: medium
STOP_CONDITION: 拿到一个 BLOCKER/MAJOR 问题或足够证明没有 MAJOR+ 问题的证据后立即返回。
OUTPUT: VERDICT、带 file:line 引用或 PR 引用的 EVIDENCE、GAPS、NEXT。
```

```text
QUESTION: 子代理提示词在哪里组装？
SCOPE: crates/tui/src/prompts*、crates/tui/src/tools/subagent/*。
ALREADY_KNOWN: 面向模型的启动器只有 `agent`；不要去找已移除的生命周期工具。
EFFORT: quick
STOP_CONDITION: 找到提示词源文件和包装委派文本的函数后停止。
OUTPUT: VERDICT、EVIDENCE、GAPS、NEXT。
```

```text
QUESTION: 聚焦的 prompt/subagent 测试过滤器是否有效，如果无效会失败什么？
SCOPE: cargo test -p codewhale-tui --bin codewhale-tui --locked prompt；需要时加 subagent 过滤器。
ALREADY_KNOWN: 不要修复失败；记录确切的命令、退出码和第一条相关断言。
EFFORT: medium
STOP_CONDITION: 一次干净的 PASS 或一条可复现的失败断言（带命令证据）后停止。
OUTPUT: VERDICT、EVIDENCE、GAPS、NEXT。
```

### 何时选择哪个角色

- **`general`** —— 当任务是"做完这一整件事"，而不是"去看"、"设计"或"验证"。这是正确的默认；只有当姿态重要时才改用更具体的角色。
- **`explore`** —— 当父代理在决定下一步之前需要证据。explore 用于快速调查；对独立区域并行开 2-3 个。他们应该先定位：确认项目根目录，在不熟悉的树中阅读相关 `AGENTS.md`/`README.md` 指南，只搜索可能的作用范围，返回 `path:line-range` 证据而不是一篇叙述式导游。要用的角色名是 `explore`。
- **`planner`** —— 当父代理有目标但没有可执行的分解。planner 写工件（`todo_write` 条目、响应体里的策略），但不执行它们。
- **`reviewer`** —— 当已经有一个变更，父代理想要它被评分。reviewer 不打补丁——他们在发现里描述修复方案，这样如果判定是"修它"，父代理可以派一个 implement。
- **`implement`** —— 当变更已经被明确指定、只需要落地。implement 保持严格的范围：最小改动，不做顺手重构，交回前跑一次快速验证。
- **`test`** —— 当父代理需要测试套件或其他验证上的权威通过/失败结论。test 角色不修失败；他们记录失败的断言 + 栈，把修复候选放在 RISKS 下。test 姿态永不写入，shell 被收窄到有界的内置验证面：Run tests/verifiers（当检查位于子目录时传 `cwd`）、Git fetch 拉取远端引用、Git merge_tree 求合并结果。写入上限为只读，无界 shell 形式会被拒绝（#5186）。被拒绝的探测上报给父代理，绝不绕行（#6298）。
- **`advisor`** —— 当操作者想在更便宜的执行继续之前得到一个高杠杆的第二意见。advisor 读足够的材料来支撑一条建议，但不能写，也不能运行 shell 命令。`oracle` 和 `consultant` 仅作为旧输入兼容接受；新的提示词、回执和 UI 使用 `advisor`。
- **`custom`** —— 只有当父代理需要显式约束工具集时。通过 legacy/internal 子代理记录上的 `allowed_tools` 字段传 allowlist；面向模型的 `agent` 工具刻意保持公共 schema 很小。

### 别名

新的调用使用规范名；旧别名只在解析或反序列化边界兼容接受。

| 规范名 | 兼容别名 |
|---|---|
| `general` | `worker`、`default`、`general-purpose`、`general_purpose` |
| `explore` | `scout`、`explorer`、`exploration` |
| `planner` | `plan`、`planning`、`awaiter` |
| `reviewer` | `review`、`code-review`、`code_review` |
| `implement` | `builder`、`implementer`、`implementation` |
| `test` | `verifier`、`verify`、`verification`、`validator`、`tester` |
| `advisor` | `consultant`、`oracle` |
| `custom` | 无；需要显式的 `allowed_tools` 数组 |

解析会去除首尾空白，且不区分 ASCII 大小写。未知值会返回列出可接受角色的错误。

## 并发上限

默认最多 **64** 个子代理并发运行（`DEFAULT_MAX_SUBAGENTS`），可通过 `~/.codewhale/config.toml` 中的 `[subagents].max_concurrent` 配置，硬上限为 **128**（`MAX_SUBAGENTS`）。会话默认接受一个有界的队列，最多 **1024** 个运行中加排队中的子代理（`MAX_SUBAGENT_ADMISSION`，`crates/tui/src/config/subagent_limits.rs:21`），因此一个回合可以请求大范围扇出，让管理器排空它，而不会产生无界群体。

默认情况下每个被接受的子代理都可以立即启动——没有人为的节流。如果想要更温和的扇出，降低 `[subagents].launch_concurrency`（一次启动多少个直接子代理）；超过该限制的子代理会为启动槽位**排队**，而不是爆发式启动。`launch_concurrency` 默认为解析后的 `max_subagents` 上限。（v0.8.61 之前的 `interactive_max_launch` 键仍作为弃用别名被接受；两个都设置时新键生效。）

高扇出 Workflow 可以用 `[subagents] max_admitted`（别名：`max_total`、`admission_limit`）调节那个有界群体。该总量上限同时计入**运行中**和**排队中**的代理，而 `launch_concurrency` 保持瞬时执行有界。已完成/失败/取消的记录会保留供检查，但不占用准入槽位。丢失了 `task_handle` 的代理（例如跨进程重启）也不计入上限。

Provider 配置档可以让一个配置对直接 API 路由保持激进，同时对订阅或聚合路由保持温和。`[subagents.providers.<provider>]` 下的每个键在省略时都从 `[subagents]` 继承。Provider 键接受规范名（如 `deepseek`、`zai`、`openrouter`）以及别名（如用于 Z.ai 的 `glm`）：

```toml
[subagents]
# 没有配置档的 provider 的全局回退。
max_concurrent = 20
launch_concurrency = 20
max_admitted = 200
max_depth = 6
# 可选的操作者步数上限；未配置时角色没有默认模型回合上限。
default_max_steps = 120
default_wall_time_secs = 1800

[subagents.providers.deepseek]
# 直连 API key，有余地扇出。
max_concurrent = 20
launch_concurrency = 20
max_admitted = 200

[subagents.providers.glm]
# Z.ai / GLM 订阅式路由：保持压力紧凑。
max_concurrent = 4
launch_concurrency = 3
max_admitted = 12
max_depth = 2
api_timeout_secs = 180
heartbeat_timeout_secs = 240

[subagents.providers.openrouter]
max_concurrent = 5
launch_concurrency = 3
max_admitted = 20

[subagents.providers.anthropic]
max_concurrent = 3
launch_concurrency = 2
max_admitted = 12
```

使用 `/config subagents status` 查看全局值和当前 provider 解析后的扇出、深度与超时配置。

## 对外公布的 agent 工具字段（v0.9.13）

面向模型的 `agent` schema 公布以下控制；每种 action 的必需字段在执行前验证。

| 用途 | 字段 |
|---|---|
| 启动与路由 | `action`、`prompt`、`type`、`profile`、`name`、`model`、`model_strength`、`thinking` |
| 作用域与交付 | `worktree`、`cwd`、`write_authority`、`write_roots`、`exact_files`、`coordination_contracts`、`deliverables`、`expected_artifact` |
| 收窄运行限制 | `max_steps`、`wall_time_secs` |
| 协调与恢复 | `agent_id`、`agent_ids`、`all_parked`、`message`、`until`、`detached`、`resume_from` |
| 检查 | `detail`、`offset`、`limit` |

`start` 需要 `prompt`；`message` 需要目标和消息；`followup` 需要消息且只能选择一种目标形式：`agent_id` / `name`、`agent_ids` 或 `all_parked: true`。`peek`、`interrupt` 和 `cancel` 需要目标；`claim` 需要作用域条目。

`agent(action="roster")` 使用与执行相同的解析器，列出内置角色实际使用的 provider、模型、思维层级、已知路由限制和能力来源。显式保存配置优先，其次是当前配置中的手动角色固定选择，再其次是唯一绑定该角色的保存成员；与固定选择冲突的任务路由会在接受任务前被拒绝。未固定的角色按任务 `model`、`model_strength`、继承默认值和会话路由的顺序解析。已选模型列表可用时，`models` 行按保存顺序列出确切路由，任务可选其中的 `provider/model`，会话模型也仍可用；列表外的选择被拒绝，同名模型跨多个 provider 时必须使用完整选择器。没有已选模型时，其他 provider 的任务级覆盖仍被拒绝。路由选择不改变子代理权限。

`profiles` 行显示现有已选 Fleet，或受信任配置、个人、工作区和插件层中的保存成员，并提供有界身份及相同的路由/费用证据。`profile="bug-hunter"` 使用该成员的指令、角色、provider/模型固定选择和深度上限。冲突的类型或模型请求会被拒绝；显式 `thinking` 可以覆盖保存的层级。缺失 provider、撤销的插件权限或禁用的项目配置会在接受子任务前失败。发现操作不会创建配置或自动添加模型。保存配置继续使用现有子任务生命周期，本身不代表持续 Bot 会话或 Computer 租约。

费用类别仅描述当前未缓存文本输入和输出的费率，不代表未来任务的总费用。缺少费率或依赖路由的价格保持未知；订阅和本地路由标记为非按金额计费。查询不会向 provider 发送请求，可达性标记为未验证。

**解析接受但未公布（兼容）。** 其他输入用于旧转录、客户端和内部/操作者兼容，仍须与实时权限求交：`max_depth`（以及 `maxDepth` / `max_spawn_depth`）、`workspace_policy`、`fork_context`、`worktree_path`、`worktree_branch`、`worktree_base`、`deliberate`、`dependencies`、`acceptance`、`allowed_tools`、`timeout_secs`、`reason` 和 `include_archived`。兼容深度值为 0..=8，只能收窄继承的绝对上限；兼容输入不能扩大权限或解除有限预算。

## 子代理预算（步数、墙钟时间、token）

`max_steps` 和 `wall_time_secs` 是可选的每次调用限制，只能收窄角色、操作者、父代理及保存运行的适用限制。省略时继承；工具解析器拒绝显式的零、null、负值和越界值。

`max_steps` 计算模型回合，接受 1..=2000；所有角色默认不限制模型回合数，除非操作者或祖先已经设置上限。内部用零表示未设上限，不会抵消继承的有限限制。`wall_time_secs` 接受 1..=86400，默认 1800 秒，可由操作者配置；计时包含排队、模型请求和工具执行，有效绝对截止时间会持久化。

启动回执及按 ID 查询得到的 `effective_limits` 才是有效限制。请求 300 秒不能延长父代理更早的截止时间。继续执行保留源任务的剩余步数、原截止时间和 token 历史；新 ID、角色变化或 `resume_from` 分叉都不能重置这些限制。

### Token 记账与部分结果

Token 预算已于 0.9.14 退役：token 用量仅被记录，不再强制执行——运行不会因 token 记账而停止。仍携带 `token_budget` 的旧输入可以正常解析但会被忽略；`max_steps` 和 `wall_time_secs` 仍是可收窄的每次调用限制。

额度依据 provider 报告的输入加输出 token；请求输出限制为剩余额度。未知的提示词用量和已在执行的请求仍可能导致超额，回执保留完整的实际报告值；未知用量不等于零。worker 自身用量与共享 `budget_spent_tokens` / `budget_remaining_tokens` 分开记录，不应按后代重复相加同一共享池。

worker 会在这些限制内为一次最终汇报预留额度：token 额度最多预留 10%（上限 8192 token，且至少能预留 1024 token 才启用）；步数上限至少为两步时预留一步；墙钟时间最多预留 10%（上限 10 秒）。正常任务执行会在用到预留额度前停止。共享作用域只预留一份 token 额度，汇报中的 worker 会原子地占用剩余空间，避免兄弟任务各自重复使用。继续执行不会退还已计量用量或重置原截止时间。

最终汇报使用 worker 已解析的同一 provider 和模型，禁用工具，输出最多 1024 token。它从有界的助手记录和工具结果中整理发现、证据、已产出文件、未完成工作及下一步；估算输入成本也占用此次额度。provider 传输重试仍受这一个逻辑回合和原墙钟截止时间约束，不另加 worker 总结重试循环。token 估算不是计费回执：未知的 provider 输入用量和已在执行的请求仍可能超额，实际用量会如实记录。

即使取得了有用的汇报，结果仍为 `BudgetExhausted`，并附具体原因、检查点、已计量用量和正常的交付文件判定。额度太小或已耗尽、此前受限请求的用量未知、provider 失败或时间耗尽时，worker 返回已有部分文本，并说明模型汇报为何不可用。已知缺失用量，以及因超时或取消而中断的请求，会跨续接任务和共享作用域保留；后来取得的用量仍只是已计量小计，不能恢复该受限作用域的汇报额度。取消优先于汇报；缺失用量保持未知。耗尽的作用域拒绝继续启动或恢复任务，部分汇报不代表成功完成。

## 各角色模型（#3018）

子代理可以运行在与父代理不同的模型上。两个配置面喂同一个覆盖映射（冲突时 `[subagents.models]` 键生效，键不区分大小写）：

```toml
[subagents]
default_model  = "deepseek-v4-flash"   # 每个角色的回退
worker_model   = "deepseek-v4-pro"     # worker
scout_model    = "deepseek-v4-flash"   # scout
planner_model  = "deepseek-v4-flash"   # planner
reviewer_model = "deepseek-v4-pro"     # reviewer
custom_model   = "deepseek-v4-pro"     # custom

[subagents.models]
# 自由形式的角色 → 模型映射；agent 接受的任何角色别名都可以。
builder = "deepseek-v4-pro"
```

v0.9.x 便利键 `explorer_model`、`awaiter_model` 和 `review_model` 仍作为弃用别名被接受，这样现有配置文件不会损坏。

模型 id 可以是**活跃 provider 接受的任何模型**——验证是 provider 感知的，发生在发起时而不是加载时。在官方 DeepSeek API 上只接受 DeepSeek id；其他每个 provider 都把 id 透传给 provider API，由它说了算。一个非 DeepSeek 示例：

```toml
provider = "moonshot"
model = "kimi-k2.7-code"

[subagents]
worker_model = "kimi-k2.6"
```

模型 id 应用到子代理路由时以同样方式验证；官方 DeepSeek API 上的非法 id 会让发起带着可接受 id 列表失败，而不是一个晦涩的 provider 400。

在 `/model auto` 下，子代理路由同样是 provider 感知的：有已知大/便宜配对的 provider（DeepSeek，以及 NVIDIA NIM、OpenRouter、Novita、SiliconFlow、SGLang、vLLM 上的托管 DeepSeek 路由）在配对之间路由；没有已知便宜档的 provider（如 Ollama、Moonshot）跳过网络路由器，把子代理留在会话模型上。

## 各 profile 的 provider 路由（#3965）

`[subagents.models]` 在活跃 provider 内部更换子代理模型。要把子代理钉到不同的 provider，使用 Fleet/AgentProfile，并通过 `profile` 把它传给面向模型的 `agent` 工具。profile 显式的 `provider` + `model` 字段胜过父会话路由；省略 `provider` 保留现有的继承行为。

示例：让父会话留在 DeepSeek，但把一个格式化子代理跑在本地 LM Studio 的 OpenAI 兼容端点上：

```toml
# ~/.codewhale/config.toml 或 workspace 配置
provider = "deepseek"

[providers.deepseek]
api_key = "YOUR_DEEPSEEK_KEY"

[providers.lm-studio]
kind = "openai-compatible"
base_url = "http://127.0.0.1:1234/v1"
api_key = "lm-studio"
model = "qwen-2.5-7b"
```

```toml
# .codewhale/agents/local-formatter.toml
id = "local-formatter"
role_hint = "formatter"
provider = "lm-studio"
model = "qwen-2.5-7b"
reasoning_effort = "off"

[instructions]
text = "使用小而本地的编辑。让格式化改动保持机械性。"
```

然后调用 `agent(profile: "local-formatter", prompt: "...")`。进程内子代理为 `lm-studio` 构建一个客户端；Fleet worker 把 `--provider lm-studio` 转发给 `codewhale exec`，它解析同一个 `[providers.lm-studio]` 表。未知或未配置的 provider id 会让发起失败，而不是悄悄回退到父 provider。

## 单步 API 超时（#1806、#1808）

每个子代理步骤把它的 DeepSeek `create_message` 调用包在一个单步超时里，这样单个卡住的请求不会无限期卡住父代理的完成唤醒通道。默认是 `600` 秒。超时的尝试以指数退避重试（最多 5 次重试），然后步骤带着保留的检查点中断。合法超过该时长的长思考子代理，例如 `agent` 后面沉重的 plan 或 review 工作，可以在 `~/.codewhale/config.toml` 中延长超时：

```toml
[subagents]
api_timeout_secs = 900  # 15 分钟；钳制到 1..=3600
```

值被钳制到 `1..=3600`。`0` 和 `unset` 保持 `600` 秒默认。

## 陈旧 agent 心跳（#2614）

运行中的代理还跟踪 manager 可见的进度。如果子代理在心跳窗口内停止发出进度，manager 会自动取消它、释放它的子代理槽位，并通过返回的转录句柄和持久化的 worker 记录保留可检查的取消记录。默认是 5 分钟（解析为至少比 `api_timeout_secs` 高 30 秒，因此在 600 秒默认 API 超时下是 630 秒）：

```toml
[subagents]
heartbeat_timeout_secs = 300  # 钳制到 30..=3600
```

有效心跳至少保持在 `api_timeout_secs` 之上 30 秒，因此一个配置的长模型请求不会在自己的请求超时触发之前被取消。

## 生命周期

每个打开的会话产生一条记录，按以下顺序推进：

```
Pending → Running → (Completed | Failed(reason) | Cancelled | Interrupted(reason) | BudgetExhausted)
```

显式中断、provider 重试耗尽或恢复失去运行句柄的记录，都可能产生带检查点的 `Interrupted`。检查 `needs_continuation` 与原因，再用 `followup` 继续可恢复的工作。`BudgetExhausted` 记录具体的 token、步数或墙钟原因，继续执行不能补充已耗尽的额度。

`wait` 只观察 worker；超时返回当前结果，不会停驻、取消或恢复它们。`until: "completion"` 等到一个子代理结束，`until: "all"` 等待调用开始时正在运行的那批子代理，`until: "activity"` 可在有进展时返回。父代理正常响应完成不会停驻健康子代理。

### 会话边界（#405）

每个 `SubAgentManager` 实例在构造时给自己分配一个全新的 `session_boot_id`。每个新会话用该 id 给代理盖章；workspace 状态文件记录它用于重启恢复。

工作条/状态投影默认聚焦当前会话的代理。不再运行的先前会话代理被视为归档记录，这样模型不会把陈旧的工作误认为活跃工作。这只是一条*先前会话*规则：在当前会话中完成的代理在会话剩余时间内保留它们的工作条行（安静完成），它们的详情仍然可以从那些行打开。

从 #405 之前的持久化状态文件加载的记录（没有 `session_boot_id` 字段）被归类为先前会话，因为 manager 无法把它们匹配到当前启动。

## 运行回执、后续消息与接管

每个兼容子代理在 `.codewhale/state/subagents.v1.json` 中有一条持久化的 worker 记录。在那些通道直接由 fleet 账本支撑之前，该记录是子代理通道当前运行账本切片：它存储 `run_id`、目标、角色/模型、workspace/分支、生命周期事件、工件引用、后续目标、接管目标、用量来源和验证来源。

正常流程是父代理继续工作并消费完成事件。默认启动和状态回执保持紧凑；完整快照和 worker 记录按 ID 作为诊断详情获取。

### 跟进与恢复

`message` 只排队消息，不唤醒子代理。`followup` 唤醒运行中的子代理，或从可继续的检查点恢复：

```json
{"action":"followup","agent_id":"child-previous-id","message":"根据已记录的证据继续检查。"}
```

后续等待和消息使用返回的 `agent_id`；`from` / `to` 标明原目标与当前续接任务。用旧 ID 重试会沿持久化的续接链定位，不会创建重复 worker；但对运行中 worker 重发同一消息仍会重复投递。已结束且不可继续的目标会明确报告未投递。

批量跟进选择 `agent_ids`（最多 32 个不同 ID）或 `all_parked: true`，不能混用目标形式。`all_parked` 只选择调用方可控制的停驻任务，超过 32 个时拒绝并要求显式分批。回执分别列出 `results` 和 `errors`；一个目标失败不会回滚其他成功目标。原目标及当前续接目标都必须通过控制权限检查。

只有需要从已结束子代理的转录创建另一项独立任务时，才用 `start` 的 `resume_from`。这会新建 worker，仍继承源任务的权限和预算限制；它不等同于用 `followup` 继续停驻工作。

### 紧凑状态与转录详情

不指定 ID 的 `agent(action="status")` 返回当前会话的名单页，最多 8 KiB；`limit` 默认及最大为 20。通过 `offset` / `limit` 分页，并跟随 `next_offset`，因为字节上限可能使实际行数更少。面向模型的名单格式使用一个固定的 `columns` 表头，`agents` 中每行是与表头对应的值数组，不能再按对象读取；空页也保留表头。`null` 表示缺失或未报告，已测得的零仍为数字 `0`。

每行保留 worker/父代理 ID、当前深度、状态、耗时、自身 token 总量、最近活动、待处理输入，以及 `resumed_from` / `resumed_as` 续接关系。验证信息保留判定、非空交付文件计数和必要的简短警告。名称、步数、路由、有效限制（含最大深度）和输入/输出 token 明细仍由按 `agent_id` 查询的对象返回，详情格式不变。总量只累计每个 worker 自身已报告的用量一次，并注明报告覆盖范围。完成回执还提供去重后的后代用量，区分未知和零。 worker 的 `has_unreported_usage` 和后代/子树的 `unreported_usage_workers` 计数表明是否仍有缺失回执，即使后来已取得部分已计量用量。

```json
{"action":"status","agent_id":"child-a","detail":true,"offset":0,"limit":20}
```

按 ID 的 `peek` 也支持 `detail: true`。详情最多 32 KiB，消息、事件和交付判定分页返回，并标注省略部分。用返回的类型化 `transcript_handle` 调用 `handle_read` 获取完整保留转录；无 ID 的 `detail: true` 不会展开所有 worker 的转录。

`result_summary` 仍是子代理自报，应检查具体的 `verification.status` 及证据。provider 尚未报告时，用量保持未知。文件判定为 `present` 或生命周期为已完成，都不能证明测试关卡通过。

## 输出契约

非 scout 子代理按此顺序以五个 Markdown 标题结尾：

```
### SUMMARY    一段；你做了什么、发生了什么
### EVIDENCE   path:line-range 引用和关键发现；每条一个要点
### CHANGES    修改过的文件，带一行描述；只读则为 "None."
### RISKS      可能出什么问题 / 父代理应该复核什么
### BLOCKERS   什么阻止了你；干净完成则为 "None."
```

它们是 `### HEADING` 行，不是 `HEADING:` 标签，而且 `EVIDENCE` 在 `CHANGES` 之前。这个五标题契约是 `crates/tui/src/prompts/text.rs` 中的 `SUBAGENT_OUTPUT_FORMAT`。`crates/tui/src/prompts.rs` 中的 `prompt_documents_structured_subagent_briefs` 断言每个标题都符合它。

Scout 是例外（#5189 F5）：它们只以 `### SUMMARY` 和 `### EVIDENCE` 结尾（`crates/tui/src/prompts/text.rs` 中的 `SUBAGENT_SCOUT_OUTPUT_FORMAT`）。`crates/tui/src/tools/subagent/mod.rs` 中的 `FleetRole::system_prompt` 为 `FleetRole::Scout` 注入 scout 契约，为所有其他角色注入五标题契约。一个子代理测试钉死 scout 包含 `## Output contract (scout)` 且不包含 `### BLOCKERS`。

父代理把 `EVIDENCE` 当作下一回合的工作集来读，所以 scout 和 reviewer 在这里要精确。

## 记忆与 `remember` 工具（#489）

当记忆启用时（`[memory] enabled = true` 或 `DEEPSEEK_MEMORY=on`），子代理共享父代理的原生记忆存储。它们可以通过 `remember` 工具追加持久化备注——方便 scout 发现值得跨会话携带的项目约定，或 verifier 学到"这个测试是 flaky"。

`remember` 接受 `global` 或 `workspace` 的 `scope`（`crates/tui/src/tools/remember.rs:79-108`），并通过 `NativeMemoryStore` 写入 `~/.codewhale/memory/global/MEMORY.md` 或 `~/.codewhale/memory/workspace/<id>/MEMORY.md`。写入不走标准的写审批流程。legacy 单文件 `memory.md` 路径在 v0.9.4 移除（remember.rs:165）；完整布局参见 `docs/MEMORY.md`。

## 实现说明

- 源码：`crates/tui/src/tools/subagent/mod.rs`。
- 持久化状态：`<workspace>/.codewhale/state/subagents.v1.json`。Schema 版本 `1`（向前兼容——新可选字段用 `#[serde(default)]`）。
- worker 记录按时间修剪：已完成/失败/取消/中断的记录在用于已结束代理的同一个保留窗口后逐出（默认 1 小时，`COMPLETED_AGENT_RETENTION`）。运行中/启动中/等待中的记录被保留。256 条记录的硬上限仍然作为安全边界存在（#4217）。
- `SubAgentRuntime::background_runtime()` 从 `child_runtime()` 开始，但把回合作用域的 child token 替换为全新的取消 token，因此父回合取消不会停止 detached 后台会话。
- `is_running` 检查忽略 `task_handle` 为 `None` 的代理；这避免把持久化但 detached 的记录计入并发上限（#509）。
- `SharedSubAgentManager` 是 `Arc<RwLock<...>>`——读路径使用读锁，因此 `/agents` 和侧边栏投影不会在多代理扇出期间阻塞主循环（#510）。
