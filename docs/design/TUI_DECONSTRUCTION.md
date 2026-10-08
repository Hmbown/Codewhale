# TUI deconstruction

The deliverable is an independently buildable headless runtime and a terminal
client of that runtime. Preserve working behavior while moving ownership out
of `codewhale-tui`. A lower line count alone does not establish the split.

## Audited baseline, 2026-09-09

Source inspection and offline Cargo metadata at `ce737266683b` found:

| Source | Physical Rust lines |
| --- | ---: |
| `crates/tui/src` | 971,321 in 817 files |
| `crates/tui/src/tui` | 266,378 |
| `crates/tui/src/tools` | 153,713 |
| `crates/tui/src/core` | 58,378 |
| `crates/tui/src/commands` | 60,951 |
| `crates/tui/src/lib.rs` | 20,327 |
| All of `crates/core/src` | 5,475 |

Counts include comments, blank lines, tests, and source files that may not be
compiled. Dedicated test-source files account for 191,058 lines within the
TUI total; additional inline tests remain in other files. The directory named
`tui` also contains domain logic. These are ownership clues, not production
LOC, a complete compiler dependency graph, or a language-port estimate.

The current dependencies explain the blockage:

- CLI imports TUI for runtime dispatch and route preferences.
- `core/engine.rs` imports approval policy, context thresholds, attachment
  parsing, and roster construction from `tui/`.
- `core/events.rs` carries the roster row and `session_manager.rs` persists
  the durable context reference; both now name their owning crate
  (`crate::agent_roster::AgentRosterRow`, `codewhale_core::ContextReference`)
  rather than a `tui::` re-export.
- `tools/subagent` imports engine policy/catalog functions, and implements
  its own repeated model-request/tool-result cycle in `run_subagent`.
- `crates/core` owns request construction and some runtime/session services;
  the main `Engine::run_turn` remains inside the TUI crate. The comment in
  `tui/src/core/mod.rs` claiming the engine has moved is incorrect.
- `core/protocol_parity.rs` exhaustively projects internal operations/events,
  but explicitly has no production consumers. Reuse or retire it during the
  migration; its existence does not establish client convergence.

`single_turn_loop.rs` currently counts functions named `run_turn`. It does
not detect `run_subagent`'s execution cycle. A passing name scan is therefore
insufficient evidence of one execution implementation.

## Runtime split sequence (2026-09-25, supersedes the order below)

The build-graph split runs first; the protocol-client contract follows it.
Nothing here is a claim that a step has landed: `git log` and the ratchet
(`scripts/runtime-boundary-baseline.json`) are the record.

1. **Rails.** `scripts/split/module_graph.py` computes the runtime closure
   (everything reachable from `core`, `tools`, `runtime_api`,
   `runtime_threads`, `client`, `llm_client`, `config` and
   `session_manager` without passing through `tui`, `commands`,
   `remote_control`, `context_report`, `composer_*` or `lib.rs`) and counts
   every reference from it into UI code, UI libraries, modules that move
   later, and UI intra-doc links. `scripts/check-command-crate-boundaries.py`
   runs it in CI: counts may only go down (RS-0).
2. **Create `crates/runtime` (`codewhale-runtime`) with live code.** The
   first batch is the leaf modules that reference nothing outside the batch
   in production or test code and touch no UI library. The crate is
   publishable, because the published `codewhale-tui` depends on it (RS-2).
3. **Cut the upward edges, one blocker per slice**, each lowering the
   ratchet: palette's `ratatui` dependency becomes an optional default
   feature (RS-3); the one `host_terminal` port carries every terminal side
   effect the runtime needs, starting with raw-mode suspension around
   interactive children (RS-4); voice capture splits from its slash command
   (RS-5); the context-window formatter moves to `utils` (RS-6);
   notification payload and sound policy move down while delivery (OSC
   writes, taskbar, title) stays in the TUI behind the port (RS-7); then the
   engine's terminal-chrome calls, auto-review and risk policy into
   `core::authority`, the remaining engine leaks, the command catalog, the
   `lib.rs` helpers, and the test-only references.
4. **Move the strongly connected core in one rename-only change** (about 64
   modules; a crate cannot hold half a cycle), after landing its visibility
   and rustdoc edits in place. Then `runtime_api` and the modules above the
   core, then the headless surfaces, then delete the path alias.
5. **Then the client contract** (behavioral, tracked separately): one engine
   owner in `runtime_threads`, runtime-owned queue and steer, approvals as
   server requests, a `runtime-client` facade, one protocol method table,
   and loop convergence.

`crates/runtime` must never depend on `codewhale-tui`, `codewhale-cli`,
ratatui, crossterm, `ansi-to-tui` or a terminal component kit. The TUI is the
one terminal-output owner: the runtime reaches the terminal only through
`host_terminal`, which the composition root installs for every host it
launches today. Withholding it from stdio hosts (ACP, MCP server,
app-server) is a later one-line change, not a code move.

