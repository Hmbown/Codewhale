// ★ AsBudy（2026-09-19）：本会话内「这类操作以后不再询问」的分组指纹集合。
//   对齐官方 CLI 的 `approval_session_approved`（`tui/ui/approval_routing.rs`）：
//   存的是引擎给的**分组指纹**（按操作族算：shell 命令前缀 / 域名 / 补丁路径），
//   **绝不存工具名** —— 官方源码明确警告：批准一条 shell 命令不能放行整个 shell 工具。
//   存的位置：内存，跟官方一样是**会话级**（页面一刷新/重开会话就清空）。
//   ⚠️ 必须定义在任何调用之前（`applyEnvelope` 会用到）—— 挪到文件后面就是 TDZ。
const abSessionApproved = new Set();

export const STREAM_EVENT_NAMES = [
  "thread.started",
  "thread.updated",
  "thread.forked",
  "turn.started",
  "turn.lifecycle",
  "turn.usage",
  "turn.steered",
  "turn.interrupt_requested",
  "turn.completed",
  "item.started",
  "item.delta",
  "item.completed",
  "item.canceled",
  "item.failed",
  "item.interrupted",
  "approval.required",
  "approval.decided",
  "approval.timeout",
  "user_input.required",
  "user_input.answered",
  "user_input.canceled",
  "sandbox.denied",
  "agent.spawned",
  "agent.progress",
  "agent.completed",
  "agent.list",
  "tool_call.requested",
  "tool_call.resolved",
  "tool_call.canceled",
  "tool_call.timeout",
];

/// AsBudy：广播给 Msgbar 状态行的事件集（2026-09-18）——
/// 只广播「开始 / 结束 / 需要等人的」这几类，**不含 item.delta**（太频繁，逐条广播会拖慢流式渲染）。
/// 2026-09-19 加 `turn.usage`：引擎每次模型调用都会推这条（`runtime_threads.rs`
/// 的 `Event::TurnUsage` 分支，payload 带 usage / duration_ms / first_token_ms /
/// request_ms），**官方前端自己订阅了它却没有任何处理分支** —— 等于数据到了浏览器
/// 就丢。CLI 的状态行正是拿它算 `ttft` / `平均 tok/s` / `↓ tokens`（`tui/infoline.rs`
/// ＋ `tui/session_metrics.rs` 的 `record_model_call`）。频率低（一次模型调用一条，
/// 远低于 item.delta），广播出去不构成渲染压力。
const ACTIVITY_EVENTS = new Set([
  "turn.started",
  "turn.completed",
  "turn.usage",
  "item.started",
  "item.completed",
  "item.failed",
  "tool_call.requested",
  "tool_call.resolved",
]);

export function createThreadState(threadId = "") {
  return {
    threadId,
    thread: null,
    turns: new Map(),
    turnOrder: [],
    items: new Map(),
    itemOrder: [],
    latestSeq: 0,
    approvals: new Map(),
    userInputs: new Map(),
    dynamicToolCalls: new Map(),
  };
}

export function applySnapshot(state, detail, expectedThreadId = state.threadId) {
  if (!detail || !detail.thread || detail.thread.id !== expectedThreadId) {
    return false;
  }
  state.threadId = expectedThreadId;
  state.thread = detail.thread;
  state.turns = new Map();
  state.turnOrder = [];
  for (const turn of Array.isArray(detail.turns) ? detail.turns : []) {
    if (!turn || !turn.id) continue;
    state.turns.set(turn.id, turn);
    state.turnOrder.push(turn.id);
  }
  state.items = new Map();
  state.itemOrder = [];
  for (const item of Array.isArray(detail.items) ? detail.items : []) {
    if (!item || !item.id) continue;
    state.items.set(item.id, item);
    state.itemOrder.push(item.id);
  }
  state.latestSeq = normalizedSequence(detail.latest_seq);
  state.approvals = new Map();
  for (const approval of Array.isArray(detail.pending_approvals) ? detail.pending_approvals : []) {
    const approvalId = approval?.approval_id || approval?.id;
    if (!approvalId) continue;
    // ★ AsBudy：这一类的分组指纹本会话已经「不再问」过 → 直接放行，不进待办列表。
    //   ⚠️ 这条路径跟 `applyEnvelope` 是**两条**来源（快照 / 事件），两处都得判 ——
    //   只判事件那条，页面一切会话/刷新就会被快照重新摆回弹窗（2026-09-19 实测踩到）。
    const gk = String(approval?.approval_grouping_key || "");
    if (gk && abSessionApproved.has(gk)) {
      resolveApproval(approvalId, "allow", false).catch(() => {});
    } else {
      state.approvals.set(approvalId, approval);
    }
  }
  state.userInputs = new Map();
  for (const input of Array.isArray(detail.pending_user_inputs) ? detail.pending_user_inputs : []) {
    const inputId = input?.input_id || input?.id;
    if (inputId) state.userInputs.set(inputId, input);
  }
  state.dynamicToolCalls = new Map();
  for (const call of Array.isArray(detail.pending_dynamic_tool_calls) ? detail.pending_dynamic_tool_calls : []) {
    if (call?.call_id) state.dynamicToolCalls.set(call.call_id, call);
  }
  return true;
}

export function applyRuntimeEvent(state, envelope) {
  if (runtimeEventContinuity(state, envelope) !== "next") {
    return false;
  }
  const sequence = normalizedSequence(envelope.seq);
  state.latestSeq = sequence;

  const eventName = envelope.event || envelope.kind || "";
  const payload = envelope.payload && typeof envelope.payload === "object"
    ? envelope.payload
    : {};

  if (
    (eventName === "thread.started" || eventName === "thread.updated" || eventName === "thread.forked")
    && payload.thread
  ) {
    state.thread = payload.thread;
  } else if (eventName === "turn.started" || eventName === "turn.completed") {
    if (payload.turn) upsertTurn(state, payload.turn);
    if (eventName === "turn.completed") {
      clearTurnAttention(state, envelope.turn_id || payload.turn?.id || "");
    }
  } else if (eventName === "turn.lifecycle") {
    const turnId = envelope.turn_id;
    const turn = turnId ? state.turns.get(turnId) : null;
    if (turn && payload.status) {
      state.turns.set(turnId, { ...turn, status: payload.status });
    }
  } else if (eventName === "turn.interrupt_requested") {
    const turnId = envelope.turn_id;
    const turn = turnId ? state.turns.get(turnId) : null;
    if (turn) state.turns.set(turnId, { ...turn, status: "in_progress" });
  } else if (
    eventName === "item.started"
    || eventName === "item.completed"
    || eventName === "item.canceled"
    || eventName === "item.failed"
    || eventName === "item.interrupted"
    || eventName === "agent.spawned"
    || eventName === "agent.progress"
    || eventName === "agent.completed"
    || eventName === "agent.list"
  ) {
    if (payload.item) upsertItem(state, payload.item);
  } else if (eventName === "item.delta") {
    appendItemDelta(state, envelope.item_id, payload);
  } else if (eventName === "approval.required") {
    const approvalId = payload.approval_id || payload.id;
    if (approvalId) {
      // ★ AsBudy：这一类的分组指纹本会话已经「不再问」过 → 直接放行，不弹窗。
      //   放行走的还是同一条正常通道（POST /v1/approvals/{id}），不绕过后端。
      const gk = String(payload.approval_grouping_key || "");
      if (gk && abSessionApproved.has(gk)) {
        resolveApproval(approvalId, "allow", false).catch(() => {});
      } else {
        state.approvals.set(approvalId, {
          ...payload,
          turn_id: payload.turn_id || envelope.turn_id || "",
        });
      }
    }
  } else if (eventName === "approval.decided" || eventName === "approval.timeout") {
    const approvalId = payload.approval_id || payload.id;
    if (approvalId) state.approvals.delete(approvalId);
  } else if (eventName === "user_input.required") {
    const inputId = payload.id;
    if (inputId) {
      state.userInputs.set(inputId, {
        ...payload,
        turn_id: payload.turn_id || envelope.turn_id || "",
      });
    }
  } else if (eventName === "user_input.answered" || eventName === "user_input.canceled") {
    const inputId = payload.input_id || payload.id;
    if (inputId) state.userInputs.delete(inputId);
  } else if (eventName === "tool_call.requested") {
    if (payload.call_id) {
      state.dynamicToolCalls.set(payload.call_id, {
        ...payload,
        turn_id: payload.turn_id || envelope.turn_id || "",
      });
    }
  } else if (
    eventName === "tool_call.resolved"
    || eventName === "tool_call.canceled"
    || eventName === "tool_call.timeout"
  ) {
    if (payload.call_id) state.dynamicToolCalls.delete(payload.call_id);
  }
  return true;
}

function clearTurnAttention(state, turnId) {
  for (const [id, approval] of state.approvals) {
    if (!approval?.turn_id || approval.turn_id === turnId) state.approvals.delete(id);
  }
  for (const [id, input] of state.userInputs) {
    if (!input?.turn_id || input.turn_id === turnId) state.userInputs.delete(id);
  }
  for (const [id, call] of state.dynamicToolCalls) {
    if (!call?.turn_id || call.turn_id === turnId) state.dynamicToolCalls.delete(id);
  }
}

export function runtimeEventContinuity(state, envelope) {
  if (!envelope || envelope.thread_id !== state.threadId) {
    return "ignore";
  }
  const sequence = normalizedSequence(envelope.seq);
  if (sequence <= state.latestSeq) {
    return "ignore";
  }
  if (Object.hasOwn(envelope, "previous_seq")) {
    const previousSequence = normalizedSequence(envelope.previous_seq);
    if (previousSequence !== state.latestSeq) {
      return "gap";
    }
  }
  return "next";
}

export async function snapshotThenSubscribe({
  state,
  threadId,
  loadSnapshot,
  subscribe,
  isCurrent = () => true,
}) {
  const detail = await loadSnapshot(threadId);
  if (!isCurrent() || !applySnapshot(state, detail, threadId)) {
    return false;
  }
  if (!isCurrent()) return false;
  // A recovery caller may return a stream-open handshake. Await it so a
  // replacement snapshot is not called continuous until the replacement SSE
  // stream has actually opened. Synchronous subscribers remain supported.
  await subscribe(threadId, state.latestSeq);
  return true;
}

export async function recoverSnapshotAndSubscribe(options, onRecovered) {
  const subscribed = await snapshotThenSubscribe(options);
  if (!subscribed) return false;
  onRecovered();
  return true;
}

// ---------------------------------------------------------------------------
// Typed selection identity (#4397)
//
// The dashboard can have two very different things selected: a *saved session*
// (a recording on disk) or a *live thread* (a running runtime object). Almost
// every safety rule in this slice reduces to "which one is it?", so the answer
// is a typed value rather than a pair of loosely-related string fields that can
// both be set, both be empty, or disagree.
// ---------------------------------------------------------------------------

// Nothing selected.
export const NO_TARGET = Object.freeze({ kind: "none" });

// A saved session: read-only. Peek only, never reply, never approve.
export function sessionTarget(sessionId) {
  return Object.freeze({ kind: "session", sessionId: String(sessionId || "") });
}

// A live thread: the only thing that can receive a reply or an approval.
export function threadTarget(threadId) {
  return Object.freeze({ kind: "thread", threadId: String(threadId || "") });
}

// May the composer send to this target?
//
// Only a live thread. A saved session has no runtime to receive a message;
// offering a composer against one would be an affordance with nothing behind
// it, and "resume it silently on send" would attach the user's message to a
// thread they never asked to create.
export function canReply(target) {
  return target?.kind === "thread" && Boolean(target.threadId);
}

// Resolve the id a reply must be POSTed to, or an explicit refusal.
//
// Fails closed on every ambiguity: no target, a session target, or a target
// whose thread is not the one the live stream is following (a stale target —
// the user changed rows while a request was in flight).
export function resolveReplyTarget(target, streamState) {
  if (!target || target.kind === "none") {
    return { ok: false, reason: "no-target" };
  }
  if (target.kind === "session") {
    return { ok: false, reason: "session-not-live" };
  }
  if (!target.threadId) {
    return { ok: false, reason: "no-target" };
  }
  if (streamState && streamState.threadId && streamState.threadId !== target.threadId) {
    return { ok: false, reason: "stale-target" };
  }
  return { ok: true, threadId: target.threadId };
}

// Resolve an approval decision to the thread that owns it, or refuse.
//
// An approval is authority: answering the wrong one, or answering one that
// has already been decided elsewhere, is worse than not answering. So the
// approval must be present in the *current* stream state, and that state must
// belong to the selected live thread.
export function resolveApprovalTarget(approvalId, target, streamState) {
  const reply = resolveReplyTarget(target, streamState);
  if (!reply.ok) return reply;
  if (!approvalId) return { ok: false, reason: "no-approval" };
  if (!streamState || streamState.threadId !== reply.threadId) {
    return { ok: false, reason: "stale-target" };
  }
  if (!streamState.approvals || !streamState.approvals.has(approvalId)) {
    // Decided, timed out, or belonging to a thread we are no longer watching.
    return { ok: false, reason: "stale-approval" };
  }
  return { ok: true, threadId: reply.threadId, approvalId };
}

// Resolve a user-input submission to the live thread and pending request that
// own it. User-input answers can resume a paused turn, so a stale card must not
// be able to answer a request from another thread or one already settled by a
// different client.
export function resolveUserInputTarget(inputId, target, streamState) {
  const reply = resolveReplyTarget(target, streamState);
  if (!reply.ok) return reply;
  if (!inputId) return { ok: false, reason: "no-user-input" };
  if (!streamState || streamState.threadId !== reply.threadId) {
    return { ok: false, reason: "stale-target" };
  }
  if (!streamState.userInputs || !streamState.userInputs.has(inputId)) {
    return { ok: false, reason: "stale-user-input" };
  }
  return { ok: true, threadId: reply.threadId, inputId };
}

function ownAnswerValue(collection, id) {
  if (collection instanceof Map) return collection.get(id);
  if (collection && typeof collection === "object" && Object.hasOwn(collection, id)) {
    return collection[id];
  }
  return undefined;
}

// Build the exact Runtime answer payload from selected options and custom
// text. This keeps single-select questions single, rejects stale/forged option
// values, and preserves the TUI rule that a custom answer is always reachable.
export function answersForUserInput(request, selections = {}, freeText = {}) {
  const questions = Array.isArray(request?.questions) ? request.questions : [];
  if (questions.length === 0) {
    return { ok: false, reason: "invalid-request", question: "the question" };
  }

  const answers = [];
  const questionIds = new Set();
  for (const question of questions) {
    const id = String(question?.id || "");
    const questionLabel = String(question?.header || question?.question || id || "the question");
    if (!id || questionIds.has(id)) {
      return { ok: false, reason: "invalid-request", question: questionLabel };
    }
    questionIds.add(id);

    const optionLabels = new Set(
      (Array.isArray(question.options) ? question.options : [])
        .map((option) => String(option?.label || ""))
        .filter((value) => Boolean(value.trim())),
    );
    const selectedValue = ownAnswerValue(selections, id);
    const selected = Array.isArray(selectedValue)
      ? [...new Set(selectedValue.map((value) => String(value)).filter((value) => value.trim()))]
      : [];
    if (selected.some((value) => !optionLabels.has(value))) {
      return { ok: false, reason: "invalid-option", question: questionLabel };
    }

    const otherValue = ownAnswerValue(freeText, id);
    const other = String(otherValue || "").trim();
    const count = selected.length + (other ? 1 : 0);
    if (count === 0) {
      return { ok: false, reason: "missing-answer", question: questionLabel };
    }
    if (!question.multi_select && count !== 1) {
      return { ok: false, reason: "multiple-answers", question: questionLabel };
    }

    for (const value of selected) answers.push({ id, label: value, value });
    if (other) answers.push({ id, label: "Other", value: other });
  }
  return { ok: true, answers };
}

// Human-readable reason for a refusal, for the status banner.
export function refusalMessage(reason) {
  switch (reason) {
    case "session-not-live":
      return "这是已保存的会话，不是进行中的对话——没有发送。先恢复它才能回复。";
    case "stale-target":
      return "那条会话已不是当前选中的——没有发送。";
    case "stale-approval":
      return "那个请求已被处理或已过期——没有发送。";
    case "no-approval":
      return "没有找到待审批的请求——没有发送。";
    case "stale-user-input":
      return "那个问题已被回答或已过期——没有发送。";
    case "no-user-input":
      return "没有找到待输入的问题——没有发送。";
    default:
      return "先选一个进行中的会话——没有发送。";
  }
}

// The SSE resume cursor and whether the stream is known to have a gap.
//
// Surfaced rather than kept internal: after a reconnect the user needs to
// know whether what they are reading is continuous or whether events were
// missed and a re-snapshot is pending.
export function streamCursor(state, { gap = false, connected = true } = {}) {
  const seq = normalizedSequence(state?.latestSeq);
  return {
    latestSeq: seq,
    gap: Boolean(gap),
    connected: Boolean(connected),
    label: !connected
      ? `重新连接中——从 #${seq} 续上`
      : gap
        ? `检测到断档——从 #${seq} 重新同步`
        : `实时——事件 #${seq}`,
  };
}

// Keep machine diagnostics available without making them the conversation's
// loudest content. Known transport failures get one calm product sentence;
// the byte-for-byte receipt remains behind the disclosure.
/* AsBudy：回执说人话（2026-09-18 老板：「全程没有任何中文、没有进度反馈」）
 *
 * 官方原样：`label = humanize(kind) · humanize(status)`（“Tool Call · Completed”）、
 *   `summary` 直接是引擎给的原文（20 行 `drwxr-xr-x` 糊在摘要位置上）。
 *   **上游 app.mjs 就长这样，不是我们改坏的** —— 但客户看不懂，而且它把
 *   「显示文件与命令明细」那个开关架空了（那个开关只能藏折叠块，主体还是原始输出）。
 *
 * 这里做两件事：
 *   ① 标签用中文（执行命令 · 完成 / 文件改动 · 未完成）
 *   ② 摘要说「做了什么」—— 引擎 metadata 里带着 tool_name 与 tool_input
 *      （完整命令 / 文件路径），以前完全没用上。
 * 原文不丢：仍然进 `raw`，挂在折叠的「查看回执」里。
 *
 * ⚠️ 2026-09-19：词典要**跟着引擎的枚举补全**，漏掉的会退化成 humanize() 的英文
 *   （引擎 `TurnItemKind` 九种、`TurnItemLifecycleStatus` 六种，见 runtime_threads.rs:640-667；
 *   以前少了 command_execution / context_compaction / error / queued —— 客户会看到
 *   "Command Execution · Completed" 这种英文标签）。
 */
const RECEIPT_KIND_ZH = {
  tool_call: "工具",
  tool_result: "工具",
  file_change: "文件改动",
  command_execution: "执行命令",
  context_compaction: "整理对话",
  status: "进度",
  error: "出错",
  agent_message: "回复",
  agent_reasoning: "思考",
};
const RECEIPT_STATUS_ZH = {
  queued: "等待中",
  completed: "完成",
  failed: "未完成",
  in_progress: "进行中",
  pending: "等待中",
  canceled: "已取消",
  cancelled: "已取消",
  interrupted: "已打断",
};

function receiptLabelZh(kind, status) {
  const head = RECEIPT_KIND_ZH[kind] || humanize(kind);
  const tail = RECEIPT_STATUS_ZH[status] || humanize(status);
  return tail ? `${head} · ${tail}` : head;
}

/* 「进行中 (Ns)」秒数徽标 —— 照 CLI 的 `running (Ns)`（2026-09-19）
 *
 * CLI 原文（`crates/tui/src/tui/history.rs:2647-2658`）：
 *   *"Below 3s the badge is suppressed to avoid visual churn for tools that resolve in
 *   milliseconds; at 3s and beyond the badge appears and **ticks every second** the tool
 *   stays in flight."*
 * ⚠️ 那个秒数**不是**引擎推的：CLI 用的是**本地 `Instant::now()`**。
 *   引擎确实有 `ToolCallHeartbeat`（每 10 秒一次，`core/engine/tool_execution.rs:55`），
 *   但它的定义注释明确写着它 *"carries no output and must not change user-visible status or
 *   the transcript"*、只用于内部 stale 判护（`core/events.rs:218-224`）—— 所以这里**不碰引擎**，
 *   前端自己算（也正因此刷新页面/换标签页不会错位）。
 */
export const RUNNING_BADGE_AFTER_SECS = 3;

export function runningElapsedSuffix(item) {
  // 只给「还在跑」的工具卡加徽标；已完成、状态句、消息不进这里。
  // 照 CLI `bash running (3s)`（`history.rs:2654`）。思考卡不用它 —— CLI 的 reasoning
  // 只在**完成后**拼时长（`… reasoning done · 521ms`），进行中只有 `live`。
  if (!item || item.status !== "in_progress" || item.kind === "status") return "";
  const started = Date.parse(item.started_at || "");
  if (!Number.isFinite(started)) return "";
  const secs = Math.max(0, Math.round((Date.now() - started) / 1000));
  return secs < RUNNING_BADGE_AFTER_SECS ? "" : ` (${secs}s)`;
}

/** 完成后的一次性耗时 —— 照 CLI 的 `… reasoning done · 521ms`
 *  （`tui/history/thinking.rs:162-169` 把 `duration_secs` 拼在头部）。
 *  数据不用引擎给：item 自带 `started_at` / `ended_at`。 */
export function completedElapsedSuffix(item) {
  if (!item || item.status !== "completed") return "";
  const started = Date.parse(item.started_at || "");
  const ended = Date.parse(item.ended_at || "");
  if (!Number.isFinite(started) || !Number.isFinite(ended) || ended <= started) return "";
  return " · " + formatElapsedMs(ended - started);
}

/** 耗时格式 —— 照官方 `format_elapsed_ms`（`elapsed.rs:35-43`）：不足 1 秒用 ms，否则用秒。 */
export function formatElapsedMs(ms) {
  const n = Math.max(0, Math.round(Number(ms) || 0));
  if (n < 1000) return `${n}ms`;
  const secs = Math.round(n / 1000);
  if (secs < 60) return `${secs}s`;
  return `${Math.floor(secs / 60)}m ${secs % 60}s`;
}

/* 思考卡「收起时的预览行数」——照官方 `thinking_preview_lines` 的默认值。
 * 官方描述原文：*"Collapsed completed-thought preview rows (default 2, 0=header-only, 10=older dump)"*
 * ⚠️ 这个键**不在 `/v1/config` 的读/写名单里**（只活在 TUI 的 /config 与 settings.toml），
 *   网页读不到 ⇒ 先写官方默认值 2（＝老板 2026-09-19 的要求「默认为我现在的设置」，
 *   而他那份 settings.toml 里正是 `thinking_preview_lines = 2`）。
 *   哪天引擎把它暴露到 `/v1/config`，改成读值即可。 */
export const THINKING_PREVIEW_LINES = 2;

/* 文件改动卡的内联 diff —— 照官方 `inline_diffs`（默认 `full`）。
 * 官方定义（`settings.rs:33-41`）：
 *   Full（默认）= *"Show a bounded red/green unified diff plus semantic change statistics"*
 *   Summary      = 只给语义统计（+A -D）   Off = 只留结果行
 * 界限照 `MAX_INLINE_DIFF_LINES = 14`（`tui/history/file_mutation.rs:20`）：
 *   官方只露前 14 行，省下的给一行提示（文件改动页那一段）。 */
