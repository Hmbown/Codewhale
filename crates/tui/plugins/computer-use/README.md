# Computer Use

This is the Computer Use plugin included in Codewhale. The Engine embeds the
runtime bundle, discovers it through the existing plugin registry, and runs
only the copy the user has reviewed and enabled. Codewhale Apps uses that same
Engine inventory and approval flow.

The bundle provides 39 MCP tools for application and window observation,
accessibility actions, screenshots and zoom, keyboard and pointer input,
clipboard access, recording, and switching between registered computers.
Implementation exists for macOS, Windows, Linux and HarmonyOS target devices;
platform support still depends on the tools, OS grants and actual device
verification described by the upstream project. A source build is not a
published or certified release.
Current launch qualification covers the local macOS candidate. Windows,
Wayland, HarmonyOS and SSH require separate device and workflow evidence.

## Included runtime

macOS Codewhale builds carry the compiled native helper. Using the included
plugin needs neither a separate Computer Use app nor a compiler. It uses the
permission identity of its hosting Codewhale app or terminal. Accessibility
and Screen Recording grants remain controlled by the user in System Settings.
Use `request_access` to inspect readiness; a loaded plugin alone does not prove
its OS permissions work.

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
The current one-shot SSH agent also loses application binding between calls;
stateful remote input needs a persistent session transport before it is ready.

## Control and session ownership

Select an application before sending input. On macOS, background selection
(`activate:false`) supports process-directed typing and accessibility actions.
It refuses gestures that would move the shared desktop pointer. Explicit
foreground selection (`activate:true`) enables guarded shared-desktop input
when the user has authorized exclusive desktop use. Neither mode is an isolated
computer; cursor restoration does not make concurrent pointer control safe.
Screenshots and zoom return actual image content to compatible vision models.
Preview and recording are explicit opt-ins.
Application observations return a concise default summary; request full detail
when needed. Text-only models can use element roles, values and advertised
actions. On macOS, optional local OCR enriches the selected window observation
with text and raster bounds; it requires Screen Recording permission and does
not invent accessibility elements or actions.
The Engine permits one inline image up to 5 MiB per tool result; use a scoped
capture or zoom when a larger image receives an omission receipt.

Each task owns its MCP connection and computer selection, observations and
held input. Subagents within that task share the task's Computer Use session.
Stopping control or closing the task releases that session's input. Stale
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
