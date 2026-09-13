---
name: computer-use
description: Full desktop control on macOS, Windows, Linux, and HarmonyOS — accessibility-first observation and actions with pixel fallback, screenshots, zoom, screen recording, and switching between registered computers as a default.
---

# Codewhale Computer Use

## Computers first

The plugin controls **computers**, not "the screen". `computer_list` shows the
registry; one computer is always **active**, and every tool acts on the active
computer unless given `computer`.

- Pass `computer: "<id>"` on any tool to act on (and stickily switch to) that
  computer. `computer_switch` changes the active computer without acting.
- `local` is the machine the plugin runs on. `ssh` computers run the bundled
  remote agent (pushed automatically at registration). `hdc` computers are
  HarmonyOS devices driven over hdc.
- Every receipt names the computer it happened on. Read it before continuing —
  never assume the action landed on the machine you meant.

## Core loop

Observe once, act once, then verify.

1. If readiness is unknown, call `request_access` once. It names missing
   permissions and missing tools per platform, and never pops dialogs. Its
   `via` field says who holds the permissions: `"app"` means the Codewhale
   Computer Use desktop app is doing the work (grants belong to it);
   `"direct"` means the hosting app or terminal is. Follow the actual
   `appHint`: bundled Codewhale builds already carry their native helper.
2. `list_apps` shows running apps only. If the user names an app that is
   absent, call `open_application` once with the original user-provided name,
   copied character-for-character — including case, spaces, punctuation, and
   suffixes such as `app` or `.exe`. Do not translate, localize, normalize,
   shorten, or retry with guesses.
3. `get_app_state` defaults to a text-first summary (macOS AX / Windows
   UIA / Linux AT-SPI / HarmonyOS uitest) with controls, values, actions,
   layout, element indices and a `state_id`. Start here without a screenshot,
   whether or not the model supports vision. Use `detail:"full"` for nested
   menus and tree structure; `compact` remains a summary alias. Missing labels
   or values mean unknown content, not something to guess.
4. If the tree contains the target, act on the element: `perform_action`
   (AXPress/Invoke/click…), `set_value` for editable fields, element click.
   The element path is background-safe on macOS and UIA platforms.
5. When accessibility cannot read visible text, macOS supports
   `get_app_state({app_ref, include_ocr:true})`. This explicitly captures the
   selected app window and recognizes text locally, without a vision model or
   remote service. Check `ocr.status`; recognized blocks include confidence,
   pixel bounds and ready-to-use coordinate targets. OCR text is not a control
   role or an advertised action. Verify uncertain text and observe again after
   changes. Other platforms return an explicit unavailable status while keeping
   their accessibility state usable. A text-only model must not infer unlabeled
   icons, charts or other graphical meaning from OCR or a screenshot file path.
   With vision, when accessibility cannot express the target: `screenshot` (optionally
   `zoom` for small targets) and act with a coordinate target. Coordinates are
   pixels **in the latest returned raster** for that computer; the server maps
   them to screen points. After a new screenshot, old pixels are stale.
   If the host reports an omitted or oversized image, capture a smaller app
   window/region or zoom, then use that returned raster. Do not guess from a
   file path or reuse coordinates from an image the model never received.
6. Verify with a fresh observation or a task oracle before claiming success.
   `action_sent: true` means it may already have happened — never replay.
   On macOS `type` also reports `verified`: `false` (with
   `verification_required: "screenshot"`) means the focused control's value
   did not reflect the text, so confirm with a screenshot before relying on
   the input.

## Choosing targets

- Element: `{"type":"element","state_id":"s-1","index":4}` — prefer this.
  Elements are revalidated against the live tree before every action: if the
  element moved, the click lands on its fresh center and the receipt carries
  `target_reacquired: true`; if it no longer resolves (or changed role) the
  call fails `element_stale` — call `get_app_state` again for a fresh
  `state_id`. A `state_id` only works on the computer that issued it
  (`state_wrong_computer`).
- Coordinate: `{"type":"coordinate","x":496,"y":331}` — pixels from the latest
  raster only; submit `x`/`y` unchanged, never transform them yourself.
  `zoom` returns a bindable raster of its own: after zooming, coordinates are
  pixels in the zoomed image. Points outside the bound raster fail
  `target_outside_raster` instead of landing somewhere unintended.