export const MAX_INLINE_DIFF_LINES = 14;

export function boundedDiffLines(diffText, limit = MAX_INLINE_DIFF_LINES) {
  const lines = String(diffText || "").replace(/\r\n?/g, "\n").split("\n");
  while (lines.length && lines[lines.length - 1] === "") lines.pop();
  if (lines.length <= limit) return { lines, omitted: 0 };
  return { lines: lines.slice(0, limit), omitted: lines.length - limit };
}

/* ── AsBudy：工具卡「默认可见的正文预览」（2026-09-19 · 照官方 CLI 的两档）──────────
 * 官方**明细关**（出厂默认）时卡里是露正文的：`tui/history.rs:469-480` 把整张卡
 * `truncate(TOOL_SUMMARY_CARD_LINES = 6)`，再补一行展开提示 —— 也就是
 * *"头部 + 最多 4 行内容 + 展开提示"*（源码注释原文 *"enough to answer 'what did that
 * do?' without opening anything"*）。而**失败卡完全不吃这套**（那个分支写着 `!cell.is_failed()`），
 * 它走完整的 `TOOL_OUTPUT_LINE_LIMIT = 20` 预算。
 * 我们以前把正文全收紧在折块里 ⇒ 默认档下客户**一个字都看不到**（比官方少）—— 这就是补的那块。 */
export const OUT_PREVIEW_SUCCESS_LINES = 4;   // 官方 TOOL_SUMMARY_CARD_LINES(6) − 头部 − 展开提示

/** 官方失败卡的预算（`TOOL_OUTPUT_LINE_LIMIT`）—— 失败卡不做小预览 */
export const OUT_PREVIEW_FAILED_LINES = 20;

/* 头/尾预算与重要性排序 —— 逐条照 `tui/history/tool_output.rs`：
 *   `selected_output_indices`（:552）的 `head = min(TOOL_OUTPUT_HEAD_LINES=10, line_limit)`、
 *   `tail = min(TOOL_OUTPUT_TAIL_LINES=6, line_limit-head)`，中间按 `output_importance_rank`（:596）挑，
 *   剩下的预算再从 head 往下补满（:582 那段注释解释了为什么不补会“少显几行却声称 omitted”）。
 * ⚠️ 正因为 `head = min(10, line_limit)`，**预算小于 10 行时这个算法退化成「取前 N 行」** ——
 *    所以只有失败卡那 20 行预算才真用得上它（成功卡那 4 行不是它的用武之地，也不强求）。 */
const OUT_PREVIEW_HEAD_LINES = 10;
const OUT_PREVIEW_TAIL_LINES = 6;

/** 照官方 `output_importance_rank`（`tool_output.rs:596`）：error → 0 · warning → 1 · 带路径/URL → 2 · 其余 null */
export function outputImportanceRank(line) {
  const lower = String(line || "").toLowerCase();
  const ERRORS = ["error", "failed", "failure", "fatal", "panic", "exception", "traceback",
    "denied", "not found", "no such file", "cannot", "can't"];
  if (ERRORS.some((needle) => lower.includes(needle))) return 0;
  if (lower.includes("warning") || lower.includes("warn")) return 1;
  if (isPathOrUrlLike(line)) return 2;
  return null;
}

/** 照官方 `is_path_or_url_like`（`tool_output.rs:619`） */
function isPathOrUrlLike(line) {
  const trimmed = String(line || "").trim();
  if (trimmed.includes("://") || trimmed.startsWith("file:")) return true;
  const hasSeparator = trimmed.includes("/") || trimmed.includes("\\");
  const hasExtension = trimmed.split(/\s+/).some((part) => {
    const dot = part.lastIndexOf(".");
    return dot > 0 && part.length - dot - 1 <= 8 && part.length - dot - 1 > 0;
  });
  return hasSeparator && hasExtension;
}

/**
 * 选取要展示的行（照官方 `selected_output_indices`）。
 * 返回 `{ parts, omitted }`：`parts` 是按**原顺序**排的段（`{text}` 或 `{omitted:N}`，
 * 后者是省略提示的位置），渲染时逐段建节点即可。
 */
export function selectedOutputLines(lines, limit) {
  const total = lines.length;
  if (limit <= 0) return { parts: [], omitted: total };
  if (total <= limit) return { parts: lines.map((text) => ({ text })), omitted: 0 };

  const head = Math.min(OUT_PREVIEW_HEAD_LINES, limit, total);
  const tail = Math.min(OUT_PREVIEW_TAIL_LINES, Math.max(0, limit - head), Math.max(0, total - head));
  const keep = new Set();
  for (let i = 0; i < head; i += 1) keep.add(i);
  for (let i = total - tail; i < total; i += 1) keep.add(i);

  const budget = limit - keep.size;
  if (budget > 0) {
    const important = [];
    for (let i = head; i < total - tail; i += 1) {
      const rank = outputImportanceRank(lines[i]);
      if (rank !== null) important.push([i, rank]);
    }
    important.sort((a, b) => (a[1] - b[1]) || (a[0] - b[0]));
    for (const [idx] of important.slice(0, budget)) keep.add(idx);
  }
  // 还有剩的预算 → 从 head 往下补满（照官方那段“别白白 forfeit 预算”的处理）
  let next = head;
  const cap = Math.min(limit, total);
  while (keep.size < cap && next < total) {
    keep.add(next);
    next += 1;
  }

  const idx = [...keep].sort((a, b) => a - b);
  const parts = [];
  let prev = -1;
  for (const i of idx) {
    const gap = i - prev - 1;
    if (gap > 0) parts.push({ omitted: gap });
    parts.push({ text: lines[i] });
    prev = i;
  }
  if (prev < total - 1) parts.push({ omitted: total - 1 - prev });
  return { parts, omitted: total - idx.length };
}

/** diff 行的语义分类（决定红/绿/灰）—— 只认无歧义的行首 */
export function diffLineKind(line) {
  const t = String(line || "");
  if (t.startsWith("+++") || t.startsWith("---")) return "meta";
  if (t.startsWith("@@")) return "hunk";
  if (t.startsWith("+")) return "add";
  if (t.startsWith("-")) return "del";
  return "";
}

/** 增/删行数（照官方的 semantic change statistics）——`inline_diffs=summary` 时只显示这行 */
export function diffStats(diffText) {
  let add = 0;
  let del = 0;
  for (const line of String(diffText || "").split("\n")) {
    if (line.startsWith("+++") || line.startsWith("---")) continue;   // 文件头不算
    if (line.startsWith("+")) add += 1;
    else if (line.startsWith("-")) del += 1;
  }
  return { add, del };
}

/** 思考预览行数：优先读界面上设的值（`data-ab-thinklines`），读不到用官方默认。 */
export function thinkingPreviewLines() {
  if (typeof document === "undefined" || !document.documentElement) return THINKING_PREVIEW_LINES;
  const raw = document.documentElement.getAttribute("data-ab-thinklines");
  if (raw === null || raw === "") return THINKING_PREVIEW_LINES;
  const n = Number.parseInt(raw, 10);
  return Number.isFinite(n) && n >= 0 ? n : THINKING_PREVIEW_LINES;
}

/** 路径只留最后两段 —— 别把 /opt/asbudy/customers/yanyijin/2dry8u/public/index.html 整条糊到界面上 */
function shortPathZh(value) {
  const parts = String(value || "").split("/").filter(Boolean);
  return parts.length <= 2 ? String(value || "") : parts.slice(-2).join("/");
}

/** 压成一行、超长截断（摘要位置放不下一整条命令） */
function oneLineZh(value, max) {
  const text = String(value || "").replace(/\s+/g, " ").trim();
  return text.length > max ? text.slice(0, max - 1) + "…" : text;
}

/* 文件改动卡的摘要 —— 照官方 `FileMutationSummary::semantic_summary()`
 * （`history/file_mutation.rs:145-155`）：单文件是 `Created src/lib.rs · +30 -0`，
 * 多文件是 `3 files · created 2 · updated 1`（M11 · 2026-09-20）。
 *
 * 为什么要有（老板报的疼点）：我们以前只有工具名猜出来的「写入 xxx」，而且**行数统计埋在
 * diff 块里**（要展开才看得见）—— 官方那行是**卡片摘要**，默认就可见。
 *
 * 数据源是引擎自己给的（不用猜）：`tools/file.rs:611` 的形状
 * `metadata.mutation = {diff, files:[{path, outcome}], renames:[]}`；
 * `outcome` 就是引擎判定的 created / updated / deleted / renamed。
 * ⚠️ 官方动作词（Created/Updated/…）在**中文语言包里没有**（0 命中）⇒ 中文是我们写的。
 * ⚠️ 从 `outcome` 取而不是从工具名猜：`apply_patch` 可能一次改/删/新建好几个文件，
 *   单看工具名说不准（官方的 outcome_label 也是读 outcome）。 */
export function mutationSummaryZh(item) {
  const meta = item && item.metadata && typeof item.metadata === "object" ? item.metadata : {};
  const mut = meta.mutation && typeof meta.mutation === "object" ? meta.mutation : null;
  if (!mut) return null;
  const files = Array.isArray(mut.files) ? mut.files : [];
  const stats = diffStats(typeof mut.diff === "string" ? mut.diff : "");
  const statsText = "+" + stats.add + " -" + stats.del;
  const outcomeZh = { created: "新建", updated: "修改", deleted: "删除", renamed: "重命名" };
  if (!files.length) return "改动文件 · " + statsText;
  if (files.length === 1) {
    const file = files[0] || {};
    const verb = outcomeZh[String(file.outcome || "").toLowerCase()] || "改动";
    const path = shortPathZh(String(file.path || ""));
    return (path ? verb + " " + path : verb + "文件") + " · " + statsText;
  }
  const counts = {};
  files.forEach(function (file) {
    const outcome = String((file && file.outcome) || "").toLowerCase();
    if (outcomeZh[outcome]) counts[outcome] = (counts[outcome] || 0) + 1;
  });
  const parts = Object.keys(outcomeZh)
    .filter(function (outcome) { return counts[outcome]; })
    .map(function (outcome) { return outcomeZh[outcome] + " " + counts[outcome]; });
  return files.length + " 个文件" + (parts.length ? " · " + parts.join(" · ") : "") + " · " + statsText;
}

/** 从工具入参里拼一句「做了什么」；拼不出来返回 null（调用方回退到原文） */
function toolIntentZh(toolName, rawInput) {
  let input = null;
  try {
    const parsed = JSON.parse(String(rawInput || ""));
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) input = parsed;
  } catch (_error) { /* 不是 JSON 就没有可读入参 */ }
  if (!input) return null;
  const command = input.command || input.cmd;
  if (command) return "执行命令：" + oneLineZh(command, 96);
  const path = input.path || input.file_path || input.target || input.filename;
  if (!path) return null;
  const verb = { write: "写入 ", edit: "修改 ", read: "读取 ", create: "新建 ", delete: "删除 " }[toolName] || "处理 ";
  return verb + shortPathZh(path);
}

/* 引擎硬编码的英文状态句 → 中文（2026-09-18 老板：「全程没有任何中文」）
 *
 * ⚠️ 这些句子写在**引擎的 Rust 源码**里（`core/engine/turn_loop.rs` 等，全仓库十来句），
 *    **不是配置项** —— 官方 TUI 显示一模一样的英文。这里只在**显示层**译一道。
 * 失败模式是安全的：引擎哪天改了句子 / 换了新句，这里匹配不上就**退回英文原文**，不会崩。
 * 重核方法：`grep -rn 'Event::status("' crates/tui/src/`。
 */
const ENGINE_STATUS_ZH = [
  // 时态跟着卡片状态走：这句引擎原文是**开始检查时**发的（"checking"），
  //   而这张状态卡通常已经 completed —— 一律写成「正在检查」会读成「完成 + 正在」自相矛盾。
  [/^Auto-Review checking '(.+?)'$/, (m, status) => `${status === "completed" ? "已检查" : "正在检查"}这一步（${m[1]}）`],
  [/^Continuing — tool results$/, () => "拿到结果，继续"],
  [/^Continuing — queued steer input$/, () => "继续（处理你插的话）"],
  [/^Session context synced$/, () => "会话已同步"],
  [/^Reconnecting…$/, () => "正在重新连接"],
  [/^Request cancelled$/, () => "请求已取消"],
  [/^Request was Paused$/, () => "请求已暂停"],
  [/^Compaction settings updated$/, () => "整理设置已更新"],
  [/^Goal set; starting goal work\.$/, () => "目标已设定，开始执行"],
];

function engineStatusZh(text, status) {
  const raw = String(text || "").trim();
  if (!raw) return null;
  for (const [pattern, to] of ENGINE_STATUS_ZH) {
    const matched = raw.match(pattern);
    if (matched) return typeof to === "function" ? to(matched, status) : to;
  }
  return null;
}

/* 工具回执的「形态」分类（2026-09-18 · 老板：「CLI 的工具卡是按类型分开渲染的，直接做了吧」）
 *
 * CLI 那边是 9 种 ToolCell（Exec / Exploring / PatchSummary / PlanUpdate / Review / Mcp /
 * ViewImage / WebSearch / Generic，见 crates/tui/src/tui/history.rs 与 tool_routing.rs），
 * 各画各的 —— 所以一眼看得出「这是在跑命令还是在改文件」。web 只有一种通用卡片。
 * 这里做**形态区分**（`data-variant` + 一枚小徽标），样式在 asbudy-my.js 注入：
 *   exec    跑命令（等宽、带耗时/退出码）   file    改写文件（路径突出）
 *   explore 查看/搜索（降噪）               web     联网检索
 *   mcp     MCP 工具                        plan    计划/清单（高亮）
 *   status  引擎节拍（最淡）                generic 其余
 */
/* ── 工具族（2026-09-20 搬 · 照 CLI 的 `widgets/tool_card.rs`）─────────────────
 * CLI 每条工具卡的头部是「族字形 + 族标签 + 细节」（`family_glyph` / `family_label`），十族：
 *   `Read ▷` `Patch ◆` `Run ▶` `Find ⌕` `Delegate ◐` `Fanout ⋮` `Rlm ⋮` `Verify ✓` `Think …` `Generic •`
 * 我们以前**只有颜色圆点** —— 一眼分不出是哪种活。
 *
 * ⚠️ **名字要两步才认得出族**（2026-09-20 实测踩到的坑）：
 *   v0.9.13 起引擎发的是**家族名 + action**（实测拿到的就是 `bash` / `agent`），
 *   而官方的族表（`tool_family_for_name`）认的是**旧别名**（`exec_shell` / `read_file`…）。
 *   桥梁是 `tools/canonical_action.rs` 的 `CANONICAL_ACTION_ALIASES`，
 *   ＋它上面的 `action_family_default`（家族缺 action 时的默认）。
 *   ⇒ **少这一步，所有族都会落成「其它」**。
 *   官方表里还有 `tasks` / `automation` / `github` 三个家族 —— 它们的别名**不在**族表里
 *   （落 Generic），所以这儿只抄**能影响族**的那些行。
 * 族命名照 `tool_card.rs:80-108` 的 `tool_family_for_name`，字形照 `family_glyph`（:282-293）。
 */
const CANONICAL_ACTION_ALIASES = {
  bash: { run: 'exec_shell', wait: 'exec_shell_wait', interact: 'exec_shell_interact', cancel: 'exec_shell_cancel' },
  Bash: { run: 'exec_shell', wait: 'exec_shell_wait', interact: 'exec_shell_interact', cancel: 'exec_shell_cancel' },
  File: {
    read: 'read_file', list: 'list_dir', search_name: 'file_search',
    search_content: 'grep_files', write: 'write_file', edit: 'edit_file', patch: 'apply_patch',
  },
  Git: {
    status: 'git_status', diff: 'git_diff', log: 'git_log',
    show: 'git_show', blame: 'git_blame', commit_plan: 'git_commit_plan',
  },
  Run: { tests: 'run_tests', verifiers: 'run_verifiers' },
  Web: { search: 'web_search', fetch: 'fetch_url', wait: 'wait_for_dev_server' },
  rlm: { open: 'rlm_open', eval: 'rlm_eval', configure: 'rlm_configure', close: 'rlm_close' },
};
/** 家族缺 `action` 时官方怎么默认（`canonical_action.rs:108-121`）；rlm/tasks/automation/github 无默认 */
const ACTION_FAMILY_DEFAULT = { bash: 'run', Bash: 'run', File: 'read', Git: 'status', Run: 'tests', Web: 'search' };
const TOOL_FAMILY_BY_NAME = {
  read_file: 'read', list_dir: 'read', view_image: 'read', git_status: 'read', git_diff: 'read',
  git_log: 'read', git_show: 'read', git_blame: 'read', git_commit_plan: 'read',
  edit_file: 'patch', apply_patch: 'patch', write_file: 'patch',
  exec_shell: 'run', exec_shell_wait: 'run', exec_shell_interact: 'run', exec_shell_cancel: 'run',
  task_shell_start: 'run', task_shell_wait: 'run', start_registry_mcp_server: 'run',
  grep_files: 'find', file_search: 'find', web_search: 'find', fetch_url: 'find', registry_sync: 'find',
  agent: 'delegate',
  rlm_open: 'rlm', rlm_eval: 'rlm', rlm_configure: 'rlm', rlm_close: 'rlm', rlm: 'rlm',
  run_tests: 'verify', run_verifiers: 'verify', task_gate_run: 'verify', validate_data: 'verify',
  wait_for_dev_server: 'verify',
  workflow: 'fanout',
};
const TOOL_FAMILY_GLYPH = {
  read: "\u25B7", patch: "\u25C6", run: "\u25B6", find: "\u2315", delegate: "\u25D0",
  fanout: "\u22EE", rlm: "\u22EE", verify: "\u2713", think: "\u2026", generic: "\u2022",
};

/** 这条 item 的**官方工具名**（canonical 别名）—— 族判定与「运行汇总」的公共前置。
 *  ⚠️ 为什么单独抽出来（2026-09-20 做 M10 时）：官方 `tool_run.rs` 的
 *  `generic_tool_name_is_collapse_guard` 判的是**canonical 名**（`read_file` / `apply_patch`…），
 *  **不是族名** —— 族名把「读」和「写」分到不同族、判不出「写改类不折叠」这条规则。
 *  空串 = 这条 item 不是工具调用（正文 / 思考 / 子代理等）。 */
export function toolAliasOf(item) {
  const meta = item && item.metadata && typeof item.metadata === "object" ? item.metadata : {};
  const raw = String(meta.tool_name || meta.tool || "").trim();
  if (!raw) return "";
  let alias = raw;
  const family = CANONICAL_ACTION_ALIASES[raw];
  if (family) {
    let input = {};
    const rawInput = meta.tool_input;
    if (typeof rawInput === "string" && rawInput) {
      try { input = JSON.parse(rawInput) || {}; } catch (e) { input = {}; }
    } else if (rawInput && typeof rawInput === "object") {
      input = rawInput;
    }
    const action = String(input.action || ACTION_FAMILY_DEFAULT[raw] || "");
    if (action && family[action]) alias = family[action];
  }
  return alias;
}

/** 这条 item 属于哪个工具族 —— 两步：家族+action → 官方别名 → 族名。
 *  调不出来时返回 `generic`（不是空 —— 卡片总得有个字形）。 */
export function toolFamilyOf(item) {
  const alias = toolAliasOf(item);
  if (!alias) return "";
  return TOOL_FAMILY_BY_NAME[alias] || "generic";
}

/* ── 运行汇总行（M10 · 2026-09-20 搬 · 照 CLI `tui/history/tool_run.rs`）─────────────
 * 官方在转录里把**连续的成功、低风险**工具卡合成一行，因为「例行的读文件 / 跑校验」
 * 不该占满默认视图 —— 注释原文：
 *   *"...keeps their raw details available without making routine verifier/shell work
 *   dominate the default transcript."*
 *
 * 三条**必须照搬**的规则（逐条读源码拿到的）：
 *   ① **阈值 = 3**（`tui/app/init.rs:901` 的 `tool_collapse_threshold: 3`）—— 连着 3 个才折。
 *   ② **什么会把运行切断**（`is_collapsible_tool_cell` ＝ `tool.is_success() && !tool.is_collapsible_guard()`）：
 *      **没跑完的 / 失败的 / 写改删提交评审类**一律不折、并把运行切开 —— 注释原文
 *      *"Failed, running, patch, review, diff, and plan-update cells split runs so important
 *      state never disappears into a summary row."*
 *      ⚠️ 但**注释与代码不一致**：那句里提了 `diff`，可 guard 词表（`contains` 那串）**没有 diff**
 *      ⇒ 名字含 "diff" 的工具**实际可折**、且计入 edits 桶。我们**照代码、不照注释**
 *      （将来官方把 diff 加进守卫，这一段跟着改 —— 回归里有断言盯着这条差异）。
 *      ⚠️ **另一条容易被漏的**：思考卡（`agent_reasoning`）不是工具 → **它也会把运行切断**，
 *      所以真实的「思考 → 工具 → 思考 → 工具」会话**根本不会成组**（实测 2026-09-20 确认，
 *      不是 bug）。M10 只在「真的连着几个工具、中间没思考」时才触发。
 *   ③ **计数口径**（`classify_tool_name_activity`，表与顺序逐行照抄）。
 *
 * ⚠️ 两个**有意偏离**（写在这儿，免得以后被当成错）：
 *   · 官方汇总带家族名（`Explored 3 files: read, grep`）—— 那是**英文工具名**，对中文客户没意义，
 *     我们只给数量；而且官方**没有中文文案**（语言包里 0 命中）⇒ 中文是我们写的。
 *   · 官方这套还受 settings 的 `tool_collapse_mode` 管（compact / expanded / calm）——
 *     那个键**不在引擎 `POST /v1/config` 白名单里**，网页端改不了 ⇒ 我们照官方默认 `compact`
 *     （总是折）。将来若引擎露出这个键，就把它接到高级设置里（别自己造一个开关）。
 */
export const TOOL_RUN_MIN = 3;
const COLLAPSE_GUARD_WORDS = ["patch", "write", "edit", "delete", "remove", "commit", "push", "review"];
const METADATA_TOOL_NAMES = [
  "update_plan", "work_update", "todo_write", "todo_add", "todo_update",
  "checklist_write", "checklist_add", "checklist_update", "checklist_list",
];
/** 精确表：名字 → 计数桶（照 `tool_run.rs:166-210` 的 match 臂） */
const RUN_ACTIVITY_BY_NAME = {
  read_file: "files", list_dir: "files", view_image: "files", explore: "files",
  git_status: "files", git_diff: "files", git_log: "files", git_show: "files",
  git_blame: "files", git_commit_plan: "files",
  grep_files: "searches", file_search: "searches", web_search: "searches",
  fetch_url: "searches", registry_sync: "searches",
  exec_shell: "commands", exec_shell_wait: "commands", exec_shell_interact: "commands",
  exec_shell_cancel: "commands", task_shell_start: "commands", task_shell_wait: "commands",
  start_registry_mcp_server: "commands", run_tests: "commands", run_verifiers: "commands",
  wait_for_dev_server: "commands", task_gate_run: "commands", validate_data: "commands",
  edit_file: "edits", apply_patch: "edits", write_file: "edits", diff: "edits",
  agent: "delegates", rlm_open: "delegates", rlm_eval: "delegates",
  rlm_configure: "delegates", rlm_close: "delegates", rlm: "delegates",
};

