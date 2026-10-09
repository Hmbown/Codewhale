---
name: recording
description: Capture screenshots on registered computers, record on macOS or HarmonyOS, and manage saved captures. Probe capabilities before recording.
---

# Recording and screenshots

1. Pick the computer (`computer_list`, or pass `computer` — it switches).
2. Check `request_access` when readiness is unknown. On a supported target,
   use `recording_start` with optional `display`, `region`, `fps` and
   `durationSec` (macOS auto-stop), or `intervalMs` (HarmonyOS snapshot cadence).
3. Do the work (or let the user do it).
4. `recording_stop` with the returned `id` → finalized file path + bytes.
5. `recording_list` shows everything saved; `recording_status` checks one.

Platform truths:

- **macOS**: ScreenCaptureKit inside the signed helper captures a display or
  region straight to .mov — no system recorder selection UI and no desktop
  dimming overlay. If capture is denied, Screen Recording permission is
  missing for whoever ran it: the Codewhale Computer Use app when
  `request_access` says `via: "app"`, else the terminal that hosts the
  server — tell the user which, do not retry.
- **Windows and Linux**: recording is unavailable pending session-owned
  recorder cleanup. Use screenshots; installing ffmpeg does not enable it.
- **HarmonyOS**: no native CLI recorder; the backend captures
  `snapshot_display` frames at `intervalMs` and muxes with ffmpeg on stop.
  The receipt labels the mode `snapshot-series` — never call it real-time.

Screenshots: `screenshot` returns a saved path, geometry and `raster_id`. Carry
that ID in pixel targets and as the parent of `zoom` when a target is too small
to read. Each zoom returns a new child ID; use it for child-image coordinates.
OCR coordinate targets already include their pin. Pins remain reusable while
current; older unpinned calls retain latest-raster behavior. A replaced capture,
app launch or route change retires the context. On `raster_stale` or `no_raster`,
observe again; never drop the ID to retry. Pins detect capture replacement,
not a changed UI: re-observe after UI changes and verify input separately.
An omitted image cannot supply coordinates.
