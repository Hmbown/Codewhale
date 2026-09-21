# Echolocation

Stateless, index-free navigation soundings. File tools echo their
surroundings fresh from source — no index to build, no background work,
no new dependencies. Every sounding is deterministic (sorted, capped,
hard-budgeted, `/`-separated paths on every OS), so a repeated call is
byte-identical and cache-friendly, and soundings never enter the
session-pinned prefix: they ride tool results (append-only history)
only.

## What each surface emits

- **read** appends a footer: `[Pod (N): …]` sibling mates (bare
  names — the dir is the file just read, so it goes unstated) plus
  `[Sound: N symbols: …; imports: …; links: …]` (line-numbered
  symbols, import roots, cross-pod file links; same-pod links render
  bare and import roots a link already names (file stem or package
  path) are dropped). Budget: 1200B.
- **Terminal buzz**: after two distinct files are read in one pod, the
  next read there adds `heard by …` — podmates whose resolved links
  include this file. Sparse clicks while searching, high-resolution
  echoes at close range.
- **read miss** (`NotFound` only — never on denylist/permission refusals)
  appends `[Miss echo: <dir>]`: podmates of the missing file's parent,
  so a typo still orients the next call.
- **grep / file_search** chart each matched dir: mates with `(match)`
  marks, under the calling search's own visibility gates (denylist,
  excludes, extensions, gitignore). Pods never enumerate names the
  calling search hides.
- **project_map** sounds each key file: one line of `name:line` symbols
  per file, so the map says what lives where.
- **write / edit** append `[Edit echo: touched …; heard by …]`: the
  symbols the mutation touched (nearest declaration at or above each
  changed span, via the same `similar` diff that builds the unified
  diff, under a 100ms timeout) and the podmates linking the file.
  The model-facing receipt is one line and the diff rides metadata
  for the TUI, so the echo is how the model learns what its edit hit.

## Languages

Symbol, import, and link sounding covers Rust, Python, JavaScript /
TypeScript, and Go via deliberately small line scanners (see
`crates/tui/src/tools/echolocation.rs` for the documented lookalike
classes). Anything unresolvable is omitted, never guessed; links
require an on-disk target.

## Tuning and ablation

`CODEWHALE_ECHO` gates sections for trials
(`off:<name>`, comma-separated; names: `mates`, `symbols`, `imports`,
`links`, `callers`, `pods`, `soundings`). Unset or malformed means
everything on. The gates are process-pinned and read-only; there is no
per-session or per-call control, by design. The squeeze follows the
same gates, so an `off:links` trial measures links alone.

## Non-goals

- No tool description teaches the footer format: the brackets are
  self-describing, and every description byte on an eager tool taxes
  the session prefix. The diet stops where behavior would degrade.
- No cross-pod callers: reverse links scan same-dir mates only, so the
  cost of a sounding is bounded by one directory listing plus a
  bounded re-read of small mates.
- No `patch` (multi-file) echoes yet; single-file write/edit only.
