# TUI deconstruction: crates as agent work partitions

The deliverable is a workspace where one agent, possibly sandboxed, can be
handed one crate plus the contracts it depends on, change it, prove it with that
crate's own tests, and return a patch that cannot collide with any other
agent's patch. The terminal UI becomes one client of a headless runtime along
the way, but the organizing question for every boundary is: **where can an
agent work without negatively affecting another agent?**

A lower `crates/tui` line count alone does not establish the split. A crate
boundary counts when it lets an agent build, test and land work without the
rest of the workspace.

## Audited baseline (2026-10-09, `bd2a7b49c`)

Physical Rust lines, tests included. These are ownership clues, not production
LOC. Regenerate before relying on them.

| Source | Lines |
| --- | ---: |
| Whole workspace (29 crates) | ~1.47M |
| `crates/tui` | 1,280,430 in 1,077 files (~87%) |
| of which test code in test files / test dirs | ~331k |
| `crates/tui/src/tui` (terminal UI proper) | 314,291 |
| `crates/tui/src/tools` | 204,115 |
| `crates/tui/src/core` (engine, turn loop) | 89,384 |
| `crates/tui/src/commands` | 71,629 |
| `crates/tui/src/lib.rs` + `lib/` | 24,827 |
| Every other crate | under 54k each; most under 10k |

Crates named for a subsystem whose implementation still lives in the TUI:
`tools` (1.6k; the implementations are `tui/src/tools`, 204k), `mcp` (109
lines; `tui/src/mcp*` is 26k), `core` (5.8k; the engine is `tui/src/core`),
`agent` (1.6k model registry with one TUI consumer, beside the TUI's own
catalog). `cli` depends on `tui` for `run`, `route_preferences`, `config_keys`
and `cloud_dispatch`; `tui` depends on `app-server` while the Runtime API
(`runtime_api`, 61k) lives inside `tui`.

`scripts/split/module_graph.py --report` puts the runtime closure (everything
reachable from the runtime roots without passing through UI modules) at **116
modules, about 813k lines**. A grep-based module graph (production paths,
approximate test stripping) shows the non-UI part of it as **one strongly
connected component of 85 modules**. Moving that component into one crate
would relocate the monolith, not partition it.

### Where agents collide

Commits touching `crates/` from 2026-08-10 to 2026-10-09: 3,380.

| Area | Commits | Why it collides |
| --- | ---: | --- |
| `tui/src/tui/ui` (event loop, dispatch, handlers) | 621 | one controller for every surface |
| `tui/src/core/engine` | 462 | features wire themselves into the turn loop |
| `tui/src/commands/groups` | 321 | every slash command in one tree |
| `tui/src/runtime_api` | 289 | routes, DTOs and logic in one module |
| `tui/src/tools/subagent` | 284 | |
| `crates/config` + `tui/src/config` | 283 + 199 | each feature adds fields to one schema |
| `tui/src/tui/app` | 275 | |
| `tui/src/lib.rs` | 270 | `mod` list plus composition root |

Central registration lists cause much of this: `tools/registry.rs` has one
`with_*_tools()` builder per tool family, `config.rs` (13k) holds every
feature's fields, `lib.rs` lists every module, `commands/groups/mod.rs` lists
every group. Adding a feature edits all of them.

### Reference workspaces

Local snapshots, inspected 2026-10-09; not claims about current upstream.

| | Crates | Lines | Largest |
| --- | ---: | ---: | --- |
| Codex `codex-rs` (`bdda5da56c`, 2026-07-30) | 126 | 1.23M | core 294k, tui 235k, app-server 126k |
| Grok Build (`500129c`, 2026-07-29) | 76 | 1.46M | pager 462k, shell 362k, tools 127k |

Patterns adopted here:

- **Contract crate apart from implementation crate** (Grok):
  `xai-tool-types` -> `xai-tool-protocol` -> `xai-grok-tools-api` ahead of the
  heavy `xai-grok-tools`; likewise `config-types`, `workspace-types`,
  `sampling-types`. `tools-api` exists so "host services ... must not depend on
  the tools implementation crate."