/** 是不是「元数据类」工具（待办 / 计划更新）—— 官方先拿它把 guard 判据排除掉 */
export function isMetadataToolName(name) {
  return METADATA_TOOL_NAMES.indexOf(String(name || "").trim().toLowerCase()) >= 0;
}

/** 写改类「永不折叠」的守卫 —— 官方 `generic_tool_name_is_collapse_guard` 逐字对应。
 *  客户必须看得见「它动了哪个文件」，这类卡片不能被汇总行吃掉。 */
export function isCollapseGuard(name) {
  const normalized = String(name || "").trim().toLowerCase();
  if (isMetadataToolName(normalized)) return false;
  return COLLAPSE_GUARD_WORDS.some(function (word) { return normalized.indexOf(word) >= 0; });
}

/** 这个工具算进哪个计数桶（照官方 `classify_tool_name_activity` 的表与**顺序**） */
export function runActivityOf(name) {
  const n = String(name || "").trim().toLowerCase();
  if (RUN_ACTIVITY_BY_NAME[n]) return RUN_ACTIVITY_BY_NAME[n];
  if (isMetadataToolName(n)) return "metadata";
  if (n.indexOf("search") >= 0 || n.indexOf("grep") >= 0 || n.indexOf("find") >= 0) return "searches";
  if (n.indexOf("read") >= 0 || n.indexOf("list") >= 0 || n.indexOf("view") >= 0 || n.indexOf("open") >= 0) return "files";
  if (n.indexOf("patch") >= 0 || n.indexOf("write") >= 0 || n.indexOf("edit") >= 0 || n.indexOf("diff") >= 0) return "edits";
  if (n.indexOf("run") >= 0 || n.indexOf("exec") >= 0 || n.indexOf("shell") >= 0 || n.indexOf("test") >= 0 || n.indexOf("check") >= 0) return "commands";
  if (n.indexOf("agent") >= 0 || n.indexOf("delegate") >= 0 || n.indexOf("fanout") >= 0 || n.indexOf("rlm") >= 0) return "delegates";
  if (n.indexOf("metadata") >= 0 || n.indexOf("session") >= 0 || n.indexOf("context") >= 0 || n.indexOf("plan") >= 0 || n.indexOf("todo") >= 0) return "metadata";
  return "other";
}

function isCollapsibleRunCell(cell) {
  return !!(cell && cell.ok === true && !isCollapseGuard(cell.alias));
}

/** 找出「可以折成一行」的连续运行组。
 *  `cells` 与转录同序：每个元素是 `{alias, ok}`，**不是工具调用的位置传 `null`**
 *  （正文 / 思考 / 子代理 / 还在跑的那张卡 —— 它们本来就该把运行切开）。 */
export function detectToolRuns(cells, minSize) {
  const runs = [];
  if (!Array.isArray(cells) || !minSize) return runs;
  let index = 0;
  while (index < cells.length) {
    if (!isCollapsibleRunCell(cells[index])) { index += 1; continue; }
    const start = index;
    const activity = { files: 0, searches: 0, commands: 0, edits: 0, delegates: 0, metadata: 0, other: 0 };
    while (index < cells.length && isCollapsibleRunCell(cells[index])) {
      activity[runActivityOf(cells[index].alias)] += 1;
      index += 1;
    }
    const count = index - start;
    if (count >= minSize) runs.push(Object.assign({ start: start, count: count }, activity));
  }
  return runs;
}

/** 汇总行上的那句话（中文 —— 官方只有英文 `Explored 3 files, 2 searches`）。
 *  分段与顺序照官方 `tool_run_summary`：文件/搜索 → 命令 → 改动 → 派发 → 元数据。 */
export function toolRunSummaryText(run) {
  const parts = [];
  if (run.files) parts.push("查看了 " + run.files + " 个文件");
  if (run.searches) parts.push("搜索了 " + run.searches + " 次");
  if (run.commands) parts.push("执行了 " + run.commands + " 条命令");
  if (run.edits) parts.push("改动了 " + run.edits + " 个文件");
  if (run.delegates) parts.push("派出 " + run.delegates + " 个子代理");
  if (run.metadata || run.other) parts.push("更新了进度");
  return parts.length ? parts.join("、") : "完成了 " + run.count + " 步";
}

/* ── 清单 / 待办卡（M12 · 2026-09-20 搬 · 照 CLI `tui/history/checklist.rs`）──────────
 * AI 用待办清单干活时，官方给它一张**专门的卡**：卡头 `3/7 · 43%`，下面每项一个符号
 * （`☑ ◐ ✗ ⊘ ☐`）。
 *
 * ⚠️ **不搬就出事（实测 2026-09-20）**：引擎把 `todo_write` 的 item 标成 `kind=file_change`，
 *   于是我们把它画成了「文件改动 · 完成」卡，**摘要里是一坨带换行的原始 JSON**：
 *   `todo_write: Todo list updated (3 items, 0% settled)\n{\n  "items": [\n    {\n  ...` —— 客户看到的就是这个。
 *
 * ⚠️ **用哪个字段（实测确认，别改用 summary）**：
 *   · `summary` 被引擎**截到 280 字**（这份 3 项清单已经被切掉结尾）
 *   · `detail` 是**完整**的（同一个 item：348~369 字，JSON 完整可解析）
 *   官方是从**完整输出**里抠 JSON 的（`parse_checklist_snapshot`：先定位第一个 `{` 再 parse）。
 *
 * ⚠️ 两种形状都认（否则客户在它「正在跑」时看不到清单）：
 *   · 跑完那一帧：`Todo list updated (3 items, 33% settled)\n{"items":[{id,content,status}],"completion_pct":33}`
 *   · 刚开始那一帧：`{"todos":[{content,status}]}`（**入参**，没有 id / 百分比）
 */
export const CHECKLIST_TOOL_NAMES = [
  "work_update", "checklist_write", "checklist_add", "checklist_update",
  "todo_write", "todo_add", "todo_update",
];
export function isChecklistToolName(name) {
  return CHECKLIST_TOOL_NAMES.indexOf(String(name || "").trim().toLowerCase()) >= 0;
}

/** 每项的符号 —— 官方 `checklist_status_marker` 逐字对应（`checklist.rs:208-217`） */
export function checklistMarkerOf(status) {
  const s = String(status || "").trim().toLowerCase();
  if (s === "completed" || s === "done") return "\u2611";                        // ☑
  if (s === "in_progress" || s === "inprogress" || s === "running") return "\u25D0"; // ◐
  if (s === "blocked" || s === "failed") return "\u2717";                        // ✗
  if (s === "cancelled" || s === "canceled" || s === "skipped") return "\u2298";  // ⊘
  return "\u2610";                                                                 // ☐ 待办
}

/** 给 CSS 用的桶名（颜色跟符号走） */
export function checklistStatusKind(status) {
  const s = String(status || "").trim().toLowerCase();
  if (s === "completed" || s === "done") return "done";
  if (s === "in_progress" || s === "inprogress" || s === "running") return "doing";
  if (s === "blocked" || s === "failed") return "failed";
  if (s === "cancelled" || s === "canceled" || s === "skipped") return "skipped";
  return "todo";
}

/** 从工具文本里抠出清单快照 —— 照官方 `parse_checklist_snapshot`：
 *  先找第一个 `{`（前面那段是给人看的说明）再 parse；没有 items / 空列表就返回 null。 */
export function parseChecklistSnapshot(text) {
  const raw = String(text || "");
  const start = raw.indexOf("{");
  if (start < 0) return null;
  let parsed = null;
  try { parsed = JSON.parse(raw.slice(start)); } catch (_error) { return null; }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return null;
  const list = Array.isArray(parsed.items) ? parsed.items
    : (Array.isArray(parsed.todos) ? parsed.todos : null);
  if (!list || !list.length) return null;
  const items = [];
  list.forEach(function (entry) {
    const source = entry && typeof entry === "object" ? entry : {};
    const content = String(source.content || "").trim();
    if (!content) return;
    items.push({ content: content, status: String(source.status || "pending") });
  });
  if (!items.length) return null;
  // 官方的 completed 计数**只认 "completed"**（`checklist.rs:72-75`）—— 这里照抄，
  //   不跟符号判定（done 也算 ☑）合并，免得两边对不上时看着像 bug。
  const completed = items.filter(function (it) { return it.status.toLowerCase() === "completed"; }).length;
  const total = items.length;
  let pct = Number(parsed.completion_pct);
  if (!Number.isFinite(pct)) pct = total ? Math.floor((completed * 100) / total) : 0;
  pct = Math.max(0, Math.min(100, Math.round(pct)));
  return { items: items, completed: completed, total: total, pct: pct };
}

/** 卡头那句 `3/7 · 43%`（官方 `header_summary`） */
export function checklistHeadText(snapshot) {
  if (!snapshot) return "";
  return snapshot.completed + "/" + snapshot.total + " \u00B7 " + snapshot.pct + "%";
}

/* ── 子代理卡（2026-09-20 搬 · 照 CLI `widgets/agent_card.rs` 的 DelegateCard）──────
 * 为什么要有：引擎**自己就在发**子代理的全过程 —— `agent` 工具的 tool_call 里带着
 *   `metadata.agent_id` / `status` / `activity` / `steps_taken` / `duration_ms`，
 *   而**官方 web 一个渲染分支都没有**（`delegate`/`fanout`/`subagent` 在 app.mjs 里各 0 次）
 *   ⇒ 客户只看到主 AI 说「我派了个子代理」，后面全黑箱。
 *   2026-09-20 实测（CAPABILITY-MAP §12.10）：子代理在我们平台**开箱就能跑**
 *   （平台默认「自动审核」档下不再被审批门拦住），所以这块值得露出来。
 * 状态名一律照**官方语言包**（`crates/localization/locales/zh-Hans.json`）：
 *   `SubagentsStatus{Running,Cancelled,Completed,Failed,Interrupted}`。
 * 状态取值全集 = `AgentWorkerStatus`（`tools/subagent/mod.rs:570`，10 个）；
 *   **认不出的状态原样显示**，不猜（不发明语义）。
 * 字形照 CLI 的 `dot_grid`（`widgets/agent_card.rs:433-440`）：● ◐ × ⊘ ○ ◌
 */
const AGENT_STATUS_ZH = {
  queued: "排队中",
  starting: "运行中", running: "运行中", model_wait: "运行中", running_tool: "运行中",
  waiting_for_user: "等待中",
  completed: "已完成", failed: "失败", cancelled: "已取消", canceled: "已取消",
  interrupted: "已中断",
};
const AGENT_TERMINAL_STATUS = new Set(["completed", "failed", "cancelled", "canceled", "interrupted"]);
const AGENT_STATUS_GLYPH = {
  queued: "○", waiting_for_user: "○",
  completed: "●", failed: "×", cancelled: "⊘", canceled: "⊘", interrupted: "◌",
};

/** 这条 item 是不是子代理调用 —— 只认 `metadata.tool_name`（引擎给的就是 `agent`）。
 *  ⚠️ 流式早期 metadata 可能还没齐 ⇒ 调用方要把它当**形态判定**，
 *     判变了就重建卡片（见 `updateItemNode` 开头那段）。 */
export function isAgentToolCall(item) {
  if (!item || item.kind !== "tool_call") return false;
  const meta = item.metadata && typeof item.metadata === "object" ? item.metadata : {};
  return String(meta.tool_name || meta.tool || "").toLowerCase() === "agent";
}

/** `agent_6c821769` → `6c821769`（照 CLI 的 `truncate_id`）。
 *  ⚠️ **只剥引擎自己的 `agent_` 前缀** —— 客户给子代理起的名字（`echo_two` 这种）不能被切成 `two`。 */
export function shortAgentId(value) {
  const s = String(value || "").trim();
  return s.startsWith("agent_") ? s.slice(6) : s;
}

/** 从一段 JSON **文本**里抠一个字段（可能被截断——所以不 JSON.parse）。 */
function grabJsonField(text, key) {
  const m = String(text || "").match(new RegExp('"' + key + '"\\s*:\\s*(?:"([^"]*)"|([^,}\\s]+))'));
  return m ? (m[1] !== undefined ? m[1] : m[2]) : "";
}

/** 把子代理的事实抠出来。
 *  **metadata 优先**（可靠，但字段少）；**summary 里的 JSON 兜底** —— 它字段更全
 *  （`steps_taken` / `duration_ms` / `status` 都在里面），但**经常被截到 280 字**
 *  （`child_route` 那坨很长），所以只用正则抠字段，**不 JSON.parse**（截断的 JSON 会直接抛）。
 *  另外还从 `metadata.tool_input` 抠 `agent_id` —— 被守护者拒绝的那条只有 tool_input，没有 agent_id。 */
export function agentCardFacts(item) {
  const meta = item && item.metadata && typeof item.metadata === "object" ? item.metadata : {};
  const raw = String((item && (item.summary || item.detail)) || "");
  const input = String(meta.tool_input || "");
  const facts = {
    agentId: String(meta.agent_id || "").trim() || grabJsonField(raw, "agent_id") || grabJsonField(input, "agent_id"),
    name: String(meta.name || "").trim() || grabJsonField(raw, "name"),
    action: String(meta.action || "").trim(),
    status: String(meta.status || "").trim().toLowerCase() || grabJsonField(raw, "status").toLowerCase(),
    steps: Number(meta.steps_taken) || Number(grabJsonField(raw, "steps_taken")) || 0,
    durationMs: Number(meta.duration_ms) || Number(grabJsonField(raw, "duration_ms")) || 0,
    activity: String(meta.activity || "").trim(),
    terminal: meta.terminal === true,
    failed: false,
    failReason: "",
  };
  // 被守护者拒绝时会走这条摘要（原文形如 `agent failed: Failed to authorize tool execution: ...`）
  const failMatch = raw.match(/^agent\s+failed:\s*([\s\S]+)$/i);
  if (failMatch) {
    facts.failed = true;
    facts.failReason = failMatch[1].trim();
  }
  if (!facts.failed && (item.status === "failed" || meta.is_error === true)) facts.failed = true;
  facts.terminal = facts.terminal || AGENT_TERMINAL_STATUS.has(facts.status) || facts.failed;
  if (facts.failed && !facts.status) facts.status = "failed";
  return facts;
}

function receiptVariant(item, toolName) {
  const n = String(toolName || "").toLowerCase();
  const kind = String(item.kind || "");
  if (kind === "status") return "status";
  if (kind === "file_change") return "file";
  if (kind === "command_execution") return "exec";
  if (/^(write|edit|create|apply_patch|delete|mkdir|move|copy)$/.test(n)) return "file";
  if (/^(read|ls|glob|grep|list_dir|list_files|find|cat)$/.test(n)) return "explore";
  if (/^(bash|shell|exec|sh)$/.test(n) || n.startsWith("exec_")) return "exec";
  if (n.includes("search") || n === "fetch" || n === "web") return "web";
  if (n.startsWith("mcp")) return "mcp";
  if (/plan|todo|checklist/.test(n)) return "plan";
  return "generic";
}

/** 回执徽标：只放客户真用得上的（耗时 / 非零退出码），没有就空着 */
function receiptMetaZh(item) {
  const meta = item.metadata && typeof item.metadata === "object" ? item.metadata : {};
  const parts = [];
  const duration = Number(meta.duration_ms);
  if (Number.isFinite(duration) && duration > 0) {
    parts.push(duration >= 1000 ? (duration / 1000).toFixed(1) + "s" : Math.round(duration) + "ms");
  }
  const code = meta.exit_code;
  if (code !== undefined && code !== null && Number(code) !== 0) parts.push("退出码 " + code);
  return parts.join(" · ");
}

/** 给任何一条回执补齐 variant / meta（MCP、工作流这些早返分支也要带上） */
function finishReceipt(presentation, item) {
  const meta = item.metadata && typeof item.metadata === "object" ? item.metadata : {};
  if (!presentation.variant) {
    presentation.variant = receiptVariant(item, String(meta.tool_name || meta.tool || ""));
  }
  if (presentation.meta === undefined) presentation.meta = receiptMetaZh(item);
  return presentation;
}

export function receiptPresentation(item = {}) {
  const detail = String(item.detail || item.summary || "");
  const raw = String(item.summary || detail || humanize(item.kind));
  const fullRaw = detail && detail !== raw ? `${raw}\n\n${detail}` : raw;
  const workflow = workflowReceiptPresentation(item, detail, fullRaw);
  if (workflow) return finishReceipt(workflow, item);
  const mcpFailure = raw.match(/Failed to connect MCP server ['"]?([^'":\s]+)['"]?/i);
  if (mcpFailure) {
    const server = mcpFailure[1] || "server";
    return finishReceipt({
      label: "MCP · 不可用",
      summary: `${server} 连不上`,
      raw,
      failed: true,
      variant: "mcp",
    }, item);
  }
  const failed = item.status === "failed" || /^(?:error|failed|failure)\b/i.test(raw);
  // AsBudy：先给「做了什么」；失败时把「未完成」缀在后面（具体原因仍在 raw / 折叠里）
  const meta = item.metadata && typeof item.metadata === "object" ? item.metadata : {};
  const toolName = String(meta.tool_name || meta.tool || "");
  const intent = toolIntentZh(toolName, meta.tool_input);
  // 进度卡是引擎自己发的状态句（英文硬编码）→ 显示层译一道；译不出就用原文
  const statusZh = item.kind === "status" ? engineStatusZh(raw, item.status) : null;
  // 文件改动的内联 diff（引擎早就带着：metadata.mutation.diff）
  const mutation = meta.mutation && typeof meta.mutation === "object" ? meta.mutation : null;
  const diffText = mutation && typeof mutation.diff === "string" ? mutation.diff : "";
  // M11（2026-09-20）：文件改动卡用官方那行「动作 + 路径 · +N -M」当摘要（**默认可见**）——
  //   优先于从工具名猜的「写入 xxx」：引擎给的 `outcome` 更准（能说出新建/删除/重命名），
  //   而且行数以前只活在 diff 块里（要展开才看得到）。
  const mutSummary = mutationSummaryZh(item);
  const summaryText = mutSummary || intent;
  return {
    label: receiptLabelZh(item.kind, item.status),
    summary: summaryText ? (failed ? `${summaryText} —— 未完成` : summaryText) : (statusZh || raw),
    raw: fullRaw,
    failed,
    variant: receiptVariant(item, toolName),
    meta: receiptMetaZh(item),
    diff: diffText,
  };
}

export function workflowReceiptPresentation(item, detail, raw) {
  const metadata = item?.metadata && typeof item.metadata === "object"
    ? item.metadata
    : {};
  let payload = null;
  try {
    const parsed = JSON.parse(detail);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) payload = parsed;
  } catch (_error) {
    // Non-JSON tool receipts continue through the ordinary presentation path.
  }
  const looksLikeWorkflow = /^workflow(?:\s|:)/i.test(String(item?.summary || ""))
    || Object.hasOwn(metadata, "dispatch_failure_count")
    || Boolean(payload && Object.hasOwn(payload, "dispatch_failure_count"));
  if (!looksLikeWorkflow) return null;

  const status = String(metadata.status || payload?.status || "").toLowerCase();
  const countValue = metadata.dispatch_failure_count ?? payload?.dispatch_failure_count;
  const count = Number(countValue);
  const rejected = Number.isSafeInteger(count) && count > 0 ? count : 0;
  if (rejected === 0 && status !== "degraded" && status !== "failed") return null;

  const summary = rejected === 1
    ? "有 1 个任务派发被拒绝了"
    : rejected > 1
      ? `${rejected} 个任务派发被拒绝`
      : status === "failed"
        ? "工作流未完成"
        : "工作流完成，但结果降级";
  return {
    label: status === "failed" ? "工作流 · 失败" : "工作流 · 需要处理",
    summary,
    raw,
    failed: true,
    variant: "plan",
  };
}

export function eventStreamUrl(threadId, latestSeq) {
  return `/v1/threads/${encodeURIComponent(threadId)}/events?since_seq=${normalizedSequence(latestSeq)}`;
}

export function saveDraft(drafts, threadId, value) {
  if (!threadId) return;
  if (value) drafts.set(threadId, value);
  else drafts.delete(threadId);
}

export function restoreDraft(drafts, threadId) {
  return drafts.get(threadId) || "";
}

export function pendingAttentionCount(summary) {
  const count = Number(summary?.pending_attention_count);
  return Number.isSafeInteger(count) && count > 0 ? count : 0;
}

// Preserve the Runtime's newest-first order within each group. The server's
// typed pending-request count is the only attention authority; status prose
// and turn lifecycle strings deliberately do not participate.
export function groupThreadSummaries(summaries) {
  const groups = { needsYou: [], recent: [] };
  for (const summary of Array.isArray(summaries) ? summaries : []) {
    const group = pendingAttentionCount(summary) > 0 ? groups.needsYou : groups.recent;
    group.push(summary);
  }
  return groups;
}

export function pendingAttentionLabel(summary) {
  const count = pendingAttentionCount(summary);
  return count === 1
    ? "有 1 项需要你处理"
    : `有 ${count} 项需要你处理`;
}

// Match the CWC composer grammar while keeping the embedded client free of a
// framework dependency: Enter sends, Shift+Enter inserts a newline, and an
// active IME composition is never interrupted.
export function isComposerSubmitKey({ key, shiftKey = false, isComposing = false } = {}) {
  return key === "Enter" && !shiftKey && !isComposing;
}

export function newThreadDefaults(catalog) {
  const providers = Array.isArray(catalog?.providers)
    ? catalog.providers.filter((provider) => String(provider?.id || "").trim())
    : [];
  const current = String(catalog?.current || "").trim();
  const provider = providers.find((entry) => entry.id === current) || providers[0] || null;
  return {
    providerId: String(provider?.id || "").trim(),
    modelProviderId: String(provider?.model_provider_id || "").trim(),
    model: String(provider?.default_model || "").trim(),
  };
}

export function imageInputPresentation(value) {
  if (value === "supported") {
    return {
      state: "supported",
      label: "支持图片",
      description: "这个模型支持图片输入。浏览器附件功能尚未开启。",
    };
  }
  if (value === "unsupported") {
    return {
      state: "unsupported",
      label: "仅文本",
      description: "这个模型不支持图片输入。",
    };
  }
  return {
    state: "unknown",
    label: "图像支持未验证",
    description: "这个模型的图片支持未验证。",
  };
}

export function modelOptionLabel(model) {
  const id = String(model?.id || "").trim();
  return model?.image_input === "supported" ? `${id} · 支持图片` : id;
}

export function providerOptionLabel(provider) {
  const id = String(provider?.id || "").trim();
  const displayName = String(provider?.display_name || id).trim();
  const exactId = String(provider?.model_provider_id || "").trim();
  return exactId && exactId !== id ? `${displayName} · ${exactId}` : displayName;
}

export function buildCreateThreadRequest(providerId, model, modelProviderId = "") {
  const modelProvider = String(providerId || "").trim();
  const exactProviderId = String(modelProviderId || "").trim();
  const selectedModel = String(model || "").trim();
  if (!modelProvider || !selectedModel) {
    throw new Error("请选择提供商和模型。");
  }
  const request = { model_provider: modelProvider, model: selectedModel };
  if (exactProviderId) request.model_provider_id = exactProviderId;
  return request;
}

