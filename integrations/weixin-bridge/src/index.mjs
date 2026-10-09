import fs from "node:fs/promises";
import path from "node:path";
import crypto from "node:crypto";

import {
  getLoginQR,
  waitForLogin,
  getUpdates,
  sendMessage,
  getConfig,
  notifyStart,
  notifyStop,
  ILinkLoginBase,
  parseList,
  parseBool,
  envFirst,
  extractText,
  parseCommand,
  commandAction,
  preservedChatStateFields,
  splitMessage,
  compactRuntimeError,
  latestRunningTurn,
  helpText,
  processUpdateBatch,
} from "./lib.mjs";
import { renderQrToText } from "./qr.mjs";
import { ThreadStore as CoreThreadStore, writeFileDurable, ApprovalOwnershipError, decideApproval as decideRuntimeApproval } from "../../bridge-core/src/lib.mjs";

// ============================================================================
// 账号持久化
// ============================================================================

function resolveAccountPath(stateDir) {
  return path.join(stateDir, "account.json");
}

async function loadAccount(stateDir) {
  const p = resolveAccountPath(stateDir);
  try {
    const raw = await fs.readFile(p, "utf8");
    return JSON.parse(raw);
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
    return null;
  }
}

async function saveAccount(stateDir, account) {
  const p = resolveAccountPath(stateDir);
  await fs.mkdir(path.dirname(p), { recursive: true, mode: 0o700 });
  await writeFileDurable(p, `${JSON.stringify(account, null, 2)}\n`, { mode: 0o600 });
}

// ============================================================================
// 配置
// ============================================================================

function requiredEnv(name) {
  const value = process.env[name];
  if (!value || !value.trim()) {
    console.error(`Missing required env: ${name}`);
    process.exit(1);
  }
  return value.trim();
}

function requiredEnvFirst(...names) {
  const value = envFirst(process.env, ...names);
  if (!value) {
    console.error(`Missing required env: one of ${names.join(", ")}`);
    process.exit(1);
  }
  return value;
}

// WEIXIN_* is the canonical spelling; the historical WEXIN_* typo names are
// still honored as deprecated aliases (each warns once per process).
const warnedEnvAliases = new Set();

function weixinEnv(name) {
  const legacy = `WEXIN_${name.slice("WEIXIN_".length)}`;
  const primary = envFirst(process.env, name);
  if (primary) return primary;
  const fallback = envFirst(process.env, legacy);
  if (fallback && !warnedEnvAliases.has(legacy)) {
    warnedEnvAliases.add(legacy);
    console.warn(`${legacy} is deprecated; rename it to ${name}.`);
  }
  return fallback;
}

const config = {
  runtimeUrl: (
    envFirst(process.env, "CODEWHALE_RUNTIME_URL", "DEEPSEEK_RUNTIME_URL") ||
    "http://127.0.0.1:7878"
  ).replace(/\/+$/, ""),
  runtimeToken: requiredEnvFirst(
    "CODEWHALE_RUNTIME_TOKEN",
    "DEEPSEEK_RUNTIME_TOKEN"
  ),
  workspace:
    envFirst(process.env, "CODEWHALE_WORKSPACE", "DEEPSEEK_WORKSPACE") ||
    process.cwd(),
  model:
    envFirst(process.env, "CODEWHALE_MODEL", "DEEPSEEK_MODEL") || "auto",
  mode:
    envFirst(process.env, "CODEWHALE_MODE", "DEEPSEEK_MODE") || "agent",
  allowShell: parseBool(
    envFirst(
      process.env,
      "CODEWHALE_ALLOW_SHELL",
      "DEEPSEEK_ALLOW_SHELL"
    ),
    true
  ),
  trustMode: parseBool(
    envFirst(
      process.env,
      "CODEWHALE_TRUST_MODE",
      "DEEPSEEK_TRUST_MODE"
    ),
    false
  ),
  autoApprove: parseBool(
    envFirst(
      process.env,
      "CODEWHALE_AUTO_APPROVE",
      "DEEPSEEK_AUTO_APPROVE"
    ),
    false
  ),
  allowlist: parseList(
    weixinEnv("WEIXIN_CHAT_ALLOWLIST") ||
      envFirst(
        process.env,
        "CODEWHALE_CHAT_ALLOWLIST",
        "DEEPSEEK_CHAT_ALLOWLIST"
      )
  ),
  allowUnlisted: parseBool(
    weixinEnv("WEIXIN_ALLOW_UNLISTED") ||
      envFirst(
        process.env,
        "CODEWHALE_ALLOW_UNLISTED",
        "DEEPSEEK_ALLOW_UNLISTED"
      ),
    false
  ),
  stateDir:
    weixinEnv("WEIXIN_STATE_DIR") ||
    "/var/lib/codewhale-weixin-bot-bridge",
  // Defaults inside stateDir rather than to a second absolute path: setting
  // only WEIXIN_STATE_DIR must not leave the thread map pointing at /var/lib,
  // which fails with EACCES on every incoming message and silently drops it.
  threadMapPath:
    weixinEnv("WEIXIN_THREAD_MAP_PATH") ||
    path.join(
      weixinEnv("WEIXIN_STATE_DIR") || "/var/lib/codewhale-weixin-bot-bridge",
      "thread-map.json"
    ),
  maxReplyChars: Number(weixinEnv("WEIXIN_MAX_REPLY_CHARS") || 3500),
  longPollTimeoutMs: Number(
    weixinEnv("WEIXIN_LONGPOLL_TIMEOUT_MS") || 35000
  ),
  turnTimeoutMs: Number(
    envFirst(
      process.env,
      "CODEWHALE_TURN_TIMEOUT_MS",
      "DEEPSEEK_TURN_TIMEOUT_MS"
    ) || 900000
  ),
};