- **Feature crates that plug in through contributor traits** (Codex `ext/`):
  `extension-api` defines `ToolContributor`, `ConfigContributor`,
  `ContextContributor`, `ThreadLifecycleContributor` and others; goal,
  memories, web-search, skills, MCP, guardian and image generation are each a
  crate.
- **Tiny utility crates** (Codex `utils/*`: `output-truncation`, `pty`,
  `path`, `string`, `sleep-inhibitor`).

Patterns rejected, because each defeats agent isolation:

- Codex `core` depends on 65 workspace crates, including three `ext` crates
  that themselves depend on `core`. The hub remains; every feature agent still
  rebuilds it.
- Grok's `xai-grok-tools` implementation crate has 13 dependent crates, so a
  tool edit rebuilds pager, shell and agent.
- Both keep 300k+ line crates. Crate count is not the goal; the rules below are.

## Rules

1. **Feature crates are leaves.** Nothing depends on a feature or adapter
   implementation crate except the composition root (`codewhale-cli`, or a
   thin host crate it owns). An edit there rebuilds that crate and the final
   binary, never a sibling.
2. **Contract crates are small, logic-free and rarely changed.** They carry
   the high fan-in, so a contract edit is the one change that affects other
   agents. Contract changes are their own slices, serialized, and each lists
   the dependents it rebuilt.
3. **No central lists.** Features register through contributor traits, and
   the composition root names each crate once; adding a feature is one
   appended line there. Adopting the contributor traits deletes the
   `with_*_tools` builders, the per-feature `config.rs` fields and the command
   group lists in the same migration. Do not run both systems.
4. **Each crate owns its tests.** Unit tests move with their code; a crate's
   tests build with only that crate's dependency closure. Cross-crate behavior
   is tested at the host level.
5. **A crate directory is the claim unit.** A Linear claim names crate paths.
   Two agents never own the same crate at once.
6. **Dependencies point down only** (tiers below). Cargo enforces it; an
   upward import is a compile error, not a ratchet count.

## Sandboxed agent contract

The rules exist so an agent can work in a sandbox that does not hold the
workspace:

- **Payload = one crate + its dependency closure.** The sandbox receives the
  owned crate, the contract and utility crates it depends on, `Cargo.lock`, and
  a generated minimal workspace manifest. Closure size per crate is the
  primary architecture metric; it must stay small for every leaf.
- **Only the owned crate is writable.** Contract crates are mounted read-only.
  A needed contract change comes back as a request, not an edit.
- **Tests are self-contained.** The owned crate's tests pass with the closure
  alone: a mock provider from `codewhale-test-support`, no `~/.codewhale`, no
  network, no sibling crates.
- **The patch is path-confined.** The integrator rejects any hunk outside the
  owned crate, applies it to canonical `main`, and runs the host-level gate.
  Disjoint paths cannot conflict, so no branch or worktree is needed.
- **Builds are local to the sandbox.** Each sandbox has its own target
  directory, so parallel agents never wait on one Cargo lock or fill a shared
  build disk. Sandbox images carry prebuilt third-party and contract artifacts keyed by
  the `Cargo.lock` hash, so the agent compiles only its crate.
- **Integrator-owned files.** `Cargo.lock`, the workspace manifest,
  `[workspace.dependencies]`, the composition root's wiring list and new-crate
  creation. A sandbox cannot add a dependency or a crate by itself.

Codewhale ships cloud sandboxes and the `agent` tool. Once crates meet this
contract, a lead session can partition work by crate and dispatch children into
per-crate sandboxes. That makes this workspace a proving ground for the
product's own delegation.

## Target map

Names are package names (`codewhale-` prefix omitted). Sizes are today's
source for those modules, tests included, measured at `bd2a7b49c`. Existing
crates are reused where their current content already fits the role.

Every crate below `services` is a leaf in the sense of rule 1 unless it is a
contract. New crates live under grouped directories:
`crates/{contracts,util,services,providers,runtime,features,tui,hosts,test}/`.
Existing crates move into those directories in one rename-only change.

