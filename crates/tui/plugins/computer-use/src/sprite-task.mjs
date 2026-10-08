// Sprite Task hold — keep a Codewhale Computer (a Fly Sprite) awake while a
// turn runs, and only then.
//
// Sprites pause when idle; a Task registered on the in-Sprite API socket
// (/.sprite/api.sock, virtual host "sprite") holds one awake until it expires.
// The contract (ARCHITECTURE §2.1, S0 Q7):
//   - acquire at turn start with a 5-minute expiry, refresh every 60 s,
//     release (DELETE) at turn end;
//   - expiries are capped at 5 minutes: a Task survives a checkpoint restore
//     and keeps the Sprite billing until it expires, so a crashed or halted
//     holder must never leave more than a short tail;
//   - the holder dies with its parent: the CLI (mcp/turn-hold.mjs) releases on
//     stdin EOF, so an Engine crash cannot leave a refreshed hold behind.
import http from "node:http";

export const DEFAULT_SOCKET = "/.sprite/api.sock";
export const MAX_EXPIRE_SEC = 300;
const NAME_RE = /^[a-z0-9][a-z0-9-]{0,62}$/;

/** "5m" | "90s" | 300 → seconds; refuses anything above the 5-minute cap. */
export function expireSeconds(expire) {
  let sec;
  if (typeof expire === "number") sec = expire;
  else {
    const m = /^(\d+)(s|m)$/.exec(String(expire ?? "").trim());
    if (!m) throw Object.assign(new Error(`expire must look like "5m" or "90s" (got ${JSON.stringify(expire)})`), { code: "bad_args" });
    sec = Number(m[1]) * (m[2] === "m" ? 60 : 1);
  }
  if (!Number.isInteger(sec) || sec < 30 || sec > MAX_EXPIRE_SEC) {
    throw Object.assign(new Error(`task expiry must be 30..${MAX_EXPIRE_SEC} s — a Task outlives restores, so long holds are refused`), { code: "bad_args" });
  }
  return sec;
}

/** One JSON request to the Sprite API socket. Resolves {status, body}. */
export function spriteApi(method, path, body, { socket = DEFAULT_SOCKET, timeoutMs = 5_000 } = {}) {
  return new Promise((resolve, reject) => {
    const payload = body == null ? null : JSON.stringify(body);
    const req = http.request({
      socketPath: socket, method, path, host: "sprite",
      headers: { Host: "sprite", ...(payload ? { "Content-Type": "application/json", "Content-Length": Buffer.byteLength(payload) } : {}) },
      timeout: timeoutMs,
    }, (res) => {
      let text = "";
      res.setEncoding("utf8");
      res.on("data", (chunk) => { text += chunk; });
      res.on("end", () => {
        let parsed = null;
        try { parsed = text ? JSON.parse(text) : null; } catch { parsed = text; }
        resolve({ status: res.statusCode, body: parsed });
      });
    });
    req.on("timeout", () => req.destroy(Object.assign(new Error(`Sprite API ${method} ${path} timed out`), { code: "timeout" })));
    req.on("error", reject);
    if (payload) req.write(payload);
    req.end();
  });
}

/**
 * A refreshed Task hold. acquire() registers it and starts the refresh timer;
 * release() stops the timer and deletes the Task. onEvent receives receipts
 * ({event:"acquired"|"refreshed"|"refresh_failed"|"released"|"release_failed", ...}).
 */
export function createTaskHold({
  name, expire = "5m", refreshMs = 60_000, socket = DEFAULT_SOCKET,
  api = (method, path, body) => spriteApi(method, path, body, { socket }),
  onEvent = () => {}, now = () => new Date().toISOString(),
} = {}) {
  if (typeof name !== "string" || !NAME_RE.test(name)) throw Object.assign(new Error("task name must be lowercase letters, digits and dashes (≤ 63)"), { code: "bad_args" });
  const sec = expireSeconds(expire);
  if (!(refreshMs > 0) || refreshMs >= sec * 1000) throw Object.assign(new Error("refresh interval must be shorter than the expiry"), { code: "bad_args" });
  const expireText = `${sec}s`;
  let timer = null;
  let held = false;
  const path = `/v1/tasks/${encodeURIComponent(name)}`;

  async function register(method, url) {
    const r = await api(method, url, { name, expire: expireText });
    if (r.status < 200 || r.status >= 300) throw Object.assign(new Error(`Sprite API ${method} ${url} returned ${r.status}`), { code: "task_api_error", status: r.status });
    return r.body;
  }

  async function refresh() {
    try {
      // PUT refreshes per the docs; a server without it gets a re-POST, which
      // S0 observed to re-register the same name with a fresh expiry.
      let body;
      try { body = await register("PUT", path); } catch (error) {
        if (error.status !== 404 && error.status !== 405) throw error;
        body = await register("POST", "/v1/tasks");
      }
      onEvent({ event: "refreshed", name, expires_at: body?.expires_at ?? null, ts: now() });
    } catch (error) {
      onEvent({ event: "refresh_failed", name, error: error.message, ts: now() });
    }
  }

  return {
    get held() { return held; },
    async acquire() {
      if (held) return;
      const body = await register("POST", "/v1/tasks");
      held = true;
      onEvent({ event: "acquired", name, expire: expireText, expires_at: body?.expires_at ?? null, ts: now() });
      timer = setInterval(refresh, refreshMs);
    },
    async release() {
      if (timer) { clearInterval(timer); timer = null; }
      if (!held) return;
      held = false;
      try {
        const r = await api("DELETE", path);
        if (r.status >= 300 && r.status !== 404) throw new Error(`Sprite API DELETE ${path} returned ${r.status}`);
        onEvent({ event: "released", name, ts: now() });
      } catch (error) {
        onEvent({ event: "release_failed", name, error: error.message, note: `the Task lapses on its own within ${sec} s`, ts: now() });
      }
    },
  };
}