### Where this sequence departs from the rest of this document

These three departures are deliberate; each is safe for the stated reason.

1. **The config hub does not have to leave first.** Once the upward edges are
   cut, `config` sits inside the runtime's strongly connected component, so
   it moves with it. Splitting config into `codewhale-config` (#6034, #6143)
   becomes internal runtime work afterwards instead of a precondition.
2. **Converging the two loops is not a precondition for moving the engine.**
   That rule assumed the engine would move while `run_subagent` stayed in the
   TUI, leaving two loops in two crates. Here both move into the same crate,
   so convergence stays a client-contract step and the one-loop guard keeps
   scanning every crate.
3. **The move uses one path alias**, a single root
   `use codewhale_runtime::{...};` block in the TUI `lib.rs`, instead of
   rewriting every caller in the moving change. A rename-only move lets
   in-flight branches rebase across it; a content rewrite of every caller
   conflicts with all of them. The alias is the only shim: no wrapper types,
   no per-item re-exports except the ones `crates/cli` needs until the
   headless surfaces move, and it is deleted by a published rewrite script
   once the moves are done.

The destination below still describes the intended layering, with one
change: the engine, turn loop and tools move into `codewhale-runtime`
together (there is no separate `codewhale-engine` crate), because the
engine, tools, config and client form one dependency cycle today.
`crates/core` stays the lower request-construction layer:
`codewhale-command-contract` depends on it, and the runtime needs the
contract, so putting the runtime into `core` would be a Cargo cycle.

## Intended ownership

This is the proposed destination, not a claim that the boundaries exist now.
Reuse existing crates; introduce only the three cohesive runtime libraries
below, with real consumers and all replaced paths migrated in each slice.

| Owner | Responsibility |
| --- | --- |
| `codewhale-tui` | Terminal lifecycle, rendering, input, pickers, terminal command presentation. No provider I/O, policy decisions, durable store, or agent loop. |
| `codewhale-cli` | Argument parsing, launch/composition, headless command presentation. Existing binary names remain compatible. |
| `codewhale-app-server` | HTTP/SSE and stdio transport adapters over the same runtime. Reconcile the embedded Runtime API and existing app-server; preserve external routes and auth. |
| New `codewhale-runtime` | Session/thread lifecycle, scheduling, recovery, child supervision, and composition of engine and stores. No model/tool execution loop. Used in process by terminal and server hosts. |
| New `codewhale-engine` | The shared parent/child execution implementation, context/compaction, tool dispatch, cancellation, approvals, and typed events. |
| New `codewhale-models` | Provider clients, live catalog/pricing resolution, and model routing against canonical config facts. Consolidate existing `agent` catalog consumers instead of retaining a second seeded registry. |
| Existing `config`, `secrets`, `execpolicy` | Canonical schema/route identity, credential storage/access, and policy decisions. UI labels stay outside these owners. |
| Existing `tools`, `mcp`, `hooks` | Tool contracts and implementations, extension transports, and hook execution. Agent/task tools call runtime capabilities; they do not own another agent loop. |
| Existing `state`, `protocol`, `core` | Persistence, shared wire/domain records, and request construction. Move the existing core runtime service owner into the runtime library as its callers migrate. |

Dependency direction: terminal/server -> runtime -> engine -> provider/tool
implementations and shared lower-level crates. Tools must not import the
concrete engine or runtime host. Use narrow service capabilities at the
composition boundary where a tool needs scheduling or agent control; do not
introduce a generic service-locator framework or a trait for every helper.

The TUI can retain in-process channels through the existing `EngineHandle`,
`Op`, and `Event` seams. HTTP remains a transport for external clients, not a
mandatory hop for local terminal use. Keep wire DTOs separate from internal
operations containing reply channels or resolved capabilities.

## Model judgment and test-time compute

Founder clarification, September 9: the harness should let the model decide
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
Codewhale should preserve that distinction without copying DSH's package count.

Implementation packet, to coordinate separately from mechanical extraction:

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
   architecture discussion, one-file repair, large migration, unrelated followup,
   mid-run correction, false success evidence, cancellation, and repeated
   failure. Judge objective quality, useful continuation, task completion,
   verification quality, latency, tokens/cost, and correct stopping. Do not
   score a run better merely for creating a goal or taking more steps.

Start with the existing model's ordinary reasoning/tool loop. Add independent
review or multiple candidate attempts only where measured failures and task
stakes justify the extra compute. No model-evaluation runs, provider spend, or
performance gains were established by this source audit.

## Implementation order (before 2026-09-25)

The runtime split sequence above replaces this order where they differ.
Every packet names the predecessor, all consumers, changed dependency edges,
and its verification. One owner handles shared manifests and integration.
Keep unrelated active work intact; follow the current workspace authority.

1. **Remove upward domain dependencies.** Finish the existing `AppMode` and
   `ApprovalMode` migration by pointing runtime consumers at their actual
   `config`/`execpolicy` owners. Move durable context-reference records out of
   file-mention UI; retain composer completion there. Separate worker receipt
   data from roster glyphs/layout. Move reasoning preference and approval
   policy out of UI modules, preserving exact route/credential identity.
   `ApiProvider` and `ProviderKind` currently differ for legacy table identity;
   do not replace one with the other through a lossy cast.
2. **Extract provider and tool foundations by cohesive subsystem.** Consolidate
   config schema and catalog facts as each affected consumer migrates. Move
   provider adapters with their tests into `codewhale-models`; reuse the
   existing model-client seam. Grow `tools`, `mcp`, `hooks`, and `state` in
   place. Move ordinary file/shell/MCP capabilities first. Leave agent
   orchestration with the execution owner until step 3; moving all of
   `tools/subagent` into a leaf tool crate would preserve a dependency cycle.
3. **Converge parent and child execution.** Inventory and preserve child
   budgets, route pins, permissions, tool activation, steering, parking,
   checkpoints, nested work, and terminal fan-in. Adapt children to the
   existing engine, then remove the old child model/tool cycle. Use actual
   parent/child call-path and behavior evidence; the function-name guard
   alone is not acceptance. Do not couple this semantic migration with the
   mechanical engine file move.
4. **Move the engine and shared host.** Once the runtime-to-UI dependencies
   are gone, move the existing execution implementation into
   `codewhale-engine`, with its owning unit tests. Establish one runtime host
   for terminal, exec, server, scheduling, and recovery. Migrate the existing
   core runtime and thread-manager consumers fully, preserving on-disk
   formats, replay cursors, locks, authority, and exact-once terminal events.
   No second runtime store or speculative replacement turn loop.
5. **Finish the clients.** Fold the embedded HTTP API into the existing
   app-server transport surface over `codewhale-runtime`. Move argument parsing
   and headless command presentation out of TUI `lib.rs` into CLI. Slash
   commands retain presentation in TUI and call the same runtime operations.
   Prune dependencies, temporary re-exports, and obsolete implementations.
   Only then resize remaining UI files according to actual responsibilities.

The first bounded source packet is step 1's existing mode imports and durable
context-reference records, including every caller. Subsequent packets are
chosen from the remaining dependency graph, not from a target crate count.

Moving a definition and repointing its internal callers belong to one complete
packet. Mechanical movement and behavioral changes should remain separately
reviewable, but do not land temporary re-export shims with their last consumers
left for an unspecified future migration. Keep shims only for genuine external
compatibility contracts and identify that contract.

## Verification and completion

- Preserve the model-facing runtime receipt, prompt/cache-prefix semantics,
  tool names/order, serialized records, and public commands for mechanical
  moves. A deliberate behavior fix names and tests the intended difference.
- Move private unit tests with their implementation. A `#[path]` module split
  remains in the same compilation unit; it does not reduce the test binary or
  establish faster builds. Do not make internals public just to relocate tests.
- Exercise local mock-provider parent and child turns, tool approval and
  denial, streaming, cancel/steer, explicit goals, pause/resume, reconnect,
  restart recovery, and terminal fan-in at the affected boundaries.
- Goal persistence is independent of Plan/Act/Operate. Let the model decide
  when persistent tracking benefits the requested work, using context and
  adequate reasoning time. Remove the host verb heuristic; do not replace it
  with a blanket explicit-command-only restriction. Explicit user opt-outs,
  cancellation, and authorized resource limits remain binding.
- The headless runtime and its tests must build with **no transitive dependency
  on `codewhale-tui`, ratatui, or crossterm**. Desktop and terminal consume the
  same lifecycle and execution authority.
- Measure warmed edit/build/test cycles and peak memory for a provider edit,
  tool edit, terminal-renderer edit, and locale edit before and after. Record
  compiler/profile/features/cache state. A leaf change must not recompile the
  unrelated TUI library test unit to run that leaf's own tests. Relinking a
  final application is a separate cost; no unmeasured speedup promises.
- Use existing `scripts/dev-test.sh` / `scripts/dev-cargo.sh` and focused checks
  during packets. Integration uses the required repository gates with actual
  counts. Local tests, full gates, hosted CI, installed artifacts, and PTY
  behavior remain separate evidence. Follow the workspace's human gates for
  publication, deploys, and spend.

`docs/BUILD_PERFORMANCE.md` retains historical measurements. Its older B3 and
micro-crate candidate lists are superseded by this dependency-led sequence.
The old all-at-once preconditions, test-file-move speed claim, and mandatory
uncompleted two-PR shim sequence are retired. A paused migration reports the
remaining monolith and unresolved consumers explicitly.