### Contracts (`crates/contracts/`)

| Crate | Role | Source |
| --- | --- | --- |
| `protocol` | shared wire and domain records | exists |
| `core` | request construction, journal, ids, fragments, prefix cache | exists (5.8k) |
| `models` | provider wire DTOs and the model-client trait | exists; trait added |
| `tools` | tool contract: `ToolResult`, `ToolError`, `ToolCapability`, specs, plus capability ports (spawn agent, schedule work, work graph) that tools call and the runtime implements | exists (1.6k); ports added |
| `ext` | contributor traits: tool, config section, context/prompt fragment, command, thread lifecycle, approval review | new; replaces the central lists |
| `config-types` | config schema and route identity, no I/O | split from `config` (53k) |
| `command-contract` | portable command metadata and results | exists |
| `runtime-wire` | Runtime API DTOs and the in-process `Op`/`Event` surface clients use | from `runtime_api` and `core/events.rs` |

### Utilities (`crates/util/`)

`paths`, `sanitize`, `release`, `build-support`, `localization` (exist);
`util-truncate` (from `tools::truncate`); `util-fslock` (from
`runtime_threads::try_lock_file_exclusive`); `util-hash` (`fast_hash`,
`hashing`, `regex_cache`, `safe_label`, now in `crates/runtime`);
`util-schema` (`tools::schema_sanitize`, 3k); `util-pty`.

### Services (`crates/services/`)

| Crate | Source | Lines |
| --- | --- | ---: |
| `config` | loading, persistence, keys (`config`, `settings`, `config_persistence`, `config_keys`) | 41k + crate |
| `execpolicy` | exists; absorbs `core::authority::auto_review` and `tool_matches_any_rule` | 13k + moved |
| `secrets` | exists | 3k |
| `credentials` | `oauth`, `credentials` | 11k |
| `state` | exists; absorbs checkpoint records from `runtime_handoff` | 4k + moved |
| `session-store` | `session_manager`, `runtime_threads` store, `session_*` | 59k |
| `sandbox` | `sandbox` | 7k |
| `billing` | `pricing`, `route_billing`, `cost_status` | 15k |
| `telemetry` | exists | 5k |

### Providers (`crates/providers/`)

| Crate | Source | Lines |
| --- | --- | ---: |
| `model-client` | shared dispatch, streaming, retry (`llm_client`, `client` core) | ~25k |
| `provider-openai-compat` | `client/chat*` | ~10k + tests |
| `provider-anthropic` | `client/anthropic.rs` | ~2.5k + tests |
| `provider-responses` | `client/responses*` | ~3k + tests |
| `provider-local` | `local_ollama` | small |
| `model-catalog` | live catalog, routing, inventory, receipts, tuning; absorbs `agent` | 24k |

### Runtime (`crates/runtime/`)

| Crate | Source | Lines |
| --- | --- | ---: |
| `engine` | `Engine::run_turn` and the tool executor only | 89k today; shrinks as contributors leave |
| `context` | `compaction`, `prompts`, `project_context*`, `runtime_handoff`, `request_manifest`, `prompt_zones`, `context_budget` | 21k + moved |
| `approvals` | `core::authority` minus policy data | ~5k |
| `agents` | `tools/subagent`, `fleet`, `agent_roster`, `worker_profile` | 107k; split again by its own seams |
| `tasks` | `task_manager`, `automation_manager`, `work_graph` | 22k |
| `runtime` | the session host composing the above; implements the `tools` ports | exists (8k) |

`BASE_PROMPT` and `Engine::run_turn` keep their single-owner contracts: one
base prompt in `context`, one turn loop in `engine`. `agents` adapts children
to that loop; it does not own another one.

### Features (`crates/features/`, all leaves)