export function threadProviderLabel(thread) {
  const exact = String(thread?.model_provider_id || "").trim();
  const generic = String(thread?.model_provider || "").trim();
  return exact || generic;
}

export function claimInFlight(inFlight, key) {
  const action = String(key || "").trim();
  if (!(inFlight instanceof Set) || !action || inFlight.has(action)) return false;
  inFlight.add(action);
  return true;
}

export function setSafeText(element, value) {
  element.textContent = value == null ? "" : String(value);
  return element;
}

function normalizedSequence(value) {
  const sequence = Number(value);
  return Number.isSafeInteger(sequence) && sequence > 0 ? sequence : 0;
}

function upsertTurn(state, turn) {
  if (!turn || !turn.id) return;
  if (!state.turns.has(turn.id)) state.turnOrder.push(turn.id);
  state.turns.set(turn.id, turn);
}

function upsertItem(state, item) {
  if (!item || !item.id) return;
  if (!state.items.has(item.id)) state.itemOrder.push(item.id);
  state.items.set(item.id, item);
}

function appendItemDelta(state, itemId, payload) {
  if (!itemId) return;
  const delta = typeof payload.delta === "string" ? payload.delta : "";
  const existing = state.items.get(itemId) || {
    id: itemId,
    turn_id: "",
    kind: payload.kind || "agent_message",
    status: "in_progress",
    summary: "",
    detail: "",
  };
  if (!state.items.has(itemId)) state.itemOrder.push(itemId);
  state.items.set(itemId, {
    ...existing,
    status: "in_progress",
    detail: `${existing.detail || ""}${delta}`,
  });
}

const PROVIDER_MODELS_PAGE_SIZE = 250;
const MAX_PROVIDER_MODELS = 10_000;
const MAX_PROVIDER_MODEL_PAGES = Math.ceil(
  MAX_PROVIDER_MODELS / PROVIDER_MODELS_PAGE_SIZE,
);

/**
 * Load every bounded page of one provider catalog.
 *
 * `fetchPage` is injected so the browser client can retain its authenticated
 * Runtime API boundary and tests can prove catalogs larger than one page are
 * not silently truncated. Cursors are opaque and may never repeat.
 */
export async function collectProviderModelPages(providerId, fetchPage) {
  const provider = String(providerId || "").trim();
  if (!provider || typeof fetchPage !== "function") {
    throw new Error("A provider and page loader are required.");
  }

  const entries = [];
  const seenCursors = new Set();
  let expectedTotal;
  let cursor = "";
  for (let page = 0; page < MAX_PROVIDER_MODEL_PAGES; page += 1) {
    const query = new URLSearchParams({ limit: String(PROVIDER_MODELS_PAGE_SIZE) });
    if (cursor) query.set("cursor", cursor);
    const response = await fetchPage(
      `/v1/providers/${encodeURIComponent(provider)}/models?${query.toString()}`,
    );
    if (String(response?.provider || "") !== provider) {
      throw new Error("引擎返回了另一个提供商的模型页。");
    }
    if (!Array.isArray(response?.models)
      || response.models.length > PROVIDER_MODELS_PAGE_SIZE
      || !Number.isSafeInteger(response.total)
      || response.total < 0
      || response.total > MAX_PROVIDER_MODELS) {
      throw new Error("引擎返回了无效的模型页。");
    }
    if (expectedTotal !== undefined && expectedTotal !== response.total) {
      throw new Error("The provider catalog changed; restart loading its models.");
    }
    expectedTotal = response.total;
    const pageEntries = response.models;
    if (entries.length + pageEntries.length > MAX_PROVIDER_MODELS) {
      throw new Error(`The provider catalog exceeds ${MAX_PROVIDER_MODELS} models.`);
    }
    entries.push(...pageEntries);

    const nextCursor = typeof response?.nextCursor === "string"
      ? response.nextCursor.trim()
      : "";
    if (!nextCursor) {
      if (entries.length !== expectedTotal) {
        throw new Error("引擎返回的模型目录不完整。");
      }
      return entries;
    }
    if (pageEntries.length === 0 || seenCursors.has(nextCursor)) {
      throw new Error("引擎返回的模型游标没有前进。");
    }
    if (entries.length >= expectedTotal) {
      throw new Error("引擎返回的模型游标超出目录范围。");
    }
    seenCursors.add(nextCursor);
    cursor = nextCursor;
  }
  throw new Error(`The provider catalog exceeds ${MAX_PROVIDER_MODELS} models.`);
}