// ============================================================================
// Runtime API 工具
// ============================================================================

function authHeaders() {
  return {
    Authorization: `Bearer ${config.runtimeToken}`,
    "Content-Type": "application/json",
  };
}

async function readJsonSafe(response) {
  try {
    return await response.json();
  } catch {
    return null;
  }
}

async function runtimeJson(subPath, { method = "GET", body = null, auth = true } = {}) {
  const url = `${config.runtimeUrl}${subPath}`;
  const options = { method, headers: auth ? authHeaders() : {}, signal: AbortSignal.timeout(10000) };
  if (body) options.body = JSON.stringify(body);
  const response = await fetch(url, options);
  const result = await readJsonSafe(response);
  if (!response.ok) {
    const error = new Error(compactRuntimeError(response.status, result));
    error.status = response.status;
    throw error;
  }
  return result;
}

async function* readSse(response) {
  let buffer = "";
  const decoder = new TextDecoder();
  for await (const chunk of response.body) {
    buffer += decoder.decode(chunk, { stream: true });
    const lines = buffer.split("\n");
    buffer = lines.pop() || "";
    for (const line of lines) {
      const trimmed = line.trim();
      if (!trimmed) continue;
      if (trimmed.startsWith("data:")) {
        yield { data: trimmed.slice(5).trim() };
      } else if (trimmed.startsWith("event:")) {
        yield { event: trimmed.slice(6).trim() };
      } else if (trimmed.startsWith("id:")) {
        yield { id: trimmed.slice(3).trim() };
      }
    }
  }
}

// ============================================================================
// 消息发送 — 通过 iLink sendMessage
// ============================================================================

async function sendText(chatId, text, { clientId } = {}) {
  if (!botAccount) {
    throw new Error("Bot account unavailable");
  }
  const chunks = splitMessage(text, config.maxReplyChars);
  for (const [index, chunk] of chunks.entries()) {
    await sendMessage({
      baseUrl: botAccount.baseUrl,
      token: botAccount.token,
      body: {
        msg: {
          to_user_id: chatId,
          client_id: clientId ? `${clientId}-${index}` : crypto.randomUUID(),
          message_type: 2, // BOT
          message_state: 2, // FINISH
          item_list: [{ type: 1, text_item: { text: chunk } }],
          context_token: await getContextToken(chatId),
        },
      },
    });
  }
}

async function getContextToken(chatId) {
  const state = await threadStore.getChat(chatId);
  // Legacy tokens without an account identity are ambiguous; the next inbound
  // message stores a fresh token bound to the active bot account.
  return state?.contextTokenAccountId === accountIdentity()
    ? state.contextToken || undefined
    : undefined;
}

// ============================================================================
// 命令处理（与 feishu/telegram/wechat bridge 一致）
// ============================================================================

async function handleCommand(chatId, command, inbound = {}) {
  const action = commandAction(command);
  switch (action.kind) {
    case "help":
      await sendText(chatId, helpText());
      return;
    case "status":
      await sendStatus(chatId);
      return;
    case "threads":
      await sendThreads(chatId);
      return;
    case "new_thread": {
      try {
        const state = await ensureThread(chatId, { forceNew: true });
        if (inbound.contextToken) {
          await threadStore.patchChat(chatId, {
            contextToken: inbound.contextToken,
            contextTokenAccountId: accountIdentity(),
          });
        }
        await sendText(chatId, `Created thread ${state.threadId}`);
      } catch (error) {
        await sendText(chatId, `New conversation failed: ${error.message}`);
      }
      return;
    }
    case "resume":
      await resumeThread(chatId, action.threadId);
      return;
    case "interrupt":
      await interruptActiveTurn(chatId);
      return;
    case "compact":
      await compactThread(chatId);
      return;
    case "approval":
      await decideApproval(chatId, action);
      return;
    case "set_model":
      await setChatModel(chatId, action.modelName);
      return;
    case "prompt":
      await runPrompt(chatId, action.prompt, inbound);
      return;
    default:
      await sendText(chatId, helpText());
  }
}

