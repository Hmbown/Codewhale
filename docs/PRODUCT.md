# Product

<!-- impeccable:product-schema 1 -->

## Platform

web (the public site and docs in `web/`), documenting a terminal application
(the Rust TUI in `crates/tui`). Paths below are relative to the repository
root.

## Users

Developers who run a coding agent in their own terminal against their own
repositories: solo maintainers, small teams, and open-source contributors. They
arrive at the site to decide whether to install, to install, and then to look up
how a command, mode, or concept works. Many already use a competing agent and
compare on model choice, cost, and control.

## Product Purpose

Codewhale is an open-source (MIT) coding agent and terminal UI written in Rust
(Ratatui + Tokio; sandboxed tools via Bubblewrap/Seatbelt). Given a model and a
task it reads the repository, edits files, runs the checks, and stops when the
job is done or it needs a human. The site exists to (1) get a developer from
"what is this" to a working install in one screen, and (2) be the canonical,
current documentation for the shipped release. Success is an install that works
and a docs answer found without leaving the page.

## Positioning

Bring your own model. Codewhale is provider-neutral: any hosted, gateway, or
local model, and a different model per role. The user's model inventory is the
**Fleet** (`codewhale fleet`, `/fleet`; `pod` remains a compatibility alias).
Modes are Plan, Work, Operate; permission levels are Ask, Auto-Review, Full
Access. The agent runs on the user's machine, in the user's terminal — there is
no hosted runtime to sell.

## Operating Context

- Install: official GitHub Releases first. New macOS/Linux installs use
  `curl -fsSL https://codewhale.net/install.sh | sh`; Windows uses the matching
  GitHub installer/archive. Existing direct installs use `codewhale update`.
  npm and Cargo are secondary packaging routes; migration and PATH handling
  follow `docs/INSTALL.md`. `latest` selects a published release, not the source
  candidate. Facts (version, provider count, tool count, license) are
  derived from the repository by `npm run prebuild` into
  `web/lib/facts.generated.ts` and must never be hand-edited.
- Docs pages mirror `docs/*.md` in the repository; `npm run check:docs`
  verifies the mapping. Public vocabulary lives in
  `web/lib/content/vocabulary.ts` and `docs/public-surface-facts.json`.
- Localised through shared dictionaries in `web/lib/i18n/dictionaries/` with
  locale-key parity enforced; no page-local copy forks.
- The 0.9.12 shell (on the integration branch): transcript first, composer
  plate, one info line, and a bottom dock with tabs Tasks / Agents / Context /
  Pinned (+ ×). There is no top bar; docs that describe the shell describe the
  dock.

## Capabilities and Constraints

- Public name is **Codewhale** (lowercase w). `CodeWhale` survives only in
  compatibility identifiers (GitHub org/repo, package scopes).
- Provider and model names are first-class and neutral; never rank providers
  in copy.
- The 0.9.12 shell is not yet released. `web/lib/media-manifest.ts` marks
  session video `pending`; the site must not ship mockups as screenshots. The
  one real screenshot on hand is `web/public/codewhale-tui.png` — the
  founder's 2026-09-04 capture of the v0.9.12 development build (new session,
  braille C-curl whale, Work mode, Full Access). It is captioned as a
  development build, never as a release.
- `/context-window` does not exist on the current base; do not document it.
- Subagent role identifiers are those the code accepts (`general`, `explore`,
  `planner`, `reviewer`, `implement`, `test`, `advisor`, `custom`); the older
  spellings `worker`, `scout`, `builder`, `verifier`, `consultant`, and `oracle`
  are accepted as compatibility aliases only. Do not invent public role names.

## Brand Commitments

- Voice: quiet, dense, factual. Terminal vocabulary, no marketing superlatives,
  no fabricated transcripts or reasoning traces.
- "It doesn't need to look special — it needs to look like Codewhale."
- Assets: the canonical vector family lives in the CWC repo at
  `codewhale-apps/packages/brand/svg/` (mark, mark-gradient, mark-mono,
  mark-reversed, wordmark, wordmark-inverted); `brand/` and
  `web/public/brand/` carry byte-identical copies — sync from there, never
  re-trace. The founder's brand sheet `brand/codewhalemarkfinal.png` remains
  the source `scripts/brand/braille-mark.py` derives TUI launch art from. The
  earlier local trace and `scripts/brand/trace-brand.py` were retired
  2026-09-15 in favor of the canonical family. Web copies live in
  `web/public/brand/`.
- Palette, type, shell direction, and the anti-slop rules are recorded in
  `docs/design/DESIGN.md`; the colour tokens are owned by `crates/palette/src/tokens.rs`
  and exported to `web/app/tokens.css`.

## Evidence on Hand

- Real: GitHub stars (live), release version and changelog (generated),
  provider/tool counts (generated), the v0.9.12 development-build screenshot.
- Absent, do not fabricate: testimonials, customer logos, benchmarks,
  pricing, session video, or media of a published 0.9.12 release.

## Product Principles

1. One owner per fact: every number and command on the site is derived from
   the repository, never typed twice.
2. Content first: no permanent side chrome on the landing page; the docs page
   is a reading surface, not a portal.
3. Show only what exists: pending media stays marked pending; commands are
   documented only once they are on the base branch.
4. Provider-neutral, model-neutral, always.
5. Accessibility is not negotiable: AA contrast, ≥12px functional text, real
   heading outline, keyboard-reachable everything.

## Accessibility & Inclusion

WCAG 2.2 AA for text and controls. The audience includes screen-reader and
keyboard-only developers; the site is also read at 390px on phones.
