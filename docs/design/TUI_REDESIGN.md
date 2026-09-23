# TUI redesign — ocean depth / Workbench index

## Overview

Fresh 0.10.0 terminal installs use **Underwater**, the restrained navy ombré.
Shoreline remains the warm charcoal alternative; saved theme choices are
preserved. `/theme` previews either treatment, Enter saves, and Escape restores
the previous choice. The same layout, whale mark and information hierarchy
serve both themes; the ocean is a continuous background, not extra chrome.

Shoreline is the terminal's warm charcoal, ivory and blue visual system,
introduced on 2026-09-15 to bring the TUI closer to the GPUI product client.
The 2026-09-19 Workbench index pass organizes that system around the next
useful action: start work, resume a session, or inspect connected tools.
The founder delegated the visual direction and explicitly allowed replacing
the previous appearance when it improved the experience.

The home screen uses a compact codewhale identity, version metadata aligned
opposite it, and one bounded reading lane centered in wide terminals. Actual
workspace and branch context make the screen specific to the current work.
A prominent New session action and a Recent heading for real history organize
the available choices. A compact canonical braille whale accompanies the identity when
space permits, yielding before session titles and actionable rows in short
terminals. The canonical brand asset is unchanged. The full-width
composer, shared session runtime, permission authority and website design
remain their existing systems. The terminal default does not change app defaults.

This document describes implemented terminal behavior, not the website's
Tidal Folio system. Source owns values and actions; this file records how
they form a coherent interface.

## Colors

Underwater's continuous water column is authored by `OceanRamp::for_theme` in
`crates/tui/src/tui/ocean.rs`: dark navy at the top, deeper near the composer.
Its existing motion policy preserves reduced/still modes and semantic surfaces.

For the charcoal alternative, the source of truth is `crates/palette/src/tokens.rs` and the
`SHORELINE_UI_THEME` / `SHORELINE_LIGHT_UI_THEME` mappings in
`crates/palette/src/themes.rs`. The dark palette is unchanged by the
Workbench index pass:

| Role | Value | Use |
| --- | --- | --- |
| Field (`surface_bg`) | `#211F23` | Warm charcoal reading surface |
| Plate (`panel_bg`, `composer_bg`) | `#2B282E` | Composer and raised surfaces |
| Elevated | `#35313A` | Hover and secondary control surfaces |
| Chrome (`header_bg`, `footer_bg`) | `#1A181C` | Recessed shell information |
| Border | `#49424D` | Quiet boundaries and inactive composer |
| Body | `#F2ECE5` | Primary ivory text |
| Soft / muted / hint / dim | `#D9D2DC` / `#B0A7B2` / `#9A919F` / `#7E7583` | Secondary information by role |
| Action / selection background | `#67B8D6` / `#2C4654` | Affordances and focused rows |
| Live | `#7FD6C6` | Live activity |
| Human | `#F6C453` | Human input and decisions |
| Warning / danger / success | `#F0A868` / `#FF8FA8` / `#A3D977` | Semantic state |
| Mode ramp | `#7EB4E8` / `#B9DCEC` / `#AD88FF` / `#FF70A0` | Existing mode distinctions |

Shoreline Light uses warm paper (`#F5F0E9` field) and blue action
(`#006684`); its full mapping remains in the same source. Neither this pass
nor the Shoreline token family re-inks `web/app/tokens.css`.

**Focus uses selection ink on selection blue.** Bright action blue is an
accent, not a background for pale text. Selected labels, affordances and
state annotations must remain readable on the selected surface. Failure,
permission and availability also retain words or marks; color never supplies
their only meaning.

Theme remapping keeps direct palette calls consistent with the active preset.
The mode ramp remains distinct from the action lane so semantic remapping
continues to work in reduced-color terminals. Contrast audits are
role-specific: the existing audit requires 4.5:1 for its primary text pairs
and 3:1 for hint/dim/status/diff pairs. A passing theme audit is not proof
that an arbitrary component foreground/background combination is safe.

## Typography

The terminal host owns the font, size and rasterization. There is no separate
application display face. Hierarchy comes from bold identity and selection,
regular body text, secondary metadata, and spacing between groups.

Home shows session titles and age; message counts stay in session details.
An empty workspace omits the Recent section. Titles take priority over age. Width calculations
and truncation use terminal display cells, including wide characters; omit
metadata before reducing a useful title to a stub. Longer labels receive an
explicit truncation marker rather than silently running under a border.

## Layout

The home screen is the transcript's launch empty state, implemented by
`underwater::launch_empty_state`. Identity and actions share a lane capped at
72 text cells, centered when the terminal is wider, with a two-cell action
gutter where width permits. Version metadata aligns to the opposite edge of
the identity row. The workspace caption uses the real workspace and adds its
actual branch only when it fits. Command help follows recent work and MCP;
optional top breathing room consumes spare space only. There is no permanent
mascot column or additional navigation sidebar. The composer remains full width.