async function ensureThread(chatId, { forceNew = false, threadRequest } = {}) {
  let existing = await threadStore.getChat(chatId);
  const previousAccounts = new Set([existing?.bindingAccountId, existing?.pendingAdmission?.accountId, existing?.turnDelivery?.accountId].filter(Boolean));
  const replacingAccount = forceNew && previousAccounts.size === 1 && !previousAccounts.has(accountIdentity());
  if (forceNew && hasPendingWork(existing) && !replacingAccount) throw new Error("This chat still has a pending turn or retained reply. Use /status first.");
  if (existing?.threadId && !forceNew) {
    if (existing.bindingAccountId !== accountIdentity()) throw new Error("This thread belongs to another or unverified bot account. Use /new to create an explicit binding.");
    return existing;
  }

  if (replacingAccount) {
    // Explicit /new retires the previous account's private receipt before any
    // new side effect. It remains in this store, outside automatic recovery.
    const { retiredAccountStates = {}, ...retired } = existing;
    const receipt = crypto.createHash("sha256").update(JSON.stringify(retired)).digest("hex");
    const retained = { ...retiredAccountStates, [receipt]: retired };
    // Fail before creating a new thread; never evict an uncertain submission
    // or reply merely to make room. Existing over-limit stores remain intact.
    if (Object.keys(retained).length > threadStore.options.actionLimit || Buffer.byteLength(JSON.stringify(retained), "utf8") > 1024 * 1024) {
      throw new Error("Retired account receipt storage is full. Ask the local operator to stop the bridge, back up thread-map.json, and reconcile retiredAccountStates before retrying /new.");
    }
    await threadStore.patchChat(chatId, { retiredAccountStates: retained });
    existing = await threadStore.getChat(chatId);
  }

  const effectiveModel = existing?.model || config.model;

  const thread = await runtimeJson("/v1/threads", {
    method: "POST",
    body: threadRequest || {
      model: effectiveModel,
      workspace: config.workspace,
      mode: config.mode,
      allow_shell: config.allowShell,
      trust_mode: config.trustMode,
      auto_approve: config.autoApprove,
      archived: false,
      system_prompt:
        "You are being controlled from a WeChat phone chat via iLink Bot. Keep status updates concise. Ask for tool approvals when needed; do not assume mobile messages imply blanket approval.",
    },
  });

  const state = {
    ...preservedChatStateFields(existing, replacingAccount
      ? ["model", "retiredAccountStates"]
      : ["model", "authorizedIdentity", "contextToken", "contextTokenAccountId", "pendingAdmission", "retiredAccountStates"]),
    threadBindings: [...threadBindings(chatId, existing), { threadId: thread.id, accountId: accountIdentity(), actorId: turnActor(chatId) }].slice(-threadStore.options.actionLimit),
    threadId: thread.id,
    bindingAccountId: accountIdentity(),
    lastSeq: 0,
    activeTurnId: null,
    updatedAt: new Date().toISOString(),
  };
  await threadStore.setChat(chatId, state);
  return state;
}

function accountIdentity() {
  return typeof botAccount?.accountId === "string" ? botAccount.accountId.trim() : "";
}

function turnActor(chatId) {
  return accountIdentity() && chatId ? `weixin-account:${accountIdentity()}:user:${chatId}` : "";
}

function threadBindings(chatId, state) {
  const bindings = (Array.isArray(state?.threadBindings) ? state.threadBindings : []).filter((entry) => entry?.threadId && entry.accountId && entry.actorId);
  if (state?.threadId && state.bindingAccountId) {
    const current = { threadId: state.threadId, accountId: state.bindingAccountId, actorId: `weixin-account:${state.bindingAccountId}:user:${chatId}` };
    if (!bindings.some((entry) => entry.threadId === current.threadId && entry.accountId === current.accountId && entry.actorId === current.actorId)) bindings.push(current);
  }
  return bindings.slice(-threadStore.options.actionLimit);
}

function ownsThread(chatId, state, threadId) {
  return threadBindings(chatId, state).some((entry) => entry.threadId === threadId && entry.accountId === accountIdentity() && entry.actorId === turnActor(chatId));
}

function inboundKey(msg) {
  const id = msg.message_id;
  if (!accountIdentity() || !msg.from_user_id || id === undefined || id === null || String(id).trim() === "") return null;
  return JSON.stringify([accountIdentity(), msg.from_user_id, String(id)]);
}

function hasPendingWork(state) {
  return Boolean(state?.pendingAdmission || state?.activeTurnId || state?.turnDelivery?.outputs?.some((entry) => entry.status !== "accepted"));
}

function ownsRecovery(chatId, state) {
  const pending = state?.pendingAdmission || state?.turnDelivery;
  return pending?.accountId === accountIdentity() && pending?.actorId === turnActor(chatId) && isAllowed(chatId);
}

function supportsAdmissionRecovery() {
  return runtimeCapabilities.turn_operation_idempotency === true && runtimeCapabilities.turn_operation_lookup === true;
}

async function runPrompt(chatId, prompt, { messageKey } = {}) {
  if (!prompt.trim()) return sendText(chatId, helpText());
  const current = await threadStore.getChat(chatId);
  if (hasPendingWork(current)) {
    startChatRecovery(chatId);
    return sendText(chatId, "This chat has a pending turn or retained reply. Use /status or /interrupt.");
  }
  if (current?.threadId && current.bindingAccountId !== accountIdentity()) return sendText(chatId, "This conversation belongs to another or unverified bot account. Use /new to create a new conversation.");
  if (!messageKey || !turnActor(chatId)) throw new Error("A stable incoming message identity is required.");
  const request = {
    prompt, input_summary: prompt.slice(0, 200), model: current?.model || config.model,
    mode: config.mode, allow_shell: config.allowShell, trust_mode: config.trustMode, auto_approve: config.autoApprove,
  };
  // Retain the frozen request before any admission side effect or poll acknowledgement.
  const operationKey = supportsAdmissionRecovery() ? crypto.createHash("sha256").update(messageKey).digest("hex") : null;
  if (operationKey) request.operation_key = operationKey;
  await threadStore.patchChat(chatId, { pendingAdmission: {
    accountId: accountIdentity(), actorId: turnActor(chatId), messageKey, request, operationKey,
    threadId: current?.threadId || null, submitted: false, sinceSeq: null,
    threadRequest: { model: request.model, workspace: config.workspace, mode: request.mode, allow_shell: request.allow_shell,
      trust_mode: request.trust_mode, auto_approve: request.auto_approve, archived: false,
      system_prompt: "You are being controlled from a WeChat phone chat via iLink Bot. Keep status updates concise. Ask for tool approvals when needed; do not assume mobile messages imply blanket approval." },
  } });
  startChatRecovery(chatId);
}

