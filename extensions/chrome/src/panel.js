/**
 * The Chromewhale side panel: chat with the local Codewhale runtime, and run
 * the browser tools it asks for against the tab the user is looking at.
 *
 * The whole client lives in this document rather than the service worker — see
 * `background.js` for why. Two consequences are deliberate and worth stating:
 *
 * - Browser tools work only while the panel is open. Close it and the stream,
 *   the tool loop, and the extension's ability to touch a page all stop. That
 *   is the intended lifetime, not a limitation to engineer around.
 * - Transcript text is written with `textContent`, never `innerHTML`. Model
 *   output and page-derived tool results both land here, and neither is
 *   trusted markup. There is no markdown renderer for the same reason.
 */

import { RuntimeClient } from "./runtime.js";
import { BROWSER_TOOLS, ownsTool, describeCall } from "./tools.js";
import { createBrowserTools } from "./browser.js";
import { withDecision } from "./policy.js";

const DEFAULT_SETTINGS = { host: "127.0.0.1", port: 7878, token: "" };

/**
 * How long an origin prompt waits for the user. Well under the runtime's
 * 300-second dynamic-tool timeout so the model gets an explicit refusal rather
 * than a silent timeout it cannot explain.
 */
const DECISION_TIMEOUT_MS = 120_000;

const MAX_ACTIVITY_ROWS = 40;

const dom = {
  status: document.getElementById("status"),
  pause: document.getElementById("pause"),
  toggleSettings: document.getElementById("toggle-settings"),
  settings: document.getElementById("settings"),
  host: document.getElementById("host"),
  port: document.getElementById("port"),
  token: document.getElementById("token"),
  save: document.getElementById("save-settings"),
  newThread: document.getElementById("new-thread"),
  sites: document.getElementById("sites"),
  activity: document.getElementById("activity"),
  transcript: document.getElementById("transcript"),
  prompts: document.getElementById("prompts"),
  composer: document.getElementById("composer"),
  input: document.getElementById("input"),
  stop: document.getElementById("stop"),
};

/** @type {RuntimeClient | undefined} */
let client;
/** @type {string | undefined} */
let threadId;
/** @type {string | undefined} */
let streamingTurnId;
let lastSeq = 0;
/** @type {AbortController | undefined} */
let streamAbort;
/** @type {Map<string, HTMLElement>} */
const itemNodes = new Map();
/** @type {Set<string>} */
const handledCalls = new Set();
/** @type {Array<{tool: string, summary: string, origin?: string, outcome: string}>} */
const activity = [];

const browserTools = createBrowserTools({
  activeTab: async () => {
    const win = await chrome.windows.getCurrent();
    const [tab] = await chrome.tabs.query({ active: true, windowId: win.id });
    return tab;
  },
  getTab: (tabId) => chrome.tabs.get(tabId).catch(() => undefined),
  navigateTab: async (tabId, url) => {
    await chrome.tabs.update(tabId, { url });
  },
  historyMove: async (tabId, action) => {
    if (action === "reload") {
      await chrome.tabs.reload(tabId);
    } else if (action === "back") {
      await chrome.tabs.goBack(tabId);
    } else {
      await chrome.tabs.goForward(tabId);
    }
  },
  executeScript: async ({ tabId, func, args }) => {
    const [injection] = await chrome.scripting.executeScript({ target: { tabId }, func, args });
    return injection?.result;
  },
  captureTab: (windowId) => chrome.tabs.captureVisibleTab(windowId, { format: "jpeg", quality: 80 }),
  hasPermission: (pattern) => chrome.permissions.contains({ origins: [pattern] }),
  readDecisions: async () => (await chrome.storage.local.get({ origins: {} })).origins,
  requestDecision,
  isPaused: async () => (await chrome.storage.local.get({ paused: false })).paused === true,
  log: recordActivity,
  sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
});

// --- settings -------------------------------------------------------------

async function loadSettings() {
  const stored = await chrome.storage.local.get({ settings: DEFAULT_SETTINGS });
  const settings = { ...DEFAULT_SETTINGS, ...stored.settings };
  dom.host.value = settings.host;
  dom.port.value = String(settings.port);
  dom.token.value = settings.token;
  return settings;
}