| Crate | Source | Lines |
| --- | --- | ---: |
| `tools-fs` | `tools/file`, `apply_patch` | 12k |
| `tools-shell` | `tools/shell`, `shell_dispatcher` | 16k |
| `tools-web` | `tools/web`, `web_search`, `web_run` | 17k |
| `tools-github` | `tools/github` | 4k |
| `tools-review` | `tools/review` | 3k |
| `goal` | `tools/goal`, `operate`, `goal_loop` | 5k + moved |
| `workflow`, `workflow-js` | exist; absorb `tools/workflow` (16k) | |
| `memory` | exists; absorbs `native_memory`, `tools/remember` | |
| `mcp` | exists; absorbs the TUI MCP pool | 26k |
| `skills` | `skills` | 14k |
| `plugins` | `plugins` | 28k |
| `extension-host` | `extension_host` | 31k |
| `hooks` | exists; absorbs `tui/src/hooks` | 9k |
| `lsp` | `lsp` | 4k |
| `rlm` | `rlm`, `repl` | 5k |
| `snapshot` | `snapshot` | 6k |
| `voice`, `vision` | `voice`; `vision`, `image_attach` | 1k, 3k |
| `cloud-dispatch` | `cloud_dispatch`, `dispatch_runner` | 5k |
| `remote-control` | `remote_control`, `runtime_chat_relay`, `remote_setup` | 13k |
| `integrations` | `integrations` | 5k |

Each feature contributes its tools, config section, slash-command logic and
context fragments through `ext`. Its terminal presentation, if any, is a view
crate in the TUI tier.

### TUI (`crates/tui/`)

| Crate | Source | Lines |
| --- | --- | ---: |
| `tui-kit` | widgets, markdown render, textarea, mouse, `palette`, the view trait | 30k + palette |
| `tui` | event loop and app-state reducer (`tui/ui`, `tui/app`) | 97k; split reducer from render first |
| `tui-view-transcript` | `tui/history` | 12k |
| `tui-view-work` | `tui/work_surface` | 13k |
| `tui-view-pickers` | `provider_picker`, `model_picker` | 17k |
| `tui-view-setup` | `tui/setup` | 9k |
| `tui-view-approval` | `tui/approval` | 6k |
| `tui-view-hotbar` | `tui/hotbar` | 4k |
| `tui-view-ambient` | `pet_watch`, `underwater`, `ambient_life` | 13k |
| remaining views | `tui/views` (35k), split by surface | |
| `tui-commands` | slash-command presentation (`commands`, 72k) | |

The TUI depends on `runtime-wire`, never on `runtime`, `engine` or a feature
crate. A TUI agent's sandbox therefore holds no runtime code. `cli` connects
the TUI to the runtime in process through the existing `EngineHandle`/`Op`/
`Event` seam; HTTP stays a transport for external clients.

### Hosts and test support

| Crate | Source |
| --- | --- |
| `app-server` | exists; absorbs `runtime_api`, `acp_server`, `mcp_server` (66k) |
| `cli` | the only composition root; absorbs `lib.rs::run`, `RuntimeOptions`, `route_preferences` and the wiring list (45k today in `lib.rs` + `lib/`) |
| `test-support` | mock provider, fixtures, `test_support` |
| `pty-harness`, `tui-goldens` | PTY and golden-frame tests |

About 70 crates. That is a consequence of the rules, not a target; split
further wherever a crate's churn still makes agents collide (`agents`,
`engine` and `tui` are the known candidates).

## Breaking the current cycle

The 85-module cycle is held together mostly by **types and pure functions in
the wrong module**, not by behavioral dependencies. Heaviest back-edges
(grep-based counts; a few are test-only references):

| Back-edge (refs) | Referenced | Destination |
| --- | --- | --- |
| `tools -> fleet` (62) | `AgentProfile`, `ChildAuthority`, `FleetRoster` | profile/authority data to a contract; behavior stays in `agents` |
| `tools -> core` (55) | `tool_catalog::enforce_tool_denial`, `tool_matches_any_rule` | `execpolicy` |
| `config -> core` (31) | `AutoReviewPolicy`, `AutoReviewAction`, `StepBudgetSource` | `execpolicy`; budget type to `core` |
| `client -> tools` (27) | `truncate` spillover, `schema_sanitize` | `util-truncate`, `util-schema` |
| `client -> compaction` (14), `client -> runtime_handoff` (10) | checkpoint and summary builders | `core` / `state` |
| `mcp -> core` (12) | `HumanDecision`, rule matching | `protocol` / `execpolicy` |
| `session_manager -> runtime_threads` (11) | `try_lock_file_exclusive`, thread store | `util-fslock`, `session-store` |

