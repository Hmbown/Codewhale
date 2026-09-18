---
name: computer-use
description: Desktop control with accessibility-first observation and actions, pixel fallback, screenshots, zoom, screen recording, and switching between registered computers. Qualified on macOS; Windows, Linux and HarmonyOS backends are experimental.
---

# Codewhale Computer Use

## Computers first

The plugin controls **computers**, not "the screen". `computer {action:"list"}` shows the
registry; one computer is always **active**, and every tool acts on the active
computer unless given `computer`.

- Pass `computer: "<id>"` on any tool to act on (and stickily switch to) that
  computer. `computer_switch` changes the active computer without acting.
- `local` is the machine the plugin runs on. `ssh` computers run the bundled
  remote agent (pushed automatically at registration). `hdc` computers are
  HarmonyOS devices driven over hdc.
- Every receipt names the computer it happened on. Read it before continuing —
  never assume the action landed on the machine you meant.

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

## Core loop

Observe once, act once, then verify.

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
  - **Background mode never moves the user's cursor.** A coordinate
    `click` first tries the bound application's accessibility action,
    including focusing a field that is not AXPressable. `right_click` uses
    advertised context-menu actions. `scroll` uses the target's accessibility
    scrollbar; prefer a scroll-area element and read the receipt's unit and
    value change. Where accessibility cannot act — a point with no pressable
    element, drag, raw double/triple/middle click, wheel scrolling without an
    AX scrollbar — the window-record route delivers genuine mouse/wheel
    events to the bound app's window: the cursor never moves, and a momentary
    no-raise front-process lease is taken and restored (every receipt says
    `strategy:"window-record"`, `pointer_moved:false`). Lease accounting is
    explicit: `front_lease:true` plus `front_restored` when a lease was taken
    (a failed restore is stated in the receipt — report it to the user), and
    `front_lease:false` when the target was already frontmost and no lease was
    needed. A taken lease also reports its borrow window (`lease_ms`) and the
    hardware-input clock around it (`idle_before_s`, `idle_after_s`); the
    verdict `user_input_during_lease:true` means the person's own input
    arrived mid-lease — treat the outcome as contested, re-observe, and say
    so. `key` chords that had no window to route through fall back to
    process delivery and say so instead of pretending.
    Only hover and held-button tools still need `activate:true`.
  - Shared-desktop gestures and foreground keyboard delivery require explicit
    user authorization for exclusive desktop use, followed by
    `open_application(activate:true)`. Do not select it merely to work around a
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
    and verifies the readback. Web-area elements take the replacement path
    automatically (focus, select-all through the record channel, type,
    read-back verify — receipt `strategy:"focus-type-replace"`) because
    Chromium silently no-ops or coerces direct `AXValue` writes.
  Use app-scoped screenshots (`app_ref`) to avoid capturing unrelated windows.
  The nonactivating preview panel is on by default while an app is bound —
  it shows the captured app window and a drawn cursor at each action's target
  so the user can watch; the real pointer never moves. `preview(enabled:false)`
  mutes it for the session. The preview is a local app view, not an isolated
  desktop; watching it does not authorize shared-desktop control. Process-directed actions still
  change the target app: do not work in an app the user is actively editing.
  Close only disposable documents created by your task; never quit a user app.
- Windows/Linux: raw input is foreground by nature; UIA/AT-SPI element actions
  are the precise path.
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