async function reconcileAdmission(chatId) {
  let state = await threadStore.getChat(chatId);
  let pending = state?.pendingAdmission;
  if (!pending || !ownsRecovery(chatId, state)) return;
  if (!pending.threadId) {
    const thread = await ensureThread(chatId, { threadRequest: pending.threadRequest });
    // ensureThread replaces chat state only for a new thread; retain the admission.
    pending = { ...pending, threadId: thread.threadId };
    await threadStore.patchChat(chatId, { pendingAdmission: pending });
  }
  if (pending.sinceSeq === null) {
    const detail = await runtimeJson(`/v1/threads/${encodeURIComponent(pending.threadId)}`);
    if (latestRunningTurn(detail)) {
      await threadStore.patchChat(chatId, { pendingAdmission: { ...pending, blocked: true } });
      return;
    }
    pending = { ...pending, sinceSeq: Number(detail.latest_seq || 0), blocked: false };
    await threadStore.patchChat(chatId, { pendingAdmission: pending });
  }
  let turn;
  if (pending.submitted) {
    if (!pending.operationKey || !supportsAdmissionRecovery()) return;
    try {
      turn = await runtimeJson(`/v1/threads/${encodeURIComponent(pending.threadId)}/turn-operations/${encodeURIComponent(pending.operationKey)}`);
    } catch (error) {
      // 409 is an incomplete durable admission, not permission to create another turn.
      if (error.status !== 404) throw error;
    }
  }
  if (!turn) {
    pending = { ...pending, submitted: true };
    await threadStore.patchChat(chatId, { pendingAdmission: pending });
    const result = await runtimeJson(`/v1/threads/${encodeURIComponent(pending.threadId)}/turns`, { method: "POST", body: pending.request });
    turn = result?.turn;
  }
  if (!turn?.id || turn.thread_id !== pending.threadId) throw new Error("Invalid Engine admission receipt");
  await threadStore.recordTurnOrigin(chatId, pending.threadId, turn.id, pending.actorId);
  await threadStore.patchChat(chatId, {
    pendingAdmission: null, activeTurnId: turn.id, lastSeq: pending.sinceSeq,
    turnDelivery: { accountId: pending.accountId, actorId: pending.actorId, threadId: pending.threadId,
      turnId: turn.id, messageKey: pending.messageKey, nextSeq: pending.sinceSeq, responseText: "", outputs: [], terminal: false },
  });
}

function queueOutput(delivery, key, text) {
  if (!text || delivery.outputs.some((entry) => entry.key === key)) return;
  // Persist one independently acknowledged chunk per API call. A stable client ID
  // helps reconciliation but iLink does not promise exactly-once recipient delivery.
  for (const [index, chunk] of splitMessage(text, config.maxReplyChars).entries()) {
    const chunkKey = `${key}:${index}`;
    if (delivery.outputs.some((entry) => entry.key === chunkKey)) continue;
    delivery.outputs.push({ key: chunkKey, text: chunk, status: "queued",
      clientId: crypto.createHash("sha256").update(`${delivery.accountId}:${delivery.turnId}:${chunkKey}`).digest("hex") });
  }
}

function finishDelivery(delivery, status) {
  if (delivery.terminal) return;
  delivery.terminal = true;
  delivery.status = status;
  const text = delivery.responseText.trim();
  queueOutput(delivery, "result", [text, status !== "completed" ? `Turn ${status}.` : (!text ? "Turn completed." : "")].filter(Boolean).join("\n\n"));
}

async function saveDelivery(chatId, delivery) {
  await threadStore.patchChat(chatId, { turnDelivery: delivery, lastSeq: delivery.nextSeq });
}

async function flushOutputs(chatId) {
  let state = await threadStore.getChat(chatId);
  if (!state?.turnDelivery || !ownsRecovery(chatId, state)) return;
  let delivery = structuredClone(state.turnDelivery);
  if (threadStore.turnOrigin(chatId, delivery.threadId, delivery.turnId)?.actorId !== delivery.actorId) return;
  for (const entry of delivery.outputs) {
    if (entry.status === "accepted" || entry.status === "uncertain") continue;
    if (entry.status === "sending") {
      entry.status = "uncertain";
      await saveDelivery(chatId, delivery);
      continue;
    }
    entry.status = "sending";
    await saveDelivery(chatId, delivery);
    try {
      await sendText(chatId, entry.text, { clientId: entry.clientId });
      entry.status = "accepted";
    } catch (error) {
      entry.status = error.deliveryStatus === "rejected" ? "queued" : "uncertain";
      await saveDelivery(chatId, delivery);
      return;
    }
    await saveDelivery(chatId, delivery);
  }
  if (delivery.terminal && delivery.outputs.every((entry) => entry.status === "accepted")) {
    await threadStore.patchChat(chatId, { activeTurnId: null });
  }
}

function approvalText(approval) {
  const id = approval.approval_id || approval.id;
  return ["Approval required", `tool=${approval.tool_name || "unknown"}`, id ? `approval_id=${id}` : "",
    approval.description || "", id ? `Reply /allow ${id} or /deny ${id}` : "Use the terminal to decide this approval."].filter(Boolean).join("\n");
}