Moving these down is the same work as creating the contract and utility tiers.

## Sequence

Every move is rename-only (`git mv` plus path updates) with one wiring line,
so in-flight work rebases across it. Behavioral changes land separately.

0. **Measure.** Add a closure report to `scripts/split/`: for each crate, the
   lines in its dependency closure and in its reverse closure, plus a
   generator for a minimal sandbox workspace. Record the baseline. Extend
   `module_graph.py` with the tier table so in-TUI back-edges ratchet down
   before their crates exist.
1. **Contracts and utilities.** Create `ext`, `config-types`, `runtime-wire`,
   the `util-*` crates, and the `execpolicy`/`state`/`core` absorptions. This
   cuts the back-edges above.
2. **Contributor registration.** Migrate every `with_*_tools` builder,
   per-feature config field and command group list to `ext` contributors
   wired in `cli`, in one slice per list, deleting the old list in that slice.
3. **Pilot leaf: `tools-web`.** Extract it, then run one sandboxed agent
   against it end to end. Record payload size, cold and warm build time, test
   result and whether the patch applied cleanly. Adjust this document from the
   result before extracting more.
4. **Remaining features**, least coupled and most churned first.
5. **Providers**, one adapter per crate.
6. **Runtime tier and hosts.** `engine`, `context`, `approvals`, `agents`,
   `tasks`, `session-store`; move `runtime_api` into `app-server`; move the
   composition root into `cli` and delete the `lib.rs` path alias.
7. **TUI.** Separate the app-state reducer from rendering, then extract views.
   Last, because it is the most entangled and benefits from every earlier
   contract.

The runtime -> UI ratchet (`scripts/split/module_graph.py`,
`scripts/runtime-boundary-baseline.json`, enforced by
`scripts/check-command-crate-boundaries.py`) stays the gate until the runtime
tier is out of `crates/tui`; then Cargo enforces the boundary and the ratchet
is deleted. At `bd2a7b49c` it stands at 11 production references
(`runtime_api -> commands` 9, `tools -> commands` 2) and 35 test references.
`crates/runtime` must never depend on `tui`, `cli`, ratatui, crossterm,
`ansi-to-tui` or a terminal component kit; the runtime reaches the terminal
only through `host_terminal`.

## Known limitations

- **Parallel agents on one machine still share one Cargo lock.** The
  isolation gain locally is smaller rebuilds. Concurrency comes from
  sandboxes, each with its own target directory. Do not create per-agent target
  directories on a shared host; each one is a full build cache.
- **The final binary still links everything.** Relinking `codewhale` after any
  change is a fixed cost no crate split removes.
- **Contract edits still fan out.** The design makes them rare and visible,
  not free.
- **Every workspace crate publishes to crates.io**
  (`scripts/release/publish-crates.sh`, order derived by
  `validate-crate-publish-order.py`). Each new crate is a new public package
  whose name must be claimed at release. Publication remains a human gate.
- **`engine`, `agents` and `tui` stay large after this map.** Their internal
  splits are decided from churn and closure data after step 6, not in advance.
- **No speedup has been measured.** Claims wait for the step 0 report and the
  step 3 pilot.

## Verification

- Preserve the model-facing runtime receipt, prompt and cache-prefix
  semantics, tool names and order, serialized records, and public commands
  across mechanical moves. A deliberate behavior change names and tests the
  intended difference.
- Move private unit tests with their implementation. A `#[path]` split stays
  in the same compilation unit and proves nothing about isolation. Do not make
  internals public just to relocate tests.
- Exercise mock-provider parent and child turns, approval and denial,
  streaming, cancel and steer, explicit goals, pause and resume, reconnect,
  restart recovery and terminal fan-in at each affected boundary.
