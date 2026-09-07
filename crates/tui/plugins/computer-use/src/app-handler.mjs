// One request handler for every out-of-process runner of the backends: the
// ssh remote agent (one request per process) and the desktop app daemon (one
// long-lived process). Only tools in ALLOWED execute, so neither the ssh
// transport nor the app socket can ever become a generic shell.
import url from "node:url";
import { exec } from "./remote-runtime.mjs";

export const ALLOWED = new Set([
  "preview", "platform", "probe", "list_displays", "switch_display", "list_apps", "list_windows",
  "open_application", "get_app_state", "resolve_element", "screenshot", "zoom",
  "left_click", "double_click", "triple_click", "right_click", "middle_click",
  "mouse_move", "left_click_drag", "left_mouse_down", "left_mouse_up", "scroll",
  "type", "key", "hold_key", "set_value", "select_text", "perform_action",
  "read_clipboard", "write_clipboard", "cursor_position",
  "recordingStart", "recordingStop", "recordingStatus", "recordingList",
]);

const backends = new Map();

async function backend(computerId) {
  if (!backends.has(computerId)) {
    // Same test hook as src/transport.mjs, so the out-of-process route can be
    // driven end to end against a recording backend (never set in production).
    const test = process.env.CODEWHALE_CU_TEST_BACKEND;
    const mod = await import(test ? url.pathToFileURL(test).href : `./backends/${process.platform}.mjs`);
    backends.set(computerId, mod.create({ exec, computer: { id: computerId, transport: "local", platform: process.platform } }));
  }
  return backends.get(computerId);
}

/**
 * Execute one {tool, args} request on this machine's backend. Never throws:
 * every outcome is a receipt object with `ok`.
 */
export async function handle(req, { computerId = "local" } = {}) {
  const tool = req?.tool;
  if (!ALLOWED.has(tool)) {
    return { ok: false, error: { code: "tool_not_allowed", message: `tool "${tool}" is not in the remote allow-list` } };
  }
  if (tool === "platform") return { ok: true, platform: process.platform };
  try {
    const fn = (await backend(computerId))[tool];
    if (typeof fn !== "function") {
      return { ok: false, error: { code: "unsupported_on_platform", message: `"${tool}" is not implemented on ${process.platform}` } };
    }
    const data = await fn(req.args ?? {});
    return { ok: true, platform: process.platform, tool, data };
  } catch (err) {
    return { ok: false, platform: process.platform, tool, error: { code: err?.code ?? "tool_error", message: String(err?.message ?? err) } };
  }
}