async function saveSettings() {
  const port = Number.parseInt(dom.port.value, 10);
  const settings = {
    host: dom.host.value.trim() || DEFAULT_SETTINGS.host,
    port: Number.isInteger(port) && port > 0 && port < 65536 ? port : DEFAULT_SETTINGS.port,
    token: dom.token.value,
  };
  await chrome.storage.local.set({ settings });
  return settings;
}

/** @param {{host: string, port: number, token: string}} settings */
function baseUrlFor(settings) {
  return `http://${settings.host}:${settings.port}`;
}

// --- connection and stream ------------------------------------------------

async function connect() {
  const settings = await loadSettings();
  client = new RuntimeClient({ baseUrl: baseUrlFor(settings), token: settings.token });
  const state = await client.connect();
  setStatus(state.kind, state.detail);
  if (state.kind !== "connected") {
    dom.settings.hidden = false;
    dom.toggleSettings.setAttribute("aria-expanded", "true");
    return;
  }
  await ensureThread();
}

async function ensureThread() {
  if (!client) {
    return;
  }
  const stored = await chrome.storage.local.get({ threadId: "" });
  if (stored.threadId) {
    try {
      const detail = await client.threadDetail(stored.threadId);
      threadId = stored.threadId;
      hydrate(detail);
      openStream();
      return;
    } catch {
      // A thread from a previous runtime process is gone; start a fresh one.
    }
  }
  const thread = await client.createThread({});
  threadId = thread?.id;
  await chrome.storage.local.set({ threadId });
  resetTranscript();
  openStream();
}

/** @param {any} detail */
function hydrate(detail) {
  resetTranscript();
  lastSeq = Number(detail?.latest_seq ?? 0);
  for (const item of detail?.items ?? []) {
    renderItem(item.id, {
      kind: item.kind,
      status: item.status,
      text: item.detail || item.summary || "",
    });
  }
  for (const approval of detail?.pending_approvals ?? []) {
    renderApprovalCard(approval);
  }
  // A turn that was mid-flight when the panel closed still has calls waiting on
  // this client; nothing else will answer them.
  for (const call of detail?.pending_dynamic_tool_calls ?? []) {
    void handleToolCall(call);
  }
}

function openStream() {
  closeStream();
  if (!client || !threadId) {
    return;
  }
  const controller = new AbortController();
  streamAbort = controller;
  void pump(client, threadId, controller);
}

/**
 * @param {RuntimeClient} runtime
 * @param {string} id
 * @param {AbortController} controller
 */
async function pump(runtime, id, controller) {
  let backoffMs = 500;
  while (!controller.signal.aborted) {
    try {
      for await (const event of runtime.events(id, lastSeq, controller.signal)) {
        backoffMs = 500;
        if (event.seq <= lastSeq) {
          continue;
        }
        lastSeq = event.seq;
        handleEvent(event);
      }
    } catch (error) {
      if (controller.signal.aborted) {
        return;
      }
      setStatus("error", `Event stream dropped: ${error instanceof Error ? error.message : error}`);
    }
    if (controller.signal.aborted) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, backoffMs));
    backoffMs = Math.min(backoffMs * 2, 10_000);
  }
}

function closeStream() {
  streamAbort?.abort();
  streamAbort = undefined;
}

