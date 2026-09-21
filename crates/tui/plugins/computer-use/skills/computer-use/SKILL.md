---
name: computer-use
description: Desktop control that picks the right interface per step — app scripting (AppleScript/JXA), accessibility-first observation and actions, pixel fallback, screenshots, zoom, screen recording, and switching between registered computers. Qualified on macOS; Windows, Linux and HarmonyOS backends are experimental.
---

# Codewhale Computer Use

## Computers first

The plugin controls **computers**, not "the screen". `computer {action:"list"}` shows the
registry; one computer is always **active**, and every tool acts on the active
computer unless given `computer`.

A computer is an execution environment, not necessarily the user's desktop.
The registry holds two kinds:

- **Spawned computers are ours.** `computer {action:"spawn", id:"<id>",
  transport:"docker"}` provisions a disposable Linux desktop container,
  registers it, and makes it active. Every tool works on it unchanged; the
  user's own machine is never touched. It is destroyed by `computer
  {action:"remove"}` or when the session ends. **Prefer a spawned computer
  for any work that does not need the user's own session** — it is the
  isolated workspace, not a workaround for sharing theirs politely.
- **Registered computers are someone's.** `local` is the machine the plugin
  runs on — the user's desktop, with their logged-in apps and their pointer.
  `ssh` computers run the bundled remote agent (pushed automatically at
  registration). `hdc` computers are HarmonyOS devices driven over hdc.
  Reach for `local` only when the task genuinely needs that session — their
  Mail, their signed-in browser, their files on screen. A spawned desktop
  cannot replace that, and pretending otherwise is the failure mode this
  distinction exists to prevent.

Other rules:

- Pass `computer: "<id>"` on any tool to act on (and stickily switch to) that
  computer. `computer_switch` changes the active computer without acting.
- Every receipt names the computer it happened on. Read it before continuing —
  never assume the action landed on the machine you meant.
- Spawned containers are task-owned: never register one as a normal computer,
  and never treat its filesystem or state as durable — it dies with the task.

## Human controls

When the local helper is installed, it owns the input route even when the
host also includes a native binary. A disconnected helper is an error, never
permission to bypass it with direct input. `control_paused` and
`control_stopped` mean the person paused or stopped Computer Use. Stop acting
and wait for them; do not change environment variables, restart the helper,
create another session or use another tool to defeat their choice. After Stop,
the old session remains invalid even when the person allows new sessions.
The helper's own setup, permission and safety controls belong to the person.
Do not operate them or approve the host's pending authorization yourself.

## Consent on the user's computer

The app, not the tool, is the unit of trust on `local`. The first call that
targets an application — `open_application`, an `app_ref`, an element or
`state_id`, or an action on the bound app — refuses `consent_required`
until the user has decided. Ask them, then record the answer:

- `consent {action:"allow"|"deny", app:"Safari"}` — `app` accepts a name, a
  bundle id, or `pid:`/a bare number for a pid; `name`, `bundle_id` and
  `pid` fields work too. Decisions cover this session; `remember:true`
  persists them for the computer until revoked.
- `consent {action:"status"}` — the ledger: every recorded app decision and
  the foreground decision, each marked session or persisted.
  `consent {action:"revoke", app:"…"}` clears a decision so the next call
  asks again.
- A deny is a wall, not a hint: every spelling of the same app fails
  `app_denied` — the ledger folds name, bundle id and pid together, and a
  denied app cannot be opened, driven, or killed through this surface.
  Only the user can change it; never work around it.

Foreground is a second, separate consent. `open_application
{activate:true}` — the shared-desktop escalation, on any platform —
additionally needs `consent {action:"allow", scope:"foreground"}`; a
refusal reads `foreground_consent_required`, a recorded denial
`foreground_denied`. Background control (`activate:false`, the default
everywhere) needs only the app consent.

Spawned computers are exempt — a task-owned desktop holds nothing of the
user's. Remote computers are covered by their transport's trust, not this
ledger. `app_script` keeps its own OS-level consent: Automation prompts
belong to macOS, not to this ledger.

Only in explicitly authorized foreground mode, where a shared surface is taken
— a front lease for window-record
input, a real-pointer gesture, foreground keys, an activation — the helper
first waits for a gap in the person's hardware input rather than cutting
between their keystrokes. The wait is bounded, never infinite, and every
successful receipt that waited reports `yield_ms`. If no quiet window
arrives, `user_busy` means no input was sent: wait for the person to finish
or use an already-authorized isolated computer; do not disable the yield
or loop on retries. It is turn-taking, not a lock:
`user_input_during_lease:true` still means the outcome is contested —
say so.

## Choose the interface

Clicking is only one way to use a computer. Before each step, pick the
interface that finishes it verifiably with the fewest moving parts — and
switch freely between steps:

1. **The host's own tools** — shell, files, HTTP, git, other MCP apps.
   A step with no reason to be on screen does not belong to this plugin:
   never drive a terminal window to run a command the host can run itself.
2. **`app_script`** — AppleScript/JXA into apps that ship a scripting
   dictionary (most native macOS apps). Deterministic, returns values,
   needs no Accessibility grant, never touches the pointer.
3. **`browser`** — CDP for web work: exact selectors, no pixels.
4. **Accessibility actions** — the GUI loop below. The route for apps
   with no better interface: background-safe, element-precise, verified.
5. **Coordinates and pixels** — last resort, when nothing else can
   express the target.

A step that *can* be clicked still costs more than the same step
scripted, and a pixel click's `action_sent` proves less than a script's
return value or a `get_value` read-back. Prefer the interface whose
receipt can prove the step happened. Switching mid-task is normal —
script Mail for the message, process it through the host, type the
answer into a GUI-only editor; `get_app_state` still verifies what a
script changed.

## The GUI loop

Once the GUI is the right interface: observe once, act once, then verify.

1. If readiness is unknown, call `request_access` once. It names missing
   permissions and missing tools per platform, and never pops dialogs. Its
   `via` field says who holds the permissions: `"app"` means the Codewhale
   Computer Use desktop app is doing the work (grants belong to it);
   `"direct"` means the hosting app or terminal is. Follow the actual
   `appHint`: bundled Codewhale builds already carry their native helper.
   `app.stale:true` means the running helper reports an older version than the
   plugin — tell the user to restart the Codewhale Computer Use app before
   debugging any behavior.
2. `list_apps` shows user-facing apps only; pass `all:true` to include
   menu-bar helpers and background processes. If the user names an app that is
   absent, call `open_application` once with the original user-provided name,
   copied character-for-character — including case, spaces, punctuation, and
   suffixes such as `app` or `.exe`. Do not translate, localize, normalize,
   shorten, or retry with guesses.
3. `get_app_state` defaults to a text-first summary (macOS AX / Windows
   UIA / Linux AT-SPI / HarmonyOS uitest) with controls, values, actions,
   layout, element indices and a `state_id`. Start here without a screenshot,
   whether or not the model supports vision. Pass `query`, `role`, `limit`
   and `offset` instead of dumping the whole tree — truncated dumps hide the
   title and search field. `detail:"compact"` is smaller (same indices,
   shorter labels). `detail:"full"` adds nested menus and tree paths.
   `find_elements` searches a cached `state_id` or observes now. Missing
   labels or values mean unknown content, not something to guess. `get_value`
   reads one field live. On macOS, browsers and Electron/webview apps expose
   page content as `AXWebArea` descendants; the first observation may arrive
   while the page is still populating — re-observe if the tree looks
   suspiciously shallow or a control you can see is absent.
4. If the tree contains the target, act on the element: `focus` then `type`
   or `key` for composers (or pass the element `target` straight to
   `type`/`key` — it focuses first, in the same call), `set_value` for
   ordinary fields, `perform_action` (AXPress/Invoke/click…) for advertised
   actions, element click. Newlines in `type` are Return/Enter;
   `press_enter:true` sends after the text. Never expect `\\n` to send a
   chat message. `run_actions` batches up to 8 steps
   (click → type → key return → get_value).
   macOS provides background element actions; Linux AT-SPI support depends
   on the control. Windows currently refuses scoped semantic mutations.
   Windows and Linux are development backends: do not assume their raw
   input is background-safe or that native Pause/Stop controls are available.
5. When accessibility cannot read visible text, macOS supports
   `get_app_state({app_ref, include_ocr:true})`. Pass `ocr_region:[x,y,w,h]`
   in screen points to recognize one rect instead of the whole window. This
   captures locally, without a vision model. Check `ocr.status`; recognized
   blocks include confidence, pixel bounds and ready-to-use coordinate
   targets. OCR text is not a control role or an advertised action. Verify
   uncertain text and observe again after changes. Other platforms return an
   explicit unavailable status while keeping their accessibility state usable.
   A text-only model must not infer unlabeled icons, charts or other graphical
   meaning from OCR or a screenshot file path.
   With vision, when accessibility cannot express the target: `screenshot`
   (optionally `zoom` for small targets) and act with a coordinate target.
   Default coordinates are pixels **in the latest returned raster**. Pass
   `space:"screen"` to send absolute screen points from the AX tree and skip
   conversion. After a new screenshot, old raster pixels are stale.
   If the host reports an omitted or oversized image, capture a smaller app
   window/region or zoom, then use that returned raster. Do not guess from a
   file path or reuse coordinates from an image the model never received.
5b. When the UI needs time — a page loading, a dialog appearing or
   dismissing, a spinner finishing — call `wait_for` instead of looping
   `get_app_state` + `wait` by hand: it polls the accessibility tree until
   a `query`/`role` match appears (`state:"present"`) or disappears
   (`state:"absent"`), then returns the matched elements bound to a fresh
   `state_id` you can target immediately. A `timed_out:true` receipt means
   the condition never held — observe and reconsider rather than repeating
   the same wait.
6. Verify with a fresh observation or a task oracle before claiming success.
   `action_sent: true` means it may already have happened — never replay.
   On macOS `type` also reports `verified`: `false` (with
   `verification_required: "screenshot"`) means the focused control's value
   did not reflect the text, so confirm with a screenshot before relying on
   the input.

## Choosing targets

- Element: `{"type":"element","index":4}` — prefer this. A bare index binds
  that computer's latest observation; add `state_id` only to pin a specific
  earlier snapshot (e.g. one returned by `wait_for` after newer observes).
  Elements are revalidated against the live tree before every action: if the
  element moved, the click lands on its fresh center and the receipt carries
  `target_reacquired: true`; if it no longer resolves (or changed role) the
  call fails `element_stale` — call `get_app_state` again. A `state_id` only
  works on the computer that issued it (`state_wrong_computer`).
- Coordinate: `{"type":"coordinate","x":496,"y":331}` — pixels from the latest
  raster only; submit `x`/`y` unchanged, never transform them yourself.
  `{"type":"coordinate","x":100,"y":200,"space":"screen"}` is an absolute
  screen point (what AX `position` uses). `zoom` returns a bindable raster of
  its own: after zooming, raster coordinates are pixels in the zoomed image.
  Points outside the bound raster fail `target_outside_raster` instead of
  landing somewhere unintended.
- Never translate pixels into an element target; never invent `state_id`s.

## Raw input reality (read before clicking)

- macOS: call `open_application` with `activate:false` to bind input to the
  intended process, even when the app is already running; pass `pid` when two
  processes share a bundle id. Then the two halves behave differently:
  - **Keyboard and element actions are quiet.** `type`, `key`, `focus`,
    `set_value`, `get_value`, `select_text` and `perform_action` reach the
    bound process without moving the pointer or changing the foreground.
    Prefer them. Text entry uses writable accessibility selection when
    available; verify the resulting value. `get_app_state`, `list_windows`
    and `screenshot` default to the selected app.
  - **Background mode does not borrow keyboard focus.** Accessibility
    click, focus, selection and scroll actions remain available. Raw pointer
    fallbacks, modified/window-targeted keys, web value replacement and typing
    paths that require a key-window lease refuse `background_focus_required`
    before delivery. Use an accessibility menu/control, browser control or an
    authorized separate computer. Do not escalate to foreground or retry the
    same action merely because the user stopped typing briefly.
  - Shared-desktop gestures and foreground keyboard delivery require explicit
    user authorization for exclusive desktop use, followed by
    `open_application(activate:true)` — which itself needs the foreground
    consent (`consent {action:"allow", scope:"foreground"}`; see Consent on
    the user's computer). Do not select it merely to work around a
    background refusal. Receipts identify `input_scope: "shared-desktop"`;
    pointer gestures use the physical cursor, even if it is restored afterward.
    Keys and raw pointer gestures stop when another app takes focus. Never
    keep reactivating after the user takes control; return to `activate:false`
    when the shared-desktop step ends.
  - Menus appear in `get_app_state`. Use the advertised action (often
    `AXPress` to open a menu, then `AXPick` on its item), then observe again.
    `invoke_menu {path:["File","New"]}` does that traversal in one call,
    through accessibility alone — no key events, no focus lease. App-level
    commands (New, Save, Quit) are reliable without a key window;
    window-targeted items (Close) can validate against a key window the
    background app does not have and legitimately no-op — close windows
    through their close-button element instead. Exact titles only; a present
    but disabled item is refused (`menu_item_disabled`) rather than pressed.
  - A pointer gesture is refused when another application's window covers the
    point; it names the owner. Observe again and use the selected control's
    accessibility action, or wait for authorized exclusive desktop use. Do not
    move or close the reported window.
  - An accessibility press refuses to cross a modal sheet
    (`window_blocked_by_modal_sheet`): deal with the sheet first.
  - Virtualized lists vend collapsed placeholder rows (zero-size frames).
    Acting on one fails `degenerate_frame` — scroll the real row into view
    and re-observe rather than retrying the same index.
  - `set_value` coerces numbers for `AXIncrementor`/`AXSlider`/`AXStepper`
    and verifies the readback. Web-area direct AXValue writes are unreliable;
    background mode refuses the focus/select-all replacement. Use browser
    control. The replacement is available only during explicitly authorized
    foreground control.
  Use app-scoped screenshots (`app_ref`) to avoid capturing unrelated windows.
  The nonactivating preview panel is on by default while an app is bound —
  it shows the captured app window and a drawn cursor at each action's target
  so the user can watch; the real pointer never moves. `preview(enabled:false)`
  mutes it for the session. The preview is a local app view, not an isolated
  desktop; watching it does not authorize shared-desktop control. Process-directed actions still
  change the target app: do not work in an app the user is actively editing.
  Close only disposable documents created by your task; never quit a user app.
- Windows/Linux: `open_application` still defaults to `activate:false` —
  Windows launches the app minimized and Linux hands focus back to the
  previous window — but raw input there is foreground by nature;
  UIA/AT-SPI element actions are the precise path.
- HarmonyOS: `uitest` synthesizes touches; there is no hover or cursor.

## Keyboard

- macOS uses `cmd` (`cmd+c`), Linux/Windows use `ctrl` (`ctrl+c`).
- `key` is the key-press tool: `return`, `enter`, `backspace`, `tab`,
  `escape`, chords and repeats. `key {duration}` holds a key for a duration.
- `type` sends unicode. Newlines and `press_enter` become Return; they do
  not insert a literal line break or U+FFFC.
- Prefer `set_value` on ordinary fields; prefer `focus` then `type`/`key`
  on chat composers.

## Recording

`recording {action:"start"}` → work → `recording {action:"stop", id}` returns the finalized file path.
macOS uses ScreenCaptureKit inside the signed helper — no system recorder UI
and no desktop dimming overlay (a receipt warning about Screen Recording
permission means the user must grant it once). Linux and Windows recording is
unavailable pending session-owned cleanup; use screenshots. HarmonyOS uses
snapshot-series (no native CLI recorder —
the receipt says so). `recording_status` / `recording_list` report bytes and
paths. Screenshots land in the same directory.

## Scripting apps (macOS)

`app_script {script, language?, timeout?}` runs AppleScript (default) or
JXA (`language:"javascript"`) through osascript on the local computer.
`result` is the script's stdout; a non-zero exit fails `script_error`
with stderr, and `script_timeout` means the script — or a consent dialog
— was still open.

- A first script targeting an app may show the person an Automation
  consent dialog; that is their choice, not your error. A declined or
  missing consent fails `automation_denied` (-1743): name the pane
  (System Settings → Privacy & Security → Automation) and stop — never
  retry it away.
- Read the dictionary before writing: `sdef /Applications/Mail.app`
  through the host's shell, or Script Editor's Library window. A guessed
  property earns `script_error` (-1728/-2740) — check the dictionary,
  don't retry with another guess.
- `tell application "X"` launches X if needed; no `open_application`
  required, and the script runs while X stays in the background.
- `do shell script "…"` inside a script works, but prefer the host's own
  shell for shell work — keep `app_script` for app control and the parts
  only a dictionary exposes.
- ssh, docker and hdc computers refuse it (`unsupported_on_transport`):
  remote channels stay computer-use only, never a shell — a spawned
  desktop is no exception. Windows and Linux backends fail
  `unsupported_on_backend` for now.

## Browser (CDP)

`browser` drives a Chromium-family browser over the DevTools protocol in a
self-owned profile — the user's own browser is never attached to, typed into,
or closed. `start` opens (or reuses) the instance and binds this session's
own tab; then `navigate`, `click` (CSS selector or viewport point), `type`
(optional focus selector, `enter`), `screenshot`, `status`, `stop`. Elements
are addressed exactly, no pixels: prefer this over screen clicking for web
work. Page screenshots are a different space from screen captures
(`space: "page-viewport"`) — coordinate clicks take that space, never screen
points. Verify effects by observing: `status` reports the tab's live url and
title, and a fresh `screenshot` shows the rendered truth. One tab per
session; the last session out closes the shared browser. Node 22+ is needed
for the WebSocket transport; older runtimes refuse with `unsupported_runtime`.

## Recording and scope

`trajectory` records every tool call this session makes into a local JSONL
(off until started; arguments are stored verbatim, so treat the file as
sensitive). `replay` re-runs a recorded file through the same pipeline —
grants, permissions and the kill switch still apply — and stops at the first
refusal; `dry_run` lists the plan first. A host may narrow the whole session
with `CODEWHALE_CU_GRANT` (read-only, or a tool list): tools outside it are
never advertised and calls fail `not_granted`. Work inside that scope; do not
look for a workaround. `set_window_frame` moves or resizes one window and
reports the app's own readback — when an app constrains or refuses part of
the frame the receipt says so (`verified:false`, `ax_errors`, or
`frame_refused`), and that is the app's answer, not a failure to retry blindly.

## Safety

- `stop_computer_control` is the kill switch; after it, actions fail closed
  for the session. Do not continue after it or after a denied permission.
- `list_sessions` shows the live sessions and the user's control mode. When
  another model or agent is mid-task on the same machine, coordinate through
  the person instead of fighting for the same window; `kill_app` quits an app
  (never the helper itself) and verifies the termination in its receipt.
- Never retry a refused action unchanged. Re-observe, choose a fresh target.
- If a permission is explicitly denied, tell the user which permission in
  which Settings pane, and end the turn. Do not promise later retries.

## Recipes

- **Screenshot** — optionally a computer id, display index, or `[x,y,w,h]`
  region; call `screenshot`; report path, size, computer/display. Black or
  empty capture means Screen Recording permission is missing (macOS) for the
  app (`via: "app"`) or the host terminal (`via: "direct"`): say which and
  stop.
- **Record** — `recording {action:"start"}` (parse computer id, fps, display, duration
  or "record for 30s" → `durationSec` on macOS), then report id, path, mode.
  To stop, find the running id via `recording {action:"list"}` and call `recording {action:"stop", id}`.
- **Switch computers** — `computer {action:"list"}`; if asked to add: ssh `user@host`
  (agent is pushed automatically) or `hdc [target]` for a HarmonyOS device;
  otherwise show the registry and remind that any tool accepts `computer`.
- **Status** — `computer {action:"list"}`, then `request_access` per computer; call out
  anything that will fail closed with the exact install hint from the receipt.

## References

The advertised tools are merged for context economy — `click`, `pointer`,
`clipboard`, `recording`, `computer`, and `key {duration}` for holds. The
per-action wire names (`left_click`, `read_clipboard`, `recording_start`,
`computer_list`, `hold_key`, …) remain callable as aliases.

- `references/quick-reference.md` — every tool on one page, plus the common
  recipes (type into a field, close a window without borrowing focus,
  switch apps mid-task).
- `references/refusal-codes.md` — the fail-closed codes, what each means,
  and the move that fixes it.