- For every extracted crate: its tests pass from a generated minimal workspace
  holding only its closure, and its reverse closure contains only the
  composition root (features) or is listed (contracts).
- Measure warm edit/build/test cycles and peak memory for a provider edit,
  tool edit, renderer edit and locale edit before and after, with compiler,
  profile, features and cache state recorded.
- Local tests, full gates, hosted CI, installed artifacts and PTY behavior
  remain separate evidence.

## Model judgment and test-time compute

Founder clarification, 2026-09-09: the harness should let the model decide
when a goal, plan, delegation, further investigation, or verification is useful.
Provide enough reasoning and tool-feedback opportunities for that judgment.
Do not interpret unwanted automatic goals as a request to forbid inferred goals.

The current goal path has conflicting authorities: `operate_goal_from_prompt`
classifies an instruction with verb/question heuristics before inference, while
`CreateGoalTool::description` tells the model to require explicit goal requests.
`runtime_handoff` additionally tells the model the host already created a goal.
Those three policies must become one model-facing contract with runtime-owned
state transitions. Explicit `/goal` commands remain a direct user control.

Reference inspection was local, not a claim about every upstream version:

| Snapshot | Useful evidence |
| --- | --- |
| Codex `45eec73b11` (2026-09-09) | `ext/goal/src/spec.rs` leaves goal tool selection to the model but instructs explicit user/system intent. Base instructions let the model choose when planning helps. Goal state and continuation live outside the terminal renderer. |
| Kimi Code `1414d4602` (2026-08-13; older snapshot) | `agent-core` exposes `CreateGoal` with completion criteria and a separate reusable turn loop. Creation guidance accepts explicit autonomous-outcome requests or host goal intake. Thinking effort is mapped against model capabilities. |
| DSH `c389f96bf3` (2026-09-08) | `goal/tool-goal` explicitly permits inferring a long-running objective from a direct human request. Execution validates top-level human-turn provenance and exact state revisions. Goal state, goal tools, and continuation scheduling are separate consumers. |

DSH most directly matches the requested goal discretion. Its runtime validates
who may mutate state; the model judges whether persistence benefits the task.

Implementation packet, coordinated separately from mechanical extraction (it
lands in the `goal` feature crate once that exists):

1. Remove host-side semantic goal classification. Present the request, session
   state, tools, and existing goal to the model before deciding on persistence.
2. Revise the existing goal-tool and Operate guidance together: infer a goal
   when the requested outcome warrants durable continuation and has a useful
   completion criterion; answer, investigate, or perform ordinary multi-step
   work without a goal when that suffices. Honor corrections and opt-outs.
   Explicit user controls and model actions use the same goal state owner.
3. Treat test-time compute as reasoning effort, useful tool-feedback rounds,
   and evidence-driven revision. `auto_reasoning::select` currently chooses
   effort using message keywords; this is another semantic heuristic to
   replace. Preserve explicit route/effort choices. Let the lead allocate
   supported effort and execution budgets to work, with additional effort
   requested for later steps when new evidence makes that useful. Reuse
   `RequestTuning` and existing runtime/tool contracts; do not add an
   always-on classifier or a second agent loop ahead of every prompt.
4. Keep accounting, supported provider limits, permission checks, input
   provenance, durable state, and cancellation in Rust. Emit current state,
   remaining authorized resources, and tool/test results as compact feedback.
   A goal does not grant new spend or execution authority. Preserve the pinned
   prefix and append changing feedback to history.
5. Qualify judgment with model-driven sessions, not only deterministic mocks.
   Compare matched tasks at explicit effort/resource settings: a greeting,
   architecture discussion, one-file repair, large migration, unrelated
   followup, mid-run correction, false success evidence, cancellation, and
   repeated failure. Judge objective quality, useful continuation, task
   completion, verification quality, latency, tokens/cost, and correct
   stopping. Do not score a run better merely for creating a goal or taking
   more steps.

Start with the existing model's ordinary reasoning/tool loop. Add independent
review or multiple candidate attempts only where measured failures and task
stakes justify the extra compute. No model-evaluation runs, provider spend, or
performance gains were established by this source audit.
