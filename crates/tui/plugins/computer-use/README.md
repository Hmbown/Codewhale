# Computer Use

This is the Computer Use plugin included in Codewhale. The Engine embeds the
runtime bundle, discovers it through the existing plugin registry, and runs
only the copy the user has reviewed and enabled. Codewhale Apps uses that same
Engine inventory and approval flow.

The bundle provides the current consolidated MCP tools for application and window observation,
accessibility actions, screenshots and zoom, keyboard and pointer input,
clipboard access, recording, and switching between registered computers.
Implementation exists for macOS, Windows, Linux and HarmonyOS target devices;
platform support still depends on the tools, OS grants and actual device
verification described by the upstream project. A source build is not a
published or certified release.
The bundled plugin is enabled on macOS only while Windows and Linux ports
are being qualified. Their raw input uses the shared desktop; they do not
yet provide equivalent background control or qualified native installers.
HarmonyOS and SSH also require separate device and workflow evidence.

## Included runtime

macOS Codewhale builds carry the compiled native helper. Using the included
plugin needs neither a separate Computer Use app nor a compiler. It uses the
permission identity of its hosting Codewhale app or terminal. Accessibility
and Screen Recording grants remain controlled by the user in System Settings.
Use `request_access` to inspect readiness; a loaded plugin alone does not prove
its OS permissions work.

When the standalone Computer Use helper is registered, it owns local input
even when Codewhale carries an embedded native helper. Version 0.11.3 keeps its
whale menu, permission setup, disposable background check and human
Pause/Stop controls, and retires the daemon when its native owner disappears. A registered helper that cannot start causes a clear
error; the client does not silently bypass its controls. Without a registered
standalone app, the included helper remains available under the host's
permission identity.

The MCP server requires Node.js 20 or newer. Codewhale Apps packages its own
Node runtime; the CLI uses Node on PATH. Homebrew declares the dependency;
Cargo and direct binary users can install Node from <https://nodejs.org/>.
Linux also needs the appropriate X11 or Wayland utilities and AT-SPI bindings.
Windows uses PowerShell and UI Automation. Linux and Windows recording is
currently unavailable until recorder ownership and shutdown cleanup are built.
HarmonyOS targets require a connected device and hdc.

Persistent holds and drags on Linux and Windows currently require the separate
session-aware Computer Use helper. Their direct bundled path refuses these
operations before sending input. macOS carries its native input owner in the
included bundle. Real Windows, Wayland and mixed-display validation is still
required before claiming equivalent platform readiness.
SSH and Docker use persistent session transports; real remote workflows still
need target-specific acceptance. The embedded Docker build context supports
first-use creation of a task-owned Linux desktop when a compatible daemon is available.

## Control and session ownership

Select an application before sending input. On macOS, background selection
(`activate:false`) supports process-directed typing and accessibility actions.
It refuses gestures and keyboard shortcuts that would borrow the user's keyboard focus or move the shared pointer. Some Unicode and hosted-panel
typing also refuses rather than taking a focus lease. Explicit
foreground selection (`activate:true`) enables guarded foreground input
when the user has authorized exclusive desktop use. In both modes pointer input
goes to the bound app's window as window-routed events; the user's cursor is
never moved. Neither mode is an isolated computer.
Screenshots and zoom return actual image content to compatible vision models.
The nonactivating preview is on by default after binding; recording is explicit.
Application observations return a concise default summary; request full detail
when needed. Text-only models can use element roles, values and advertised
actions. On macOS, optional local OCR enriches the selected window observation
with text and raster bounds; it requires Screen Recording permission and does
not invent accessibility elements or actions.
The Engine permits one inline image up to 5 MiB per tool result; use a scoped
capture or zoom when a larger image receives an omission receipt.

Each task owns its MCP connection and computer selection, observations and
held input. Sub-agents never receive Computer Use tools: only the task's own
agent operates the computer. Stopping control or closing the task releases that session's input. Stale
observations, unexpected foreground changes and unavailable capabilities fail
closed with a receipt; successful dispatch still needs application-state
verification.

## Development

The exact upstream source revision is recorded beside this directory in
`computer-use.upstream-sha`. This tree contains the runtime and its tests;
standalone app installers and release tooling belong to the upstream project.

Run `npm test` here for unit and protocol coverage. Those tests do not type or
click in the user's applications. `npm run smoke` is a separate legacy live
check: it captures and records the selected display, so run it only when that
capture is intended. The upstream parity suite contains scoped application
fixtures for interactive verification.

On macOS, ordinary observations follow the selected background app. Field
focus, selection, context menus and scrolling use supported accessibility
operations; raw mouse gestures stop if the user changes foreground apps.
Arbitrary background dragging remains unavailable. Rebuild Core to include
the updated native helper; updating a separate marketplace checkout alone
does not update an already-installed Core binary.

This embedded source matches canonical 9a261c4. Windows controlled-desktop
acceptance passed in upstream CI; signed Windows distribution, mixed-DPI/raw
input and continuous keyboard coexistence remain unqualified. Core discovery
and materialization tests do not constitute an installed model-driven trial.