/** @param {{seq: number, event: string, itemId?: string, turnId?: string, payload: any}} event */
function handleEvent(event) {
  const payload = event.payload ?? {};
  switch (event.event) {
    case "item.started":
    case "item.completed":
    case "item.failed":
    case "item.interrupted":
    case "item.canceled": {
      const item = payload.item && typeof payload.item === "object" ? payload.item : payload;
      const id = event.itemId ?? item.id;
      if (id) {
        renderItem(id, {
          kind: item.kind,
          status: item.status ?? statusForEvent(event.event),
          text: item.detail || item.summary || undefined,
        });
      }
      break;
    }
    case "item.delta": {
      if (event.itemId && typeof payload.delta === "string") {
        appendDelta(event.itemId, payload.kind ?? "agent_message", payload.delta);
      }
      break;
    }
    case "approval.required": {
      renderApprovalCard(payload);
      break;
    }
    case "approval.decided":
    case "approval.timeout": {
      document.getElementById(`approval-${payload.approval_id ?? payload.id}`)?.remove();
      break;
    }
    case "tool_call.requested": {
      void handleToolCall(payload);
      break;
    }
    case "tool_call.resolved":
    case "tool_call.timeout":
    case "tool_call.canceled": {
      handledCalls.delete(payload.call_id);
      break;
    }
    case "turn.completed":
    case "turn.failed":
    case "turn.interrupted":
    case "turn.ended": {
      if (event.turnId === streamingTurnId) {
        streamingTurnId = undefined;
        dom.stop.hidden = true;
      }
      break;
    }
    default:
      break;
  }
}

/** @param {string} event */
function statusForEvent(event) {
  if (event === "item.completed") {
    return "completed";
  }
  if (event === "item.failed") {
    return "failed";
  }
  if (event === "item.interrupted" || event === "item.canceled") {
    return "interrupted";
  }
  return "in_progress";
}

// --- dynamic tool calls ---------------------------------------------------

/**
 * @param {{thread_id?: string, turn_id?: string, call_id?: string, tool?: string,
 *          arguments?: Record<string, unknown>}} call
 */
async function handleToolCall(call) {
  const { call_id: callId, turn_id: turnId, tool } = call ?? {};
  if (!client || !threadId || !callId || !turnId || !ownsTool(tool)) {
    return;
  }
  if (handledCalls.has(callId)) {
    return;
  }
  handledCalls.add(callId);
  const args = call.arguments && typeof call.arguments === "object" ? call.arguments : {};
  renderItem(`call-${callId}`, {
    kind: "tool_call",
    status: "in_progress",
    text: `${tool}: ${describeCall(tool, args)}`,
  });
  const result = await browserTools.execute(tool, args);
  renderItem(`call-${callId}`, {
    kind: "tool_call",
    status: result.success ? "completed" : "failed",
    text: `${tool}: ${describeCall(tool, args)}`,
  });
  try {
    await client.submitToolResult(threadId, turnId, callId, result);
  } catch (error) {
    setStatus("error", `Could not deliver a tool result: ${error instanceof Error ? error.message : error}`);
  }
}

/**
 * Ask the user whether Chromewhale may work on an origin.
 *
 * Resolving to `"allow"` means both gates passed: the user said yes *and*
 * Chrome granted the optional host permission. `chrome.permissions.request` is
 * called first thing inside the click handler so the user gesture is still live.
 *
 * @param {{origin: string, tool: string, summary: string, reason: "ask" | "permission"}} request
 * @returns {Promise<"allow" | "block" | "denied">}
 */
function requestDecision(request) {
  return new Promise((resolve) => {
    const card = document.createElement("div");
    card.className = "card";

    const title = document.createElement("h3");
    title.textContent = `Let Codewhale ${request.summary}?`;
    card.append(title);

    const detail = document.createElement("p");
    detail.textContent =
      request.reason === "permission"
        ? `${request.origin} is allowed, but Chrome no longer holds access to it. Grant it again to continue.`
        : `Codewhale wants to use ${request.tool} on ${request.origin}. Allowing lets it read and act on every page of that site while this panel is open.`;
    card.append(detail);

    const row = document.createElement("div");
    row.className = "row";
    const allow = document.createElement("button");
    allow.type = "button";
    allow.textContent = `Allow ${request.origin}`;
    const block = document.createElement("button");
    block.type = "button";
    block.className = "ghost danger";
    block.textContent = "Block";
    row.append(allow, block);
    card.append(row);
    dom.prompts.append(card);

    let settled = false;
    /** @param {"allow" | "block" | "denied"} answer */
    const finish = (answer) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      card.remove();
      resolve(answer);
    };
    const timer = setTimeout(() => finish("denied"), DECISION_TIMEOUT_MS);

    allow.addEventListener("click", () => {
      // Kept synchronous: an await before this call would spend the gesture.
      chrome.permissions
        .request({ origins: [`${request.origin}/*`] })
        .then(async (granted) => {
          if (!granted) {
            finish("denied");
            return;
          }
          await persistDecision(request.origin, "allow");
          finish("allow");
        })
        .catch(() => finish("denied"));
    });
    block.addEventListener("click", () => {
      void persistDecision(request.origin, "block").then(() => finish("block"));
    });
  });
}

