// codewhale-cu tool schemas — single source of truth for tools/list.
// Every action/observation tool accepts an optional `computer` id; supplying it
// switches the active computer first (switch-by-use is the default model).
const computerParam = {
  type: "string",
  description: "Computer id to act on. Defaults to the active computer. Providing a different registered id switches to it first (sticky).",
};

const strategyParam = {
  enum: ["auto", "a11y", "event", "app"],
  description: "macOS auto (default): element targets press that exact revalidated element and fail closed, with no coordinate fallback; coordinate targets hit-test the point for an accessibility press, including focus of a field that is not AXPressable. a11y: require an accessibility press or focus and fail closed otherwise. app: if accessibility cannot act, send the click as a window-routed event to the bound app's window. event: skip the accessibility hit-test and send the window-routed click directly. On macOS the user's cursor is never moved; raw pointer input needs activate:true because the window route briefly makes the app key. Other platforms use raw events. action_sent confirms dispatch, not the effect; observe again before deciding another action.",
};

const elementTargetSchema = {
  type: "object",
  description: "Element target: the flat index from the latest get_app_state on this computer. state_id is optional — supply it only to pin a specific earlier observation.",
  required: ["type", "index"],
  properties: {
    type: { const: "element" },
    state_id: { type: "string" },
    index: { type: "integer", minimum: 0 },
  },
  additionalProperties: false,
};

const targetSchema = {
  oneOf: [
    elementTargetSchema,
    {
      type: "object",
      description: "Pixel coordinates in the latest returned raster (screenshot or zoom) for this computer.",
      required: ["type", "x", "y"],
      properties: {
        type: { const: "coordinate" },
        x: { type: "integer" },
        y: { type: "integer" },
        space: { enum: ["raster", "screen"], description: "raster (default): pixels in the latest screenshot/OCR/zoom. screen: absolute screen points; do not convert them yourself." },
      },
      additionalProperties: false,
    },
  ],
};

