---
description: Drive a computer's screen, mouse, and keyboard
usage: /computer [status|setup|look|computers]
---

$ARGUMENTS

- With `setup`: explain the three targets: Apps on this Mac (this plugin), My
  Chrome (Chromewhale), and Isolated browser (`browser`). Check the chosen
  route's existing status first. For local apps, use `request_access` and
  follow its `via`/`appHint`; a bundled helper does not need a second install.
  OS permission grants and plugin review/enablement remain the person's choice.
- With no arguments or `status`: report whether computer use is usable on
  the active computer — server reachable, platform, screen size, whether the
  Codewhale Computer Use app is doing the work (`via`), and which
  permissions or platform tools are missing (`request_access`) — without
  taking any action.
- With `look`: take one screenshot of the active computer and describe what
  is on screen.
- With `computers`: list registered computers (`computer {action:"list"}`) and say which
  one is active; every tool also takes `computer` to switch.
- Anything else (clicking, typing, operating apps, recording) goes through
  the computer-use skill's observe-act-verify loop with per-action approval.
  A denied permission or a missing tool fails closed and is reported, never
  retried blindly.