async function reconcileSnapshot(chatId) {
  const state = await threadStore.getChat(chatId);
  if (!state?.turnDelivery || !ownsRecovery(chatId, state)) return;
  const delivery = structuredClone(state.turnDelivery);
  const detail = await runtimeJson(`/v1/threads/${encodeURIComponent(delivery.threadId)}`);
  const turn = detail.turns?.find((entry) => entry.id === delivery.turnId && entry.thread_id === delivery.threadId);
  if (!turn) throw new Error("Accepted Engine turn is unavailable");
  const items = (detail.items || []).filter((item) => item.turn_id === delivery.turnId && item.kind === "agent_message");
  if (items.some((item) => typeof item.detail !== "string")) throw new Error("Engine snapshot lacks full assistant output");
  if (items.length) delivery.responseText = items.map((item) => item.detail).join("\n\n");
  delivery.nextSeq = Math.max(delivery.nextSeq, Number(detail.latest_seq || 0));
  for (const approval of detail.pending_approvals || []) {
    if (approval.turn_id === delivery.turnId) queueOutput(delivery, `approval:${approval.approval_id || approval.id}`, approvalText(approval));
  }
  if (["completed", "failed", "canceled", "interrupted"].includes(turn.status)) finishDelivery(delivery, turn.status);
  await saveDelivery(chatId, delivery);
}

async function streamTurnEvents(chatId, controller) {
  let state = await threadStore.getChat(chatId);
  const delivery = structuredClone(state.turnDelivery);
  // Bound headers and streaming together; this never interrupts the Engine turn.
  const timeout = setTimeout(() => controller.abort(), config.turnTimeoutMs);
  try {
    const response = await fetch(`${config.runtimeUrl}/v1/threads/${encodeURIComponent(delivery.threadId)}/events?since_seq=${delivery.nextSeq}`,
      { headers: authHeaders(), signal: controller.signal });
    if (!response.ok) throw new Error("Engine event observation unavailable");
    for await (const event of readSse(response)) {
      if (!event.data) continue;
      const record = JSON.parse(event.data);
      if (record.event === "stream.end") {
        if (record.payload?.retryable === false || record.retryable === false) return false;
        break;
      }
      const seq = Number(record.seq || 0);
      if (seq <= delivery.nextSeq) continue;
      delivery.nextSeq = seq;
      if (record.turn_id === delivery.turnId) {
        if (record.event === "item.delta" && record.payload?.kind === "agent_message") delivery.responseText += record.payload.delta || "";
        if (record.event === "approval.required") {
          const approval = record.payload || {};
          queueOutput(delivery, `approval:${approval.approval_id || approval.id || seq}`, approvalText(approval));
        }
        const status = record.payload?.turn?.status || record.payload?.status;
        if (record.event === "turn.completed") finishDelivery(delivery, status || "completed");
        if (record.event === "turn.lifecycle" && ["failed", "canceled", "interrupted"].includes(status)) finishDelivery(delivery, status);
      }
      // Cursor, accumulator and pending output are committed together before send.
      await saveDelivery(chatId, delivery);
      await flushOutputs(chatId);
      // flushOutputs mutates output states; retain them for the next journal event.
      delivery.outputs = structuredClone((await threadStore.getChat(chatId)).turnDelivery.outputs);
      if (delivery.terminal) return true;
      if (stopping) break;
    }
    return true;
  } finally {
    clearTimeout(timeout);
    controller.abort();
  }
}

function startChatRecovery(chatId) {
  if (stopping || recoveryTasks.has(chatId)) return;
  const task = (async () => {
    while (!stopping) {
      let state = await threadStore.getChat(chatId);
      if (!ownsRecovery(chatId, state)) return;
      try {
        if (state.pendingAdmission) {
          await reconcileAdmission(chatId);
          state = await threadStore.getChat(chatId);
          if (state.pendingAdmission) {
            if (state.pendingAdmission.submitted && !state.pendingAdmission.operationKey) return;
            await sleep(1000);
            continue;
          }
        }
        if (!state.turnDelivery) return;
        await reconcileSnapshot(chatId);
        await flushOutputs(chatId);
        state = await threadStore.getChat(chatId);
        if (state.turnDelivery.terminal) {
          if (!state.turnDelivery.outputs.some((entry) => entry.status === "queued")) return;
          await sleep(1000);
          continue;
        }
        const controller = new AbortController();
        recoveryControllers.set(chatId, controller);
        const retryable = await streamTurnEvents(chatId, controller);
        recoveryControllers.delete(chatId);
        if (retryable === false) return;
      } catch {
        if (stopping) return;
        console.warn("Turn observation interrupted; retaining the accepted turn for recovery.");
      }
      await sleep(1000);
    }
  })().finally(() => { recoveryTasks.delete(chatId); recoveryControllers.delete(chatId); });
  recoveryTasks.set(chatId, task);
}

async function sendStatus(chatId) {
  try {
    const state = await threadStore.getChat(chatId);
    const [health, runtimeInfo, workspace] = await Promise.all([
      runtimeJson("/health", { auth: false }),
      runtimeJson("/v1/runtime/info"),
      runtimeJson("/v1/workspace/status"),
    ]);
    await sendText(
      chatId,
      [
        `user_id=${chatId}`,
        state?.pendingAdmission ? "Turn admission is pending; its original request is retained." : "",
        state?.activeTurnId && ownsRecovery(chatId, state) ? `accepted_turn=${state.activeTurnId}` : "",
        state?.turnDelivery?.outputs?.some((entry) => entry.status === "uncertain") ? "A reply has unconfirmed API acceptance. It is retained for review and will not be resent automatically." : "",
        `runtime=${health.status || "unknown"}`,
        `version=${runtimeInfo.version || "unknown"}`,
        `bind=${runtimeInfo.bind_host}:${runtimeInfo.port}`,
        `auth_required=${runtimeInfo.auth_required}`,
        `workspace=${workspace.workspace}`,
        `git_repo=${workspace.git_repo}`,
        workspace.branch ? `branch=${workspace.branch}` : "",
        `staged=${workspace.staged} unstaged=${workspace.unstaged} untracked=${workspace.untracked}`,
      ]
        .filter(Boolean)
        .join("\n")
    );
  } catch (error) {
    await sendText(chatId, `Status check failed: ${error.message}`);
  }
}