The compact ladder removes spacing first, then migration notice and MCP
detail, then help, workspace context and identity headings, before shortening
the recent list.
Hidden sessions remain reachable through the overflow action while space
allows it. The MCP summary survives longer than its per-server detail. At
extreme dimensions, New session is the final action retained. The painted
row list is also the keyboard and mouse ordering: shrinking the terminal
cannot leave an invisible recent session selected.

Shared full-screen settings geometry lives in `views::render_underwater_surface`.
At fewer than 24 rows it removes outer vertical margins and top padding;
bottom padding is zero. Horizontal outer margins disappear below 44 columns.
These are shared layout decisions, not separate compact implementations for
each settings page. Config's option editor only expands its header when
three choices plus detail still fit. Model/Thinking panes stack when narrow;
in short stacked layouts the inactive pane becomes one clickable summary
and the focused pane receives the remaining space.

**Keep the two footer owners until their interactions migrate together.**
The posture/activity row in `phase_strip` carries permissions, mode, work
navigation and transient state. The metrics row in `ui/frame.rs` carries
route, model, context and cost information with its own user configuration
and pointer targets. A one-row merge was an earlier proposal, not the
implemented design. Hiding metrics would remove model/context interactions
unless their measured targets and configuration migrated in the same slice.
The Workbench index deliberately retains both rows and their shedding rules.

## Elevation & Depth

The terminal uses tonal surfaces and cell borders. The composer is a plate
above the field; an inactive outline recedes without making the input vanish.
Full-screen settings use restrained top and bottom rules. Protected-focus
modals retain the existing terminal-cell shadow and border treatment.
There are no new glow, blur, texture or decorative motion effects.

## Shapes

Actions are measured rows and rectangular controls sized in terminal cells.
Their painted area is their pointer target. Home actions share a leading
marker, keyboard selection uses a continuous filled band and bold text,
and hover uses the elevated surface with an underline. New session has a
quiet plate fill before focus; Recent's trailing rule separates the list
without enclosing each session in a box. Pointer hover does
not silently move keyboard selection. Borders and glyphs use the terminal's
existing vocabulary, including its reduced-capability fallbacks.

## Components

### Home and composer

Typing begins through the existing composer. Up/Down and Enter navigate and
activate visible home rows; clicking a recent row enters the same resume
flow. The MCP summary opens the existing manager by mouse or keyboard. A
problem row inserts its stated remedy into the composer so the user can see
it before submission. No separate command or session authority is introduced.

The composer outline uses action blue only while the composer owns focus.
Selecting a home action or opening another surface returns it to the quiet
border tone. Permission and mode retain their own footer status instead of
being repeated in a multicolor composer outline. Model metadata remains
secondary to the message and its controls.

### Resume confirmation

Resuming names the target session and explicitly states that its history
replaces the current context. Warning and button space are reserved before
the title and metadata; long titles truncate to one line and cannot push the
consequence under a button. Both Resume and Cancel are real mouse targets.
Tab, BackTab or Left/Right switches selection; Enter activates the selected
button and its visible hint moves with selection. Escape always cancels.
An outside click dismisses and never confirms.

Successful restoration announces the sanitized session title through a
localized success toast, using the existing status-toast owner. The string
is supplied in all 15 locale packs. It no longer appends a filesystem path,
session ID and message-count receipt to the restored transcript, so compact
terminals return their space to the conversation.

### Settings and pickers

- **Models:** one provider context row retains catalog freshness without a
  duplicate route banner. Only the focused pane receives the filled blue
  selection; the inactive pane retains its current-choice marker. Hover
  covers the measured row. Compact hints prioritize browsing, searching,
  switching, applying and canceling; secondary bindings remain available.
- **Providers:** ordinary management uses the full-screen shell regardless
  of configured-provider count. Initial setup, credentials and consent keep
  their modal flows. The borderless inspector shares the list's canvas and
  keeps provider identity, credential source, route, endpoint, concise warning
  lines and consent facts ahead of model choices and prices. The underlined,
  clickable Open details action and shared Alt+V shortcut (⌥V on macOS)
  open the existing scrollable pager. That projection retains every warning
  and the full protocol/capability diagnostics rather than crowding them into
  the overview. Escape returns to the provider manager.
- **Provider choice stages:** Kimi plan tier, Stepfun billing route, xAI auth
  and ChatGPT auth choices use wrapping, measured selectable rows. A first
  click selects through the existing key action; a second click activates
  through the existing Enter action. This does not change billing, consent
  or credential policy. Key entry, custom-provider text fields and final
  credential/consent confirmations retain their existing keyboard behavior.
- **Extensions:** tabs and inventory rows share selection and independent
  hover styles. Plugins initially selects the first actual item when one
  exists; group headings remain reachable for folding. Inventory rows carry
  identity and state, while the selected description/details have a separate
  wrapping area of three rows when space permits, one otherwise. Resize
  clears stale hitboxes before an invisible panel can retain actions. Trust,
  enablement and removal continue through existing guarded flows.