- Never translate pixels into an element target; never invent `state_id`s.

## Raw input reality (read before clicking)

- macOS: call `open_application` with `activate:false` to bind input to the
  intended process, even when the app is already running; pass `pid` when two
  processes share a bundle id. Then the two halves behave differently:
  - **Keyboard and element actions are quiet.** `type`, `key`, `set_value`,
    `select_text` and `perform_action` reach the bound process without moving
    the pointer or changing the foreground. Prefer them. Text entry uses writable
    accessibility selection when available; verify the resulting value.
    `get_app_state`, `list_windows` and `screenshot` default to the selected app.
  - **Background mode never takes the shared pointer.** A coordinate
    `left_click` first tries the bound application's accessibility action,
    including field focus and row selection. `right_click` uses advertised
    context-menu actions. `scroll` uses the target's accessibility scrollbar;
    prefer a scroll-area element and read the receipt's unit and value change.
    Raw double/triple/middle click, drag and hover fail with
    `shared_pointer_required` before moving the cursor. Missing semantic
    scrolling or context-menu support is a refusal, never permission to activate.
    Use another advertised accessibility action or a separate computer.
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
  - A pointer gesture is refused when another application's window covers the
    point; it names the owner. Observe again and use the selected control's
    accessibility action, or wait for authorized exclusive desktop use. Do not
    move or close the reported window.
  - An accessibility press refuses to cross a modal sheet
    (`window_blocked_by_modal_sheet`): deal with the sheet first.
  Use app-scoped screenshots (`app_ref`) to avoid capturing unrelated windows.
  Watching the preview does not authorize shared-desktop control. Enable it
  only when the user asks to watch; disable it when finished. The preview is a
  local app view, not an isolated desktop. Process-directed actions still
  change the target app: do not work in an app the user is actively editing.
  Close only disposable documents created by your task; never quit a user app.
- Windows/Linux: raw input is foreground by nature; UIA/AT-SPI element actions
  are the precise path.
- HarmonyOS: `uitest` synthesizes touches; there is no hover or cursor.

## Keyboard

- macOS uses `cmd` (`cmd+c`), Linux/Windows use `ctrl` (`ctrl+c`).
- `key` for chords and repeats, `hold_key` for a duration, `type` for text.
- Prefer `set_value` on editable elements over typing.

## Recording

`recording_start` → work → `recording_stop` returns the finalized file path.
macOS uses ScreenCaptureKit inside the signed helper — no system recorder UI
and no desktop dimming overlay (a receipt warning about Screen Recording
permission means the user must grant it once). Linux and Windows recording is
unavailable pending session-owned cleanup; use screenshots. HarmonyOS uses
snapshot-series (no native CLI recorder —
the receipt says so). `recording_status` / `recording_list` report bytes and
paths. Screenshots land in the same directory.

## Safety

- `stop_computer_control` is the kill switch; after it, actions fail closed
  for the session. Do not continue after it or after a denied permission.
- Never retry a refused action unchanged. Re-observe, choose a fresh target.
- If a permission is explicitly denied, tell the user which permission in
  which Settings pane, and end the turn. Do not promise later retries.

## Recipes

- **Screenshot** — optionally a computer id, display index, or `[x,y,w,h]`
  region; call `screenshot`; report path, size, computer/display. Black or
  empty capture means Screen Recording permission is missing (macOS) for the
  app (`via: "app"`) or the host terminal (`via: "direct"`): say which and
  stop.
- **Record** — `recording_start` (parse computer id, fps, display, duration
  or "record for 30s" → `durationSec` on macOS), then report id, path, mode.
  To stop, find the running id via `recording_list` and call `recording_stop`.
- **Switch computers** — `computer_list`; if asked to add: ssh `user@host`
  (agent is pushed automatically) or `hdc [target]` for a HarmonyOS device;
  otherwise show the registry and remind that any tool accepts `computer`.
- **Status** — `computer_list`, then `request_access` per computer; call out
  anything that will fail closed with the exact install hint from the receipt.