async function sendThreads(chatId) {
  try {
    const state = await threadStore.getChat(chatId);
    const ownedIds = threadBindings(chatId, state)
      .filter((entry) => entry.accountId === accountIdentity() && entry.actorId === turnActor(chatId))
      .map((entry) => entry.threadId);
    if (!ownedIds.length) {
      await sendText(chatId, "No recent runtime threads owned by this account and chat.");
      return;
    }
    const query = new URLSearchParams({ limit: "8", include_archived: "true", thread_ids: ownedIds.join(",") });
    const threads = (await runtimeJson(`/v1/threads/summary?${query}`))
      .filter((thread) => ownsThread(chatId, state, thread.id));
    if (!threads.length) {
      await sendText(chatId, "No recent runtime threads owned by this account and chat.");
      return;
    }
    await sendText(
      chatId,
      threads
        .map((thread) => {
          const status = thread.latest_turn_status || "none";
          return `${thread.id} [${status}] ${thread.title || thread.preview || ""}`;
        })
        .join("\n")
    );
  } catch (error) {
    await sendText(chatId, `Thread listing failed: ${error.message}`);
  }
}

async function resumeThread(chatId, args) {
  const threadId = args.trim();
  if (!threadId) {
    await sendText(chatId, "Usage: /resume <thread_id>");
    return;
  }
  try {
    const existing = await threadStore.getChat(chatId);
    if (hasPendingWork(existing)) throw new Error("This chat still has a pending turn or retained reply. Use /status first.");
    if (!ownsThread(chatId, existing, threadId)) throw new ApprovalOwnershipError("Thread is not owned by this account and chat");
    const detail = await runtimeJson(
      `/v1/threads/${encodeURIComponent(threadId)}`
    );
    await threadStore.setChat(chatId, {
      ...preservedChatStateFields(existing, ["model", "authorizedIdentity", "contextToken", "contextTokenAccountId", "retiredAccountStates"]),
      threadBindings: threadBindings(chatId, existing),
      threadId,
      bindingAccountId: accountIdentity(),
      lastSeq: Number(detail.latest_seq || 0),
      activeTurnId: null,
      updatedAt: new Date().toISOString(),
    });
    await sendText(chatId, `Resumed thread ${threadId}`);
  } catch (error) {
    await sendText(chatId, `Resume failed: ${error.message}`);
  }
}

async function interruptActiveTurn(chatId) {
  const state = await threadStore.getChat(chatId);
  if (!state?.threadId) {
    await sendText(chatId, "No runtime thread recorded for this chat.");
    return;
  }
  try {
    const detail = await runtimeJson(
      `/v1/threads/${encodeURIComponent(state.threadId)}`
    );
    const turnId = state.activeTurnId;
    if (state.turnDelivery?.accountId !== accountIdentity() || threadStore.turnOrigin(chatId, state.threadId, turnId)?.actorId !== turnActor(chatId)) throw new ApprovalOwnershipError("Turn is not owned by this account and chat");
    if (!turnId) {
      await sendText(chatId, "No active turn recorded for this chat.");
      return;
    }
    await runtimeJson(
      `/v1/threads/${encodeURIComponent(state.threadId)}/turns/${encodeURIComponent(turnId)}/interrupt`,
      { method: "POST" }
    );
    await threadStore.patchChat(chatId, {
      activeTurnId: turnId,
      updatedAt: new Date().toISOString(),
    });
    await sendText(chatId, `Interrupt requested for ${turnId}`);
  } catch (error) {
    await sendText(chatId, `Interrupt failed: ${error.message}`);
  }
}

async function compactThread(chatId) {
  try {
    const state = await ensureThread(chatId);
    const result = await runtimeJson(
      `/v1/threads/${encodeURIComponent(state.threadId)}/compact`,
      {
        method: "POST",
        body: { reason: "weixin-bot bridge request" },
      }
    );
    await sendText(
      chatId,
      `Compaction started: ${result.turn?.id || "unknown turn"}`
    );
  } catch (error) {
    await sendText(chatId, `Compact failed: ${error.message}`);
  }
}

async function decideApproval(chatId, action) {
  const decision = action.decision;
  const { approvalId, remember } = action;
  if (!approvalId) {
    await sendText(
      chatId,
      `Usage: /${decision} <approval_id>${decision === "allow" ? " [remember]" : ""}`
    );
    return;
  }
  try {
    await decideRuntimeApproval(runtimeJson, { store: threadStore, chatId, actorId: turnActor(chatId), approvalId, decision, remember });
    await sendText(
      chatId,
      `Approval ${approvalId}: ${decision}${remember ? " and remember" : ""}`
    );
  } catch (error) {
    if (error instanceof ApprovalOwnershipError) {
      await sendText(chatId, `Approval ${approvalId} is not waiting in this chat.`);
      return;
    }
    await sendText(chatId, `Approval failed: ${error.message}`);
  }
}