function startBrowserClient() {
  const dom = {
    shell: document.querySelector("#app-shell"),
    rail: document.querySelector("#thread-rail"),
    railOpen: document.querySelector("#rail-open"),
    railClose: document.querySelector("#rail-close"),
    railScrim: document.querySelector("#rail-scrim"),
    search: document.querySelector("#thread-search"),
    threadList: document.querySelector("#thread-list"),
    newThread: document.querySelector("#new-thread"),
    newThreadDialog: document.querySelector("#new-thread-dialog"),
    newThreadForm: document.querySelector("#new-thread-form"),
    newThreadProvider: document.querySelector("#new-thread-provider"),
    newThreadModel: document.querySelector("#new-thread-model"),
    newThreadModelSelectField: document.querySelector("#new-thread-model-select-field"),
    newThreadModelInput: document.querySelector("#new-thread-model-input"),
    newThreadModelInputField: document.querySelector("#new-thread-model-input-field"),
    newThreadCapability: document.querySelector("#new-thread-capability"),
    newThreadStatus: document.querySelector("#new-thread-status"),
    newThreadCancel: document.querySelector("#new-thread-cancel"),
    newThreadCreate: document.querySelector("#new-thread-create"),
    connectionDot: document.querySelector("#connection-dot"),
    connectionLabel: document.querySelector("#connection-label"),
    runtimeProvenance: document.querySelector("#runtime-provenance"),
    kicker: document.querySelector("#session-kicker"),
    title: document.querySelector("#session-title"),
    facts: document.querySelector("#session-facts"),
    rename: document.querySelector("#rename-thread"),
    archive: document.querySelector("#archive-thread"),
    status: document.querySelector("#status-banner"),
    transcript: document.querySelector("#transcript"),
    attention: document.querySelector("#attention"),
    composer: document.querySelector("#composer"),
    composerInput: document.querySelector("#composer-input"),
    send: document.querySelector("#send-message"),
    interrupt: document.querySelector("#interrupt-turn"),
    renameDialog: document.querySelector("#rename-dialog"),
    renameForm: document.querySelector("#rename-form"),
    renameInput: document.querySelector("#rename-input"),
    archivedToggle: document.querySelector("#archived-toggle"),
    archivedDialog: document.querySelector("#archived-dialog"),
    archivedList: document.querySelector("#archived-list"),
    archivedClose: document.querySelector("#archived-close"),
    archivedEmpty: document.querySelector("#archived-empty"),
    peek: document.querySelector("#session-peek"),
    savedSessions: document.querySelector("#saved-sessions"),
    sessionList: document.querySelector("#session-list"),
    session: document.querySelector(".session"),
  };

  const app = {
    summaries: [],
    sessionSummaries: [],
    archivedSummaries: [],
    // Typed selection: `none`, a read-only `session`, or a live `thread`.
    // Every reply/approval authority check reads this, not a loose id.
    target: NO_TARGET,
    // Bounded, redacted peek for the selected saved session, or null.
    peek: null,
    // Set when the SSE stream reported a sequence gap and a re-snapshot is
    // pending. Surfaced in the connection label rather than hidden.
    streamGap: false,
    selectedThreadId: "",
    threadState: createThreadState(),
    workspace: null,
    runtimeInfo: null,
    drafts: new Map(),
    stream: null,
    streamOpenCancel: null,
    reconnectTimer: null,
    generation: 0,
    searchTimer: null,
    railReturnFocus: null,
    inFlightActions: new Set(),
    providerCatalog: null,
    newThreadModels: [],
    newThreadGeneration: 0,
    newThreadLoading: false,
    creatingThread: false,
    // AsBudy：对话区是否**粘住最新** —— 照官方 CLI 的 TranscriptScroll 语义，不用像素差猜。
    // CLI 那份（crates/tui/src/tui/scrolling.rs）是状态机：`to_bottom()` 哨兵 = 粘住 live tail，
    // 只有**用户自己滚动**（scrolled_by / 滚轮 / 拖动滚动条）才变成 `at_line(n)`，滚回底部再收回。
    // 所以内容长高、容器变高变矮都**不改变**状态 —— 这两件事在「每次渲染量像素差」的写法里
    // 都会把状态误判成「用户翻上去了」（2026-09-19 手机版实测：键盘弹起把对话区从 361px 压到
    // 70px，距底瞬间 291px > 120 ⇒ 之后回复一概不再跟随）。默认粘住，与 CLI 默认一致。
    transcriptFollow: true,
    // 我们程序化贴底时落到的 scrollTop。用来区分「这次滚动是我们干的还是用户干的」——
    // 内容增高本身**不触发** scroll 事件（只改 scrollHeight，不动 scrollTop），
    // 所以 scroll 事件一定意味着滚动位置真的变了，比一下就知道是谁。
    transcriptAutoScrollTop: null,
    // ResizeObserver 已经把一次贴底排进下一帧（合并同帧内的多次高度变化）
    transcriptPinScheduled: false,
  };

  const narrowRail = globalThis.matchMedia("(max-width: 800px)");
  const composerSendAction = "composer-send";

  function element(tag, className, text) {
    const created = document.createElement(tag);
    if (className) created.className = className;
    if (text != null) setSafeText(created, text);
    return created;
  }

  function setInert(element, inert) {
    element.inert = inert;
    if (inert) element.setAttribute("inert", "");
    else element.removeAttribute("inert");
  }

  function applyDesktopRailAccessibility() {
    dom.shell.classList.remove("rail-visible");
    dom.rail.removeAttribute("aria-hidden");
    dom.rail.removeAttribute("aria-modal");
    dom.rail.removeAttribute("role");
    dom.session.removeAttribute("aria-hidden");
    setInert(dom.rail, false);
    setInert(dom.session, false);
    dom.railScrim.hidden = true;
    dom.railOpen.setAttribute("aria-expanded", "false");
    app.railReturnFocus = null;
  }

  function applyClosedMobileRailAccessibility() {
    dom.shell.classList.remove("rail-visible");
    dom.session.removeAttribute("aria-hidden");
    setInert(dom.session, false);
    dom.rail.setAttribute("role", "dialog");
    dom.rail.setAttribute("aria-modal", "true");
    dom.rail.setAttribute("aria-hidden", "true");
    setInert(dom.rail, true);
    dom.railScrim.hidden = true;
    dom.railOpen.setAttribute("aria-expanded", "false");
  }

  function openRail() {
    if (!narrowRail.matches) return;
    app.railReturnFocus = document.activeElement;
    dom.rail.setAttribute("role", "dialog");
    dom.rail.setAttribute("aria-modal", "true");
    dom.rail.setAttribute("aria-hidden", "false");
    setInert(dom.rail, false);
    dom.railScrim.hidden = false;
    dom.shell.classList.add("rail-visible");
    dom.railOpen.setAttribute("aria-expanded", "true");
    dom.railClose.focus({ preventScroll: true });
    dom.session.setAttribute("aria-hidden", "true");
    setInert(dom.session, true);
  }

  function closeRail({ restoreFocus = true } = {}) {
    if (!narrowRail.matches) {
      applyDesktopRailAccessibility();
      return;
    }
    dom.session.removeAttribute("aria-hidden");
    setInert(dom.session, false);
    const returnTarget = app.railReturnFocus?.isConnected
      ? app.railReturnFocus
      : dom.railOpen;
    if (restoreFocus) returnTarget.focus({ preventScroll: true });
    applyClosedMobileRailAccessibility();
    app.railReturnFocus = null;
  }

  function syncRailAccessibility() {
    if (!narrowRail.matches) {
      applyDesktopRailAccessibility();
      return;
    }
    if (dom.shell.classList.contains("rail-visible")) {
      dom.rail.setAttribute("role", "dialog");
      dom.rail.setAttribute("aria-modal", "true");
      dom.rail.setAttribute("aria-hidden", "false");
      setInert(dom.rail, false);
      dom.session.setAttribute("aria-hidden", "true");
      setInert(dom.session, true);
      dom.railScrim.hidden = false;
      dom.railOpen.setAttribute("aria-expanded", "true");
      return;
    }
    if (dom.rail.contains(document.activeElement)) {
      dom.railOpen.focus({ preventScroll: true });
    }
    applyClosedMobileRailAccessibility();
  }

  function syncVisualViewport() {
    const viewport = globalThis.visualViewport;
    const height = viewport?.height || globalThis.innerHeight;
    const offsetTop = viewport?.offsetTop || 0;
    if (Number.isFinite(height) && height > 0) {
      dom.shell.style.setProperty("--visual-viewport-height", `${Math.round(height)}px`);
    }
    dom.shell.style.setProperty("--visual-viewport-offset-top", `${Math.max(0, Math.round(offsetTop))}px`);
    // AsBudy：手机上键盘弹出/地址栏收放会改可视高（`.shell` 高 = 上面那个变量），
    // 对话区跟着变矮 —— 这**不是**「用户翻上去了」。官方 CLI 的等价场景（终端 resize）
    // 在 `app.rs` 的 `handle_resize` 里写得很直白：已在尾部就继续粘住尾部，
    // 只有「用户自己翻上去过」才保留他的位置。照做。
    if (app.transcriptFollow) requestAnimationFrame(() => scrollTranscriptToLatest());
    else publishTranscriptTail();   // 没在尾部：容器变矮可能让人以为按钮该出现了，其实是位置问题
  }

  function focusableWithin(container) {
    return [...container.querySelectorAll(
      'button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])',
    )].filter((node) => !node.closest("[hidden]") && node.getAttribute("aria-hidden") !== "true");
  }

  function trapFocusWithin(event, container) {
    if (event.key !== "Tab") return false;
    const focusable = focusableWithin(container);
    if (focusable.length === 0) {
      event.preventDefault();
      container.focus({ preventScroll: true });
      return true;
    }
    const first = focusable[0];
    const last = focusable.at(-1);
    const active = document.activeElement;
    if (event.shiftKey && (active === first || !container.contains(active))) {
      event.preventDefault();
      last.focus({ preventScroll: true });
      return true;
    }
    if (!event.shiftKey && (active === last || !container.contains(active))) {
      event.preventDefault();
      first.focus({ preventScroll: true });
      return true;
    }
    return false;
  }

  function trapRailFocus(event) {
    if (
      event.key !== "Tab"
      || !narrowRail.matches
      || !dom.shell.classList.contains("rail-visible")
    ) return false;
    return trapFocusWithin(event, dom.rail);
  }

  function setConnection(kind, message) {
    dom.connectionDot.className = `connection-dot ${kind || ""}`.trim();
    setSafeText(dom.connectionLabel, message);
  }

  function showStatus(message) {
    setSafeText(dom.status, message || "");
    dom.status.hidden = !message;
  }

  async function api(path, options = {}) {
    const headers = new Headers(options.headers || {});
    if (options.body != null && !headers.has("content-type")) {
      headers.set("content-type", "application/json");
    }
    const response = await fetch(path, {
      ...options,
      headers,
      credentials: "same-origin",
      cache: "no-store",
    });
    if (!response.ok) {
      let message = `${response.status} ${response.statusText}`.trim();
      try {
        const body = await response.json();
        message = body?.error?.message || body?.message || message;
      } catch (_error) {
        // The status line is enough when the response is not JSON.
      }
      if (response.status === 401) {
        // AsBudy：文案改「专业克制」（原来写着 `codewhale web` 这种给开发者看的命令）
        message = "会话已过期，请刷新页面重新登录。";
      }
      throw new Error(message);
    }
    if (response.status === 204) return null;
    const contentType = response.headers.get("content-type") || "";
    return contentType.includes("application/json") ? response.json() : response.text();
  }

  function renderThreadList() {
    dom.threadList.replaceChildren();
    if (app.summaries.length === 0) {
      // AsBudy：文案精简（2026-09-16 老板：「界面都是大白话会显得不专业，要有个度）」
      const empty = element("p", "thread-preview", "无匹配会话");
      empty.style.padding = "8px 10px";
      dom.threadList.append(empty);
      return;
    }

    const groups = groupThreadSummaries(app.summaries);
    if (groups.needsYou.length > 0) {
      appendThreadGroup("需要你处理", "needs-you", groups.needsYou);
    }
    if (groups.recent.length > 0) {
      appendThreadGroup("最近会话", "recent", groups.recent);
    }
  }

  function appendThreadGroup(label, idSuffix, summaries) {
    const group = element("section", `thread-group thread-group-${idSuffix}`);
    const headingId = `thread-group-${idSuffix}-title`;
    const heading = element("h2", "rail-section-title thread-group-title", label);
    heading.id = headingId;
    group.setAttribute("aria-labelledby", headingId);
    group.append(heading);

    for (const summary of summaries) {
      const row = element("button", "thread-row");
      row.type = "button";
      row.dataset.threadId = summary.id;
      row.setAttribute("aria-current", summary.id === app.selectedThreadId ? "true" : "false");
      const titleRow = element("span", "thread-title-row");
      titleRow.append(element("span", "thread-title", displayTitle(summary.title)));
      const indicators = element("span", "thread-row-indicators");
      const attentionCount = pendingAttentionCount(summary);
      if (attentionCount > 0) {
        const attention = element("span", "thread-attention-count", String(attentionCount));
        attention.setAttribute("aria-label", pendingAttentionLabel(summary));
        indicators.append(attention);
      }
      const status = element("span", `status-pip ${summary.latest_turn_status === "inprogress" || summary.latest_turn_status === "in_progress" ? "running" : summary.latest_turn_status === "failed" ? "failed" : ""}`);
      status.setAttribute("aria-label", summary.latest_turn_status || "idle");
      indicators.append(status);
      titleRow.append(indicators);
      row.append(titleRow);
      row.append(element("span", "thread-preview", stripMarkdown(summary.preview) || "还没有消息"));
      const branch = summary.branch || basename(summary.workspace) || "本地";
      row.append(element("span", "thread-meta", `${branch} · ${relativeTime(summary.updated_at)}`));
      row.addEventListener("click", () => selectThread(summary.id));
      group.append(row);
    }
    dom.threadList.append(group);
  }

  async function loadThreads(search = dom.search.value.trim()) {
    const query = new URLSearchParams({ limit: "100" });
    if (search) query.set("search", search);
    app.summaries = await api(`/v1/threads/summary?${query.toString()}`);
    renderThreadList();
    return app.summaries;
  }

  // Saved sessions are the durable session store the terminal browses. They
  // are rendered with the same row shape as threads because
  // /v1/sessions/summary and /v1/threads/summary are field-compatible
  // projections — one vocabulary, not two.
  function renderSessionList() {
    dom.sessionList.replaceChildren();
    // The section only exists when the backend actually returned sessions;
    // an affordance for an empty store would imply a capability that has
    // nothing behind it.
    dom.savedSessions.hidden = app.sessionSummaries.length === 0;
    if (app.sessionSummaries.length === 0) return;

    for (const summary of app.sessionSummaries) {
      const row = element("button", "thread-row");
      row.type = "button";
      row.dataset.sessionId = summary.id;
      const titleRow = element("span", "thread-title-row");
      titleRow.append(element("span", "thread-title", displayTitle(summary.title)));
      row.append(titleRow);
      row.append(element("span", "thread-preview", stripMarkdown(summary.preview || summary.title)));
      const scope = basename(summary.workspace) || "本地";
      row.append(
        element(
          "span",
          "thread-meta",
          `${scope} · ${summary.message_count} msg · ${relativeTime(summary.updated_at)}`,
        ),
      );
      row.setAttribute(
        "aria-current",
        app.target.kind === "session" && app.target.sessionId === summary.id ? "true" : "false",
      );
      // Click peeks; resuming is a separate, explicit button inside the peek.
      row.addEventListener("click", () => peekSession(summary.id));
      dom.sessionList.append(row);
    }
  }

  async function loadSessions(search = dom.search.value.trim()) {
    const query = new URLSearchParams({ limit: "50" });
    if (search) query.set("search", search);
    try {
      app.sessionSummaries = await api(`/v1/sessions/summary?${query.toString()}`);
    } catch (_error) {
      // A runtime without a readable session store is not a broken dashboard;
      // hide the section rather than blocking the thread view behind an error.
      app.sessionSummaries = [];
    }
    renderSessionList();
    return app.sessionSummaries;
  }

  // Resume goes through the existing endpoint, which seeds a real thread from
  // the saved messages. The dashboard does not reconstruct history itself.
  // Selecting a saved session shows a read-only peek. It does NOT resume:
  // resuming spawns a real thread and an engine, which must be a deliberate
  // act, not a side effect of clicking a row to see what it was about.
  async function peekSession(sessionId) {
    stopStream();
    app.selectedThreadId = "";
    app.threadState = createThreadState();
    app.target = sessionTarget(sessionId);
    showStatus("");
    renderThreadList();
    renderSessionList();
    try {
      // `?peek=true` returns a bounded, redacted projection — twelve entries,
      // tool payloads summarised — so the browser never receives the full
      // transcript in order to display a preview of it.
      app.peek = await api(
        `/v1/sessions/${encodeURIComponent(sessionId)}?peek=true&entries=12`,
      );
    } catch (error) {
      app.peek = null;
      showStatus(error.message);
    }
    renderAll();
  }

  async function resumeSession(sessionId) {
    showStatus("");
    try {
      const resumed = await api(`/v1/sessions/${encodeURIComponent(sessionId)}/resume-thread`, {
        method: "POST",
        body: "{}",
      });
      app.peek = null;
      await loadThreads("");
      // `selectThread` sets the live thread target; only after this can the
      // composer or an approval act.
      await selectThread(resumed.thread_id);
      showStatus(resumed.summary || "");
    } catch (error) {
      showStatus(error.message);
    }
  }

  // Render the read-only peek pane for a selected saved session.
  function renderPeek() {
    if (!dom.peek) return;
    const showing = app.target.kind === "session" && app.peek;
    dom.peek.hidden = !showing;
    if (!showing) {
      dom.peek.replaceChildren();
      return;
    }
    const peek = app.peek;
    dom.peek.replaceChildren();

    const header = element("div", "peek-header");
    header.append(element("p", "eyebrow", "已保存的会话 — 只读"));
    header.append(element("h2", "", displayTitle(peek.title)));
    header.append(
      element(
        "p",
        "thread-meta",
        `${basename(peek.workspace) || "本地"} · ${peek.message_count} 条消息 · ${relativeTime(peek.updated_at)}${peek.archived ? " · 已归档" : ""}`,
      ),
    );
    dom.peek.append(header);

    if (peek.omitted_before > 0) {
      dom.peek.append(
        element("p", "peek-omitted", `还有 ${peek.omitted_before} 条更早的消息未显示`),
      );
    }

    for (const entry of peek.entries || []) {
      const row = element("div", `peek-entry peek-${entry.kind}`);
      row.append(element("span", "peek-kind", entry.kind));
      // `element()` assigns via textContent. Peek text is recorded user/model
      // content and must never reach an HTML sink; this is the XSS boundary.
      row.append(element("p", "peek-text", entry.text));
      if (entry.redacted) row.append(element("span", "peek-flag", "redacted"));
      if (entry.truncated) row.append(element("span", "peek-flag", "truncated"));
      dom.peek.append(row);
    }

    const resume = element("button", "primary-button", "恢复为进行中的会话");
    resume.type = "button";
    resume.addEventListener("click", () => resumeSession(peek.session_id));
    dom.peek.append(resume);
  }

  function stopStream() {
    if (app.streamOpenCancel) app.streamOpenCancel();
    app.streamOpenCancel = null;
    if (app.stream) app.stream.close();
    app.stream = null;
    if (app.reconnectTimer) clearTimeout(app.reconnectTimer);
    app.reconnectTimer = null;
  }

  async function selectThread(threadId) {
    if (!threadId) return;
    saveDraft(app.drafts, app.selectedThreadId, dom.composerInput.value);
    stopStream();
    app.selectedThreadId = threadId;
    // A live thread is now the target: from here the composer and approvals
    // may act. Clear any saved-session peek so the two surfaces are exclusive.
    app.target = threadTarget(threadId);
    app.peek = null;
    app.streamGap = false;
    app.threadState = createThreadState(threadId);
    app.generation += 1;
    const generation = app.generation;
    dom.composerInput.value = restoreDraft(app.drafts, threadId);
    resizeComposer();
    renderThreadList();
    renderSessionList();
    renderAll();
    closeRailIfNarrow();
    setConnection("", "正在加载会话…");
    showStatus("");

    try {
      const subscribed = await snapshotThenSubscribe({
        state: app.threadState,
        threadId,
        loadSnapshot: (id) => api(`/v1/threads/${encodeURIComponent(id)}`),
        subscribe: (id, sequence) => {
          connectStream(id, sequence, generation);
        },
        isCurrent: () => generation === app.generation && threadId === app.selectedThreadId,
      });
      if (!subscribed) return;
      renderAll();
      setConnection("ready", "本地引擎已连接");
    } catch (error) {
      if (generation !== app.generation) return;
      showStatus(error.message);
      setConnection("error", "引擎连接失败");
    }
  }

  function connectStream(threadId, sequence, generation, waitForOpen = false) {
    if (generation !== app.generation || threadId !== app.selectedThreadId) return;
    if (app.streamOpenCancel) app.streamOpenCancel();
    app.streamOpenCancel = null;
    if (app.stream) app.stream.close();
    const stream = new EventSource(eventStreamUrl(threadId, sequence), { withCredentials: true });
    app.stream = stream;
    let opened = false;
    let resolveOpen;
    let rejectOpen;
    const openHandshake = waitForOpen
      ? new Promise((resolve, reject) => {
          resolveOpen = resolve;
          rejectOpen = reject;
        })
      : undefined;
    const cancelOpen = () => {
      if (!rejectOpen) return;
      const reject = rejectOpen;
      resolveOpen = null;
      rejectOpen = null;
      reject(new Error("引擎事件流打开被取消"));
    };
    if (waitForOpen) app.streamOpenCancel = cancelOpen;
    const clearOpenHandshake = () => {
      if (app.streamOpenCancel === cancelOpen) app.streamOpenCancel = null;
    };
    stream.onopen = () => {
      opened = true;
      setConnection("ready", "本地引擎已连接");
      clearOpenHandshake();
      if (resolveOpen) resolveOpen();
      resolveOpen = null;
      rejectOpen = null;
    };
    const receive = (message) => {
      if (
        app.stream !== stream
        || generation !== app.generation
        || threadId !== app.selectedThreadId
      ) return;
      try {
        const envelope = JSON.parse(message.data);
        if (runtimeEventContinuity(app.threadState, envelope) === "gap") {
          app.streamGap = true;
          renderStreamCursor();
          showStatus("运行事件连续性变化，正在刷新会话…");
          void recoverProjection(threadId, generation, stream);
          return;
        }
        if (!applyRuntimeEvent(app.threadState, envelope)) return;
        renderAll(true);
        // AsBudy：对话可能在项目目录里产出文件 —— 广播给侧栏「项目文件」树
        // （监听见 asbudy-files.js；官方 web 没有文件树，这是我们的扩展点）
        // 2026-09-18 扩：以前只广播 item.completed / turn.completed（“事后”），
        //   现在把「开始」也广播出去 —— 顶部那条状态行靠它显示「正在做什么」。
        //   CLI 的进度感就来自底部那条实时状态行，web 一直没有（老板：「不像 cli 那样知道进度」）。
        //   ⚠️ 不含 item.delta：它太频繁，逐条广播会拖慢流式渲染。
        if (ACTIVITY_EVENTS.has(envelope.event)) {
          try {
            window.dispatchEvent(new CustomEvent("asbudy:activity", {
              detail: { event: envelope.event, payload: envelope.payload || {} },
            }));
          } catch { /* 无 window 的环境（如测试）忽略 */ }
        }
        if (
          envelope.event === "turn.completed"
          || envelope.event === "thread.updated"
          || envelope.event === "approval.required"
          || envelope.event === "approval.decided"
          || envelope.event === "approval.timeout"
          || envelope.event === "user_input.required"
          || envelope.event === "user_input.answered"
          || envelope.event === "user_input.canceled"
        ) {
          loadThreads().catch((error) => showStatus(error.message));
        }
      } catch (error) {
        showStatus(`读取运行事件失败：${error.message}`);
      }
    };
    for (const name of STREAM_EVENT_NAMES) stream.addEventListener(name, receive);
    stream.onerror = () => {
      if (app.stream !== stream) {
        stream.close();
        return;
      }
      stream.close();
      app.stream = null;
      if (generation !== app.generation || threadId !== app.selectedThreadId) return;
      if (waitForOpen && !opened) {
        clearOpenHandshake();
        const reject = rejectOpen;
        resolveOpen = null;
        rejectOpen = null;
        reject?.(new Error("引擎事件流未重新打开"));
        return;
      }
      setConnection("", "正在重新连接本地引擎…");
      app.reconnectTimer = setTimeout(
        () => connectStream(threadId, app.threadState.latestSeq, generation),
        900,
      );
    };
    return openHandshake;
  }

  async function recoverProjection(threadId, generation, sourceStream = null) {
    if (
      generation !== app.generation
      || threadId !== app.selectedThreadId
      || (sourceStream && app.stream !== sourceStream)
    ) return;

    if (app.stream) app.stream.close();
    app.stream = null;
    if (app.reconnectTimer) clearTimeout(app.reconnectTimer);
    app.reconnectTimer = null;
    setConnection("", "正在刷新会话…");

    try {
      const subscribed = await recoverSnapshotAndSubscribe({
        state: app.threadState,
        threadId,
        loadSnapshot: (id) => api(`/v1/threads/${encodeURIComponent(id)}`),
        subscribe: (id, sequence) => connectStream(id, sequence, generation, true),
        isCurrent: () => generation === app.generation && threadId === app.selectedThreadId,
      }, () => {
        // A gap is continuous again only after both the replacement snapshot
        // and the replacement EventSource open handshake have succeeded.
        app.streamGap = false;
      });
      if (!subscribed) return;
      renderAll();
      showStatus("");
      setConnection("ready", "本地引擎已连接");
    } catch (error) {
      if (generation !== app.generation || threadId !== app.selectedThreadId) return;
      showStatus(`刷新会话失败：${error.message}`);
      setConnection("error", "引擎恢复失败");
      app.reconnectTimer = setTimeout(
        () => recoverProjection(threadId, generation),
        900,
      );
    }
  }

  function renderAll(preserveScroll = false) {
    renderHeader();
    renderPeek();
    renderTranscript(preserveScroll);
    renderAttention();
    renderComposer();
    renderStreamCursor();
  }

  // Show the SSE resume cursor so "am I reading everything?" is answerable.
  function renderStreamCursor() {
    if (app.target.kind !== "thread") return;
    const cursor = streamCursor(app.threadState, {
      gap: app.streamGap,
      connected: Boolean(app.stream),
    });
    setConnection(cursor.gap ? "error" : cursor.connected ? "ready" : "", cursor.label);
  }

  function renderHeader() {
    const thread = app.threadState.thread;
    const summary = app.summaries.find((item) => item.id === app.selectedThreadId);
    const title = displayTitle(thread?.title || summary?.title) || (thread ? "新建会话" : "选择一个会话");
    setSafeText(dom.title, title);
    setSafeText(dom.kicker, thread ? "AsBudy 会话" : "AsBudy");
    dom.rename.disabled = !thread;
    dom.archive.disabled = !thread;
    dom.facts.replaceChildren();
    if (!thread) return;

    const workspace = summary?.workspace || thread.workspace || app.workspace?.workspace;
    const branch = summary?.branch || app.workspace?.branch;
    dom.facts.append(factChip("工作区", basename(workspace) || "本地"));
    if (branch) dom.facts.append(factChip("Branch", branch));
    const provider = threadProviderLabel(thread);
    if (provider) dom.facts.append(factChip("Provider", provider));
    dom.facts.append(factChip("模型", thread.model || "引擎默认"));
    dom.facts.append(factChip("模式", modeLabel(thread.mode)));
    dom.facts.append(factChip("Permission", permissionLabel(thread)));
  }

  function factChip(label, value) {
    const chip = element("span", "fact-chip");
    chip.dataset.fact = String(label || "").toLowerCase();
    chip.append(element("span", "", label));
    chip.append(element("strong", "", value));
    return chip;
  }

  function renderTranscript(preserveScroll) {
    if (!app.threadState.thread) {
      // AsBudy：空状态文案改「专业克制」（2026-09-16 老板定，见 AGENTS.md「✍️ 界面文案」）
      renderTranscriptEmpty(
        "choose-thread",
        "准备就绪。",
        "从左侧选择会话，或直接输入需求。",
      );
      return;
    }
    if (app.threadState.itemOrder.length === 0) {
      renderTranscriptEmpty(
        "ready",
        "准备就绪。",
        "AI Builder, As Your Buddy",
      );
      return;
    }

    const selection = captureTranscriptSelection();
    const existing = new Map(
      [...dom.transcript.children]
        .filter((node) => node.dataset.itemId)
        .map((node) => [node.dataset.itemId, node]),
    );
    // AsBudy M10（2026-09-20）：先按过滤后的顺序算出「运行汇总组」，再逐条渲染 ——
    //   组内成员不单独画，只在组头画一行汇总（可点开看原来的卡）。
    //   规则与阈值全在 detectToolRuns 那一块（照 CLI `tool_run.rs`），这里只管接线。
    const visible = [];
    for (const itemId of app.threadState.itemOrder) {
      const item = app.threadState.items.get(itemId);
      if (!item) continue;
      // AsBudy：引擎的「进度」回执（kind=status，如「拿到结果，继续」「Executing tools
      // sequentially…」）是**过程性状态**、不是产出。上方 msgbar 那条实时状态行
      // （asbudy-my.js 的 #asbudy-live：「正在执行命令 · 3s」）已经把同一件事实时报了一遍，
      // 两处重复。老板 2026-09-19：「对话框上方和下方的会话状态显示是不是重复了？下方的可以
      // 去掉只保留上方的」。⇒ 不再把这一类回执画进对话区。
      // ⚠️ 只去这一类：工具（exec/file/explore/web/mcp）、思考、回复、出错、文件改动、
      //    整理对话 都照旧渲染 —— 那些是客户要看的东西，不是状态。
      //   ⚠️ 注意它**只影响画面、不参与运行汇总的分组**：官方的序列里本来就没有这一类 cell，
      //     所以「过滤掉它」与「把它当切断点」等价（一个都没有，就无组可分）。
      if (item.kind === "status") continue;
      // ⚠️ 2026-09-19 回滚一次错方向：我曾把「进行中的回执」也挡在转录外，理由是与上方状态行重复。
      //   查了官方 CLI 才发现**错的是上面那条状态行、不是这里的卡片**：
      //   · CLI 的转录里工具卡在跑时**是显示的**（`⠋ bash running (3s) · 摘要`，`history.rs:2654`）
      //   · CLI 的底部条（posture bar）只报**阶段**（`使用工具中 12s`，`underwater.rs:616`）
      //   ⇒ 两者层级不同、不重复。真正重复的是我们自创的上方状态行**用了工具级描述**
      //     （「正在执行命令」）—— 已改成 CLI 的阶段词（见 asbudy-my.js 的 liveWordFor）。
      //   ⇒ 所以卡片照旧落进转录。老板：「我们对齐官方 cli 版就是了，不用创新」
      //   ⚠️ 同一个道理在这里也成立：**还在跑的工具卡不准被折进汇总** ——
      //     官方 `is_collapsible_tool_cell` 要求 `tool.is_success()`，Our `ok` 同样只在 completed 时为真，
      //     所以它天然会把运行切开、自己单独一张卡（客户看得见它在干什么）。
      visible.push(item);
    }
    const runCells = visible.map(function (entry) {
      const alias = toolAliasOf(entry);
      if (!alias) return null;              // 正文 / 思考 / 子代理……不是工具调用（会把运行切开）
      const meta = entry.metadata && typeof entry.metadata === "object" ? entry.metadata : {};
      return { alias: alias, ok: entry.status === "completed" && meta.is_error !== true };
    });
    const runs = detectToolRuns(runCells, TOOL_RUN_MIN);
    const runOfIndex = new Map();
    runs.forEach(function (run, ri) {
      for (let k = run.start; k < run.start + run.count; k += 1) runOfIndex.set(k, ri);
    });

    const desired = [];
    for (let index = 0; index < visible.length; index += 1) {
      const item = visible[index];
      const ri = runOfIndex.get(index);
      if (ri !== undefined) {
        if (index !== runs[ri].start) continue;   // 组内其余成员由汇总行代表
        const run = runs[ri];
        let node = existing.get("run:" + visible[run.start].id);
        if (!node) node = renderToolRun(run, visible);
        else updateToolRunNode(node, run, visible);
        observeTranscriptItem(node);
        desired.push(node);
        continue;
      }
      let node = existing.get(item.id);
      if (!node || !updateItemNode(node, item)) node = renderItem(item);
      observeTranscriptItem(node);
      desired.push(node);
    }
    reconcileChildren(dom.transcript, desired);
    restoreTranscriptSelection(selection);
    // 粘住最新时，新内容追加后跟着往下（CLI 的 to_bottom 哨兵在渲染时解析成 max_start，同一个意思）。
    // 容器变矮（手机键盘）**不算**离开尾部 —— 状态没变，所以这里照旧跟随。
    // （整屏重画 = 换会话 / 初始化 / 动作之后 → 一律回到最新，状态跟着归位；
    //   CLI 同样在换会话和开新一轮时把「用户滚过」的锁清掉。）
    if (!preserveScroll) setTranscriptFollow(true);
    if (!preserveScroll || app.transcriptFollow) {
      requestAnimationFrame(() => scrollTranscriptToLatest());
    }
  }

  /** 算作「就停在最新」的容差（px）。照 CLI 是行级严格判定，web 上留给触摸回弹/亚像素。 */
  const TRANSCRIPT_TAIL_EPSILON = 16;

  function isTranscriptAtTail(element = dom.transcript) {
    if (!element) return true;
    return element.scrollHeight - element.scrollTop - element.clientHeight <= TRANSCRIPT_TAIL_EPSILON;
  }

  /** 改「是否粘住最新」并广播出去 —— 「回到最新」按钮（asbudy-latest.js）跟这一个真相走，
   *  不再自己量像素（CLI 里按钮显隐就是 `!is_at_tail()`，同一个状态）。 */
  function setTranscriptFollow(follow) {
    const next = Boolean(follow);
    if (next === app.transcriptFollow) return;
    app.transcriptFollow = next;
    try {
      document.dispatchEvent(new CustomEvent("asbudy:tail", { detail: { atTail: next } }));
    } catch (error) { /* 无 document 的环境（测试）忽略 */ }
  }

  /** 用户自己滚了对话区 → 按**位置**定状态（CLI 的 scrolled_by：往上一行就离开尾部，
   *  滚回 max_start 就重新粘住）。 */
  function handleTranscriptScroll() {
    const transcript = dom.transcript;
    if (!transcript) return;
    // ① 停在最新就一律回到「粘住」（CLI：offset 落到 max_start 就收回 to_bottom）。
    //    先判这条，是因为「用户自己滚回底部」与「我们程序化贴底」落到的位置一样，
    //    靠「是谁滚的」分不出来 —— 而两个方向都应该粘住。
    if (isTranscriptAtTail(transcript)) {
      setTranscriptFollow(true);
      return;
    }
    // ② 不在最新，但是**我们自己刚放下的那个位置** → 内容在长高过程中的中间态，别动状态。
    if (app.transcriptAutoScrollTop !== null
      && Math.abs(transcript.scrollTop - app.transcriptAutoScrollTop) <= 2) return;
    // ③ 其余就是用户自己翻上去了 → 离开尾部（不打扰他）。
    setTranscriptFollow(false);
  }

  /** 贴在最新（若内容还没长到需要滚，状态也归位）。 */
  function pinTranscriptToLatest() {
    setTranscriptFollow(true);
    requestAnimationFrame(() => scrollTranscriptToLatest());
  }

  function publishTranscriptTail() {
    try {
      document.dispatchEvent(new CustomEvent("asbudy:tail", {
        detail: { atTail: app.transcriptFollow && isTranscriptAtTail() },
      }));
    } catch (error) { /* 同上 */ }
  }

  /* AsBudy：把对话区**立即**贴到底（2026-09-19 老板报「发消息后没法自动显示最新回复，
     界面停在最开始几次回复」）。
     ⚠️ 别写回 `dom.transcript.scrollTop = dom.transcript.scrollHeight` —— 只要
     `.transcript` 带着 `scroll-behavior:smooth`，这一句就只是**发起一个约 1 秒的平滑动画**，
     而流式回复每几十毫秒就把内容加高一次，动画永远追不上（实测：赋值后同一 tick
     scrollTop 仍是 0，500ms 才走到 4830/7285）⇒ 上面那句 `wasNearBottom` 的读数长期落在
     底部 120px 之外 ⇒ 从某一轮起**永久**判 false，之后所有回复都不再自动显示。
     自动跟随必须即时；平滑只留给用户主动点的「回到最新」（asbudy-latest.js，自带
     behavior:'smooth'）。 */
  function scrollTranscriptToLatest() {
    const transcript = dom.transcript;
    if (!transcript) return;
    try {
      transcript.scrollTo({ top: transcript.scrollHeight, behavior: "instant" });
    } catch (error) {
      /* 老浏览器不认 options 形式 → 交给下面的兜底 */
    }
    // 兜底：仍有距离说明上一步没即时生效（比如被降级成了平滑滚动）→ 临时用行内样式
    // 压掉 CSS 的 scroll-behavior 再赋一次（"auto" 就是即时，不会被外层的 smooth 继承）。
    if (transcript.scrollHeight - transcript.scrollTop - transcript.clientHeight > 1) {
      const previous = transcript.style.scrollBehavior;
      transcript.style.scrollBehavior = "auto";
      transcript.scrollTop = transcript.scrollHeight;
      transcript.style.scrollBehavior = previous;
    }
    // 记下「我们把它放在哪」—— handleTranscriptScroll 靠它认出「这不是用户滚的」。
    app.transcriptAutoScrollTop = transcript.scrollTop;
  }

  /* AsBudy：内容「长高」有两种。一种是渲染（上面那条 rAF 贴底管的）；另一种是**渲染之后**
     才落定的资源（图片解码完成、字体换上）—— 后者不触发任何渲染，于是贴底后还能差出小半屏。
     2026-09-19 实测：切一条会话，贴底那一刻内容还没长完，14ms 后 +147px，而之后没人再贴一次
     （桌面能滚 8561px，147px 就停在那儿）。官方 CLI 没这个问题：它每一帧都按行数重新解析尾部哨兵。
     web 里能听到「内容高度变了」的只有 ResizeObserver —— 所以照上。 */
  const transcriptItemObserver = typeof ResizeObserver === "function"
    ? new ResizeObserver(() => {
      if (!app.transcriptFollow || app.transcriptPinScheduled) return;
      app.transcriptPinScheduled = true;
      requestAnimationFrame(() => {
        app.transcriptPinScheduled = false;
        scrollTranscriptToLatest();
      });
    })
    : null;
  const observedTranscriptItems = new WeakSet();

  /** 盯住每一条记录的高度（幂等 —— 同一个节点只挂一次）。 */
  function observeTranscriptItem(node) {
    if (!transcriptItemObserver || !node || observedTranscriptItems.has(node)) return;
    observedTranscriptItems.add(node);
    transcriptItemObserver.observe(node);
  }

  function renderTranscriptEmpty(kind, title, description) {
    const current = dom.transcript.children.length === 1
      ? dom.transcript.firstElementChild
      : null;
    if (current?.dataset.emptyState === kind) return;
    const empty = emptyState(title, description);
    empty.dataset.emptyState = kind;
    reconcileChildren(dom.transcript, [empty]);
  }

  function reconcileChildren(container, desired) {
    const keep = new Set(desired);
    let cursor = container.firstElementChild;
    for (const node of desired) {
      if (node === cursor) {
        cursor = cursor.nextElementSibling;
      } else {
        container.insertBefore(node, cursor);
      }
    }
    for (const child of [...container.children]) {
      if (!keep.has(child)) child.remove();
    }
  }

  function emptyState(title, description) {
    const empty = element("div", "empty-state");
    const mark = document.createElement("img");
    mark.className = "empty-mark";
    mark.src = "/assets/asbudy-empty-mark.png";
    mark.alt = "";
    empty.append(mark);
    empty.append(element("h2", "", title));
    empty.append(element("p", "", description));
    return empty;
  }

  function renderItem(item) {
    let card;
    if (item.kind === "user_message" || item.kind === "agent_message") {
      const role = item.kind === "user_message" ? "user" : "agent";
      card = element("article", `message ${role}`);
      const label = element("div", "message-label");
      label.dataset.itemPart = "label";
      const body = element("div", "message-body");
      body.dataset.itemPart = "body";
      card.append(label, body);
    } else if (item.kind === "agent_reasoning") {
      card = element("article", "reasoning");
      // AsBudy：收起时也露「前几行预览」（照官方 thinking_preview_lines，见 THINKING_PREVIEW_LINES）
      const preview = element("div", "ab-think-preview");
      preview.dataset.itemPart = "preview";
      preview.hidden = true;
      const disclosure = element("details");
      const summary = element("summary");
      summary.dataset.itemPart = "summary";
      const detail = element("pre");
      detail.dataset.itemPart = "detail";
      disclosure.append(summary, detail);
      card.append(preview, disclosure);
    } else if (isAgentToolCall(item)) {
      // ── 子代理卡：头部（字形+「子代理」+名字）· 动作行 · 终态行（照 CLI DelegateCard）
      card = element("article", "agent-card");
      card.dataset.agentCard = "1";
      const head = element("div", "agent-head");
      const glyph = element("span", "agent-glyph");
      glyph.dataset.itemPart = "glyph";
      const role = element("strong", "agent-role");
      role.dataset.itemPart = "role";
      const name = element("span", "agent-name");
      name.dataset.itemPart = "name";
      head.append(glyph, role, name);
      const activity = element("div", "agent-activity");
      activity.dataset.itemPart = "activity";
      activity.hidden = true;
      const foot = element("div", "agent-foot");
      foot.dataset.itemPart = "foot";
      foot.hidden = true;
      card.append(head, activity, foot);
    } else if (isChecklistToolName(toolAliasOf(item)) && parseChecklistSnapshot(item.detail || item.summary || "")) {
      // ── 清单 / 待办卡（M12）—— 照 CLI `history/checklist.rs`：卡头 `3/7 · 43%` + 每项一个符号
      //   ⚠️ 判据里**带上「能解析」**：否则 updateItemNode 那边判「该改形态麻 rebuild」时
      //   跟这里不一致，会来回重建（每帧重画，看着像卡在闪）。
      card = element("article", "checklist-card");
      card.dataset.checklistCard = "1";
      const clHead = element("div", "cl-head");
      const clGlyph = element("span", "cl-glyph");
      clGlyph.dataset.itemPart = "glyph";
      const clRole = element("strong", "cl-role");
      clRole.dataset.itemPart = "role";
      const clCount = element("span", "cl-count");
      clCount.dataset.itemPart = "count";
      clHead.append(clGlyph, clRole, clCount);
      const clList = element("ul", "cl-items");
      clList.dataset.itemPart = "items";
      card.append(clHead, clList);
    } else {
      card = element("article", "receipt");
      card.append(element("span", "receipt-dot"));
      const copy = element("span", "receipt-copy");
      const label = element("strong");
      label.dataset.itemPart = "label";
      const summary = element("span", "receipt-summary");
      summary.dataset.itemPart = "summary";
      // AsBudy：徽标位（耗时 / 非零退出码）—— 形态区分见 receiptVariant / asbudy-my.js 的 CSS
      const meta = element("span", "receipt-meta");
      meta.dataset.itemPart = "meta";
      meta.hidden = true;
      copy.append(label, summary, meta);
      card.append(copy);
      // AsBudy：**默认可见的正文预览**（2026-09-19 · 照官方明细关：头部 ＋ 最多 4 行正文 ＋ 展开提示）
      //   以前正文只活在折块里 ⇒ 默认档客户一个字都看不到（比官方还少）。
      const outPreview = element("pre", "ab-out-preview");
      outPreview.dataset.itemPart = "outpreview";
      outPreview.hidden = true;
      card.append(outPreview);
      // AsBudy：文件改动的内联 diff（照官方 inline_diffs=full，界限见 MAX_INLINE_DIFF_LINES）
      const diff = element("pre", "ab-diff");
      diff.dataset.itemPart = "diff";
      diff.hidden = true;
      card.append(diff);
    }
    card.dataset.itemId = item.id;
    card.dataset.itemKind = item.kind;
    updateItemNode(card, item);
    return card;
  }

  /** 一行「运行汇总」（M10）—— 官方 `v` 展开在网页上的对应物：点一下看回原来的卡。
   *
   *  ⚠️ 两个非显然的点（都试过才会发现）：
   *   ① 汇总节点是 `<details>`，组内卡片是它的**子节点** ⇒ 它们不参与 `desired` 的顶层
   *      reconcile（reconcileChildren 只管直接子节点）—— 展开内容每次重画都重建。
   *      组内全是**已终态**的卡，不会流式更新，所以重建是安全的。
   *   ② **展开状态记在 `app.expandedRuns`（不是 DOM）** —— 重画会让 details 重建，
   *      状态不自己记住的话，客户刚点开就被合上（M3/M4 那两个
   *      「看起来没报错、就是没反应」的坑同源）。 */
  function renderToolRun(run, items) {
    const key = "run:" + items[run.start].id;
    const card = document.createElement("details");
    card.className = "item tool-run";
    card.dataset.itemId = key;
    card.dataset.itemKind = "tool_run";
    card.dataset.runSig = toolRunSignature(run, items);
    if (app.expandedRuns && app.expandedRuns.has(key)) card.open = true;
    const summary = document.createElement("summary");
    summary.className = "tool-run-head";
    summary.title = "点开看这一步的细节";
    const glyph = document.createElement("span");
    glyph.className = "tool-run-glyph";
    glyph.textContent = "\u22EF";           // ⋯ — 与「还有几个在里」的收拢语义一致
    const text = document.createElement("span");
    text.className = "tool-run-text";
    text.textContent = toolRunSummaryText(run);
    summary.append(glyph, text);
    const body = document.createElement("div");
    body.className = "tool-run-body";
    for (let k = run.start; k < run.start + run.count; k += 1) {
      body.appendChild(renderItem(items[k]));
    }
    card.append(summary, body);
    card.addEventListener("toggle", function () {
      if (!app.expandedRuns) app.expandedRuns = new Set();
      if (card.open) app.expandedRuns.add(key);
      else app.expandedRuns.delete(key);
    });
    return card;
  }

  /** 组的「签名」—— 用来判「要不要重建展开内容」。
   *  ⚠️ 不复用会出事（做 M10 时差点漏掉）：每帧重画都新建汇总节点的话，
   *  ① 展开着的内容每帧重建（白花力气）；② `details` 重建后 `open` 得靠 app 状态重贴，
   *  滚动位置也可能跳。所以：**签名没变就原样留着**。 */
  function toolRunSignature(run, items) {
    return run.count + "|" + items[run.start].id + "|" + toolRunSummaryText(run);
  }

  function updateToolRunNode(card, run, items) {
    const sig = toolRunSignature(run, items);
    if (card.dataset.runSig === sig) return;      // 组没变 —— 连展开状态一起原样留着
    card.dataset.runSig = sig;
    const text = card.querySelector(".tool-run-text");
    if (text) text.textContent = toolRunSummaryText(run);
    const body = card.querySelector(".tool-run-body");
    if (body) {
      body.textContent = "";
      for (let k = run.start; k < run.start + run.count; k += 1) body.appendChild(renderItem(items[k]));
    }
  }

  function updateItemNode(card, item) {
    if (card.dataset.itemKind !== item.kind) return false;
    // 子代理卡：**形态判定**（metadata 可能要等 item.completed 才齐）——
    //   判变了就返回 false，让上层重建卡片（调用处：`!updateItemNode(...) → renderItem(item)`）。
    //   不这么写的话：第一帧 metadata 还没 tool_name ⇒ 先画成普通回执就**再也回不去了**。
    if ((card.dataset.agentCard === "1") !== isAgentToolCall(item)) return false;
    const detail = item.detail || item.summary || "";
    if (item.kind === "user_message" || item.kind === "agent_message") {
      const role = item.kind === "user_message" ? "user" : "agent";
      card.className = `message ${role} ${item.status === "in_progress" ? "in-progress" : ""}`.trim();
      const time = item.started_at ? fmtDateTime(item.started_at) : "";
      setTextIfChanged(card.querySelector('[data-item-part="label"]'), role === "user" ? time : ("AsBudy" + (time ? " · " + time : "")));
      // AI 的回复是 Markdown → 渲染给人看；客户自己打的字原样显示（不改他的输入）
      const body = card.querySelector('[data-item-part="body"]');
      if (role === "user") { body.__asbudyHtml = undefined; setTextIfChanged(body, detail); }
      else setHtmlIfChanged(body, renderMarkdown(detail));
      return true;
    }
    if (item.kind === "agent_reasoning") {
      // 照官方 CLI（`history/thinking.rs:148-170`）：进行中是 `… reasoning live` —— **不带秒数**
      // （时长只在完成后拼：`… reasoning done · 521ms`）。我们的上方状态行已经有「推理中 12s」，
      // 这里再写一个秒数就是同一件事两遍（老板 2026-09-19「1 做了」）。
      setTextIfChanged(
        card.querySelector('[data-item-part="summary"]'),
        item.status === "in_progress"
          ? "思考中…"
          : "思考过程" + completedElapsedSuffix(item),
      );
      setTextIfChanged(card.querySelector('[data-item-part="detail"]'), detail);
      // 收起时的预览（照官方 thinking_preview_lines，见 THINKING_PREVIEW_LINES）：
      //   只在**已完成**时露前几行；展开着的时候由 CSS（:has(details[open])）藏掉，不重复显示
      const preview = card.querySelector('[data-item-part="preview"]');
      if (preview) {
        const head = item.status === "completed"
          ? String(detail || "").replace(/\r\n?/g, "\n").split("\n").filter((l) => l.trim()).slice(0, thinkingPreviewLines())
          : [];
        const text = head.join("\n");
        preview.hidden = !text;
        setTextIfChanged(preview, text);
      }
      return true;
    }

    // 清单卡：形态判定（照子代理卡那套 —— 判变了就让上层重建）
    const clText = item.detail || item.summary || "";
    const clSnapshot = parseChecklistSnapshot(clText);
    const clWanted = isChecklistToolName(toolAliasOf(item)) && !!clSnapshot;
    if ((card.dataset.checklistCard === "1") !== clWanted) return false;
    if (card.dataset.checklistCard === "1") {
      card.dataset.clState = item.status === "in_progress" ? "running" : (item.status === "failed" ? "failed" : "completed");
      card.dataset.clPct = String(clSnapshot.pct);
      setTextIfChanged(card.querySelector('[data-item-part="glyph"]'), clSnapshot.pct >= 100 ? "\u2611" : "\u25D0");
      // 「清单」这个叫法照官方（`checklist.rs` 的卡头就是 `checklist`）
      setTextIfChanged(card.querySelector('[data-item-part="role"]'), "清单");
      setTextIfChanged(card.querySelector('[data-item-part="count"]'), checklistHeadText(clSnapshot));
      const clList = card.querySelector('[data-item-part="items"]');
      if (clList) {
        // 用签名决定要不要重画：项内容/状态没变就不重建（否则每帧重画会把选中/滚动弄丢）
        const sig = clSnapshot.items.map(function (it) { return it.content + ":" + it.status; }).join("|");
        if (clList.dataset.clSig !== sig) {
          clList.dataset.clSig = sig;
          const frag = document.createDocumentFragment();
          clSnapshot.items.forEach(function (entry) {
            const li = document.createElement("li");
            li.className = "cl-item";
            li.dataset.clSt = checklistStatusKind(entry.status);
            const marker = document.createElement("span");
            marker.className = "cl-mk";
            marker.textContent = checklistMarkerOf(entry.status);
            const text = document.createElement("span");
            text.className = "cl-tx";
            text.textContent = entry.content;      // 文本节点 —— 清单内容是 AI 写的，不当 HTML 解
            li.append(marker, text);
            frag.append(li);
          });
          clList.replaceChildren(frag);
        }
      }
      return true;
    }

    if (isAgentToolCall(item)) {
      // ── 子代理卡填充（照 CLI DelegateCard：头部 → 动作尾 → 终态行）──
      const facts = agentCardFacts(item);
      const status = facts.status;
      card.dataset.agentStatus = status || "unknown";
      setTextIfChanged(card.querySelector('[data-item-part="glyph"]'),
        AGENT_STATUS_GLYPH[status] || (facts.failed ? "×" : "◐"));
      // 「子代理」这个译名直接用官方语言包的 `ApprovalCategoryAgent`
      setTextIfChanged(card.querySelector('[data-item-part="role"]'), "子代理");
      const name = shortAgentId(facts.name || facts.agentId);
      setTextIfChanged(card.querySelector('[data-item-part="name"]'), name);
      // 动作行：引擎给的 `activity`（形如 `step 3: requesting model response`）优先；
      // 被守护者拒绝时改拿拒绝理由（那是客户唯一能看到的「为什么没成」）
      const activity = facts.activity || (facts.failed ? facts.failReason : "");
      const activityEl = card.querySelector('[data-item-part="activity"]');
      if (activityEl) {
        activityEl.hidden = !activity;
        setTextIfChanged(activityEl, activity ? "│ " + activity : "");
      }
      // 终态行：`╰ 已完成 · 3.9s · 2 步`（照 CLI 的 `╰ done · 12s · 摘要`）。
      // ⚠️ **只有终态才画** —— 照 CLI（`agent_card.rs:255` 那个 `if self.status.is_terminal()`）。
      //   不这么写的话：历史里那条 `start` 记录会永远挂着「运行中」，
      //   子代理明明早就结束了，客户却以为它还在跑（误导）。
      const footEl = card.querySelector('[data-item-part="foot"]');
      if (footEl) {
        if (!facts.terminal) {
          footEl.hidden = true;
          setTextIfChanged(footEl, "");
        } else {
          const parts = [AGENT_STATUS_ZH[status] || status || "已完成"];
          if (facts.durationMs > 0) parts.push(formatElapsedMs(facts.durationMs));
          if (facts.steps > 0) parts.push(facts.steps + " 步");
          footEl.hidden = false;
          setTextIfChanged(footEl, "╰ " + parts.join(" · "));
        }
      }
      return true;
    }

    const presentation = receiptPresentation(item);
    card.className = `receipt ${presentation.failed ? "failed" : ""}`.trim();
    // AsBudy：按形态上色/排版（exec 等宽、file 高亮、status 降噪…，规则见 asbudy-my.js）
    card.dataset.variant = presentation.variant || "generic";
    // 工具族字形（2026-09-20 搬 · 照 CLI 的 `family_glyph`）—— 取代原来的 7×7 圆点，
    //   客户一眼分得出「在读文件 / 在改代码 / 在跑命令 / 在找东西 / 在验证」。
    //   颜色由 `asbudy-my.js` 里 `[data-family=…]` 那组规则定。
    const family = toolFamilyOf(item);
    const dotEl2 = card.querySelector(".receipt-dot");
    if (dotEl2) {
      card.dataset.family = family || "generic";
      setTextIfChanged(dotEl2, TOOL_FAMILY_GLYPH[family] || TOOL_FAMILY_GLYPH.generic);
    }
    setTextIfChanged(card.querySelector('[data-item-part="label"]'), presentation.label + runningElapsedSuffix(item));
    setTextIfChanged(card.querySelector('[data-item-part="summary"]'), presentation.summary);
    // 徽标（耗时/退出码）：老卡片可能还没这个节点，取不到就跳过
    const metaEl = card.querySelector('[data-item-part="meta"]');
    if (metaEl) {
      setTextIfChanged(metaEl, presentation.meta || "");
      metaEl.hidden = !presentation.meta;
    }
    // AsBudy：默认可见的正文预览（展开「查看回执」时由 CSS 让位，不重复显示）
    const outPreviewEl = card.querySelector('[data-item-part="outpreview"]');
    if (outPreviewEl) {
      // 用 `detail`（完整输出）；没有才退回 summary。
      // ⚠️ **不用 `presentation.raw`** —— 它是 `summary + "\n\n" + detail`，
      //    开头那段是引擎的 280 字摘要回显，我们的头部已经有人话摘要了，再来一遍是噪音。
      const previewText = String(item.detail || item.summary || "").replace(/\r\n?/g, "\n");
      const previewLines = previewText.split("\n");
      while (previewLines.length && previewLines[previewLines.length - 1] === "") previewLines.pop();
      const limit = presentation.failed ? OUT_PREVIEW_FAILED_LINES : OUT_PREVIEW_SUCCESS_LINES;
      const { parts } = selectedOutputLines(previewLines, limit);
      if (!parts.length) {
        if (!outPreviewEl.hidden) { outPreviewEl.hidden = true; outPreviewEl.replaceChildren(); outPreviewEl.dataset.previewKey = ""; }
      } else if (outPreviewEl.dataset.previewKey !== previewText) {
        // ⚠️ 逐段建节点，**绝不用 innerHTML** —— 正文来自工具输出，不能当 HTML 解析（XSS）
        const fragment = document.createDocumentFragment();
        parts.forEach((part, index) => {
          const row = document.createElement("span");
          if (part.omitted !== undefined) {
            row.className = "ab-out-more";
            row.textContent = `… 还有 ${part.omitted} 行（展开「查看回执」看全部）`;
          } else {
            row.className = "ab-out-line";
            row.textContent = part.text === "" ? " " : part.text;
          }
          fragment.append(row);
          if (index < parts.length - 1) fragment.append(document.createTextNode("\n"));
        });
        outPreviewEl.replaceChildren(fragment);
        outPreviewEl.dataset.previewKey = previewText;
        outPreviewEl.hidden = false;
      }
    }
    // AsBudy：文件改动的内联 diff（照官方 inline_diffs=full，界限见 MAX_INLINE_DIFF_LINES）
    const diffEl = card.querySelector('[data-item-part="diff"]');
    if (diffEl) {
      const diffText = presentation.diff || "";
      if (!diffText) {
        if (!diffEl.hidden) { diffEl.hidden = true; diffEl.replaceChildren(); diffEl.dataset.diffText = ""; }
      } else if (diffEl.dataset.diffText !== diffText) {
        // ⚠️ 逐行建节点，**绝不用 innerHTML** —— diff 来自文件内容，不能当 HTML 解析（XSS）
        const { lines, omitted } = boundedDiffLines(diffText);
        const fragment = document.createDocumentFragment();
        // 第一行是统计（照官方的 semantic change statistics）——`inline_diffs=summary` 时只显这行
        const stats = diffStats(diffText);
        const statRow = document.createElement("span");
        statRow.className = "ab-diff-line ab-diff-stat";
        statRow.textContent = `${stats.add} 行新增 · ${stats.del} 行删除`;
        fragment.append(statRow, document.createTextNode("\n"));
        for (const line of lines) {
          const row = document.createElement("span");
          const kind = diffLineKind(line);
          row.className = kind ? `ab-diff-line ab-diff-${kind}` : "ab-diff-line";
          row.textContent = line === "" ? " " : line;
          fragment.append(row, document.createTextNode("\n"));
        }
        if (omitted > 0) {
          const more = document.createElement("span");
          more.className = "ab-diff-line ab-diff-more";
          more.textContent = `… 还有 ${omitted} 行（展开「查看回执」看全部）`;
          fragment.append(more);
        }
        diffEl.replaceChildren(fragment);
        diffEl.dataset.diffText = diffText;
        diffEl.hidden = false;
      }
    }
    const copy = card.querySelector(".receipt-copy");
    let disclosure = copy.querySelector("details");
    if (presentation.raw && presentation.raw !== presentation.summary) {
      if (!disclosure) {
        disclosure = element("details");
        const summary = element("summary", "", "查看回执");
        const raw = element("pre");
        raw.dataset.itemPart = "raw";
        disclosure.append(summary, raw);
        copy.append(disclosure);
      }
      setTextIfChanged(disclosure.querySelector('[data-item-part="raw"]'), presentation.raw);
    } else if (disclosure) {
      disclosure.remove();
    }
    return true;
  }

  /* 每秒刷新「正在跑」的徽标 —— 对齐 CLI（它的 `running (Ns)` 也是每秒 tick）。
   * 只在真有进行中的 turn 时才碰 DOM；没在跑就立刻返回，零开销。
   * ⚠️ 2026-09-19：中途我误删过一次（以为卡片都不显示了）—— 现已恢复：工具卡照旧带秒数
   *   （CLI `bash running (3s)`）；思考卡不带（CLI 是 `… reasoning live`，时长只在完成后拼）。 */
  function refreshRunningBadges() {
    if (!dom.transcript) return;
    let busy = false;
    for (const turn of app.threadState.turns.values()) {
      if (turn.status === "in_progress") { busy = true; break; }
    }
    if (!busy) return;
    for (const node of dom.transcript.children) {
      const id = node.dataset && node.dataset.itemId;
      if (!id) continue;
      const item = app.threadState.items.get(id);
      if (!item || item.status !== "in_progress") continue;
      if (node.classList.contains("receipt")) {
        setTextIfChanged(
          node.querySelector('[data-item-part="label"]'),
          receiptPresentation(item).label + runningElapsedSuffix(item),
        );
      }
    }
  }

  function setTextIfChanged(target, value) {
    const next = value == null ? "" : String(value);
    if (target.textContent !== next) setSafeText(target, next);
  }

  /* ── 极简 Markdown 渲染（2026-09-16 老板：「回复全都好像是 md 格式，带着很多符号」）──
   * 为什么必须有：AI 的回复本来就是 Markdown，而**官方 web 前端只做纯文本显示**
   * （CLI / TUI 是渲染的，web 没有 —— 全文 grep 不到任何 markdown 库）→ 客户看到一堆 `##`、`**`、`|`。
   * 按平台原则「系统能 100% 保证的就做进系统」：这是**显示层的确定性行为**，不该靠 prompt 求模型别用符号。
   * ⚠️ 安全第一：**先把整段 HTML 转义**，再套自己的标记 —— AI 或用户文本里可能有 `<script>`，
   *    直接 innerHTML 就是 XSS。只支持实际会遇上的语法，不引第三方库、不发网络请求。
   */
  function renderMarkdown(md) {
    const esc = String(md == null ? "" : md)
      .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    // 行内：代码 → 链接 → 粗体 → 删除线 → 斜体。
    // 链接**只放行 http/https** —— `javascript:` / `data:` 一律退化成普通文字（XSS 防线）。
    const inline = (s) => s
      .replace(/`([^`]+)`/g, "<code>$1</code>")
      .replace(/\[([^\]\n]+)\]\((https?:[^)\s]+)\)/g,
        '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>')
      .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
      .replace(/~~([^~\n]+)~~/g, "<del>$1</del>")
      .replace(/(^|[^*])\*([^*\n]+)\*/g, "$1<em>$2</em>");

    const out = [];
    let lists = [];      // 嵌套列表栈：[{ tag, li }]；下标 = 层级-1，li = 这一层的 <li> 还开着
    let table = false;
    let quote = false;
    let code = null;     // 代码块（**流式时常常还没闭合**，那种也要当代码块）
    const closeTable = () => { if (table) { out.push("</tbody></table>"); table = false; } };
    const closeQuote = () => { if (quote) { out.push("</blockquote>"); quote = false; } };
    // 关到剩 keep 层：**每层都要先把它的 <li> 收掉**，否则子列表会跑到 <li> 外面
    const closeLists = (keep) => {
      while (lists.length > keep) {
        const top = lists[lists.length - 1];
        if (top.li) { out.push("</li>"); top.li = false; }
        out.push("</" + top.tag + ">");
        lists.pop();
      }
    };
    // 开一个列表项：嵌套时**父层的 <li> 保持开着** —— 子列表要嵌在它里面
    const openItem = (level, tag, html) => {
      while (lists.length > level) {
        const top = lists[lists.length - 1];
        if (top.li) { out.push("</li>"); top.li = false; }
        out.push("</" + top.tag + ">");
        lists.pop();
      }
      if (lists.length === level && lists[level - 1].tag !== tag) {
        const top = lists.pop();                         // 同一层换了类型
        if (top.li) out.push("</li>");
        out.push("</" + top.tag + ">");
      }
      while (lists.length < level) { out.push("<" + tag + ">"); lists.push({ tag: tag, li: false }); }
      if (lists[level - 1].li) { out.push("</li>"); lists[level - 1].li = false; }
      out.push(html);
      lists[level - 1].li = true;
    };
    const closeAll = () => { closeLists(0); closeTable(); closeQuote(); };
    const flushCode = () => {
      const b = code; code = null;
      const cls = b.lang ? ' class="lang-' + b.lang.replace(/[^a-zA-Z0-9_+-]/g, "") + '"' : "";
      out.push("<pre><code" + cls + ">" + b.lines.join("\n") + "</code></pre>");
    };

    for (const raw of esc.split(/\r?\n/)) {
      const line = raw.replace(/\s+$/, "");
      // 代码围栏：三个及以上反引号；开 / 关都走这里
      const fence = line.match(/^\s*```+\s*([^\s`]*)\s*$/);
      if (fence) {
        if (code) flushCode(); else { closeAll(); code = { lang: fence[1] || "", lines: [] }; }
        continue;
      }
      if (code) { code.lines.push(line); continue; }   // 代码块内部：原样，不解析任何标记

      const h = line.match(/^(#{1,4})\s+(.*)$/);
      if (h) {
        closeAll();
        out.push("<h" + h[1].length + ">" + inline(h[2]) + "</h" + h[1].length + ">");
        continue;
      }
      if (/^\s*\|.*\|\s*$/.test(line)) {                 // 表格行
        if (/^\s*\|[-\s:|]+\|\s*$/.test(line)) continue; // |---|---| 分隔行丢掉
        closeLists(0); closeQuote();
        const cells = line.trim().replace(/^\||\|$/g, "").split("|").map((c) => c.trim());
        if (!table) { out.push("<table><tbody>"); table = true; }
        out.push("<tr>" + cells.map((c) => "<td>" + inline(c) + "</td>").join("") + "</tr>");
        continue;
      }
      closeTable();

      const q = line.match(/^\s*&gt;\s?(.*)$/);          // 引用块（`>` 已被转义成 &gt;）
      if (q) {
        closeLists(0);
        if (!quote) { out.push("<blockquote>"); quote = true; }
        if (q[1] !== "") out.push("<p>" + inline(q[1]) + "</p>");
        continue;
      }
      closeQuote();

      const ul = line.match(/^(\s*)[-*+]\s+(.*)$/);
      const ol = line.match(/^(\s*)\d+[.)]\s+(.*)$/);
      if (ul || ol) {
        const hit = ul || ol;
        // 缩进 / 2 = 层级（1 起）；两个空格一级。**夹一下**：跳级时（比如直接从 0 到 2）
        // 只允许比当前深一层，否则会生成没有父 <li> 的孤儿列表。
        const rawLevel = Math.floor(hit[1].replace(/\t/g, "  ").length / 2) + 1;
        const level = Math.min(rawLevel, lists.length + 1);
        const tag = ul ? "ul" : "ol";
        const item = hit[2];
        const task = item.match(/^\[([ xX])\]\s+(.*)$/);   // 任务列表 - [ ] / - [x]
        const html = task
          ? '<li class="task' + (task[1].toLowerCase() === "x" ? " done" : "") +
            '"><span class="task-box">' + (task[1].toLowerCase() === "x" ? "☑" : "☐") +
            "</span>" + inline(task[2]) + "</li>"
          : "<li>" + inline(item) + "</li>";
        // ⚠️ openItem 把整个 <li> 一起推出去，所以先把 html 尾部的 </li> 去掉重拼：
        //    嵌套时那个 </li> 要留到子列表之后才关。
        openItem(level, tag, html.replace(/<\/li>$/, ""));
        continue;
      }
      closeLists(0);
      if (line === "") continue;
      out.push("<p>" + inline(line) + "</p>");
    }
    if (code) flushCode();   // 流式：围栏还没闭合 —— 也要收成代码块，别把反引号露给客户
    closeAll();
    return out.join("");
  }

  /** 会话预览/标题是一行纯文本，不能上 HTML 渲染 —— 但也不能把 `**` `#` 这类符号原样露给客户
   *  （2026-09-16 老板截图：右侧正文已经干净了，而**左侧会话预览摘要里还露着** `**都已经做进程序里了**`、`# 一句话结论`）。
   *  预览只有一行，渲染成表格/标题没意义 → 只把符号剥掉、文字一字不改。 */
  function stripMarkdown(text) {
    return String(text == null ? "" : text)
      .replace(/```[\s\S]*?```/g, " ")
      .replace(/`([^`]*)`/g, "$1")
      // `#` 标题号：行首，或前面是空白/中英标点（`情况是这样：# 一句话结论` 这种行中的也要剥）。
      // ⚠️ 不能无脑剥 `# ` —— 会把 `C# 语言` 这种真的代码名也削掉。
      .replace(/(^|[\s:：,，;；、])#{1,6}\s+/g, "$1")
      .replace(/^\s{0,3}>\s?/gm, "")
      .replace(/^\s{0,3}[-*+]\s+/gm, "")
      .replace(/\*\*([^*]*)\*\*/g, "$1")
      .replace(/__([^_]*)__/g, "$1")
      .replace(/(^|[^*])\*([^*\n]+)\*/g, "$1$2")
      .replace(/^\s*\|.*\|\s*$/gm, " ")
      .replace(/\s+/g, " ")
      .trim();
  }

  /** 只在 HTML 真变了才写 DOM（每帧重渲染时避免白刷 + 不打断选中） */
  function setHtmlIfChanged(target, html) {
    if (!target || target.__asbudyHtml === html) return;
    target.__asbudyHtml = html;
    target.innerHTML = html;
  }

  /* ── Markdown 渲染的配套样式（2026-09-16）──
   * 为什么不写在 styles.css 里：app.mjs 和 styles.css 是**两个文件、各自缓存** ——
   * 出现过「JS 是新的（会渲染 markdown）＋ CSS 还是旧的（没有配套样式）」→ 标题按浏览器
   * **默认**渲染成 2em（约 33px）加粗（老板当天就撞上，形容“非常大、而且加粗了”）。
   * 把样式跟渲染器写在**同一个文件**里 → 两者版本永远一致，不会再错位。
   * 尺寸都收在正文一档：标题只是一段回复里的小标题，**不能像网页 h1 那样巨大**。 */
  const MARKDOWN_CSS = [
    '.message-body h1,.message-body h2,.message-body h3,.message-body h4{font-size:1em;font-weight:600;margin:12px 0 6px;line-height:1.4}',
    '.message-body h1:first-child,.message-body h2:first-child,.message-body h3:first-child,.message-body h4:first-child{margin-top:0}',
    '.message-body strong{font-weight:600}',
    '.message-body p{margin:6px 0}',
    '.message-body p:first-child{margin-top:0}',
    '.message-body ul,.message-body ol{margin:6px 0;padding-left:22px}',
    '.message-body li{margin:3px 0}',
    '.message-body code{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:.92em;background:rgba(110,118,129,.25);padding:1px 5px;border-radius:4px}',
    '.message-body table{border-collapse:collapse;margin:8px 0;font-size:.95em;display:block;overflow-x:auto;max-width:100%}',
    '.message-body td{border:1px solid #30363d;padding:4px 9px;text-align:left;white-space:nowrap}',
    '.message-body tr:first-child td{font-weight:600;background:rgba(110,118,129,.12)}',
    /* ── 代码块 / 链接 / 引用 / 删除线 / 任务列表（2026-09-20 补，照 CLI 的 markdown_render.rs）── */
    '.message-body pre{margin:8px 0;padding:10px 12px;background:rgba(110,118,129,.15);border:1px solid var(--line,rgba(72,215,255,.14));border-radius:6px;overflow-x:auto;max-width:100%}',
    '.message-body pre code{background:none;padding:0;font-size:.9em;line-height:1.5;white-space:pre;border-radius:0}',
    '.message-body blockquote{margin:8px 0;padding:2px 0 2px 12px;border-left:3px solid var(--line,rgba(72,215,255,.35));color:var(--text-soft,#b6c0d4)}',
    '.message-body blockquote p{margin:4px 0}',
    '.message-body a{color:var(--action,#6aaef2);text-decoration:underline;word-break:break-word}',
    '.message-body del{opacity:.65}',
    '.message-body li.task{list-style:none;margin-left:-18px}',
    '.message-body li.task .task-box{margin-right:6px;opacity:.85}',
    '.message-body li.task.done{opacity:.7}',
    /* 兜底：万一有人往正文里写了更高级的选择器，也不能让标题爆尺寸 */
    '.message-body h1,.message-body h2,.message-body h3,.message-body h4,.message-body h5,.message-body h6{font-size:1em!important}',
  ].join("\n");
  (function injectMarkdownStyles() {
    if (!globalThis.document || !document.head) return;
    let tag = document.getElementById("asbudy-md-styles");
    if (!tag) {
      tag = document.createElement("style");
      tag.id = "asbudy-md-styles";
      document.head.appendChild(tag);
    }
    tag.textContent = MARKDOWN_CSS;
  })();

  /* ── 子代理卡的样式（2026-09-20）──
   * 跟 markdown 那套一样，**写在 app.mjs 里**（不往 styles.css 里塞）：
   *   ① 两边各自缓存，写一起才不会「JS 新的 + CSS 旧的」；
   *   ② styles.css 与 index.html 在门卫的 `/assets/` 白名单 + `fork-manifest.sh` 的 precheck
   *      里有硬编码清单（§166 踩过），新加一套样式没必要去动那两个地方。
   * 配色只用官方语义变量（见 styles.css 的 :root），不另创颜色。 */
  const AGENT_CARD_CSS = [
    '.agent-card{display:block;margin:6px 0;padding:5px 10px 6px;border-left:2px solid var(--line,rgba(72,215,255,.14));font-size:13px;line-height:1.5}',
    '.agent-card[data-agent-status="running"],.agent-card[data-agent-status="model_wait"],.agent-card[data-agent-status="starting"],.agent-card[data-agent-status="running_tool"]{border-left-color:var(--status-live,#4fd1c5)}',
    '.agent-card[data-agent-status="queued"],.agent-card[data-agent-status="waiting_for_user"]{border-left-color:var(--status-human,#f0b429)}',
    '.agent-card[data-agent-status="completed"]{border-left-color:var(--ok,#4fd1c5)}',
    '.agent-card[data-agent-status="failed"]{border-left-color:var(--danger,#f87171)}',
    '.agent-card[data-agent-status="cancelled"],.agent-card[data-agent-status="interrupted"]{opacity:.75}',
    '.agent-head{display:flex;align-items:baseline;gap:5px;flex-wrap:wrap}',
    '.agent-glyph{flex:0 0 auto;color:var(--text-dim,#b6c0d4)}',
    '.agent-role{font-weight:600;color:var(--text,#f6f2e8)}',
    '.agent-name{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:12.5px;color:var(--text-dim,#b6c0d4)}',
    '.agent-activity{margin-left:11px;color:var(--text-dim,#b6c0d4);font-size:12.5px;word-break:break-word}',
    '.agent-foot{margin-left:11px;color:var(--text-muted,#8b97ab);font-size:12.5px}',
  ].join("\n");
  (function injectAgentCardStyles() {
    if (!globalThis.document || !document.head) return;
    let tag = document.getElementById("asbudy-agent-styles");
    if (!tag) {
      tag = document.createElement("style");
      tag.id = "asbudy-agent-styles";
      document.head.appendChild(tag);
    }
    tag.textContent = AGENT_CARD_CSS;
  })();

  /* ── 运行汇总行的样式（M10 · 2026-09-20）──
   * 跟 markdown / 子代理卡那两套一样，**写在 app.mjs 里**（不往 styles.css 里塞，理由同上一段）。
   * 颜色只用官方语义变量（styles.css 的 :root）。 */
  const TOOL_RUN_CSS = [
    '.tool-run{display:block;margin:6px 0;font-size:13px;line-height:1.5}',
    '.tool-run-head{display:flex;align-items:baseline;gap:6px;cursor:pointer;color:var(--text-dim,#b6c0d4);list-style:none}',
    '.tool-run-head::-webkit-details-marker{display:none}',
    '.tool-run-head::before{content:"\\25B8";flex:0 0 auto;font-size:10px;opacity:.7}',
    '.tool-run[open]>.tool-run-head::before{content:"\\25BE"}',
    '.tool-run-glyph{flex:0 0 auto;opacity:.55}',
    '.tool-run-text{flex:1 1 auto}',
    '.tool-run-body{margin:4px 0 0 5px;padding-left:9px;border-left:2px solid var(--line,rgba(72,215,255,.14))}',
  ].join("\n");
  (function injectToolRunStyles() {
    if (!globalThis.document || !document.head) return;
    let tag = document.getElementById("asbudy-toolrun-styles");
    if (!tag) {
      tag = document.createElement("style");
      tag.id = "asbudy-toolrun-styles";
      document.head.appendChild(tag);
    }
    tag.textContent = TOOL_RUN_CSS;
  })();

  /* ── 清单 / 待办卡的样式（M12 · 2026-09-20）── */
  const CHECKLIST_CSS = [
    '.checklist-card{display:block;margin:6px 0;padding:5px 10px 6px;border-left:2px solid var(--line,rgba(72,215,255,.14));font-size:13px;line-height:1.5}',
    '.checklist-card[data-cl-state="running"]{border-left-color:var(--status-live,#4fd1c5)}',
    '.checklist-card[data-cl-state="completed"]{border-left-color:var(--ok,#4fd1c5)}',
    '.checklist-card[data-cl-state="failed"]{border-left-color:var(--danger,#f87171)}',
    '.cl-head{display:flex;align-items:baseline;gap:6px;flex-wrap:wrap}',
    '.cl-glyph{flex:0 0 auto;color:var(--text-dim,#b6c0d4)}',
    '.cl-role{font-weight:600;color:var(--text,#f6f2e8)}',
    '.cl-count{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:12.5px;color:var(--text-dim,#b6c0d4)}',
    '.cl-items{margin:3px 0 0 11px;padding:0;list-style:none}',
    '.cl-item{display:flex;gap:6px;align-items:baseline;margin:1px 0}',
    '.cl-mk{flex:0 0 auto}',
    '.cl-item[data-cl-st="done"] .cl-mk{color:var(--ok,#4fd1c5)}',
    '.cl-item[data-cl-st="doing"] .cl-mk{color:var(--status-live,#4fd1c5)}',
    '.cl-item[data-cl-st="failed"] .cl-mk{color:var(--danger,#f87171)}',
    '.cl-item[data-cl-st="todo"] .cl-mk,.cl-item[data-cl-st="skipped"] .cl-mk{color:var(--text-dim,#b6c0d4)}',
    '.cl-tx{flex:1 1 auto;word-break:break-word}',
  ].join("\n");
  (function injectChecklistStyles() {
    if (!globalThis.document || !document.head) return;
    let tag = document.getElementById("asbudy-checklist-styles");
    if (!tag) {
      tag = document.createElement("style");
      tag.id = "asbudy-checklist-styles";
      document.head.appendChild(tag);
    }
    tag.textContent = CHECKLIST_CSS;
  })();

  function captureTranscriptSelection() {
    const selection = globalThis.getSelection?.();
    if (!selection || selection.rangeCount === 0 || selection.isCollapsed) return null;
    const range = selection.getRangeAt(0);
    const start = transcriptSelectionEndpoint(range.startContainer, range.startOffset);
    const end = transcriptSelectionEndpoint(range.endContainer, range.endOffset);
    return start && end ? { start, end } : null;
  }

  function transcriptSelectionEndpoint(node, offset) {
    const elementNode = node.nodeType === 1 ? node : node.parentElement;
    const item = elementNode?.closest?.("[data-item-id]");
    if (!item || !dom.transcript.contains(item)) return null;
    const prefix = document.createRange();
    prefix.selectNodeContents(item);
    try {
      prefix.setEnd(node, offset);
    } catch (_error) {
      return null;
    }
    return { itemId: item.dataset.itemId, offset: prefix.toString().length };
  }

  function restoreTranscriptSelection(captured) {
    if (!captured) return;
    const startRoot = [...dom.transcript.children]
      .find((node) => node.dataset.itemId === captured.start.itemId);
    const endRoot = [...dom.transcript.children]
      .find((node) => node.dataset.itemId === captured.end.itemId);
    if (!startRoot || !endRoot) return;
    const start = textPointAt(startRoot, captured.start.offset);
    const end = textPointAt(endRoot, captured.end.offset);
    if (!start || !end) return;
    const range = document.createRange();
    try {
      range.setStart(start.node, start.offset);
      range.setEnd(end.node, end.offset);
    } catch (_error) {
      return;
    }
    const selection = globalThis.getSelection?.();
    if (!selection) return;
    selection.removeAllRanges();
    selection.addRange(range);
  }

  function textPointAt(root, requestedOffset) {
    const walker = document.createTreeWalker(
      root,
      globalThis.NodeFilter?.SHOW_TEXT || 4,
    );
    let remaining = Math.max(0, requestedOffset);
    let last = null;
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      last = node;
      const length = node.data.length;
      if (remaining <= length) return { node, offset: remaining };
      remaining -= length;
    }
    return last ? { node: last, offset: last.data.length } : null;
  }

  function renderAttention() {
    const existing = new Map(
      [...dom.attention.children]
        .filter((node) => node.dataset.attentionKey)
        .map((node) => [node.dataset.attentionKey, node]),
    );
    const desired = [];
    for (const [approvalId, approval] of app.threadState.approvals) {
      const key = `approval:${approvalId}`;
      const card = existing.get(key) || renderApproval(approvalId, approval);
      card.dataset.attentionKey = key;
      setAttentionCardBusyNode(card, app.inFlightActions.has(key));
      desired.push(card);
    }
    for (const [inputId, input] of app.threadState.userInputs) {
      const key = `input:${inputId}`;
      const card = existing.get(key) || renderUserInput(inputId, input);
      card.dataset.attentionKey = key;
      setAttentionCardBusyNode(card, app.inFlightActions.has(key));
      desired.push(card);
    }
    reconcileChildren(dom.attention, desired);
    dom.attention.hidden = desired.length === 0;
  }

  function setAttentionCardBusy(key, busy) {
    const card = [...dom.attention.children]
      .find((node) => node.dataset.attentionKey === key);
    if (card) setAttentionCardBusyNode(card, busy);
  }

  function setAttentionCardBusyNode(card, busy) {
    card.setAttribute("aria-busy", busy ? "true" : "false");
    for (const control of card.querySelectorAll("button, input, textarea, select")) {
      control.disabled = busy;
    }
  }

  function renderApproval(approvalId, approval) {
    const card = element("article", "attention-card");
    const titleId = `attention-approval-${safeDomId(approvalId)}`;
    card.setAttribute("role", "group");
    card.setAttribute("aria-labelledby", titleId);
    card.append(element("p", "eyebrow", "需要审批"));
    const title = element("h2", "", approval.tool_name || "工具请求");
    title.id = titleId;
    card.append(title);
    card.append(element("p", "", approval.intent_summary || approval.description || "AsBudy 正在等你确认能不能做这一步。"));
    const actions = element("div", "attention-actions");
    const rememberLabel = element("label", "remember-field");
    // ★ 2026-09-19 起这个勾选框由**我们**接管（不再是官方那个「整条对话全自动」）：
    //   勾上 = 把**这一类的分组指纹**记进 `abSessionApproved`，之后同类直接放行；
    //   **不给引擎传 `remember:true`**（那会把整条对话设成永久全自动 —— 我们不要那个）。
    //   分组键由引擎给（`approval_grouping_key`，本次引擎改动才发出来）：它是按操作族算的，
    //   不是工具名 —— 所以批准一条 shell 命令不会连带放行所有 shell 命令。
    const remember = document.createElement("input");
    remember.type = "checkbox";
    rememberLabel.append(remember, document.createTextNode("这类操作以后不再询问"));
    rememberLabel.title = '勾上并点「允许」：这条对话里同类的操作以后直接执行、不再问你。只对同一类生效，不会让整条对话变成全自动。';
    const abGroupingKey = String(approval.approval_grouping_key || "");
    const deny = element("button", "quiet-button danger", "不允许");
    deny.type = "button";
    deny.addEventListener("click", () => resolveApproval(approvalId, "deny", false));
    const allow = element("button", "primary-button", "允许");
    allow.type = "button";
    allow.addEventListener("click", () => {
      if (remember.checked && abGroupingKey) abSessionApproved.add(abGroupingKey);
      resolveApproval(approvalId, "allow", false);   // 永不传 remember:true
    });
    actions.append(rememberLabel, deny, allow);
    card.append(actions);
    return card;
  }

  async function resolveApproval(approvalId, decision, remember) {
    // Authority check before authority action. The approval must belong to the
    // thread we are actually watching, and that thread must be the selected
    // live target — never a saved-session peek, never a row the user has since
    // moved off. Refusals are loud and send nothing.
    const resolved = resolveApprovalTarget(approvalId, app.target, app.threadState);
    if (!resolved.ok) {
      showStatus(refusalMessage(resolved.reason));
      renderAttention();
      return;
    }
    const action = `approval:${approvalId}`;
    if (!claimInFlight(app.inFlightActions, action)) return;
    setAttentionCardBusy(action, true);
    try {
      await api(`/v1/approvals/${encodeURIComponent(resolved.approvalId)}`, {
        method: "POST",
        body: JSON.stringify({ decision, remember }),
      });
      app.threadState.approvals.delete(approvalId);
      showStatus("");
      renderAttention();
    } catch (error) {
      showStatus(error.message);
    } finally {
      app.inFlightActions.delete(action);
      setAttentionCardBusy(action, false);
    }
  }

  function renderUserInput(inputId, envelope) {
    const card = element("form", "attention-card");
    const titleId = `attention-input-${safeDomId(inputId)}`;
    card.setAttribute("role", "group");
    card.setAttribute("aria-labelledby", titleId);
    card.append(element("p", "eyebrow", "需要输入"));
    const title = element("h2", "", "AsBudy 有个问题");
    title.id = titleId;
    card.append(title);
    const questions = Array.isArray(envelope.request?.questions) ? envelope.request.questions : [];
    const groups = [];
    for (const question of questions) {
      const fieldset = element("fieldset", "question-fieldset");
      fieldset.append(element("legend", "", question.question || question.header || "选择一个选项"));
      const controls = [];
      for (const option of Array.isArray(question.options) ? question.options : []) {
        const label = element("label", "answer-option");
        const input = document.createElement("input");
        input.type = question.multi_select ? "checkbox" : "radio";
        input.name = `question-${inputId}-${question.id}`;
        input.value = option.label || "";
        label.append(input);
        const copy = element("span", "", option.label || "Option");
        if (option.description) copy.append(element("small", "", option.description));
        label.append(copy);
        fieldset.append(label);
        controls.push({ input, label: option.label || "", value: option.label || "" });
      }
      // Match the terminal surface: a custom answer stays available even when
      // an older request omitted or disabled the legacy allow_free_text hint.
      const other = document.createElement("input");
      other.className = "other-answer";
      other.type = "text";
      other.placeholder = "其他回答";
      other.setAttribute("aria-label", `${question.header || "Question"} other response`);
      if (!question.multi_select) {
        other.addEventListener("input", () => {
          if (other.value.trim()) {
            for (const control of controls) control.input.checked = false;
          }
        });
        for (const control of controls) {
          control.input.addEventListener("change", () => {
            if (control.input.checked) other.value = "";
          });
        }
      }
      fieldset.append(other);
      card.append(fieldset);
      groups.push({ question, controls, other });
    }
    const actions = element("div", "attention-actions");
    const submit = element("button", "primary-button", "提交回答");
    submit.type = "submit";
    actions.append(submit);
    card.append(actions);
    card.addEventListener("submit", async (event) => {
      event.preventDefault();
      const selections = new Map();
      const freeText = new Map();
      for (const group of groups) {
        const selected = [];
        for (const control of group.controls) {
          if (control.input.checked) selected.push(control.value);
        }
        selections.set(group.question.id, selected);
        freeText.set(group.question.id, group.other.value);
      }
      const resolved = resolveUserInputTarget(inputId, app.target, app.threadState);
      if (!resolved.ok) {
        showStatus(refusalMessage(resolved.reason));
        renderAttention();
        return;
      }
      const built = answersForUserInput(envelope.request, selections, freeText);
      if (!built.ok) {
        const message = built.reason === "missing-answer"
          ? `为 ${built.question} 选择一个答案。`
          : built.reason === "multiple-answers"
            ? `为 ${built.question} 选择一个答案。`
            : `那个问题在提交前发生了变化——没有发送。`;
        showStatus(message);
        return;
      }
      const action = `input:${inputId}`;
      if (!claimInFlight(app.inFlightActions, action)) return;
      setAttentionCardBusy(action, true);
      try {
        await api(`/v1/user-input/${encodeURIComponent(resolved.threadId)}/${encodeURIComponent(resolved.inputId)}`, {
          method: "POST",
          body: JSON.stringify({ answers: built.answers }),
        });
        app.threadState.userInputs.delete(inputId);
        showStatus("");
        renderAttention();
      } catch (error) {
        showStatus(error.message);
      } finally {
        app.inFlightActions.delete(action);
        setAttentionCardBusy(action, false);
      }
    });
    return card;
  }

  function safeDomId(value) {
    return String(value || "item").replace(/[^a-zA-Z0-9_-]/g, "-");
  }

  function latestTurn() {
    const id = app.threadState.turnOrder.at(-1);
    return id ? app.threadState.turns.get(id) : null;
  }

  function activeTurn() {
    const turn = latestTurn();
    return turn && (turn.status === "in_progress" || turn.status === "queued") ? turn : null;
  }

  function renderComposer() {
    // 能不能打字：有一条在看的会话，**或者还没有会话**。
    //   ⚠️ AsBudy：不能只看 threadState.thread。新建的项目进来时引擎里一条会话都没有，
    //   threadState.thread 为空 → 原来的判定把输入框**永久禁用**（客户点进去一个字也打不了，
    //   而空状态还写着「直接在下面说一句你要做什么」—— 文案在骗人）。
    //   没有会话时发送由 sendMessage() 自己建一条（官方逻辑本来就有）。
    //   只读的历史快照（target.kind === "session"，官方叫 peek）仍然禁用：
    //   那种情况下消息没有落点，官方原意如此（见 resolveReplyTarget）。
    const readonlyPeek = app.target?.kind === "session";
    const ready = Boolean(app.threadState.thread) || !readonlyPeek;
    const active = activeTurn();
    const sending = app.inFlightActions.has(composerSendAction);
    dom.composerInput.disabled = sending || !ready;
    dom.send.disabled = sending || !ready || !dom.composerInput.value.trim();
    dom.composer.setAttribute("aria-busy", sending ? "true" : "false");
    dom.interrupt.hidden = !active;
    setSafeText(dom.send, sending ? (active ? "插话中…" : "发送中…") : active ? "插话" : "发送");
  }

  function selectedNewThreadProvider() {
    const providerId = dom.newThreadProvider.value;
    return app.providerCatalog?.providers?.find((provider) => provider.id === providerId) || null;
  }

  function selectedNewThreadModel() {
    const provider = selectedNewThreadProvider();
    return provider?.has_model_catalog
      ? dom.newThreadModel.value
      : dom.newThreadModelInput.value.trim();
  }

  function setNewThreadStatus(message, state = "") {
    setSafeText(dom.newThreadStatus, message || "");
    dom.newThreadStatus.dataset.state = state;
  }

  function renderNewThreadCapability() {
    const provider = selectedNewThreadProvider();
    const modelId = selectedNewThreadModel();
    if (!provider || !modelId) {
      setSafeText(dom.newThreadCapability, "");
      dom.newThreadCapability.dataset.state = "unknown";
      return;
    }
    const model = provider.has_model_catalog
      ? app.newThreadModels.find((entry) => entry.id === modelId)
      : null;
    const presentation = imageInputPresentation(model?.image_input);
    dom.newThreadCapability.dataset.state = presentation.state;
    setSafeText(
      dom.newThreadCapability,
      `${presentation.label} — ${presentation.description}`,
    );
  }

  function syncNewThreadControls() {
    const provider = selectedNewThreadProvider();
    const hasCatalog = Boolean(provider?.has_model_catalog);
    const busy = app.newThreadLoading || app.creatingThread;
    dom.newThreadProvider.disabled = app.creatingThread || !app.providerCatalog?.providers?.length;
    dom.newThreadModel.disabled = busy || !hasCatalog || app.newThreadModels.length === 0;
    dom.newThreadModelInput.disabled = busy || !provider || hasCatalog;
    dom.newThreadCancel.disabled = app.creatingThread;
    dom.newThreadCreate.disabled = busy || !provider || !selectedNewThreadModel();
    renderNewThreadCapability();
  }

  function setNewThreadModelSurface(provider) {
    const hasCatalog = Boolean(provider?.has_model_catalog);
    dom.newThreadModelSelectField.hidden = !hasCatalog;
    dom.newThreadModelInputField.hidden = hasCatalog;
  }

  async function loadNewThreadModels(providerId, preferredModel, generation) {
    if (generation !== app.newThreadGeneration || !dom.newThreadDialog.open) return;
    const provider = app.providerCatalog?.providers?.find((entry) => entry.id === providerId);
    app.newThreadModels = [];
    dom.newThreadModel.replaceChildren();
    dom.newThreadModelInput.value = "";
    setNewThreadModelSurface(provider);
    if (!provider) {
      app.newThreadLoading = false;
      setNewThreadStatus("请选择提供商。", "error");
      syncNewThreadControls();
      return;
    }

    const modelDefault = String(preferredModel || provider.default_model || "").trim();
    if (!provider.has_model_catalog) {
      dom.newThreadModelInput.value = modelDefault;
      app.newThreadLoading = false;
      setNewThreadStatus("");
      syncNewThreadControls();
      return;
    }

    app.newThreadLoading = true;
    setNewThreadStatus("正在加载模型…");
    syncNewThreadControls();
    try {
      const modelEntries = await collectProviderModelPages(provider.id, async (path) => {
        const page = await api(path);
        if (generation !== app.newThreadGeneration || !dom.newThreadDialog.open) {
          throw new Error("The model request was superseded.");
        }
        return page;
      });
      if (generation !== app.newThreadGeneration || !dom.newThreadDialog.open) return;
      const seen = new Set();
      const models = [];
      for (const entry of modelEntries) {
        const id = String(entry?.id || "").trim();
        const key = id.toLowerCase();
        if (!id || seen.has(key)) continue;
        seen.add(key);
        models.push({
          id,
          image_input: ["supported", "unsupported", "unknown"].includes(entry?.image_input)
            ? entry.image_input
            : "unknown",
        });
      }
      if (modelDefault && !seen.has(modelDefault.toLowerCase())) {
        models.unshift({ id: modelDefault, image_input: "unknown" });
      }
      app.newThreadModels = models;
      for (const model of models) {
        const option = document.createElement("option");
        option.value = model.id;
        setSafeText(option, modelOptionLabel(model));
        dom.newThreadModel.append(option);
      }
      const selectedDefault = models.find(
        (model) => model.id.toLowerCase() === modelDefault.toLowerCase(),
      );
      if (selectedDefault) dom.newThreadModel.value = selectedDefault.id;
      app.newThreadLoading = false;
      setNewThreadStatus(
        models.length ? "" : "这个提供商下面没有可用模型。",
        models.length ? "" : "error",
      );
      syncNewThreadControls();
    } catch (error) {
      if (generation !== app.newThreadGeneration || !dom.newThreadDialog.open) return;
      app.newThreadLoading = false;
      setNewThreadStatus(`加载模型失败：${error.message}`, "error");
      syncNewThreadControls();
    }
  }

  // 老板 2026-09-14（待修复清单第 5 条）：不懂编程的客户看到「选提供商/模型」
  // 技术对话框全是噪音 → 直接建好，用运行时默认。失败（如默认没配）才退回老对话框，
  // 不把路堵死。想换模型 → 对话顶部「模型」标签随点随换。
  async function quickNewThread() {
    if (app.creatingThread) return;
    app.creatingThread = true;
    showStatus("");
    const thread = await createThread({}, (message) => showStatus(message));
    app.creatingThread = false;
    if (thread) {
      dom.composerInput.focus();
      return;
    }
    await openNewThreadDialog();
  }

  // 引擎侧默认标题是英文 "New Thread"（runtime_api.rs:1683，改它要编译）→ 显示层兜底翻中文。
  function displayTitle(value) {
    const text = String(value ?? "").trim();
    if (!text || text === "New Thread" || text === "Untitled") return "新会话";
    return text;
  }

  async function openNewThreadDialog() {
    if (dom.newThreadDialog.open || app.creatingThread) return;
    dom.newThreadDialog.showModal();
    dom.newThreadCancel.focus({ preventScroll: true });
    const generation = ++app.newThreadGeneration;
    app.providerCatalog = null;
    app.newThreadModels = [];
    app.newThreadLoading = true;
    dom.newThreadProvider.replaceChildren();
    dom.newThreadModel.replaceChildren();
    dom.newThreadModelInput.value = "";
    setNewThreadStatus("正在加载提供商…");
    renderNewThreadCapability();
    syncNewThreadControls();
    try {
      const catalog = await api("/v1/providers");
      if (generation !== app.newThreadGeneration || !dom.newThreadDialog.open) return;
      const providers = Array.isArray(catalog?.providers)
        ? catalog.providers.filter((provider) => String(provider?.id || "").trim())
        : [];
      if (providers.length === 0) throw new Error("引擎没有返回任何提供商。");
      app.providerCatalog = { ...catalog, providers };
      for (const provider of providers) {
        const option = document.createElement("option");
        option.value = provider.id;
        setSafeText(option, providerOptionLabel(provider));
        dom.newThreadProvider.append(option);
      }
      const defaults = newThreadDefaults(app.providerCatalog);
      dom.newThreadProvider.value = defaults.providerId;
      dom.newThreadProvider.disabled = false;
      dom.newThreadProvider.focus({ preventScroll: true });
      await loadNewThreadModels(defaults.providerId, defaults.model, generation);
    } catch (error) {
      if (generation !== app.newThreadGeneration || !dom.newThreadDialog.open) return;
      app.newThreadLoading = false;
      setNewThreadStatus(`加载提供商失败：${error.message}`, "error");
      syncNewThreadControls();
    }
  }

  async function submitNewThread(event) {
    event.preventDefault();
    const provider = selectedNewThreadProvider();
    let request;
    try {
      request = buildCreateThreadRequest(
        dom.newThreadProvider.value,
        selectedNewThreadModel(),
        provider?.model_provider_id,
      );
    } catch (error) {
      setNewThreadStatus(error.message, "error");
      return;
    }
    app.creatingThread = true;
    setNewThreadStatus("正在创建会话…");
    dom.newThreadDialog.focus({ preventScroll: true });
    syncNewThreadControls();
    const thread = await createThread(
      request,
      (message) => setNewThreadStatus(message, message ? "error" : ""),
    );
    app.creatingThread = false;
    if (thread) {
      dom.newThreadDialog.close();
      dom.composerInput.focus();
      return;
    }
    syncNewThreadControls();
    dom.newThreadProvider.focus({ preventScroll: true });
  }

  async function createThread(request = {}, reportError = showStatus) {
    showStatus("");
    reportError("");
    try {
      const thread = await api("/v1/threads", {
        method: "POST",
        body: JSON.stringify(request),
      });
      await loadThreads("");
      await selectThread(thread.id);
      dom.composerInput.focus();
      return thread;
    } catch (error) {
      reportError(error.message);
      return null;
    }
  }

  async function sendMessage() {
    const prompt = dom.composerInput.value.trim();
    if (!prompt) return;
    // A reply goes to a live thread or nowhere. A saved-session peek must not
    // silently resume-and-send: that would attach the user's message to a
    // thread they never asked to create.
    if (app.target.kind === "session") {
      showStatus(refusalMessage("session-not-live"));
      return;
    }
    if (!claimInFlight(app.inFlightActions, composerSendAction)) return;
    renderComposer();
    showStatus("");
    try {
      let threadId = app.selectedThreadId;
      if (!threadId) {
        const thread = await createThread();
        if (!thread) return;
        threadId = thread.id;
      }
      const resolved = resolveReplyTarget(threadTarget(threadId), app.threadState);
      if (!resolved.ok) {
        showStatus(refusalMessage(resolved.reason));
        return;
      }
      threadId = resolved.threadId;
      const turn = activeTurn();
      if (turn) {
        await api(`/v1/threads/${encodeURIComponent(threadId)}/turns/${encodeURIComponent(turn.id)}/steer`, {
          method: "POST",
          body: JSON.stringify({ prompt }),
        });
      } else {
        await api(`/v1/threads/${encodeURIComponent(threadId)}/turns`, {
          method: "POST",
          body: JSON.stringify({ prompt }),
        });
      }
      // AsBudy：照官方 CLI —— 自己发消息一律回到最新（`ui/dispatch.rs` 的
      // `paint_user_turn_cell` 在用户消息画出来的那一刻就 `scroll_to_bottom()`）。
      // 手机上一屏只有 ~360px，用户看完旧回复常常不在底部；此时发消息更该直接回到最新。
      pinTranscriptToLatest();
      saveDraft(app.drafts, threadId, "");
      dom.composerInput.value = "";
      resizeComposer();
      renderComposer();
      loadThreads().catch((error) => showStatus(error.message));
    } catch (error) {
      showStatus(error.message);
    } finally {
      app.inFlightActions.delete(composerSendAction);
      renderComposer();
    }
  }

  async function interruptTurn() {
    const turn = activeTurn();
    if (!turn || !app.selectedThreadId) return;
    dom.interrupt.disabled = true;
    try {
      await api(`/v1/threads/${encodeURIComponent(app.selectedThreadId)}/turns/${encodeURIComponent(turn.id)}/interrupt`, { method: "POST" });
    } catch (error) {
      showStatus(error.message);
    } finally {
      dom.interrupt.disabled = false;
    }
  }

  async function archiveThread() {
    if (!app.selectedThreadId) return;
    if (!globalThis.confirm("要归档这条会话吗？归档后可在「已归档」列表里找回。")) return;
    try {
      await api(`/v1/threads/${encodeURIComponent(app.selectedThreadId)}`, {
        method: "PATCH",
        body: JSON.stringify({ archived: true }),
      });
      saveDraft(app.drafts, app.selectedThreadId, "");
      stopStream();
      app.selectedThreadId = "";
      app.threadState = createThreadState();
      await loadThreads();
      if (app.summaries[0]) await selectThread(app.summaries[0].id);
      else renderAll();
    } catch (error) {
      showStatus(error.message);
    }
  }

  function openRenameDialog() {
    if (!app.threadState.thread) return;
    dom.renameInput.value = app.threadState.thread.title === "New Thread" ? "" : app.threadState.thread.title || "";
    dom.renameDialog.showModal();
    dom.renameInput.focus();
    dom.renameInput.select();
  }

  // ── 已归档列表（找回入口）──
  async function loadArchivedList() {
    try {
      const list = await api("/v1/threads/summary?include_archived=true&limit=100");
      app.archivedSummaries = Array.isArray(list) ? list.filter((s) => s && s.archived) : [];
    } catch (e) {
      app.archivedSummaries = [];
    }
    renderArchivedList();
  }

  function renderArchivedList() {
    dom.archivedList.replaceChildren();
    const rows = app.archivedSummaries || [];
    dom.archivedEmpty.hidden = rows.length !== 0;
    dom.archivedToggle.hidden = rows.length === 0;
    if (rows.length) dom.archivedToggle.textContent = `已归档（${rows.length}）`;
    for (const s of rows) {
      const row = element("div", "archived-row");
      const title = element("span", "thread-title", displayTitle(s.title));
      const meta = element("span", "thread-meta", relativeTime(s.updated_at));
      const restore = element("button", "quiet-button", "恢复");
      restore.type = "button";
      restore.addEventListener("click", async () => {
        try {
          await api(`/v1/threads/${encodeURIComponent(s.id)}`, {
            method: "PATCH",
            body: JSON.stringify({ archived: false }),
          });
          await loadArchivedList();
          await loadThreads();
        } catch (e) { showStatus(e.message); }
      });
      row.append(title, meta, restore);
      dom.archivedList.append(row);
    }
  }

  function openArchivedDialog() {
    loadArchivedList();
    dom.archivedDialog.showModal();
  }

  async function submitRename(event) {
    event.preventDefault();
    const action = event.submitter?.value;
    if (action !== "save") {
      dom.renameDialog.close();
      return;
    }
    const title = dom.renameInput.value.trim();
    if (!title || !app.selectedThreadId) return;
    try {
      const thread = await api(`/v1/threads/${encodeURIComponent(app.selectedThreadId)}`, {
        method: "PATCH",
        body: JSON.stringify({ title }),
      });
      app.threadState.thread = thread;
      dom.renameDialog.close();
      await loadThreads();
      renderHeader();
    } catch (error) {
      showStatus(error.message);
    }
  }

  function resizeComposer() {
    dom.composerInput.style.height = "auto";
    dom.composerInput.style.height = `${Math.min(dom.composerInput.scrollHeight, 220)}px`;
  }

  function closeRailIfNarrow() {
    if (globalThis.matchMedia("(max-width: 800px)").matches) closeRail();
  }

  dom.railOpen.addEventListener("click", openRail);
  dom.railClose.addEventListener("click", closeRail);
  dom.railScrim.addEventListener("click", closeRail);
  dom.newThread.addEventListener("click", () => void quickNewThread());
  dom.newThreadForm.addEventListener("submit", submitNewThread);
  dom.newThreadProvider.addEventListener("change", () => {
    const provider = selectedNewThreadProvider();
    const generation = ++app.newThreadGeneration;
    void loadNewThreadModels(
      provider?.id || "",
      provider?.default_model || "",
      generation,
    );
  });
  dom.newThreadModel.addEventListener("change", syncNewThreadControls);
  dom.newThreadModelInput.addEventListener("input", syncNewThreadControls);
  dom.newThreadCancel.addEventListener("click", () => {
    if (!app.creatingThread) dom.newThreadDialog.close();
  });
  dom.newThreadDialog.addEventListener("cancel", (event) => {
    if (app.creatingThread) event.preventDefault();
  });
  dom.newThreadDialog.addEventListener("keydown", (event) => {
    trapFocusWithin(event, dom.newThreadDialog);
  });
  dom.newThreadDialog.addEventListener("close", () => {
    app.newThreadGeneration += 1;
    app.newThreadLoading = false;
    setNewThreadStatus("");
  });
  dom.rename.addEventListener("click", openRenameDialog);
  dom.archive.addEventListener("click", archiveThread);
  dom.archivedToggle.addEventListener("click", openArchivedDialog);
  dom.archivedClose.addEventListener("click", () => dom.archivedDialog.close());
  dom.renameForm.addEventListener("submit", submitRename);
  dom.interrupt.addEventListener("click", interruptTurn);
  dom.composer.addEventListener("submit", (event) => {
    event.preventDefault();
    sendMessage();
  });
  dom.composerInput.addEventListener("input", () => {
    saveDraft(app.drafts, app.selectedThreadId, dom.composerInput.value);
    resizeComposer();
    renderComposer();
  });
  dom.composerInput.addEventListener("keydown", (event) => {
    if (!isComposerSubmitKey(event)) return;
    event.preventDefault();
    sendMessage();
  });
  dom.search.addEventListener("input", () => {
    if (app.searchTimer) clearTimeout(app.searchTimer);
    app.searchTimer = setTimeout(() => {
      loadThreads().catch((error) => showStatus(error.message));
      loadSessions().catch(() => {});
    }, 180);
  });
  document.addEventListener("keydown", (event) => {
    if (dom.newThreadDialog.open) return;
    if (trapRailFocus(event)) return;
    if (event.key === "Escape" && narrowRail.matches && dom.shell.classList.contains("rail-visible")) {
      event.preventDefault();
      closeRail();
    }
  });
  narrowRail.addEventListener("change", syncRailAccessibility);
  // AsBudy：官方 web 从来不监听对话区的滚动（只靠每次渲染量像素差猜）——
  // CLI 的滚动状态只由用户输入改变（scrolled_by），这里补上同一件事。
  dom.transcript.addEventListener("scroll", handleTranscriptScroll, { passive: true });
  globalThis.visualViewport?.addEventListener("resize", syncVisualViewport);
  globalThis.visualViewport?.addEventListener("scroll", syncVisualViewport);
  globalThis.addEventListener("resize", syncVisualViewport);
  globalThis.addEventListener("beforeunload", stopStream);

  async function initialize() {
    syncVisualViewport();
    publishTranscriptTail();
    syncRailAccessibility();
    try {
      [app.runtimeInfo, app.workspace] = await Promise.all([
        api("/v1/runtime/info"),
        api("/v1/workspace/status"),
      ]);
      renderRuntimeProvenance(dom.runtimeProvenance, app.runtimeInfo);
      setConnection("ready", "本地引擎已连接");
      await loadThreads();
      await loadSessions();
      loadArchivedList().catch(() => {});
      if (app.summaries[0]) await selectThread(app.summaries[0].id);
      else renderAll();
    } catch (error) {
      setConnection("error", "引擎连接失败");
      showStatus(error.message);
      renderAll();
    }
  }

  initialize();

  // AsBudy：徽标每秒 tick（CLI 同款）—— 见 runningElapsedSuffix 的注释
  setInterval(refreshRunningBadges, 1000);
}

function basename(path) {
  if (!path) return "";
  const normalized = String(path).replaceAll("\\", "/").replace(/\/$/, "");
  return normalized.split("/").at(-1) || normalized;
}

function humanize(value) {
  if (!value) return "状态";
  return String(value)
    .replaceAll("_", " ")
    .replace(/\b\w/g, (letter) => letter.toUpperCase());
}

export function modeLabel(mode) {
  if (mode === "agent") return "工作";
  if (mode === "plan") return "计划";
  if (mode === "operate") return "运维";
  return humanize(mode || "引擎默认");
}

export function formatRuntimeProvenance(runtimeInfo) {
  const version = String(
    runtimeInfo?.codewhale_version || runtimeInfo?.version || "",
  ).trim() || "版本未知";
  const commit = String(runtimeInfo?.codewhale_commit || "").trim();
  const source = /^[0-9a-f]{40}$/i.test(commit)
    ? commit.slice(0, 12)
    : "来源未知";
  return `${version} · ${source}`;
}

export function renderRuntimeProvenance(element, runtimeInfo) {
  return setSafeText(element, formatRuntimeProvenance(runtimeInfo));
}

function permissionLabel(thread) {
  if (thread.trust_mode) return "完全访问";
  if (thread.auto_approve) return "自动审核";
  return "每次询问";
}

function fmtDateTime(value) {
  const ts = Date.parse(value);
  if (!Number.isFinite(ts)) return "";
  const d = new Date(ts);
  const p = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

function relativeTime(value) {
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp)) return "最近";
  const seconds = Math.max(0, Math.round((Date.now() - timestamp) / 1000));
  if (seconds < 60) return "刚刚";
  if (seconds < 3600) return `${Math.floor(seconds / 60)} 分钟前`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)} 小时前`;
  return `${Math.floor(seconds / 86400)} 天前`;
}

if (typeof document !== "undefined") {
  startBrowserClient();
}
