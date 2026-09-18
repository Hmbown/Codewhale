# Refusal codes and the move that fixes them

Every refusal is structured: `ok:false` with an `error.code` you can branch on.
Never retry a refusal unchanged — re-observe, re-target, or change route.

## Target and state

| code | meaning | move |
| --- | --- | --- |
| `element_stale` | the live tree no longer matches the observation (user or app changed it) | `get_app_state` again and re-target |
| `unknown_state` | no observation on this computer (or the app was rebound) | observe first; bare indices bind the latest observation of the bound app |
| `state_wrong_computer` | `state_id` came from a different computer | observe on the computer you are acting on |
| `unknown_element` | index outside the cached tree | observe again; use `query`/`role` filters |
| `element_no_geometry` | element has no frame | use a coordinate target from a fresh raster |
| `degenerate_frame` | zero-size placeholder row (virtualized list) | scroll the real row into view, re-observe |
| `target_outside_raster` | coordinate outside the bound screenshot | take a fresh screenshot/zoom and use its pixels |
| `no_raster` | coordinate target with no raster bound | `screenshot` first |
| `window_blocked_by_modal_sheet` | an accessibility press would cross a sheet | deal with the sheet first |
| `window_ambiguous` | two windows share the resolved window's frame (stacked or identical geometry) | `list_windows`, pick one, pass `window_id` (or re-target the press) |
| `window_target_not_found` | PID has no eligible window | `list_windows`; open or pick the right app |

## Route and policy

| code | meaning | move |
| --- | --- | --- |
| `shared_pointer_required` | background mode refuses pointer gestures | use element targets; shared desktop needs the user's explicit authorization |
| `background_scroll_unavailable` | no scrollbar at that point | target an observed scroll area |
| `menu_item_not_found` | exact title not present (menus expose items only while open) | check the exact title; an ellipsis is part of it |
| `menu_item_disabled` | item present but the app refuses it right now (often a missing key window) | use the window's own control element instead |
| `app_not_found` | selector missed — `open_application` names/bundle ids that resolve nowhere and dead pids report it too | `list_apps` (or `all:true`) for exact names/pids |
| `ambiguous_application` | `kill_app` name matched several running apps | pass `pid` to choose one |
| `protected_application` | the target is the Computer Use helper or its host | name the intended app instead; these cannot be terminated through the plugin |
| `browser_not_running` | browser action before `browser {action:"start"}` (or the browser went away) | start it; a closed CDP connection clears the session state |
| `browser_not_installed` | no Chromium-family browser found | install one, or set `CODEWHALE_CU_BROWSER_APP` to the app path |
| `selector_not_found` | no element matches the CSS selector on the current page | re-check the selector against a fresh `browser {action:"screenshot"}` or `browser {action:"status"}` |
| `unsupported_runtime` | this Node has no global WebSocket (browser transport) | use Node 22+ for the daemon/server running the plugin |
| `not_granted` | the session's capability grant (`CODEWHALE_CU_GRANT`) does not include this tool | work inside the grant; the host narrowed it deliberately |
| `frame_refused` | the app refused both the position and the size write | the window is fullscreen, tiled or otherwise not movable by the app |
| `trajectory_not_found` | no trajectory file matches the id (or none exist) | `trajectory {action:"status"}` lists recent files |
| `replay_too_large` | the trajectory exceeds the 200-turn replay cap | split it, or replay a pruned copy |
| `app_upgrade_required` | the helper predates the feature or is not running | restart/update the Codewhale Computer Use app |
| `unsupported_on_backend` | tool not implemented on that platform backend | check the platform note in the main skill |
| `permission` / `permissions_denied` | a grant is missing | name the permission and the Settings pane, then stop |
| `control_stopped` | the kill switch ended this session | report to the user; the session cannot resume |
| `cancelled` | the host cancelled the request | the input may or may not have landed — observe before retrying |
| `timeout` | the request exceeded its deadline | observe; only retry after confirming the first attempt did not land |

## Reading a receipt

- `action_sent` / `verified` mean dispatch (and, where available, read-back) —
  not task success. Verify the effect with a fresh observation.
- `front_lease` / `front_restored` describe focus accounting for window-record
  deliveries. `front_restored:false` is a person-visible event: say it out loud.
- `input_may_have_been_sent` on an error means the press left before the
  failure: observe the target before doing anything else.