- **Fleet:** the compact roster header has a genuine Workers destination,
  reachable by mouse and the existing keyboard action. The decorative Setup
  pseudo-tab is gone. Setup/edit belongs to the selected real member; the
  display-only Coordinator does not advertise an Enter action it cannot run.
  Navigation rows preserve identity, role, shadow and edit markers, adding
  route text only for explicit overrides. Repeated inherited-route sentences
  and species mosaics no longer tax every row; the selected inspector retains
  full member identity, route and detail with quiet inline property labels.
- **Config:** category tabs and Apply use the shared selection treatment.
  Selected row annotations inherit readable selection ink instead of keeping
  dim or action-colored text over the selection band.

### Camera readability and evidence

The founder's acceptance criterion includes pictures and video: recognizable
identity, clear hierarchy when reduced, stable composition during interaction,
and consistent state colors. The compact wordmark, repeated action gutter
and shared selection treatment serve that criterion. Extra ornament is not
evidence of camera readability.

The validation set is 40×12, 60×16, 80×24, 100×32 and 140×40: populated home,
MCP failure, selection, resumed conversation, long-title confirmation and
Cancel selected, plus representative settings surfaces and provider details.
Use fresh evidence for the exact source/binary being delivered; earlier
captures do not qualify a later presentation slice. The capture method
reconstructs actual PTY cells and their RGB/SGR state with Menlo and Apple
Color Emoji fallback. These images are not screenshots of the host terminal
and do not prove host font behavior. Static frames do not prove motion,
transition timing or video quality.

The saved conversation and failing MCP server in this evidence are synthetic
local fixtures, not customer sessions or working-provider claims. Passing
interaction tests demonstrates those tested paths; it does not establish
provider success, installation, hosted CI, publication or whole-release
readiness. Symbol-only goldens cannot validate color. Use color-preserving
PTY evidence and the contrast audit alongside layout/interaction checks;
record exact build and installation receipts separately.

## Do's and Don'ts

- **Do** preserve useful content before spacing, branding and secondary hints.
- **Do** share geometry between painting and input, and clear targets on resize.
- **Do** keep keyboard selection, pointer hover and consequential state distinct.
- **Do** reserve consequence text before decorative or variable-length content.
- **Do** check long titles, wide characters, compact choices and selected text
  on the surface where users actually read them.
- **Don't** restore a large launch mark at the expense of session-title width.
- **Don't** hide footer owners or turn decorative labels into apparent controls.
- **Don't** invent new tokens, runtime owners or permissive mutation paths to
  implement a visual treatment.
- **Don't** describe reconstructed cells as host screenshots or static captures
  as motion proof, and don't imply every settings subview gained mouse parity.


## Working-screen performance readings (2026-09-20)

TTFT and output rate reuse the existing session accumulator. The compact footer
keeps selected performance readings when they fit, shedding secondary counts
and help first. `/statusline` offers separate Time to first token and Output
rate controls with immediate preview, Enter to save and Esc to restore. Old
`session_metrics` settings continue to enable both and become separate choices
when edited. Full, compact and hidden row settings remain in `/config`.

The motion focal point remains the shared activity marker: request progress,
verification and completion use one cadence and the existing bounded completion
settle. Numbers stay still between measured receipts, preserving legibility on
video. There is no synthetic live speed counter, new timer or extra footer row.
Reduced and still motion retain the same readings and explicit phase words.
TTFT is a session average; throughput includes first-token wait and stream
pauses, but excludes tool/idle gaps. Missing measurements stay absent.


## Identity, theme and motion refinement (2026-09-20)

Claude Fable 5.1 reviewed real terminal-cell captures and current motion source.
The founder explicitly chose a brief whale reveal: the canonical braille mark
resolves through nested dot masks over 360 ms, once from its first launch paint.
Text and controls are complete immediately. Typing, paste or resize settles the
mark; reduced/still motion shows the complete asset immediately. This reuses the
existing frame scheduler and requests no reveal frames after the endpoint.

Completion keeps the word Done stable while its existing glyph settles. Generic
working status uses a direct verb. The send control uses action ink only when the
same predicate used by its click handler permits submission; otherwise it is dim.
Locked model rows retain readable keyboard focus and their availability warning.
New session uses body ink on its filled plate to meet text contrast in light and
warm themes. Uwu now participates in the same remapping as other named presets.

Themes are being checked against five color families: surface, neutral text,
action, live/outcome, and attention/danger. Shades preserve contrast and severity;
labels and symbols retain meaning without color. Underwater keeps its ambient
identity within the same chrome discipline. Nonempty NO_COLOR selects monochrome
output: terminal-owned foreground, background, and underline colors, preserving
text modifiers and selection symbols. ANSI16 remains a distinct colored fallback
for terminals with a limited palette.
