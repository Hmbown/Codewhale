/**
 * Codewhale Runtime client for the browser: the `/v1` contract documented in
 * `docs/RUNTIME_API.md`, over `fetch`.
 *
 * The runtime binds loopback only. The extension reaches it because
 * `manifest.json` holds `http://127.0.0.1/*` and `http://localhost/*` as
 * required host permissions — the one host access Chromewhale takes up front,
 * and the only one it takes without asking, because it is the user's own agent.
 *
 * The bearer token lives in `chrome.storage.local`, never in a URL: a query
 * string reaches the runtime's request log and the extension's own history.
 */

import { SseParser } from "./sse.js";

const HEALTH_TIMEOUT_MS = 2_500;
const READ_TIMEOUT_MS = 10_000;
const MUTATE_TIMEOUT_MS = 30_000;

export class RuntimeClient {
  /**
   * @param {{baseUrl: string, token?: string}} config
   */
  constructor(config) {
    this.baseUrl = config.baseUrl.replace(/\/+$/, "");
    this.token = config.token ?? "";
  }

  /**
   * Probe the runtime. Mirrors the VS Code client's reading of the same route:
   * `/v1/runtime/info` answers 200 even when protected, and `auth_required` in
   * the body is the real signal.
   *
   * @returns {Promise<{kind: "connected"|"offline"|"auth-required"|"error", detail: string, version?: string}>}
   */
  async connect() {
    let info;
    try {
      info = await this.#json("GET", "/v1/runtime/info", undefined, HEALTH_TIMEOUT_MS);
    } catch (error) {
      return {
        kind: "offline",
        detail:
          `No Codewhale runtime at ${this.baseUrl} (${messageOf(error)}). Start one with ` +
          "`codewhale app-server --http`, then reconnect.",
      };
    }
    if (info.status === 401) {
      return { kind: "auth-required", detail: "The runtime requires a token. Add one in Settings." };
    }
    if (!info.ok) {
      return { kind: "error", detail: `The runtime answered HTTP ${info.status}.` };
    }
    if (info.body?.auth_required === true && !this.token) {
      return {
        kind: "auth-required",
        detail: "The runtime requires a bearer token. Add one in Settings.",
      };
    }
    const version =
      typeof info.body?.version === "string"
        ? info.body.version
        : typeof info.body?.codewhale_version === "string"
          ? info.body.codewhale_version
          : undefined;
    return {
      kind: "connected",
      detail: version ? `Connected to Codewhale ${version}.` : "Connected to the Codewhale runtime.",
      version,
    };
  }

  /**
   * @param {{model?: string, workspace?: string, mode?: string, title?: string}} [body]
   */
  async createThread(body = {}) {
    const response = await this.#json("POST", "/v1/threads", body, MUTATE_TIMEOUT_MS);
    this.#ensureOk(response, "Create thread");
    return response.body;
  }