async function setChatModel(chatId, modelName) {
  if (!modelName || modelName === "default") {
    await threadStore.patchChat(chatId, {
      model: null,
      updatedAt: new Date().toISOString(),
    });
    await sendText(
      chatId,
      `Reset per-chat model. Using bridge default: ${config.model}`
    );
    return;
  }
  await threadStore.patchChat(chatId, {
    model: modelName,
    updatedAt: new Date().toISOString(),
  });
  await sendText(chatId, `Per-chat model set to: ${modelName}`);
}

// ============================================================================
// 主循环 — 长轮询 getUpdates
// ============================================================================

let botAccount = null;
let stopping = false;
let threadStore;
const recoveryTasks = new Map();
const recoveryControllers = new Map();
let runtimeCapabilities = {};

function resolveSyncBufPath(stateDir) {
  return path.join(stateDir, "sync-buf.txt");
}

async function loadSyncBuf(stateDir) {
  const p = resolveSyncBufPath(stateDir);
  try {
    return await fs.readFile(p, "utf8");
  } catch {
    return "";
  }
}

async function saveSyncBuf(stateDir, buf) {
  const p = resolveSyncBufPath(stateDir);
  // The state dir may not exist yet on a first run whose first persisted write
  // is the poll cursor rather than account.json.
  await fs.mkdir(path.dirname(p), { recursive: true, mode: 0o700 });
  await writeFileDurable(p, buf, { mode: 0o600 });
}

/** One durably claimed inbound message; agent observation runs separately. */
async function handleInbound(msg) {
  const fromUser = msg.from_user_id || "";
  if (msg.message_type !== undefined && msg.message_type !== 1) return;
  if (!isAllowed(fromUser)) return;
  const text = extractText(msg.item_list);
  if (msg.context_token) {
    const state = await threadStore.getChat(fromUser);
    // A pending admission already owns its context before a thread is bound.
    // Preserve foreign or unattributed receipts until /new retires them.
    if ((!state?.bindingAccountId || state.bindingAccountId === accountIdentity())
      && (!state?.pendingAdmission || state.pendingAdmission.accountId === accountIdentity())
      && (!state?.turnDelivery || state.turnDelivery.accountId === accountIdentity())) {
      await threadStore.patchChat(fromUser, {
        contextToken: msg.context_token,
        contextTokenAccountId: accountIdentity(),
      });
    }
  }
  if (!text) return sendText(fromUser, "仅支持文本消息。图片/语音/视频/文件暂不支持。");
  await handleCommand(fromUser, parseCommand(text), {
    messageKey: inboundKey(msg),
    contextToken: msg.context_token,
  });
}

async function monitorLoop() {
  const { baseUrl, token } = botAccount;
  let getUpdatesBuf = await loadSyncBuf(config.stateDir);
  let nextTimeoutMs = config.longPollTimeoutMs;
  let consecutiveFailures = 0;

  console.log(`Monitor started: baseUrl=${baseUrl} timeoutMs=${nextTimeoutMs}`);

  while (!stopping) {
    try {
      const abortController = new AbortController();
      const timer = setTimeout(
        () => abortController.abort(),
        nextTimeoutMs + 5000
      );

      const resp = await getUpdates({
        baseUrl,
        token,
        get_updates_buf: getUpdatesBuf,
        timeoutMs: nextTimeoutMs,
        signal: abortController.signal,
      });

      clearTimeout(timer);

      if (resp.longpolling_timeout_ms) {
        nextTimeoutMs = resp.longpolling_timeout_ms;
      }

      // 检查错误
      const isApiError =
        (resp.ret !== undefined && resp.ret !== 0) ||
        (resp.errcode !== undefined && resp.errcode !== 0);

      if (isApiError) {
        consecutiveFailures += 1;
        console.error(
          `getUpdates error: ret=${resp.ret} errcode=${resp.errcode} errmsg=${resp.errmsg}`
        );
        if (consecutiveFailures >= 3) {
          console.error("3 consecutive failures, backing off 30s");
          await sleep(30000);
          consecutiveFailures = 0;
        } else {
          await sleep(2000);
        }
        continue;
      }

      consecutiveFailures = 0;

      await processUpdateBatch({
        messages: resp.msgs,
        nextCursor: resp.get_updates_buf,
        store: threadStore,
        keyOf: (msg) => {
          const key = inboundKey(msg);
          if (!key) console.warn("Ignored inbound message without a stable account/peer/message identity.");
          return key;
        },
        handle: handleInbound,
        interrupted: async (msg) => {
          const key = inboundKey(msg);
          const state = await threadStore.getChat(msg.from_user_id);
          if (state?.pendingAdmission?.messageKey === key || state?.turnDelivery?.messageKey === key) {
            startChatRecovery(msg.from_user_id);
          } else if (commandAction(parseCommand(extractText(msg.item_list))).kind === "prompt") {
            // Prompt admission always persists before side effects. No receipt
            // means this claim crashed before admission, so it can be handled.
            await handleInbound(msg);
          } else if (isAllowed(msg.from_user_id)) {
            await sendText(msg.from_user_id, "The bridge restarted while handling this command. Its effect is uncertain; check /status before repeating it.");
          }
        },
        commitCursor: async (cursor) => {
          await saveSyncBuf(config.stateDir, cursor);
          getUpdatesBuf = cursor;
        },
      });
      for (const [chatId, state] of threadStore.listChats()) {
        if (hasPendingWork(state)) startChatRecovery(chatId);
      }
    } catch (error) {
      if (error.name === "AbortError" || error.message?.includes("abort")) {
        // 长轮询超时是正常的，立即重试
        continue;
      }
      if (stopping) break;

      consecutiveFailures += 1;
      console.error(
        `getUpdates exception (${consecutiveFailures}/3):`,
        error.message
      );
      if (consecutiveFailures >= 3) {
        console.error("3 consecutive exceptions, backing off 30s");
        await sleep(30000);
        consecutiveFailures = 0;
      } else {
        await sleep(2000);
      }
    }
  }
}

