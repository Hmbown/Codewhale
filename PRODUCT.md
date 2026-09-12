# Product

<!-- impeccable:product-schema 1 -->

## Platform

The public site and docs in `web/`, and the local browser client served by
`codewhale web` in `crates/tui/src/runtime_web/`. Both describe or connect to
Codewhale's canonical Engine and Runtime; neither owns a second agent loop.
Paths below are relative to the repository root.

## Users

Developers who work with an agent in their terminal or local browser against
their own repositories: solo maintainers, small teams, and open-source contributors. They
arrive at the site to decide whether to install, to install, and then to look up
how a command, mode, or concept works. Many already use a competing agent and
compare on model choice, cost, and control.

## Product Purpose

Codewhale is an open-source, provider-neutral agentic computing system. One
Engine owns task execution and session state across its interfaces. Given a
model and a task, it reads the repository, edits files, runs checks, and asks
for human input when needed. The site exists to (1) get a developer from
"what is this" to a working install in one screen, and (2) be the canonical,
current documentation for the shipped release. Success is an install that works
and a docs answer found without leaving the page.

## Positioning

Bring your own model. Codewhale is provider-neutral: any hosted, gateway, or
local model, and a different model per role. The user's model inventory is the
**Fleet** (`codewhale fleet`, `/fleet`; `pod` remains a compatibility alias).
Modes are Plan, Work, Operate; permission levels are Ask, Auto-Review, Full
Access. The terminal and `codewhale web` work with the local Runtime. The
signed-in web app, desktop, cloud computers, and connected apps are part of the
broader product; their availability must come from the current public surface
facts rather than be inferred from the local client's capabilities.

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
- The embedded browser client ships inside the binary. Its thread list, saved
  session previews, transcript, approvals, and composer use the Runtime API.
  `docs/WEB.md` owns its launch, storage, and authentication contract.

## Capabilities and Constraints

- Public name is **Codewhale** (lowercase w). `CodeWhale` survives only in
  compatibility identifiers (GitHub org/repo, package scopes).
- Provider and model names are first-class and neutral; never rank providers
  in copy.
- `web/lib/media-manifest.ts` owns real capture paths, dimensions, build
  identity, and video availability. A capture's build is independent of the
  latest source or published release. Preserve its caption and provenance;
  never substitute a mockup for product evidence.
- `/context-window` does not exist on the current base; do not document it.
- Subagent role identifiers are those the code accepts (`general`, `explore`,
  `planner`, `reviewer`, `implement`, `test`, `advisor`, `custom`); the older
  spellings `worker`, `scout`, `builder`, `verifier`, `consultant`, and `oracle`
  are accepted as compatibility aliases only. Do not invent public role names.

## Brand Commitments

- Voice: quiet, dense, factual. Terminal vocabulary, no marketing superlatives,
  no fabricated transcripts or reasoning traces.
- "It doesn't need to look special — it needs to look like Codewhale."
- Assets: retain the approved transparent whale in
  `web/public/brand/mark-gradient.svg` and the existing wordmark SVGs in that
  directory. The embedded client uses the same whale artwork. Do not redraw
  or regenerate brand art as part of a layout change.
- Palette, type, shell direction, and the anti-slop rules are recorded in
  `DESIGN.md`; the colour tokens are owned by `crates/tui/src/palette/tokens.rs`
  and exported to `web/app/tokens.css`.

## Evidence on Hand

- Real: GitHub stars (live), release version and changelog (generated),
  provider/tool counts (generated), and captures declared in the media manifest.
- Use the canonical public content for pricing and surface availability.
  Testimonials, customer logos, benchmarks, and session video require their own
  evidence; a local UI fixture is not a real provider session or release proof.

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