/**
 * @param {string} origin
 * @param {"allow" | "block" | "ask"} decision
 */
async function persistDecision(origin, decision) {
  const stored = await chrome.storage.local.get({ origins: {} });
  await chrome.storage.local.set({ origins: withDecision(stored.origins, origin, decision) });
  if (decision !== "allow") {
    await chrome.permissions.remove({ origins: [`${origin}/*`] }).catch(() => false);
  }
  await renderSites();
}

// --- rendering ------------------------------------------------------------

function resetTranscript() {
  dom.transcript.replaceChildren();
  dom.prompts.replaceChildren();
  itemNodes.clear();
  handledCalls.clear();
  lastSeq = 0;
}

/**
 * @param {string} id
 * @param {{kind?: string, status?: string, text?: string}} patch
 */
function renderItem(id, patch) {
  if (patch.kind === "agent_reasoning" || patch.kind === "context_compaction") {
    return;
  }
  let node = itemNodes.get(id);
  if (!node) {
    node = document.createElement("div");
    node.className = "msg";
    const who = document.createElement("div");
    who.className = "who";
    const body = document.createElement("div");
    body.className = "body";
    node.append(who, body);
    itemNodes.set(id, node);
    dom.transcript.append(node);
  }
  const kind = patch.kind ?? node.dataset.kind ?? "status";
  node.dataset.kind = kind;
  node.className = `msg${isToolish(kind) ? " tool" : ""}${patch.status === "failed" ? " failed" : ""}`;
  node.querySelector(".who").textContent = labelFor(kind);
  if (patch.text !== undefined) {
    node.querySelector(".body").textContent = patch.text;
  }
  dom.transcript.scrollTop = dom.transcript.scrollHeight;
}

/**
 * @param {string} id
 * @param {string} kind
 * @param {string} delta
 */
function appendDelta(id, kind, delta) {
  if (!itemNodes.has(id)) {
    renderItem(id, { kind, status: "in_progress", text: "" });
  }
  const body = itemNodes.get(id)?.querySelector(".body");
  if (body) {
    body.textContent += delta;
    dom.transcript.scrollTop = dom.transcript.scrollHeight;
  }
}

/** @param {string} kind */
function isToolish(kind) {
  return kind !== "agent_message" && kind !== "user_message";
}

/** @param {string} kind */
function labelFor(kind) {
  switch (kind) {
    case "user_message":
      return "You";
    case "agent_message":
      return "Codewhale";
    case "error":
      return "Error";
    default:
      return kind.replace(/_/g, " ");
  }
}

/** @param {any} approval */
function renderApprovalCard(approval) {
  const id = approval?.approval_id ?? approval?.id;
  if (!id || document.getElementById(`approval-${id}`)) {
    return;
  }
  const card = document.createElement("div");
  card.className = "card";
  card.id = `approval-${id}`;

  const title = document.createElement("h3");
  title.textContent = `Codewhale wants to run ${approval.tool_name ?? "a tool"}`;
  const detail = document.createElement("p");
  detail.textContent = approval.intent_summary || approval.description || "";
  const row = document.createElement("div");
  row.className = "row";
  const allow = document.createElement("button");
  allow.type = "button";
  allow.textContent = "Allow";
  const deny = document.createElement("button");
  deny.type = "button";
  deny.className = "ghost danger";
  deny.textContent = "Deny";
  row.append(allow, deny);
  card.append(title, detail, row);
  dom.prompts.append(card);

  /** @param {"allow" | "deny"} decision */
  const settle = (decision) => {
    card.remove();
    void client?.decideApproval(id, decision).catch((error) => {
      setStatus("error", `Approval failed: ${error instanceof Error ? error.message : error}`);
    });
  };
  allow.addEventListener("click", () => settle("allow"));
  deny.addEventListener("click", () => settle("deny"));
}

