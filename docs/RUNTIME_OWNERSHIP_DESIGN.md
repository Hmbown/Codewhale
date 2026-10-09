# Runtime Ownership and Frontend Lifecycle Design

**Status:** Partially implemented (Phase A: store-derived control sockets) +
workspace drive-lease design below (§11). Decision record: per-client topology
("Plan C" — A's mechanism, client-specific defaults), not single-owner-everywhere
("Plan B"). Written against `wave/0.10.1-next` (`5cf09da77` and descendants).

**Audience:** Engine runtime maintainers, desktop client, web client, VS Code
extension owners.

**Problem in one line:** 0.10.1 gave the engine an authenticated single-owner
control plane, but left every client spawning self-contained engines — so two
legitimate clients of the same user now refuse each other
(`authenticated Runtime owner belongs to another selected store`).

---

## 1. What actually broke, precisely

The 0.10.1 owner model resolved two identities from **disjoint inputs**:

| Identity | Resolved from | Code |
|---|---|---|
| Runtime store (data_dir, execution_scope) | `CODEWHALE_RUNTIME_DIR` / `DEEPSEEK_RUNTIME_DIR`, else `default_tasks_dir()/runtime` | `runtime_threads.rs:runtime_dir_override`, `default_runtime_store_root` |
| Control plane (socket path) | explicit flag > `CODEWHALE_HOME/run` > `XDG_RUNTIME_DIR` > OS app-support default | `daemon_socket.rs:resolve_socket_path` |

A `serve` that finds a live owner on the socket compares the owner's receipt
store against its **own** store selection
(`runtime_api.rs:validate_selected_owner`). Same store → attach as a frontend.
Different store → exit 1. Store choice is client input; socket choice is not a
function of it; therefore two clients of the same user on one machine can
trivially construct a mismatch. The refusal is the correct *security* outcome
of an inconsistent *topology* model — the engine can't tell "another user's
engine" from "my user's other window", because nothing relates the two
identities.

The second structural flaw: **the owner is a child of whichever client won the
election.** The VS Code extension stops its child when the window closes
("Stopping the engine started by this window"). Under a shared-owner model
that kills the engine hosting every other window's listeners. Ownership that
outlives frontends cannot live inside a frontend's process tree.

Everything needed to fix this already exists in 0.10.1 and is merely
under-composed:

- workspace scopes inside one owner (≤64, canonical-path verified, per-scope
  MCP/LSP/worker admission) — `RuntimeWorkspaceScopes`;
- guest-requested listeners bound inside the owner with the **guest's own**
  auth token, workspace, CORS, web/mobile flags — `RuntimeListenerSelection`;
- the attach stdout compatibility line (`Runtime API listening on …`) so a
  guest child is indistinguishable from a standalone engine to a spawning
  client — `run_attached_frontend`;
- stale-socket reclaim, peer-uid checks, receipt re-validation, version-skew
  guard, lease generations.

## 2. Design invariants

1. **One owner process per runtime boundary.** Not per window, not per
   workspace, not per user-machine (see §3 for when a user legitimately has
   several).
2. **Control-plane identity is a pure function of runtime identity.** The
   socket path is derived from the same inputs that select the store. Same
   boundary ⇒ same socket ⇒ the store always matches; different boundary ⇒
   different socket ⇒ different owner, and the mismatch refusal becomes
   structurally unreachable except for genuine races.
3. **The owner is nobody's child.** Election daemonizes the owner; every
   client — including the one that triggered election — attaches as a guest.
   No client's exit can take the engine down.
4. **Clients attach; they never demand a store.** `CODEWHALE_RUNTIME_DIR`
   stops being a per-window workaround and becomes an explicit boundary
   selector (isolation for CI, tests, experiments).
5. **Workspace multiplicity lives inside the owner.** A window is a frontend
   scope over one workspace; threads carry their workspace; one store serves
   all of them (this is already how the shared store filters per workspace).
6. **Engine lifetime is decoupled from any single client**: idle retirement
   with a grace period, blocked by active turns/tasks/automations or an
   explicit `linger` holder.
7. **Refusals are typed protocol outcomes** reserved for true races, version
   skew, and trust denials — never for legitimate topology.

## 3. The runtime boundary

A **boundary** is the tuple of everything that must be identical for two
engine processes to be interchangeable hosts of the same store. The key
insight: hash the **resolved store root**, not the env vars that produced it —
otherwise any input the store chain reads but the tuple omits (`DEEPSEEK_TASKS_DIR`
in the fallback chain) silently forks a second owner:

| Input | Canonicalization |
|---|---|
| Home root (`CODEWHALE_HOME` / default) | resolved absolute path |
| **Resolved runtime store root** (env overrides `CODEWHALE_RUNTIME_DIR`/`DEEPSEEK_RUNTIME_DIR`, else the tasks-dir fallback chain, all canonicalized) | canonical absolute path |
| Config source (`--config` / loaded path) | `canonical_runtime_config_source` |
| Config profile (`--profile` / env) | literal |
| ACP-only base (`serve --acp`) | boolean |

`boundary_id = sha256(canonical(serialization))[0..12]`.

Socket layout (unix; macOS path budget is 103 bytes — verified
`MAX_SOCKET_PATH_BYTES` — and this fits with room to spare):

- Default boundary (no overrides, default profile): **today's well-known
  path stays** (`~/Library/Application Support/codewhale/daemon.sock` on
  macOS, `$XDG_RUNTIME_DIR/codewhale/daemon.sock`, …). Discovery and doctor
  keep one obvious place to look.
- Non-default boundaries: a **sibling** of the default socket in the same
  0700 parent — `<default socket's parent>/b-<boundary_id>.sock`
  (macOS ≈ 56 bytes, well inside the 103-byte kernel budget).
  Receipts continue to ride beside their socket (`<name>.owner.json`), so
  publication, re-validation, and stale reclaim work unchanged per socket.
- Windows (transport currently reserved, unimplemented): the derived pipe
  name is `\\.\pipe\codewhale-daemon-<boundary_id>`; follow-up work.

Consequences for today's refusal paths:

| Today's check | Becomes |
|---|---|
| `authenticated Runtime owner belongs to another selected store` | Unreachable: store and socket derive from the same tuple |
| `selected config profile differs from the captured owner scope` | Unreachable on the same socket: profile is part of the boundary; a different profile lands on a different socket (a deliberate second engine, not a refusal) |
| `selected worker setting differs from the held scheduler` | A typed control error (`owner_scheduler_mismatch`) carrying the owner's setting; the receipt advertises `workers`; clients retry with it. The scheduler is per-owner, first election wins |
| `stale owner receipt belongs to a different selected store` | Unreachable: a receipt at a given socket can only belong to that boundary |
| Version skew, owner-died-mid-attach, trust denial | Remain — these are genuine races/skew |

Scope cap: `MAX_RUNTIME_WORKSPACE_SCOPES = 64` stays, but scopes stop being
"retained until owner retirement" (today's wart: *restart the owner to retire
unused scopes*). See §4.

## 4. Owner and frontend lifecycle

### States

```
absent → electing → ready ⇄ serving → draining → retired
```

- **electing:** bind the boundary socket (kernel-atomic election; losers
  probe, find the winner, and attach). Stale reclaim (dead socket, refused
  connection) is existing behavior.
- **ready:** store opened under its process-owner lock, receipt published,
  scheduler and task manager started, first workspace scope admitted.
- **serving:** frontends attach/detach; each attachment gets
  `daemon/frontend_ready` with its endpoint (reused listener or a
  guest-requested one carrying the guest's token).
- **draining:** stop admitting; let accepted turns finish (existing
  `shutdown_and_wait` semantics); release the store lock; retire socket and
  receipt.
- **retired:** nothing on disk claims ownership; the next client re-elects.

### Election daemonizes

The client that wins election does not *stay* the owner as a child of the
client's process tree. The elected `serve` re-execs a detached supervisor
(single child, reparented to the OS, stdio closed) which opens the store and
publishes the receipt; the original child then attaches like any guest. Every
frontend is therefore killable by its own client with zero engine-wide
blast radius. The VS Code extension's "stop the engine started by this
window" keeps meaning exactly "stop *my* frontend".

### Idle retirement

The owner retires when, for a configurable grace (default 60 s):

- zero live frontend connections,
- no accepted turns, running tasks, or due/running automations,
- no `linger` holder.

`linger` is an attach-mode property, not a flag only the desktop knows:
a desktop app attaches a pure control connection with linger for
app-lifetime; an account-owned remote-control session (mobile/web remote)
holds linger while active; automations due within the grace block retirement
on their own. Retirement is graceful drain, never SIGKILL.

### Crash and recovery

An owner crash EOFs every guest's control connection. Guest children
re-run election (one winner), re-attach, and re-request their listeners.
Accepted turns in flight at crash are lost — that is the honest cost of one
owner; stores, events, and receipts are durable, so recovery is
re-elect → re-attach → reload thread state, all idempotent (scope admission
re-canonicalizes the workspace; listener admission is a fresh bind).
Guest children may print a **new** `Runtime API listening on …` line after
re-election; clients treat every matching line as the current endpoint, not
just the first (extension change, §5.3).

### Upgrades

Version skew already refuses cross-version attach. Add one control
operation, `daemon/drain_restart`: the desktop (or CLI) tells the owner to
drain and exit, then starts the new binary and lets frontends re-attach.
No orphaned sockets: drain is the same path as retirement.

### Scope retirement

Workspace scopes refcount live frontend attachments; on last detach the
scope's MCP/LSP pools close and the scope slot frees after the same grace.
Disk state (items, events) stays; re-admission is cheap. The 64-cap now
bounds *concurrent* workspaces, not lifetime workspaces.

## 5. Client contracts

All clients speak the same three verbs: **find** (connect boundary socket),
**elect** (spawn detached owner if absent), **attach** (frontend scope +
listener selection). Nothing else.

### 5.1 Desktop (canonical consumer)

Attaches over the control socket directly — JSON-RPC control transport, no
TCP hop, one connection per window, one linger connection for the app.
Orchestrates upgrades via `daemon/drain_restart`. Surfaces owner facts
(pid, version, boundary, scopes, frontends, uptime) in its status UI from
the receipt plus a `daemon/status` read.

### 5.2 Web (`codewhale web`)

Finds/elects the default boundary, attaches a web frontend. Bootstrap-nonce
URLs are per-attachment (existing `web_bootstrap_url`). Multi-tab is N SSE
connections to one listener — never N engines.

### 5.3 VS Code extension

Keeps its child-per-window shape — the compat line makes a guest child look
identical to a standalone engine — but the child attaches to the default
boundary instead of demanding a private store:

- stop setting `CODEWHALE_RUNTIME_DIR` **and** `DEEPSEEK_TASKS_DIR` on
  engines ≥ 0.10 — the runtime-dir override was the isolation workaround,
  and the tasks-dir env forks the resolved store root (hence the boundary);
  tasks land in the user-level tasks dir, which is what `codewhale doctor`
  and receipts already read;
- one-time version probe (`codewhale --version`) chooses the strategy:
  ≥ 0.10 → attach; < 0.10 → legacy per-workspace store (current 0.8.3
  behavior, which is correct for the old one-process-per-store model);
- accept repeated `Runtime API listening on` lines (re-election) and update
  the base URL;
- surface typed attach failures verbatim instead of
  `Engine exited before becoming ready`;
- add a "Codewhale: engine status" command (doctor owner panel: boundary,
  socket, pid, scopes, frontends) and "Codewhale: restart engine"
  (`daemon/drain_restart`).

Window close kills only the guest child; its listener drains in the owner;
other windows are untouched. Same-workspace-in-two-windows now works (same
scope, two listeners, two tokens).

### 5.4 ACP (`serve --acp`)

An ACP base owner is immutable by design (`acp_only`); in this design it is
simply its own boundary (the acp-only bit is in the tuple), so Zed sessions
never collide with the interactive owner and never attach into it.

### 5.5 Mobile / remote control

Account-owned remote control attaches to the default boundary with
watch/drive device-token intents (existing) and holds linger while a remote
session is live. Nothing new.

### 5.6 Interactive TUI, `exec`, MCP stdio — explicit non-goals

The interactive TUI keeps its per-session store (`for_session`), `exec`
stays a one-shot worker, `serve --mcp` stays stdio-scoped. They are not
frontends of the owner and must not become so implicitly; the boundary model
governs the `app-server`/`serve` daemon family only. (A future where the
TUI attaches as a frontend is possible but is not this design.)

## 6. Security

Kept unchanged: peer-uid checks, 0600/0700, per-connection receipts that are
never bearers, loopback-only default binds, per-listener guest tokens,
constant-time token comparison, canonical workspace verification
(`O_DIRECTORY|O_NOFOLLOW` + same-file identity), device-token intents.

Added or sharpened:

- **Boundary hashing uses canonical inputs only** — a symlinked config path
  or non-canonical runtime dir must not fork a second owner silently.
- **Workspace admission is an engine-side trust decision**: scope admission
  consults project trust (`[projects]` in config); an untrusted workspace's
  first attach surfaces an approval (the engine already runs approvals; the
  extension's VS Code-side trust is a client precondition, not the
  engine's).
- **Audit events** for `owner_elected`, `scope_admitted`, `frontend_attached`,
  `frontend_detached`, `owner_retired` land in the boundary store's event
  log — the owner election stops being observable only through
  `engine.log`.
- Derived sockets live under the same 0700 parent as the default socket; no
  new world-writable surface. Hash collision on a derived socket shows up as
  a receipt mismatch → refuse (the true-race class).

## 7. Mechanism changes (touchpoints)

Phase 1 — engine, identity + typed errors (no client changes required):

- `daemon_socket.rs`: extend `SocketPathInputs` with the runtime-dir override
  and config profile/source; derive non-default socket names from
  `boundary_id`; keep the default path for the default boundary.
- `runtime_api.rs`: `validate_selected_owner` loses the store-compare (now
  structurally guaranteed) and keeps pid/lease/principal checks; worker
  mismatch becomes a typed control error; scheduler setting joins the
  receipt.
- Election daemonization: a detached re-exec path in `serve` startup
  (`--supervise-owner` internal flag); guest children always attach.
- Scope refcounting + grace retirement in `RuntimeWorkspaceScopes`; idle
  retirement loop honoring turns/tasks/automations/linger.
- `doctor`: owner panel (boundary, socket, receipt, scopes, frontends).

Phase 2 — extension migration (version probe, drop per-workspace runtime
dir, repeated-line handling, status/restart commands, typed error surface).

Phase 3 — desktop/web speak control-protocol attach directly; linger;
`daemon/drain_restart`; `daemon/status`.

Phase 4 (optional, only if a real requirement appears) — per-scope worker
pools, or per-workspace store partitions inside a boundary.

## 8. Rollout and compatibility

| Client | Engine 0.8.x | Engine ≥ 0.10 (phase 1) | After phase 2/3 |
|---|---|---|---|
| Extension ≤ 0.8.3 | works (old model) | **broken today** (the reported error) | works: version probe keeps per-workspace stores on old engines |
| Extension ≥ phase 2 | works via probe | attaches; multi-window works | same |
| `codewhale web` | standalone | unchanged behavior | attaches to default boundary |
| Desktop socket client | n/a | attach works when boundary matches | canonical; linger; upgrades |
| TUI / exec / MCP | unchanged | unchanged | unchanged |

No transport changes, no wire-format changes beyond additive typed errors and
control ops; the stdout readiness line contract is preserved for every
spawn-and-scan client.

## 9. Rejected alternatives

- **Socket-per-store only (keep N independent engines).** Smallest diff, but
  leaves N provider catalogs, N schedulers contending over the shared
  automations/tasks locations (`AutomationManager::default_location`,
  `default_tasks_dir` are per-user, not per-store), and N heavy processes.
  The 0.10.1 owner model exists to end exactly this.
- **HTTP-first discovery (port files as the control plane).** Port files
  already exist per workspace; HTTP cannot authenticate the local peer the
  way the socket + receipt do. The socket stays the control plane.
- **OS supervision (launchd/systemd user units).** Installs state per
  machine for a problem self-election + idle retirement solves with zero
  install; desktop *may* supervise, but nothing requires it.
- **Owner stays inside the electing client, extensions learn "shared"
  semantics.** Makes every client's stop path a protocol negotiation about
  other clients' needs; fragile under SIGTERM. Daemonized election is
  simpler and total.
- **Store-per-workspace partitions inside the owner.** Unnecessary today —
  threads are workspace-filtered in one shared store, and the extension
  already depends on shared saved sessions. Revisit only with a concrete
  isolation requirement.

## 10. Open questions

1. Grace default (60 s?) and whether linger is per-connection or
   per-boundary with a count.
2. Should the automations scheduler run in the owner unconditionally, or
   only when at least one frontend has ever attached (headless-schedule
   mode)?
3. `daemon/drain_restart` authorization: any attached frontend, or only
   linger holders / an explicit operator path?
4. Whether the desktop wants a *named* multi-boundary story (e.g., one
   boundary per account profile) — the tuple supports it; the UX does not
   have to yet.
5. Windows named-pipe implementation timing (transport is reserved today).

---

## 11. Workspace drive lease (single-writer per workspace)

**Status:** Design accepted; implementation alongside Phase A.

### The invariant

At most one **drive** holder per workspace (canonical path), process-wide,
across stores and clients. Watch-only observers are unlimited. This is the
precise form of "regardless of client (web, desktop, TUI, VS Code), only one
client at a time drives a workspace".

### Why the lease unit is the workspace, not the store

Three engine facts make the workspace the real contended resource:

1. **The workspace is a shared write surface.** Two engines with different
   stores editing the same working tree is *worse* than store double-writes:
   it corrupts user code, not history.
2. **The snapshot side-repo is keyed by workspace, store-independent**
   (`snapshot/paths.rs` — user-level state dir hashed by canonical workspace
   path; the 500 MB cap warning in the wild lands there). Two engines on one
   workspace write the same snapshot repo with no serialization today.
3. **The in-process git-write mutex only covers windows on the same server**
   (`git_writes`, #6647). Cross-process it does not exist.

The existing exclusivity unit — the store process-owner lock — is scoped to
   one store. Per-workspace stores (extension), per-session stores (TUI), and
   the user-level default store never contend on it, yet may share a
   workspace. The lease lifts the unit to where the actual conflict is.

### Drive vs watch

The engine already separates these intents in device tokens
   (`client_token_intents`: `watch` default, `drive` explicit). The lease
   gates exactly the drive path: turn admission (accepted turns that mutate
   the workspace), workspace scope admission for driving frontends, and CLI
   resume/fork. Watch flows (read-only observers, remote watch sessions,
   status panels) never take the lease.

### Mechanism

- **Lease file**: one per workspace under the existing snapshot-state base —
  `<snapshot_state_base()>/leases/<project_hash>/<worktree_hash>.lease` —
  reusing the same canonical-path hashing as the snapshot side-repo, so worktrees
  of one checkout share a project bucket and a worktree lease is exact.
- **Acquire**: open-create + `fd_lock` write lock (same primitive as the
  store's process-owner lock and automations). Write holder facts: pid,
  `process_start` (the existing liveness token), client label, store root,
  timestamp.
- **Liveness**: the lock releases when the holder dies (kernel closes fds),
  so a crashed holder self-heals. A *live-but-wedged* holder is detected by
  pid + `process_start` mismatch (`unix_process_start`, the same check the
  owner receipt uses); `--force` (or an engine-side takeover when the
  recorded pid is provably dead) rewrites the lease after breaking the
  stale lock.
- **Refusal is legible**: on contention the engine reports the recorded
  holder (pid, client label, store root, age) so the second client can offer
  a real choice: stay watch-only, or take over.
- **Where it lives**: one small module in the TUI crate beside the store
   process-owner lock, taking only `&Path` (workspace) + holder facts.

### Admission points

1. `RuntimeWorkspaceScopes::admit` — the engine-side choke point every
   driving frontend already passes through (it holds the workspace dir fd
   there). Driving attach without a lease → typed refusal.
2. Turn admission for workspace-mutating turns (defense in depth below the
   scope check, covering engines that somehow opened before the lease
   existed).
3. CLI `codewhale thread resume` / `fork` workspace scopes (they go through
   scope admission, so (1) covers them).

Watch paths attach without the lease; their token intent already forbids
   writes.

### Costs and caveats (honest list)

- Refusal UX is a product surface: the second client must present holder
  facts and a takeover path, or this becomes a support burden.
- fd-lock semantics weaken on NFS/network volumes; the lease degrades to
  advisory there (same as every existing lock in the engine).
- pid namespaces in containers make `process_start` checks conservative:
  refuse takeover rather than false-positive "dead".
- Test matrix cost: client-pair mutex behavior (VS Code × desktop, TUI ×
  VS Code, …) is a durable maintenance cost.
- Rollout: engines older than the lease have no lease; the lease binds only
  when both sides run the new engine (first-ready-wins, no migration).

### Relationship to the phases

The lease is orthogonal to the topology phases: A/C decide *where history
   lives*, the lease decides *who may write*, B's lifecycle machinery remains
   optional. Combined semantics: anyone can watch; history converges by
   domain; one driver per workspace at a time.