  /** @param {string} threadId */
  async threadDetail(threadId) {
    const response = await this.#json(
      "GET",
      `/v1/threads/${encodeURIComponent(threadId)}`,
      undefined,
      READ_TIMEOUT_MS,
    );
    this.#ensureOk(response, "Thread detail");
    return response.body;
  }

  /**
   * Start a turn.
   *
   * `dynamic_tools` is sent on **every** turn on purpose: `POST /v1/threads`
   * accepts the field and discards it, so the per-turn array is what actually
   * reaches the model's catalog. It is the same frozen array each time, which
   * is what keeps the pinned prefix byte-stable (see `tools.js`).
   *
   * @param {string} threadId
   * @param {string} prompt
   * @param {ReadonlyArray<Record<string, unknown>>} dynamicTools
   */
  async startTurn(threadId, prompt, dynamicTools) {
    const response = await this.#json(
      "POST",
      `/v1/threads/${encodeURIComponent(threadId)}/turns`,
      { prompt, dynamic_tools: dynamicTools },
      MUTATE_TIMEOUT_MS,
    );
    this.#ensureOk(response, "Start turn");
    return response.body;
  }

  /**
   * @param {string} threadId
   * @param {string} turnId
   */
  async interrupt(threadId, turnId) {
    const response = await this.#json(
      "POST",
      `/v1/threads/${encodeURIComponent(threadId)}/turns/${encodeURIComponent(turnId)}/interrupt`,
      {},
      MUTATE_TIMEOUT_MS,
    );
    this.#ensureOk(response, "Interrupt");
  }

  /**
   * @param {string} approvalId
   * @param {"allow" | "deny"} decision
   */
  async decideApproval(approvalId, decision) {
    const response = await this.#json(
      "POST",
      `/v1/approvals/${encodeURIComponent(approvalId)}`,
      { decision, remember: false },
      MUTATE_TIMEOUT_MS,
    );
    this.#ensureOk(response, "Approval");
  }

  /**
   * Deliver a client-executed tool result.
   *
   * @param {string} threadId
   * @param {string} turnId
   * @param {string} callId
   * @param {{success: boolean, content: Array<Record<string, unknown>>}} result
   */
  async submitToolResult(threadId, turnId, callId, result) {
    const path =
      `/v1/threads/${encodeURIComponent(threadId)}` +
      `/turns/${encodeURIComponent(turnId)}` +
      `/tool-calls/${encodeURIComponent(callId)}/result`;
    const response = await this.#json("POST", path, result, MUTATE_TIMEOUT_MS);
    // 404 means the call is already terminal — timed out, canceled, or settled
    // by an earlier delivery. That is not an error worth surfacing: the runtime
    // owns settlement and has already told the model something.
    if (response.status === 404) {
      return false;
    }
    this.#ensureOk(response, "Tool result");
    return true;
  }

  /**
   * Open the event stream and yield parsed events until `signal` aborts.
   *
   * @param {string} threadId
   * @param {number} sinceSeq
   * @param {AbortSignal} signal
   */
  async *events(threadId, sinceSeq, signal) {
    const url =
      `${this.baseUrl}/v1/threads/${encodeURIComponent(threadId)}` +
      `/events?since_seq=${encodeURIComponent(String(sinceSeq))}`;
    const response = await fetch(url, {
      headers: { Accept: "text/event-stream", ...this.#authHeader() },
      signal,
    });
    if (!response.ok || !response.body) {
      throw new Error(`Event stream returned HTTP ${response.status}.`);
    }
    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    const parser = new SseParser();
    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) {
          return;
        }
        for (const event of parser.push(decoder.decode(value, { stream: true }))) {
          yield event;
        }
      }
    } finally {
      reader.cancel().catch(() => {});
    }
  }

  #authHeader() {
    return this.token ? { Authorization: `Bearer ${this.token}` } : {};
  }

  /**
   * @param {string} method
   * @param {string} path
   * @param {unknown} body
   * @param {number} timeoutMs
   */
  async #json(method, path, body, timeoutMs) {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), timeoutMs);
    try {
      const response = await fetch(`${this.baseUrl}${path}`, {
        method,
        headers: {
          Accept: "application/json",
          ...(body === undefined ? {} : { "Content-Type": "application/json" }),
          ...this.#authHeader(),
        },
        body: body === undefined ? undefined : JSON.stringify(body),
        signal: controller.signal,
      });
      const text = await response.text();
      let parsed;
      try {
        parsed = text ? JSON.parse(text) : undefined;
      } catch {
        parsed = undefined;
      }
      return { ok: response.ok, status: response.status, body: parsed, text };
    } finally {
      clearTimeout(timer);
    }
  }

  /**
   * @param {{ok: boolean, status: number, text: string}} response
   * @param {string} label
   */
  #ensureOk(response, label) {
    if (!response.ok) {
      const detail = response.text ? `: ${response.text.slice(0, 300)}` : "";
      throw new Error(`${label} failed with HTTP ${response.status}${detail}`);
    }
  }
}

/** @param {unknown} error */
function messageOf(error) {
  if (error instanceof Error) {
    return error.name === "AbortError" ? "timed out" : error.message;
  }
  return typeof error === "string" ? error : "unreachable";
}