export const TOOLS = [
  { name: "preview", description: "macOS: show or hide the nonactivating app preview with the drawn agent cursor. On by default while an app is bound — each action updates the captured window and cursor without moving the real pointer. Set enabled:false to mute it for the session.", inputSchema: { type: "object", properties: { enabled: { type: "boolean" }, computer: computerParam }, additionalProperties: false } },
  // ---- computers (switching is a default) ----
  {
    name: "computer", description: "The computer registry. action list | switch | register | spawn | remove. switch/register/spawn/remove take `id`; register also takes transport (local|ssh|hdc) plus host/port/user/target/installAgent; spawn takes transport (docker) plus optional image/label and creates a task-owned disposable desktop that remove or session end destroys. Prefer a spawned computer for work that does not need the user's own session. Every other tool also accepts `computer` to switch stickily on use.",
    inputSchema: { type: "object", required: ["action"], properties: { action: { enum: ["list", "switch", "register", "spawn", "remove"] }, id: { type: "string", description: "Short id for the registered computer (letters, digits, dot, dash)" }, transport: { enum: ["local", "ssh", "hdc", "docker"] }, label: { type: "string" }, image: { type: "string", description: "spawn/docker: image to run (default the plugin's Linux desktop image)" }, host: { type: "string", description: "ssh: hostname" }, port: { type: "integer", description: "ssh: port (default 22)" }, user: { type: "string", description: "ssh: user" }, target: { type: "string", description: "hdc: target key (omit for the only connected device)" }, installAgent: { type: "boolean", description: "ssh: push the remote agent before first use (default true)" } }, additionalProperties: false },
  },
  {
    name: "computer_list",
    description: "List registered computers (local, ssh, docker, harmony/hdc) and which one is active. Every other tool acts on the active computer unless given `computer`.",
    inputSchema: { type: "object", properties: {}, additionalProperties: false },
  },
  {
    name: "computer_switch",
    description: "Switch the active computer. Subsequent tools act on it by default.",
    inputSchema: { type: "object", required: ["computer"], properties: { computer: { type: "string", description: "Registered computer id (see computer_list)" } }, additionalProperties: false },
  },
  {
    name: "computer_register",
    description: "Register or update a computer. transport=local (this machine), ssh (runs the bundled remote agent over ssh; agent is pushed automatically), hdc (HarmonyOS device via hdc).",
    inputSchema: {
      type: "object",
      required: ["computer", "transport"],
      properties: {
        computer: { type: "string", description: "Short id for the computer (letters, digits, dot, dash)" },
        transport: { enum: ["local", "ssh", "hdc"] },
        label: { type: "string" },
        host: { type: "string", description: "ssh: hostname" },
        port: { type: "integer", description: "ssh: port (default 22)" },
        user: { type: "string", description: "ssh: user" },
        target: { type: "string", description: "hdc: target key (omit for the only connected device)" },
        installAgent: { type: "boolean", description: "ssh: push the remote agent before first use (default true)" },
      },
      additionalProperties: false,
    },
  },
  {
    name: "computer_spawn",
    description: "Spawn a task-owned disposable computer. transport=docker provisions an isolated Linux desktop container registered under `computer`; every other tool works on it unchanged. The spawned computer is destroyed by computer_remove or when the session ends. Prefer it over local when the task does not need the user's own session.",
    inputSchema: {
      type: "object",
      required: ["computer", "transport"],
      properties: {
        computer: { type: "string", description: "Short id for the spawned computer (letters, digits, dot, dash)" },
        transport: { enum: ["docker"] },
        image: { type: "string", description: "docker image (default the plugin's Linux desktop image)" },
        label: { type: "string" },
      },
      additionalProperties: false,
    },
  },
  {
    name: "computer_remove",
    description: "Remove a registered computer. 'local' cannot be removed.",
    inputSchema: { type: "object", required: ["computer"], properties: { computer: { type: "string" } }, additionalProperties: false },
  },
  {
    name: "consent",
    description: "Per-app consent on the local computer. Any call that targets an app — open_application, an app_ref, an element, or an action on the bound app — refuses consent_required until the user decides; record their answer here. action status | allow | deny | revoke. app is a name or bundle id (or pid:/number for a pid); scope 'foreground' is the separate darwin decision for foreground control (open_application activate:true). Decisions apply to this session; remember:true persists them.",
    inputSchema: {
      type: "object",
      required: ["action"],
      properties: {
        action: { enum: ["status", "allow", "deny", "revoke"] },
        app: { type: "string", description: "App identity: name ('Safari'), bundle id ('com.apple.Safari'), or pid ('pid:1234')" },
        name: { type: "string" }, bundle_id: { type: "string" }, pid: { type: "integer" },
        scope: { enum: ["app", "foreground"], description: "app (default): consent to use one application. foreground: consent to foreground control and key focus (darwin activate:true)" },
        remember: { type: "boolean", description: "Persist the decision across sessions (default: this session only)" },
        confirm: { type: "string", description: "allow only: the token from a confirmation_required refusal. Record it only after the user approved that exact action (pay, buy, send, transfer, delete) in their own words; it admits one identical call." },
        computer: computerParam,
      },
      additionalProperties: false,
    },
  },
  {
    name: "consent_status",
    description: "List recorded app-consent decisions for a computer (persisted and this session's) plus the foreground decision.",
    inputSchema: { type: "object", properties: { computer: computerParam }, additionalProperties: false },
  },
  {
    name: "consent_allow",
    description: "Record an allow decision: app (name/bundle_id/pid/app string) or scope:'foreground'. remember:true persists it.",
    inputSchema: { type: "object", properties: { computer: computerParam, app: { type: "string" }, name: { type: "string" }, bundle_id: { type: "string" }, pid: { type: "integer" }, scope: { enum: ["app", "foreground"] }, remember: { type: "boolean" }, confirm: { type: "string", description: "Token from a confirmation_required refusal, recorded only after the user approved that exact action." } }, additionalProperties: false },
  },
  {
    name: "consent_deny",
    description: "Record a deny decision: app (name/bundle_id/pid/app string) or scope:'foreground'. remember:true persists it.",
    inputSchema: { type: "object", properties: { computer: computerParam, app: { type: "string" }, name: { type: "string" }, bundle_id: { type: "string" }, pid: { type: "integer" }, scope: { enum: ["app", "foreground"] }, remember: { type: "boolean" } }, additionalProperties: false },
  },
  {
    name: "consent_revoke",
    description: "Remove recorded decisions for an app or scope:'foreground' (session and persisted).",
    inputSchema: { type: "object", properties: { computer: computerParam, app: { type: "string" }, name: { type: "string" }, bundle_id: { type: "string" }, pid: { type: "integer" }, scope: { enum: ["app", "foreground"] } }, additionalProperties: false },
  },
  // ---- observe & resolve ----
  {
    name: "request_access",
    description: "Probe permissions and capabilities of a computer (accessibility, screen capture, recording, missing tools). Call once when readiness is unknown or a permission failure is explicitly named.",
    inputSchema: { type: "object", properties: { computer: computerParam }, additionalProperties: false },
  },
  {
    name: "list_displays",
    description: "List displays/panels with geometry and pixel scale.",
    inputSchema: { type: "object", properties: { computer: computerParam }, additionalProperties: false },
  },
  {
    name: "switch_display",
    description: "Set which display subsequent screenshots/recordings capture on this computer.",
    inputSchema: { type: "object", required: ["index"], properties: { index: { type: "integer", minimum: 1 }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "list_apps",
    description: "List running applications (name, pid, bundle id, frontmost). Defaults to regular user-facing apps; pass all:true to include background agents and helpers (menu-bar extras, XPC services, CLI processes); pass installed:true for the installed catalog of openable apps (running or not, with a running flag) — that scan takes a moment.",
    inputSchema: { type: "object", properties: { all: { type: "boolean", description: "Include accessory/background processes, not just regular apps. Use when looking for a menu-bar or helper process; keep the default for picking an app to control." }, installed: { type: "boolean", description: "List installed apps (openable, running or not) from the standard Applications folders instead of running processes." }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "list_windows",
    description: "List application windows. On macOS, app_ref selects the app; omission follows the app selected by open_application, or the frontmost app before a selection. Other platforms list all windows and reject app_ref selectors.",
    inputSchema: {
      type: "object",
      properties: {
        app_ref: {
          type: "object",
          properties: {
            pid: { type: "integer" }, name: { type: "string" }, bundle_id: { type: "string" },
          },
          additionalProperties: false,
        },
        computer: computerParam,
      },
      additionalProperties: false,
    },
  },
  {
    name: "wait_for",
    description: "Poll this computer's accessibility state until elements matching query/role appear (state:\"present\", default) or until none remain (state:\"absent\"). Returns the matched elements bound to a fresh state_id, ready to target. Prefer this over a get_app_state/wait loop after actions that load, animate or dismiss UI.",
    inputSchema: {
      type: "object",
      properties: {
        query: { type: "string", description: "Case-insensitive substring over label, value and role. At least one of query/role is required." },
        role: { type: "string", description: "Exact accessibility role, e.g. AXButton, AXTextField." },
        state: { enum: ["present", "absent"], default: "present", description: "present: wait until a match exists. absent: wait until no match remains (dialogs dismissed, loading finished)." },
        timeout: { type: "number", minimum: 0.5, maximum: 60, description: "Seconds to poll before giving up; default 10." },
        interval: { type: "integer", minimum: 100, maximum: 5000, description: "Milliseconds between observations; default 400." },
        limit: { type: "integer", minimum: 1, maximum: 100, description: "Max matched elements to return; default 20." },
        app_ref: { type: "object", properties: { pid: { type: "integer" }, name: { type: "string" }, bundle_id: { type: "string" } }, additionalProperties: false, description: "Same selector rules as get_app_state; omission follows the app selected by open_application." },
        window_id: { type: "integer", description: "macOS only: zero-based window index within the app." },
        computer: computerParam,
      },
      additionalProperties: false,
    },
  },
  {
    name: "get_app_state",
    description: "Read an application's text, controls, actions and layout without requiring vision. The default summary keeps app content and top-level menus; full adds nested menus and tree structure. Act on observed elements with {type:'element', index} and refresh after UI changes. Missing labels or values are unknown, not an invitation to guess; request a screenshot only when useful.",
    inputSchema: {
      type: "object",
      properties: {
        app_ref: { type: "object", properties: { pid: { type: "integer" }, name: { type: "string" }, bundle_id: { type: "string" } }, additionalProperties: false, description: "macOS accepts PID, name and bundle identity. Linux accepts only a unique exact AT-SPI app name. Windows accepts only a unique exact window title in name (from list_windows.title). HarmonyOS rejects explicit app selectors." },
        window_id: { type: "integer", description: "macOS only: zero-based window index within the app. Other platforms reject this selector." },
        detail: { enum: ["summary", "compact", "full"], default: "summary", description: "Summary is the concise default (controls, values, actions, layout). compact is smaller: same indices, shorter labels, no nested menus. full includes nested menus and tree paths." },
        query: { type: "string", description: "Case-insensitive substring over label, value and role. Use this instead of downloading the whole tree." },
        role: { type: "string", description: "Exact accessibility role filter, e.g. AXButton, AXTextField." },
        limit: { type: "integer", minimum: 1, maximum: 200, description: "Max elements to return after filtering. Prefer this over a second unfiltered dump." },
        offset: { type: "integer", minimum: 0, description: "Skip this many matching elements (pagination)." },
        include_ocr: { type: "boolean", default: false, description: "On macOS, also recognize visible text locally from the selected app window. Requires Screen Recording permission. Returns text, confidence and raster coordinate targets for UI that accessibility cannot read; no vision model is required. Do not combine with compact unless you need the blocks." },
        ocr_region: { type: "array", items: { type: "number" }, minItems: 4, maxItems: 4, description: "[x, y, w, h] in screen points. When include_ocr is true, recognize only this rect instead of the whole window." },
        computer: computerParam,
      },
      additionalProperties: false,
    },
  },
  {
    name: "screenshot",
    description: "Capture the screen (all or one display, optional region) as PNG/JPEG. The receipt carries raster geometry; later coordinate targets refer to this raster.",
    inputSchema: {
      type: "object",
      properties: {
        app_ref: { type: "object", properties: { name: { type: "string" }, bundle_id: { type: "string" }, pid: { type: "integer" } }, description: "macOS: capture this app window even when it is in the background." },
        display: { type: ["integer", "string"], description: "Display index or 'all'" },
        region: { type: "array", items: { type: "number" }, minItems: 4, maxItems: 4, description: "[x, y, w, h] in screen points" },
        path: { type: "string", description: "Optional output path (absolute). Defaults into the recordings directory." },
        computer: computerParam,
      },
      additionalProperties: false,
    },
  },
  {
    name: "zoom",
    description: "Close-up crop of the latest screenshot. Choose points from the returned child raster only.",
    inputSchema: {
      type: "object",
      required: ["region"],
      properties: {
        region: { type: "array", items: { type: "number" }, minItems: 4, maxItems: 4, description: "[x, y, w, h] in last-raster pixels" },
        path: { type: "string" },
        computer: computerParam,
      },
      additionalProperties: false,
    },
  },
  {
    name: "cursor_position",
    description: "Read the current pointer position in screen points.",
    inputSchema: { type: "object", properties: { computer: computerParam }, additionalProperties: false },
  },
  {
    name: "list_sessions",
    description: "List the live computer sessions on this machine: bound target, delivery mode, current action, idle age, and whether any session currently holds a pointer. Read-only and content-free (no task text is ever recorded). Use it to see who else — another model or agent — is driving the computer before you act.",
    inputSchema: { type: "object", properties: { computer: computerParam }, additionalProperties: false },
  },
  {
    name: "kill_app",
    description: "Quit a running application by exact name, bundle_id or pid. Refuses when several running applications match (pass pid) and never terminates the Computer Use helper itself. force:true force-quits an unresponsive app — unsaved work is discarded.",
    inputSchema: { type: "object", properties: { name: { type: "string" }, bundle_id: { type: "string" }, pid: { type: "integer" }, force: { type: "boolean", description: "force-quit when the graceful quit does not complete" }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "browser",
    description: "Drive a Chromium-family browser over the DevTools protocol — exact element addressing instead of pixel clicking, in a self-owned profile (the user's own browser is never touched). Actions: start {url?} | status | navigate {url} | click {selector | point} | type {text, selector?, enter?} | screenshot {full?} | stop. Elements are CSS selectors; coordinates are page-viewport pixels from screenshot (never screen points). One tab per session; the last session out closes the browser.",
    inputSchema: {
      type: "object", required: ["action"],
      properties: {
        action: { enum: ["start", "status", "navigate", "click", "type", "screenshot", "stop"] },
        url: { type: "string", description: "http(s):// or about:blank (start, navigate)" },
        tab: { type: "string", description: "attach mode only (start): a tab id from status to work in — a person's tab is used only when named" },
        selector: { type: "string", description: "CSS selector (click, or type focus)" },
        point: { type: "object", properties: { x: { type: "number" }, y: { type: "number" } }, required: ["x", "y"], additionalProperties: false, description: "page-viewport pixels — the browser screenshot space, never screen points" },
        text: { type: "string", description: "text to insert (type)" },
        enter: { type: "boolean", description: "press Enter after typing" },
        full: { type: "boolean", description: "capture the full page, not just the viewport" },
        computer: computerParam,
      },
      additionalProperties: false,
    },
  },
  {
    name: "browser_start",
    description: "Launch or reuse the self-owned Chromium profile and open this session's tab. The user's own browser is never touched. On a Codewhale Computer (attach mode) it attaches to the computer's shared browser instead; `tab` picks one of its tabs.",
    inputSchema: { type: "object", properties: { url: { type: "string", description: "optional http(s) URL to open" }, tab: { type: "string", description: "attach mode: tab id from browser_status" }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "browser_status",
    description: "Read the browser session: running, tabs, and the active tab's url/title.",
    inputSchema: { type: "object", properties: { computer: computerParam }, additionalProperties: false },
  },
  {
    name: "browser_navigate",
    description: "Navigate this session's tab to an http(s) or about:blank URL and wait for load.",
    inputSchema: { type: "object", required: ["url"], properties: { url: { type: "string" }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "browser_click",
    description: "Click in the page: a CSS selector's box center, or a page-viewport point.",
    inputSchema: { type: "object", properties: { selector: { type: "string" }, point: { type: "object", properties: { x: { type: "number" }, y: { type: "number" } }, required: ["x", "y"], additionalProperties: false }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "browser_type",
    description: "Insert text into the page (optionally focusing a CSS selector first); enter:true presses Enter.",
    inputSchema: { type: "object", required: ["text"], properties: { text: { type: "string" }, selector: { type: "string" }, enter: { type: "boolean" }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "browser_screenshot",
    description: "Capture the page (viewport, or the full page with full:true) as a PNG in the recordings dir.",
    inputSchema: { type: "object", properties: { full: { type: "boolean" }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "browser_stop",
    description: "Close this session's tab; the shared browser closes when no tabs remain.",
    inputSchema: { type: "object", properties: { computer: computerParam }, additionalProperties: false },
  },
  {
    name: "trajectory",
    description: "Record this session's tool calls to a local JSONL and replay them later. Actions: start | stop | status (file, turns, recent files) | replay {id?, dry_run?} — replay re-enters the normal tool pipeline, so permissions, grants and the kill switch still apply, and it stops at the first refusal. Off unless started; entered text (typed text, set values, clipboard writes) is redacted and those steps are not replayable; files are owner-only and stay in the recordings dir on this machine.",
    inputSchema: { type: "object", required: ["action"], properties: { action: { enum: ["start", "stop", "status", "replay"] }, id: { type: "string", description: "traj-*.jsonl name from status; defaults to the most recent" }, dry_run: { type: "boolean", description: "list what replay would do without executing anything" }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "trajectory_start",
    description: "Start recording this session's tool calls to a local JSONL.",
    inputSchema: { type: "object", properties: { computer: computerParam }, additionalProperties: false },
  },
  {
    name: "trajectory_stop",
    description: "Stop recording and report the file and turn count.",
    inputSchema: { type: "object", properties: { computer: computerParam }, additionalProperties: false },
  },
  {
    name: "trajectory_status",
    description: "Report whether a trajectory is recording, the file, and recent trajectories.",
    inputSchema: { type: "object", properties: { computer: computerParam }, additionalProperties: false },
  },
  {
    name: "trajectory_replay",
    description: "Replay a recorded trajectory through the normal tool pipeline, stopping at the first refusal.",
    inputSchema: { type: "object", properties: { id: { type: "string" }, dry_run: { type: "boolean" }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "set_window_frame",
    description: "Move or resize one window by exact geometry and read the result back. frame is in screen points, the same space list_windows reports: {x,y,w,h}. window_id is the zero-based window index from list_windows. Some windows refuse (fullscreen, tiled); the receipt carries the app's own before/after readback and `verified`.",
    inputSchema: { type: "object", required: ["window_id", "frame"], properties: { app_ref: { type: "object", properties: { pid: { type: "integer" }, name: { type: "string" }, bundle_id: { type: "string" } }, additionalProperties: false, description: "defaults to the bound app" }, window_id: { type: "integer", minimum: 0, description: "zero-based window index from list_windows" }, frame: { type: "object", properties: { x: { type: "number" }, y: { type: "number" }, w: { type: "number" }, h: { type: "number" } }, required: ["x", "y", "w", "h"], additionalProperties: false }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "open_application",
    description: "Launch or activate an application. Copy user-provided names character-for-character; never translate, normalize, or strip suffixes. On macOS prefer bundle_id when known.",
    inputSchema: {
      type: "object",
      properties: {
        name: { type: "string" }, bundle_id: { type: "string" }, url: { type: "string" },
        pid: { type: "integer", description: "Bind to this exact process. Use when two processes share a bundle id (list_apps shows both); it takes precedence over name and bundle_id and never launches anything." },
        activate: { type: "boolean", description: "Bring to foreground; defaults to false — background is the default on every platform. On macOS false keeps process-bound keyboard/accessibility control and refuses raw pointer gestures (they would borrow key focus); on Windows it launches the app minimized; on Linux it restores the previously focused window after launch. True selects foreground control and requires the separate foreground consent — pointer input still goes to the app's window, never the user's cursor; use only when the user has authorized exclusive desktop use. Neither mode is an isolated computer." },
        computer: computerParam,
      },
      additionalProperties: false,
    },
  },
  // ---- pointer ----
  {
    name: "click", description: "Click a target: `button` left/right/middle (left default) and `clicks` 1..3 (left only). Element targets press that exact accessibility element; coordinate targets need a fresh raster. The per-action names (left_click, double_click, right_click, middle_click, triple_click) stay callable as aliases.",
    inputSchema: { type: "object", required: ["target"], properties: { target: targetSchema, button: { enum: ["left", "right", "middle"], default: "left" }, clicks: { type: "integer", minimum: 1, maximum: 3, default: 1 }, strategy: strategyParam, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "pointer", description: "Raw pointer primitives: action \"move\" (hover without clicking), \"down\" (press and hold), \"up\" (release; target optional — releases at the last point). On macOS these drive the Codewhale pointer, never the user\'s cursor: move is a window-routed hover, and down/move/up buffer a drag that reaches the window on up. They need activate:true.",
    inputSchema: { type: "object", required: ["action"], properties: { action: { enum: ["move", "down", "up"] }, target: targetSchema, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "left_click", description: "Left-click a coordinate (pixels in the latest raster) or perform an element's press action. macOS background mode uses accessibility and refuses fallbacks that require keyboard focus.",
    inputSchema: { type: "object", required: ["target"], properties: { target: targetSchema, strategy: strategyParam, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "double_click", description: "Double-click a target.",
    inputSchema: { type: "object", required: ["target"], properties: { target: targetSchema, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "triple_click", description: "Triple-click a target (e.g. select a paragraph).",
    inputSchema: { type: "object", required: ["target"], properties: { target: targetSchema, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "right_click", description: "Right-click a target (context menu).",
    inputSchema: { type: "object", required: ["target"], properties: { target: targetSchema, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "middle_click", description: "Middle-click a target.",
    inputSchema: { type: "object", required: ["target"], properties: { target: targetSchema, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "mouse_move", description: "Move the pointer without clicking (hover).",
    inputSchema: { type: "object", required: ["target"], properties: { target: targetSchema, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "left_click_drag", description: "Press at from_target, move in steps, release at to. macOS requires explicit foreground control; background mode refuses because window-routed events still take keyboard focus.",
    inputSchema: { type: "object", required: ["from_target", "to"], properties: { from_target: targetSchema, to: targetSchema, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "left_mouse_down", description: "Press and hold the left button at a target. Release with left_mouse_up.",
    inputSchema: { type: "object", required: ["target"], properties: { target: targetSchema, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "left_mouse_up", description: "Release the left button pressed by left_mouse_down. An optional target releases at that point instead of where the button went down.",
    inputSchema: { type: "object", properties: { target: targetSchema, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "scroll", description: "Scroll up/down/left/right at a target. macOS background mode uses accessibility scrollbars; amount counts native increments or 5% normalized steps, named in the receipt. It refuses wheel-event fallbacks that take focus. Other raw routes use lines/notches. Prefer an observed scroll-area element.",
    inputSchema: { type: "object", required: ["target"], properties: { target: targetSchema, direction: { enum: ["up", "down", "left", "right"] }, amount: { type: "integer", minimum: 1, maximum: 100 }, computer: computerParam }, additionalProperties: false },
  },
  // ---- text & keyboard ----
  {
    name: "type", description: "Type unicode text into the focused control. Newlines in `text` are Return/Enter key presses, not literal characters — never put \\n in a composer by hoping it will send. Focus the field first (click, focus, or set_value), or pass an element `target` to focus it in the same call. On macOS the receipt carries `verified:true` only when the focused control's value actually reflects the typed text; on `verified:false` the text may have gone nowhere — observe again before relying on it.",
    inputSchema: { type: "object", required: ["text"], properties: { text: { type: "string" }, press_enter: { type: "boolean", description: "After typing, press Return/Enter once. Prefer this to putting a newline in `text` when you want to send." }, target: { ...elementTargetSchema, description: "Element target from get_app_state; it is accessibility-focused first, then the text is typed. Element targets only." }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "key", description: "Press a named key or chord. Examples: return, enter, backspace, tab, escape, cmd+c (macOS), ctrl+c (Linux/Windows). This is the key-press tool; type() cannot send modifiers or Return by itself except via newlines/press_enter. macOS background mode refuses modified or window-targeted keys that need keyboard focus; prefer invoke_menu. Repeat with `repeat`. Pass an element `target` to accessibility-focus it first. `duration` holds the key instead of tapping (hold_key semantics) and cannot be combined with repeat or target.",
    inputSchema: { type: "object", required: ["text"], properties: { text: { type: "string" }, repeat: { type: "integer", minimum: 1, maximum: 100 }, duration: { type: "number", minimum: 0.05, maximum: 30, description: "Hold the key for this many seconds instead of tapping." }, target: { ...elementTargetSchema, description: "Element target from get_app_state; it is accessibility-focused first, then the key is sent. Element targets only." }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "hold_key", description: "Hold a key for `duration` seconds (0.05..30).",
    inputSchema: { type: "object", required: ["text", "duration"], properties: { text: { type: "string" }, duration: { type: "number", minimum: 0.05, maximum: 30 }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "set_value", description: "Set an editable element's value with readback verification. On macOS native controls use AXValue; web-area replacement requires foreground control and refuses in background mode. Prefer browser control for web fields. Element targets only.",
    inputSchema: { type: "object", required: ["target", "value"], properties: { target: elementTargetSchema, value: { type: "string" }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "focus", description: "Focus an observed element through the accessibility layer (background-safe). Prefer this before type() on composers that ignore AXPress.",
    inputSchema: { type: "object", required: ["target"], properties: { target: targetSchema, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "get_value", description: "Read the live accessibility value of an observed element (text fields, sliders). Prefer this over dumping the whole tree.",
    inputSchema: { type: "object", required: ["target"], properties: { target: targetSchema, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "find_elements", description: "Search the latest get_app_state (or take a fresh one) for elements matching query/role without returning the full dump.",
    inputSchema: {
      type: "object",
      properties: {
        query: { type: "string" },
        role: { type: "string" },
        state_id: { type: "string", description: "Reuse a previous observation; omit to observe now." },
        limit: { type: "integer", minimum: 1, maximum: 100 },
        app_ref: { type: "object", properties: { pid: { type: "integer" }, name: { type: "string" }, bundle_id: { type: "string" } }, additionalProperties: false },
        computer: computerParam,
      },
      additionalProperties: false,
    },
  },
  {
    name: "run_actions", description: "Run up to 8 computer-use tools in order on this computer. Stops on the first failure. Each step is {tool, arguments}. Use for click→type→key(return)→get_value without extra round trips.",
    inputSchema: {
      type: "object",
      required: ["steps"],
      properties: {
        steps: {
          type: "array",
          minItems: 1,
          maxItems: 8,
          items: {
            type: "object",
            required: ["tool"],
            properties: {
              tool: { type: "string" },
              arguments: { type: "object" },
            },
            additionalProperties: false,
          },
        },
        computer: computerParam,
      },
      additionalProperties: false,
    },
  },
  {
    name: "select_text", description: "Select a text range [start, length] in an element, or place the caret when the range is omitted. Element targets only.",
    inputSchema: { type: "object", required: ["target"], properties: { target: elementTargetSchema, text_range: { type: "array", items: { type: "integer" }, minItems: 2, maxItems: 2 }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "perform_action", description: "Invoke a named accessibility action on an element (e.g. AXPress on macOS, Invoke on Windows/UIA, click on harmony). Only actions the element advertises. Element targets only.",
    inputSchema: { type: "object", required: ["target", "action"], properties: { target: elementTargetSchema, action: { type: "string" }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "invoke_menu", description: "macOS: invoke an application menu item by title path (e.g. [\"File\",\"New\"]). Runs through accessibility with no focus lease and no key events — prefer this over cmd-key chords for app commands (New, Save, Quit and menu-only actions). App-level commands work without a key window; window-targeted items (Close) can validate against the app's key window and may no-op in the background — prefer the window's close-button element for those. Acts on the app bound with open_application. Verify the effect (list_windows / get_app_state) before reporting success.",
    inputSchema: {
      type: "object", required: ["path"],
      properties: {
        path: { type: "array", minItems: 1, maxItems: 3, items: { type: "string", minLength: 1 }, description: "Menu titles from the menu bar inward, e.g. [\"File\",\"Close Window\"]. Exact titles as shown, including an ellipsis when the app shows one. Application menus (the second menu bar group named after the app) work too." },
        computer: computerParam,
      },
      additionalProperties: false,
    },
  },
  // ---- clipboard / runtime ----
  {
    name: "clipboard", description: "Read or write the system clipboard as UTF-8 text: action \"read\" or \"write\" (write requires text). This is the user's real clipboard — restore it when a round-trip is needed.",
    inputSchema: { type: "object", required: ["action"], properties: { action: { enum: ["read", "write"] }, text: { type: "string" }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "read_clipboard", description: "Read the system clipboard as UTF-8 text.",
    inputSchema: { type: "object", properties: { computer: computerParam }, additionalProperties: false },
  },
  {
    name: "write_clipboard", description: "Write UTF-8 text to the system clipboard.",
    inputSchema: { type: "object", required: ["text"], properties: { text: { type: "string" }, computer: computerParam }, additionalProperties: false },
  },
  // ---- recording ----
  {
    name: "recording", description: "Screen recordings: action start | stop | status | list. `start` accepts display/fps/region/app_ref/window_id/durationSec/intervalMs; stop/status take the recording `id`; list reports what exists. Darwin records through ScreenCaptureKit; other platforms state their own limits in the receipt.",
    inputSchema: { type: "object", required: ["action"], properties: { action: { enum: ["start", "stop", "status", "list"] }, id: { type: "string", description: "Recording id for stop/status" }, display: { type: "integer" }, fps: { type: "number" }, region: { type: "array", items: { type: "number" }, minItems: 4, maxItems: 4 }, app_ref: { type: "object", properties: { pid: { type: "integer" }, name: { type: "string" }, bundle_id: { type: "string" } }, additionalProperties: false }, window_id: { type: "integer" }, durationSec: { type: "number" }, intervalMs: { type: "integer" }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "recording_start",
    description: "Start screen recording on a computer (mp4/mov). Darwin: ScreenCaptureKit via the native helper (timed or until recording_stop; honors region, no recorder overlay, stops on session exit). Pass app_ref to record only the selected app's window rect — captured at start and not tracked across moves. Linux and Windows: unavailable pending session-owned recorder cleanup; use screenshots. HarmonyOS: snapshot-series muxed with ffmpeg.",
    inputSchema: {
      type: "object",
      properties: {
        display: { type: ["integer", "string"] },
        fps: { type: "integer", minimum: 1, maximum: 60, description: "Linux/Windows/harmony-series only" },
        region: { type: "array", items: { type: "number" }, minItems: 4, maxItems: 4, description: "[x, y, w, h] in screen points" },
        app_ref: { type: "object", properties: { pid: { type: "integer" }, name: { type: "string" }, bundle_id: { type: "string" } }, additionalProperties: false, description: "macOS only: record the rect this app's window occupies at start. Omission follows the app selected by open_application." },
        window_id: { type: "integer", description: "macOS only: zero-based window index within the app; requires or implies app_ref." },
        durationSec: { type: "number", minimum: 1, maximum: 7200, description: "macOS only: auto-stop after N seconds" },
        intervalMs: { type: "integer", minimum: 150, maximum: 5000, description: "harmony snapshot-series frame interval" },
        computer: computerParam,
      },
      additionalProperties: false,
    },
  },
  {
    name: "recording_stop",
    description: "Stop a running recording and finalize the file.",
    inputSchema: { type: "object", required: ["id"], properties: { id: { type: "string" }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "recording_status",
    description: "Status of one recording (running, bytes so far).",
    inputSchema: { type: "object", required: ["id"], properties: { id: { type: "string" }, computer: computerParam }, additionalProperties: false },
  },
  {
    name: "recording_list",
    description: "List recordings and screenshots saved on a computer.",
    inputSchema: { type: "object", properties: { computer: computerParam }, additionalProperties: false },
  },
  // ---- programmatic interface ----
  {
    name: "app_script",
    description: "macOS, local computer only: run an AppleScript or JXA (JavaScript for Automation) script through osascript — the programmatic interface inside apps that have a scripting dictionary (Finder, Mail, Safari, Calendar, Notes, Reminders, Music, System Events and most native apps). Prefer this over clicking when the app exposes one: deterministic, returns values, needs no Accessibility grant and never touches the pointer. The receipt carries stdout as `result`; a non-zero exit fails `script_error` with stderr, a user-declined consent fails `automation_denied` (the fix is System Settings → Privacy & Security → Automation, not a retry). Refused on ssh/hdc computers (`unsupported_on_transport`) — the remote channel stays computer-use only, never a shell. Not a shell locally either: shell escapes (do shell script, doShellScript), the ObjC bridge, dynamic code and terminal apps fail `script_refused`, and every app the script names needs the user's consent like any other target.",
    inputSchema: {
      type: "object", required: ["script"],
      properties: {
        script: { type: "string", minLength: 1, description: "Script source. For app arguments use `on run argv` in JXA or read them inside the script; keep scripts single-purpose." },
        language: { enum: ["applescript", "javascript"], description: "applescript (default) or javascript for JXA" },
        timeout: { type: "number", minimum: 1, maximum: 120, description: "Seconds before the script is killed; default 30." },
        computer: computerParam,
      },
      additionalProperties: false,
    },
  },
  // ---- kill switch ----
  {
    name: "stop_computer_control",
    description: "Kill switch: refuse all further computer-use actions for the rest of the session. Read-only probes stay available.",
    inputSchema: { type: "object", properties: { reason: { type: "string" } }, additionalProperties: false },
  },
  {
    name: "wait",
    description: "Pause before the next observation (0..30s). Use after actions that animate or load.",
    inputSchema: { type: "object", properties: { seconds: { type: "number", minimum: 0, maximum: 30 } }, additionalProperties: false },
  },
];

export const TOOL_NAMES = new Set(TOOLS.map((t) => t.name));

/** Required argument names per tool, straight from each inputSchema. */
export const REQUIRED_ARGS = new Map(TOOLS.map((t) => [t.name, t.inputSchema.required ?? []]));

/** Tools whose target must be an observed element — a coordinate reaches the
 *  backend unresolvable and fails opaquely, so refuse it at the boundary. */
export const ELEMENT_ONLY_TARGET = new Set(["set_value", "select_text", "perform_action"]);

/** Tools that never touch a computer (available even after kill switch). */
export const READ_ONLY_TOOLS = new Set([
  "computer_list", "stop_computer_control", "wait", "request_access", "recording_list", "recording_status",
  "find_elements", "get_value", "list_sessions", "browser_status", "trajectory_status", "trajectory_start", "trajectory_stop",
  "consent_status",
]);

/** Tools dispatchable to a remote agent over ssh (allow-list must match agent.mjs). */
export const REMOTE_TOOLS = new Set([
  "preview", "probe", "list_displays", "switch_display", "list_apps", "list_sessions", "list_windows",
  "open_application", "kill_app", "set_window_frame", "get_app_state", "resolve_element", "screenshot", "zoom",
  "browser_start", "browser_status", "browser_navigate", "browser_click", "browser_type", "browser_screenshot", "browser_stop",
  "left_click", "double_click", "triple_click", "right_click", "middle_click",
  "mouse_move", "left_click_drag", "left_mouse_down", "left_mouse_up", "scroll",
  "type", "key", "hold_key", "set_value", "focus", "get_value", "select_text", "perform_action", "invoke_menu",
  "read_clipboard", "write_clipboard", "cursor_position",
  "recordingStart", "recordingStop", "recordingStatus", "recordingList",
  "app_script",
]);

/** Map public tool name -> backend method name. */
export const BACKEND_METHOD = Object.fromEntries(
  TOOLS.filter((t) => !["computer_list", "computer_switch", "computer_register", "computer_spawn", "computer_remove", "consent_status", "consent_allow", "consent_deny", "consent_revoke", "stop_computer_control", "wait", "wait_for", "find_elements", "run_actions", "trajectory_start", "trajectory_stop", "trajectory_status", "trajectory_replay"].includes(t.name))
    .map((t) => [t.name, {
      request_access: "probe",
      recording_start: "recordingStart",
      recording_stop: "recordingStop",
      recording_status: "recordingStatus",
      recording_list: "recordingList",
    }[t.name] ?? t.name]),
);

/**
 * Wire-name expansion for merged tools, used by capability grants: naming a
 * merged tool admits every action it can dispatch to.
 */
export const MERGED_EXPANSION = {
  click: ["left_click", "double_click", "triple_click", "right_click", "middle_click"],
  pointer: ["mouse_move", "left_mouse_down", "left_mouse_up"],
  clipboard: ["read_clipboard", "write_clipboard"],
  recording: ["recording_start", "recording_stop", "recording_status", "recording_list"],
  computer: ["computer_list", "computer_switch", "computer_register", "computer_spawn", "computer_remove"],
  consent: ["consent_status", "consent_allow", "consent_deny", "consent_revoke"],
  key: ["key", "hold_key"],
  browser: ["browser_start", "browser_status", "browser_navigate", "browser_click", "browser_type", "browser_screenshot", "browser_stop"],
  trajectory: ["trajectory_start", "trajectory_stop", "trajectory_status", "trajectory_replay"],
};

/**
 * Parse CODEWHALE_CU_GRANT — "read-only", or a comma list of tool names —
 * into a wire-name set. The grant is fixed when the server starts (there is
 * no tool that can widen it) and it is enforced twice: here, so the model
 * never sees or reaches an ungranted tool, and at the app daemon, so a
 * narrowed server cannot smuggle one through. Returns null when unset.
 */
export function parseGrant(value) {
  if (value == null || (typeof value === "string" && !value.trim())) return null;
  const out = new Set();
  for (const raw of String(value).split(",")) {
    const name = raw.trim();
    if (!name) continue;
    if (name === "read-only") { for (const tool of OBSERVATION_TOOLS) out.add(tool); continue; }
    if (MERGED_EXPANSION[name]) { for (const tool of MERGED_EXPANSION[name]) out.add(tool); continue; }
    out.add(name);
  }
  return out.size ? out : null;
}

// ---------- MCP tool annotations ----------
// Host-facing hints for approval and sandbox policy (MCP spec `annotations`).
// Hints describe the tool's design; they are not runtime gates. Observation
// tools read local state; `openWorld` is true when a tool acts on applications
// or computers outside this process; `destructive` marks tools that change what
// the user sees or holds (input, clipboard, registrations).
const READ_ONLY_ANNOTATION = { readOnlyHint: true, destructiveHint: false, idempotentHint: true, openWorldHint: false };
const INPUT_ANNOTATION = { readOnlyHint: false, destructiveHint: true, idempotentHint: false, openWorldHint: true };
const TOOL_ANNOTATIONS = {
  // Observation — reads only.
  request_access: READ_ONLY_ANNOTATION, computer_list: READ_ONLY_ANNOTATION, list_displays: READ_ONLY_ANNOTATION,
  list_apps: READ_ONLY_ANNOTATION, list_windows: READ_ONLY_ANNOTATION, wait_for: READ_ONLY_ANNOTATION,
  list_sessions: READ_ONLY_ANNOTATION,
  kill_app: { readOnlyHint: false, destructiveHint: true, idempotentHint: false, openWorldHint: true },
  set_window_frame: { readOnlyHint: false, destructiveHint: true, idempotentHint: true, openWorldHint: true },
  get_app_state: READ_ONLY_ANNOTATION, find_elements: READ_ONLY_ANNOTATION, get_value: READ_ONLY_ANNOTATION,
  screenshot: READ_ONLY_ANNOTATION, zoom: READ_ONLY_ANNOTATION, cursor_position: READ_ONLY_ANNOTATION,
  read_clipboard: READ_ONLY_ANNOTATION, recording_list: READ_ONLY_ANNOTATION, recording_status: READ_ONLY_ANNOTATION,
  wait: READ_ONLY_ANNOTATION,
  // Session controls — local state, not the user's apps.
  preview: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: false },
  stop_computer_control: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: false },
  switch_display: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: false },
  recording_start: { readOnlyHint: false, destructiveHint: false, idempotentHint: false, openWorldHint: false },
  recording_stop: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: false },
  // Computer registry — touches other machines.
  computer_switch: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: true },
  computer_register: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: true },
  computer_spawn: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: false },
  computer_remove: { readOnlyHint: false, destructiveHint: true, idempotentHint: true, openWorldHint: true },
  // Consent — the user's own decision record, not an action on apps.
  consent: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: false },
  consent_status: READ_ONLY_ANNOTATION,
  consent_allow: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: false },
  consent_deny: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: false },
  consent_revoke: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: false },
  open_application: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: true },
  // Input — changes what the user sees.
  left_click: INPUT_ANNOTATION, double_click: INPUT_ANNOTATION, triple_click: INPUT_ANNOTATION,
  right_click: INPUT_ANNOTATION, middle_click: INPUT_ANNOTATION, left_click_drag: INPUT_ANNOTATION,
  left_mouse_down: INPUT_ANNOTATION, left_mouse_up: INPUT_ANNOTATION,
  type: INPUT_ANNOTATION, key: INPUT_ANNOTATION, hold_key: INPUT_ANNOTATION, invoke_menu: INPUT_ANNOTATION,
  perform_action: INPUT_ANNOTATION, run_actions: INPUT_ANNOTATION,
  // Scripting — acts on apps through their own dictionaries, not through input.
  app_script: INPUT_ANNOTATION,
  mouse_move: { readOnlyHint: false, destructiveHint: true, idempotentHint: true, openWorldHint: true },
  scroll: { readOnlyHint: false, destructiveHint: true, idempotentHint: false, openWorldHint: true },
  set_value: { readOnlyHint: false, destructiveHint: true, idempotentHint: true, openWorldHint: true },
  focus: { readOnlyHint: false, destructiveHint: true, idempotentHint: true, openWorldHint: true },
  select_text: { readOnlyHint: false, destructiveHint: true, idempotentHint: true, openWorldHint: true },
  write_clipboard: { readOnlyHint: false, destructiveHint: true, idempotentHint: true, openWorldHint: false },
  // Merged surface (aliases keep the wire names above callable).
  click: INPUT_ANNOTATION,
  pointer: INPUT_ANNOTATION,
  browser: INPUT_ANNOTATION,
  trajectory: { readOnlyHint: false, destructiveHint: false, idempotentHint: false, openWorldHint: false },
  trajectory_status: READ_ONLY_ANNOTATION,
  trajectory_start: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: false },
  trajectory_stop: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: false },
  trajectory_replay: { readOnlyHint: false, destructiveHint: true, idempotentHint: false, openWorldHint: true },
  browser_status: READ_ONLY_ANNOTATION,
  browser_screenshot: READ_ONLY_ANNOTATION,
  browser_start: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: true },
  browser_stop: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: true },
  browser_navigate: INPUT_ANNOTATION,
  browser_click: INPUT_ANNOTATION,
  browser_type: INPUT_ANNOTATION,
  clipboard: { readOnlyHint: false, destructiveHint: true, idempotentHint: false, openWorldHint: false },
  recording: { readOnlyHint: false, destructiveHint: false, idempotentHint: false, openWorldHint: false },
  computer: { readOnlyHint: false, destructiveHint: true, idempotentHint: true, openWorldHint: true },
};
for (const tool of TOOLS) {
  tool.annotations = TOOL_ANNOTATIONS[tool.name] ?? INPUT_ANNOTATION;
}

/**
 * Tools that only observe, straight from their annotations. This is the
 * "read-only" capability grant — distinct from READ_ONLY_TOOLS (the smaller
 * post-kill-switch set that also drives the safety valve).
 */
export const OBSERVATION_TOOLS = new Set(TOOLS.filter((t) => t.annotations.readOnlyHint === true).map((t) => t.name));

/**
 * Tools refused with `computer_busy_human_driving` while a person holds the
 * control lease (src/lease.mjs). Derived fail-closed: every tool that is not
 * an observation and acts on the world is gated unless it is listed here as
 * session bookkeeping. run_actions and trajectory_replay are gated per step
 * (they re-enter callTool); browser_stop only detaches in attach mode.
 */
const LEASE_EXEMPT = new Set([
  "computer", "computer_switch", "computer_register", "computer_spawn", "computer_remove",
  "trajectory_replay", "run_actions", "browser_stop",
]);
export const LEASE_GATED_TOOLS = new Set(TOOLS.filter((t) =>
  !OBSERVATION_TOOLS.has(t.name) && !READ_ONLY_TOOLS.has(t.name) && !LEASE_EXEMPT.has(t.name)
  && (t.annotations.openWorldHint === true || t.annotations.destructiveHint === true)).map((t) => t.name));

/**
 * Merged-away names. They stay callable as aliases (receipts, pinned hosts and
 * existing tests keep working) but never appear in tools/list — the advertised
 * surface is what costs every session context.
 */
const HIDDEN_FROM_LIST = new Set([
  "left_click", "double_click", "triple_click", "right_click", "middle_click",
  "mouse_move", "left_mouse_down", "left_mouse_up",
  "read_clipboard", "write_clipboard",
  "recording_start", "recording_stop", "recording_status", "recording_list",
  "computer_list", "computer_switch", "computer_register", "computer_spawn", "computer_remove",
  "consent_status", "consent_allow", "consent_deny", "consent_revoke",
  "hold_key",
  "browser_start", "browser_status", "browser_navigate", "browser_click", "browser_type", "browser_screenshot", "browser_stop",
  "trajectory_start", "trajectory_stop", "trajectory_status", "trajectory_replay",
]);
for (const tool of TOOLS) {
  if (HIDDEN_FROM_LIST.has(tool.name)) tool.hidden = true;
}

/**
 * Expand a merged, advertised tool into the wire tool it dispatches to.
 * Runs before every gate in the dispatcher (required args, kill switch,
 * routing), so a merged call can never bypass one; validation that the wire
 * schema cannot express (which action, what each action needs) lives here and
 * fails as bad_args with the requested name. Unknown names pass through
 * unchanged — the alias surface is the rest of TOOLS.
 */
export function resolveTool(name, args = {}) {
  const bad = (message) => Object.assign(new Error(message), { code: "bad_args" });
  switch (name) {
    case "click": {
      const button = args.button ?? "left";
      const clicks = args.clicks ?? 1;
      const rest = { ...args };
      delete rest.button;
      delete rest.clicks;
      const wire = button === "left" && clicks === 1 ? "left_click"
        : button === "left" && clicks === 2 ? "double_click"
        : button === "left" && clicks === 3 ? "triple_click"
        : button === "right" && clicks === 1 ? "right_click"
        : button === "middle" && clicks === 1 ? "middle_click"
        : null;
      if (!wire) throw bad(`click supports left with 1-3 clicks, right x1 or middle x1 (got ${JSON.stringify(button)} x${clicks})`);
      if (button !== "left") delete rest.strategy; // strategy is an a11y-left-click concept
      return { name: wire, args: rest };
    }
    case "pointer": {
      const rest = { ...args };
      delete rest.action;
      const wire = { move: "mouse_move", down: "left_mouse_down", up: "left_mouse_up" }[args.action];
      if (!wire) throw bad(`pointer action must be "move", "down" or "up" (got ${JSON.stringify(args.action)})`);
      return { name: wire, args: rest };
    }
    case "clipboard": {
      const rest = { ...args };
      delete rest.action;
      if (args.action === "read") return { name: "read_clipboard", args: { computer: rest.computer } };
      if (args.action === "write") {
        if (typeof rest.text !== "string") throw bad("clipboard action \"write\" requires text");
        return { name: "write_clipboard", args: { text: rest.text, computer: rest.computer } };
      }
      throw bad(`clipboard action must be "read" or "write" (got ${JSON.stringify(args.action)})`);
    }
    case "recording": {
      const rest = { ...args };
      delete rest.action;
      const wire = { start: "recording_start", stop: "recording_stop", status: "recording_status", list: "recording_list" }[args.action];
      if (!wire) throw bad(`recording action must be start, stop, status or list (got ${JSON.stringify(args.action)})`);
      if ((args.action === "stop" || args.action === "status") && rest.id == null) throw bad(`recording action "${args.action}" requires id`);
      return { name: wire, args: rest };
    }
    case "computer": {
      const rest = { ...args };
      delete rest.action;
      const wire = { list: "computer_list", switch: "computer_switch", register: "computer_register", spawn: "computer_spawn", remove: "computer_remove" }[args.action];
      if (!wire) throw bad(`computer action must be list, switch, register, spawn or remove (got ${JSON.stringify(args.action)})`);
      if (args.action === "list") return { name: wire, args: {} };
      if (rest.id == null) throw bad(`computer action "${args.action}" requires id`);
      const id = rest.id;
      delete rest.id;
      return { name: wire, args: { ...rest, computer: id } };
    }
    case "consent": {
      const rest = { ...args };
      delete rest.action;
      const wire = { status: "consent_status", allow: "consent_allow", deny: "consent_deny", revoke: "consent_revoke" }[args.action];
      if (!wire) throw bad(`consent action must be status, allow, deny or revoke (got ${JSON.stringify(args.action)})`);
      if (args.action === "status") return { name: wire, args: { computer: rest.computer } };
      const foreground = rest.scope === "foreground";
      const confirming = args.action === "allow" && typeof rest.confirm === "string";
      if (!foreground && !confirming && rest.app == null && rest.name == null && rest.bundle_id == null && rest.pid == null) {
        throw bad(`consent action "${args.action}" needs an app (name, bundle_id, pid or app string) — or scope:"foreground" for the shared-pointer decision`);
      }
      return { name: wire, args: rest };
    }
    case "key": {
      if (args.duration == null) return { name, args };
      const { duration, repeat, target, ...rest } = args;
      if (repeat != null || target != null) throw bad("key with duration holds the key — repeat and target cannot be combined with it");
      if (!Number.isFinite(duration) || duration < 0.05 || duration > 30) throw bad("duration must be 0.05..30 seconds");
      return { name: "hold_key", args: { ...rest, duration } };
    }
    case "browser": {
      const rest = { ...args };
      delete rest.action;
      switch (args.action) {
        case "start":
          if (rest.url != null && typeof rest.url !== "string") throw bad("browser start url must be a string");
          return { name: "browser_start", args: rest };
        case "status": return { name: "browser_status", args: rest };
        case "navigate":
          if (typeof rest.url !== "string" || !rest.url.trim()) throw bad('browser action "navigate" requires url');
          return { name: "browser_navigate", args: rest };
        case "click": {
          const hasSelector = typeof rest.selector === "string" && rest.selector.trim();
          const hasPoint = rest.point != null && Number.isFinite(rest.point?.x) && Number.isFinite(rest.point?.y);
          if (hasSelector && hasPoint) throw bad('browser action "click" takes selector or point, not both — pick one target');
          if (!hasSelector && !hasPoint) throw bad('browser action "click" needs selector (CSS) or point {x,y}');
          if (!hasSelector) delete rest.selector;
          if (!hasPoint) delete rest.point;
          return { name: "browser_click", args: rest };
        }
        case "type":
          if (typeof rest.text !== "string" || !rest.text.length) throw bad('browser action "type" requires text');
          return { name: "browser_type", args: rest };
        case "screenshot": return { name: "browser_screenshot", args: rest };
        case "stop": return { name: "browser_stop", args: rest };
        default:
          throw bad(`browser action must be start, status, navigate, click, type, screenshot or stop (got ${JSON.stringify(args.action)})`);
      }
    }
    case "trajectory": {
      const rest = { ...args };
      delete rest.action;
      switch (args.action) {
        case "start": return { name: "trajectory_start", args: rest };
        case "stop": return { name: "trajectory_stop", args: rest };
        case "status": return { name: "trajectory_status", args: rest };
        case "replay": return { name: "trajectory_replay", args: rest };
        default:
          throw bad(`trajectory action must be start, stop, status or replay (got ${JSON.stringify(args.action)})`);
      }
    }
    default:
      return { name, args };
  }
}
