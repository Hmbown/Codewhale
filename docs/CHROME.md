# Chromewhale: Codewhale in Chrome

Chromewhale (`extensions/chrome`) is a Manifest V3 side-panel client for the
local Codewhale runtime. It does two things the VS Code client does not: it
gives the model five tools that act on the browser tab the user is looking at,
and it owns the permission gate for them.

Install and day-to-day use live in
[`extensions/chrome/README.md`](../extensions/chrome/README.md). This document
is the design: how the browser tools reach the engine, where the trust boundary
sits, and what the design does not do.

## It needed no engine change

The runtime already had a reverse channel for tools a client executes — built
for benchmark harnesses, documented under **Client-executed dynamic tools** in
[`RUNTIME_API.md`](RUNTIME_API.md). Chromewhale is another consumer of it:

```
side panel                          codewhale app-server --http
    │                                         │
    │  POST /v1/threads/{id}/turns            │
    │  { prompt, dynamic_tools: [browser_*] } │   registers the 5 specs
    ├────────────────────────────────────────>│   for this turn's catalog
    │                                         │
    │  GET  /v1/threads/{id}/events (SSE)     │   model calls browser_snapshot
    │<────────── tool_call.requested ─────────┤
    │                                         │
    │  (gate → chrome.scripting → the tab)    │
    │                                         │
    │  POST …/tool-calls/{call_id}/result     │
    ├────────────────────────────────────────>│   result reaches the model
    │<────────── tool_call.resolved ──────────┤
```

Three properties of that channel shape the client:

- **Registration is per turn.** `POST /v1/threads` accepts `dynamic_tools` and
  discards it; only `POST /v1/threads/{id}/turns` reaches the catalog. The panel
  therefore sends the same frozen array on every turn.
- **Its KV-cache effect is the pinned prefix.** These specs join the
  session-pinned system prompt + tool catalog ([`CACHE.md`](CACHE.md)). The array
  is a module constant in `src/tools.js` precisely so the serialized catalog
  stays byte-stable turn to turn. A catalog built from mutable state — granted
  origins, the active tab, a toggle — would invalidate the prefix on every
  message. Do not make this list dynamic.
- **Results reach the session log by the ordinary tool path.** Page text is a
  new model-visible input, so it has to be reconstructable
  (`AGENTS.md`). `execute_dynamic_tool` converts the submitted result into a
  plain `ToolResult` that flows through the engine's normal tool pipeline and is
  recorded with every other tool result; the `tool_call.*` events carry
  identifiers and status only, and are not where the content lives.
- **The runtime settles a call exactly once**, with a 300-second timeout
  (`DYNAMIC_TOOL_RESULT_TIMEOUT`). The panel answers every call it owns, with a
  refusal if that is the answer, rather than letting the model wait out the
  timeout with nothing to report.

## The tools

| Tool | Acts on | Returns |
| --- | --- | --- |
| `browser_snapshot` | active tab | URL, title, and a flat outline of visible text with interactive elements marked `[eN]` |
| `browser_navigate` | active tab | the page it landed on, after waiting for load |
| `browser_click` | one `[eN]` ref | what was clicked, and the URL afterwards |
| `browser_type` | one `[eN]` ref | where the text landed |
| `browser_screenshot` | active tab | the visible area as an image part |

Refs come from the most recent snapshot and live in the injected ISOLATED
world, which Chrome discards on navigation. A ref used after a page load reports
staleness instead of resolving to whatever element now sits at that index.

## The trust boundary

Runtime dynamic tools register with `ApprovalRequirement::Auto`
(`crates/tui/src/tools/dynamic.rs`), so **Codewhale's own approval gate never
sees a browser tool call**. Chromewhale's gate is the only one. It runs in this
order, and a call reaches the page only if all five agree:

1. **Pause.** A single panel toggle refuses every browser tool.
2. **Tab.** There must be an active tab in the panel's own window.
3. **Scheme.** `chrome://`, extension pages, `file:`, `data:`, `view-source:`,
   `javascript:` and the Web Store are refused outright, with a reason, and
   never prompted about.
4. **The user's decision**, stored per origin. `block` refuses without
   re-asking; an unknown origin prompts in the panel with a 120-second window
   that fails closed.
5. **Chrome's own optional host permission** for that origin. The manifest
   requests no web-site access up front; `chrome.permissions.request` runs
   inside the Allow click so the user gesture is still live. An origin the user
   allowed but Chrome no longer holds access for is re-prompted, not assumed.

Two guards sit below the gate:

- **Credential fields are never typed into.** `browser_type` inspects the
  element first and refuses password, one-time-code, and payment-card fields —
  by input type, by `autocomplete` token, and by name. `page.js` repeats the
  password check as defence in depth.
- **Page text is wrapped as untrusted data** before it reaches the model, with
  an instruction not to obey anything inside it. Any site the user visits is
  attacker-controlled; the envelope reduces prompt injection and does not
  eliminate it.

The guards that decide are pure functions in `src/policy.js`, exercised by
`node --test` rather than only in a browser. `src/browser.js` takes every
`chrome.*` API through injected dependencies for the same reason: the refusals
are the part worth testing.

## What this design does not do

- **It does not run without the panel open.** The stream and the tool loop live
  in the panel document, not the service worker, which MV3 evicts after ~30
  seconds idle and terminates on a schedule even mid-fetch. Closing the panel
  ends Chromewhale's ability to touch a page. That is the intended lifetime.
- **It does not see cross-origin iframes.** A snapshot walks the top frame only,
  so text and controls inside an embedded frame are invisible and no ref can
  point into one.
- **It does not enumerate or switch tabs.** There is no tab tool. A model that
  could list tabs could read every page the user has open.
- **It does not dispatch trusted events.** `element.click()` from an isolated
  world is `isTrusted: false`; sites that gate on that will ignore it.
- **It grants per origin, not per page or per action.** That is Chrome's own
  granularity, and a finer-looking grant would be a promise the platform cannot
  keep.
- **It ships no packaged build.** Load unpacked; there is no Web Store listing,
  no bundler, and no dependencies.

## Relationship to the other clients

`codewhale web` serves a loopback browser client for the runtime;
`extensions/vscode` is the editor client. Chromewhale is neither a replacement
nor a port of them — it is the client that happens to sit where the pages are,
and the browser tools are the only reason it exists. Its SSE framing is a
deliberate port of `extensions/vscode/src/sse.ts` rather than a shared module:
the two read different transports (`node:http` versus a `fetch` stream), and
sharing would mean adding a build step to an extension that needs none. If the
runtime's event envelope changes, both copies change.