function isAllowed(fromUser) {
  if (config.allowUnlisted) return true;
  const allowed = new Set(config.allowlist);
  return allowed.has(fromUser);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// ============================================================================
// 启动流程 — QR 登录 → 长轮询
// ============================================================================

async function main() {
  console.log("Starting CodeWhale Weixin Bot Bridge");
  console.log(`Runtime: ${config.runtimeUrl}`);
  console.log(`Workspace: ${config.workspace}`);
  console.log(`State dir: ${config.stateDir}`);
  console.log(`Thread map: ${config.threadMapPath}`);

  // 初始化 ThreadStore。`open()` 只读，真正的写入发生在第一条消息到达时；
  // 那时失败会被 getUpdates 的 catch 吞掉，表现为“微信没有回应”。所以这里
  // 先建目录并真实写一次探针文件，把问题在启动时就暴露出来。
  try {
    const dir = path.dirname(config.threadMapPath);
    await fs.mkdir(dir, { recursive: true, mode: 0o700 });
    const probe = path.join(dir, ".write-probe");
    await fs.writeFile(probe, "", { mode: 0o600 });
    await fs.rm(probe, { force: true });
  } catch (error) {
    console.error(
      `Thread map directory is not writable: ${path.dirname(config.threadMapPath)} (${error.message})`
    );
    console.error(
      "Set WEIXIN_STATE_DIR (or WEIXIN_THREAD_MAP_PATH) to a writable directory."
    );
    process.exit(1);
  }
  threadStore = await CoreThreadStore.open(config.threadMapPath, { messageLimit: 500, privateMode: true });

  // 尝试加载已有账号
  botAccount = await loadAccount(config.stateDir);

  if (botAccount?.token) {
    console.log("Loaded existing bot account, trying to resume...");
    console.log(`  accountId: ${botAccount.accountId}`);
    console.log(`  baseUrl: ${botAccount.baseUrl}`);
  } else {
    // QR 登录
    console.log("No bot account found. Starting QR login...");
    console.log("");

    const { qrcodeUrl, sessionKey } = await getLoginQR();
    console.log("请用微信扫描以下二维码登录：");
    // Render the login URL as a scannable terminal QR. The URL is printed too,
    // so a terminal that mangles the half-block glyphs still has a way through.
    try {
      if (qrcodeUrl) console.log(renderQrToText(qrcodeUrl));
    } catch (error) {
      console.warn(`Could not render QR in terminal: ${error.message}`);
    }
    console.log(qrcodeUrl);
    console.log("");

    const result = await waitForLogin({ sessionKey, timeoutMs: 300_000 });

    if (!result.connected) {
      console.error(`Login failed: ${result.message}`);
      process.exit(1);
    }

    botAccount = {
      accountId: result.accountId,
      token: result.botToken,
      baseUrl: result.baseUrl,
      userId: result.userId,
    };

    await saveAccount(config.stateDir, botAccount);
    console.log(`✅ Login successful! accountId=${botAccount.accountId}`);
  }

  if (!accountIdentity()) throw new Error("The saved bot account has no stable account identity; pair again before receiving messages.");
  try { runtimeCapabilities = (await runtimeJson("/v1/runtime/info"))?.capabilities || {}; } catch { console.warn("Engine recovery capabilities unavailable; uncertain admissions will remain retained."); }
  for (const [chatId, state] of threadStore.listChats()) {
    if (hasPendingWork(state)) startChatRecovery(chatId);
  }

  // 通知上线
  try {
    const startResp = await notifyStart({
      baseUrl: botAccount.baseUrl,
      token: botAccount.token,
    });
    if (startResp.ret && startResp.ret !== 0) {
      console.warn(`notifyStart: ret=${startResp.ret} errmsg=${startResp.errmsg}`);
    } else {
      console.log("notifyStart: OK");
    }
  } catch (error) {
    console.error("notifyStart failed:", error.message);
  }

  // 信号处理
  process.once("SIGINT", shutdown);
  process.once("SIGTERM", shutdown);

  if (!config.allowlist.length && !config.allowUnlisted) {
    console.log(
      "No allowlist configured. Incoming chats will receive their user IDs and be refused."
    );
  }

  // 进入长轮询循环
  await monitorLoop();

  console.log("Bridge stopped.");
}

async function shutdown() {
  if (stopping) return;
  stopping = true;
  console.log("Shutting down...");
  for (const controller of recoveryControllers.values()) controller.abort();

  if (botAccount?.token) {
    try {
      const stopResp = await notifyStop({
        baseUrl: botAccount.baseUrl,
        token: botAccount.token,
      });
      console.log(
        `notifyStop: ret=${stopResp.ret} errmsg=${stopResp.errmsg ?? "OK"}`
      );
    } catch (error) {
      console.error("notifyStop failed:", error.message);
    }
  }

  setTimeout(() => process.exit(0), 2000);
}

main().catch((error) => {
  console.error("Fatal error:", error);
  process.exit(1);
});