/** @param {{tool: string, summary: string, origin?: string, outcome: string}} entry */
function recordActivity(entry) {
  activity.unshift(entry);
  activity.splice(MAX_ACTIVITY_ROWS);
  dom.activity.replaceChildren(
    ...activity.map((row) => {
      const li = document.createElement("li");
      const what = document.createElement("span");
      what.textContent = `${row.summary}${row.origin ? ` · ${row.origin}` : ""}`;
      const outcome = document.createElement("span");
      outcome.className = "outcome";
      outcome.dataset.outcome = row.outcome;
      outcome.textContent = row.outcome;
      li.append(what, outcome);
      return li;
    }),
  );
}

async function renderSites() {
  const stored = await chrome.storage.local.get({ origins: {} });
  const entries = Object.entries(stored.origins ?? {});
  if (entries.length === 0) {
    const empty = document.createElement("li");
    empty.textContent = "No site has been allowed or blocked yet.";
    dom.sites.replaceChildren(empty);
    return;
  }
  dom.sites.replaceChildren(
    ...entries.sort(([a], [b]) => a.localeCompare(b)).map(([origin, decision]) => {
      const li = document.createElement("li");
      const label = document.createElement("span");
      label.textContent = origin;
      const verdict = document.createElement("span");
      verdict.className = "verdict";
      verdict.dataset.decision = String(decision);
      verdict.textContent = String(decision);
      const forget = document.createElement("button");
      forget.type = "button";
      forget.className = "ghost";
      forget.textContent = "Forget";
      forget.addEventListener("click", () => void persistDecision(origin, "ask"));
      li.append(label, verdict, forget);
      return li;
    }),
  );
}

/**
 * @param {string} kind
 * @param {string} detail
 */
function setStatus(kind, detail) {
  dom.status.dataset.kind = kind;
  dom.status.textContent = detail;
  dom.status.title = detail;
}

// --- interactions ---------------------------------------------------------

dom.composer.addEventListener("submit", async (event) => {
  event.preventDefault();
  const prompt = dom.input.value.trim();
  if (!prompt || !client || !threadId) {
    return;
  }
  dom.input.value = "";
  try {
    const started = await client.startTurn(threadId, prompt, BROWSER_TOOLS);
    streamingTurnId = started?.turn?.id;
    dom.stop.hidden = !streamingTurnId;
  } catch (error) {
    setStatus("error", `Could not send: ${error instanceof Error ? error.message : error}`);
  }
});

dom.input.addEventListener("keydown", (event) => {
  if (event.key === "Enter" && !event.shiftKey) {
    event.preventDefault();
    dom.composer.requestSubmit();
  }
});

dom.stop.addEventListener("click", () => {
  if (client && threadId && streamingTurnId) {
    void client.interrupt(threadId, streamingTurnId).catch(() => {});
  }
});

dom.pause.addEventListener("click", async () => {
  const stored = await chrome.storage.local.get({ paused: false });
  const paused = !stored.paused;
  await chrome.storage.local.set({ paused });
  dom.pause.setAttribute("aria-pressed", String(paused));
  dom.pause.textContent = paused ? "Paused" : "Pause";
});

dom.toggleSettings.addEventListener("click", () => {
  dom.settings.hidden = !dom.settings.hidden;
  dom.toggleSettings.setAttribute("aria-expanded", String(!dom.settings.hidden));
});

dom.save.addEventListener("click", async () => {
  await saveSettings();
  closeStream();
  await connect();
});

dom.newThread.addEventListener("click", async () => {
  closeStream();
  await chrome.storage.local.remove("threadId");
  threadId = undefined;
  resetTranscript();
  await ensureThread();
});

window.addEventListener("pagehide", closeStream);

// --- start ----------------------------------------------------------------

void (async () => {
  const stored = await chrome.storage.local.get({ paused: false });
  dom.pause.setAttribute("aria-pressed", String(stored.paused === true));
  dom.pause.textContent = stored.paused === true ? "Paused" : "Pause";
  await renderSites();
  await connect();
})();
