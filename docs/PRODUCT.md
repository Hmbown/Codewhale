# Product

<!-- impeccable:product-schema 1 -->

## Platform

web (the public site and docs in `web/`), documenting Codewhale's Engine,
terminal and connected tools. The terminal is one product surface, not the
definition of the product. Paths below are relative to the repository root.

## Users

People building apps, automating recurring work, researching and working across
files and connected services. This includes solo developers, small teams and
open-source contributors. They arrive to understand what they can accomplish,
install, connect their models and tools, and look up how a command, mode or
concept works. Model choice, cost and control matter to that decision.

## Product Purpose

Codewhale is an open-source (MIT), provider-neutral agentic computing system.
Given a task, it works through files, commands and connected tools to build
something useful, while keeping access, approvals and work history legible.
The Rust Engine supplies the shared Runtime; Ratatui is its terminal surface.
The site should make the result understandable, get a visitor to a working
install, and provide current documentation. Success is a task someone can
start, an install that works and a useful docs answer.

## Positioning

Bring your own model. Codewhale is provider-neutral: use your model API
accounts, compatible gateways, or local and self-hosted inference, with a
different model per role. The user's model inventory is the
**Fleet** (`codewhale fleet`, `/fleet`; `pod` remains a compatibility alias).
Modes are Plan, Work, Operate; permission levels are Ask, Auto-Review, Full
Access. Plugins and MCP servers add tools; scripts can use service APIs.
Each external service requires its own setup and authentication. General MCP
support does not establish that a particular service has been connected or
qualified with Codewhale. Terminal, local browser, native
app and hosted product availability must be stated separately from the common
Runtime model: source readiness is not deployment or customer acceptance.

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
- The terminal puts the conversation above the composer, posture and metrics.
  Its workbar exposes Tasks, Fleet, Jobs, Files, Notes, Context, Git and Cost;
  the development candidate also adds a Terminal panel for its real PTY sessions
  on macOS and Linux. Product previews come from the built native TUI's actual PTY cells;
  the Ratatui explorer separately demonstrates reusable components with example data.

## Capabilities and Constraints

- Public name is **Codewhale** (lowercase w). `CodeWhale` survives only in
  compatibility identifiers (GitHub org/repo, package scopes).
- Provider and model names are first-class and neutral; never rank providers
  in copy.
- The homepage explains what Codewhale does, the models and tools it connects
  to, and how to install it. Keep it straightforward. Invented task briefs,
  sample reports and terminal view galleries do not belong on the homepage.
  Show one actual native TUI capture below the introduction. The product page
  and component explorer can show the other views.
- Use plain, concrete marketing copy about what people can build and do.
  Headings name the benefit or action. Avoid robot jokes, choppy slogan
  fragments, vague promises and internal agent-verification language in the
  public interface. Keep service claims tied to actual setup and qualification.
- `web/lib/media-manifest.ts` records the exact captured native build and
  shared README image. Native views on the product page and getting-started
  guide retain their original PTY text and colors. An isolated local demo may
  prove real file edits, checks and delegated review, but does not prove a paid
  provider call. Session video remains `pending`. Build captions distinguish
  development builds from releases.
- `/context-window` does not exist on the current base; do not document it.
- Subagent role identifiers are those the code accepts (`general`, `explore`,
  `planner`, `reviewer`, `implement`, `test`, `advisor`, `custom`); the older
  spellings `worker`, `scout`, `builder`, `verifier`, `consultant`, and `oracle`
  are accepted as compatibility aliases only. Do not invent public role names.

## Brand Commitments

- Voice: direct, useful and factual. Lead with what someone can build or
  accomplish. Use product controls where they help the task; avoid marketing
  superlatives, fabricated transcripts and reasoning traces.
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
  `docs/design/DESIGN.md`. Shared interface tokens come from
  `vendor/codewhale-design/tokens.json` and are exported to `web/app/tokens.css`.
  Website ocean and brand roles live in `web/app/styles/tokens-roles.css`;
  `crates/palette/src/rgb.rs` owns the terminal presets.

## Evidence on Hand

- Real: GitHub stars (live), release version and changelog (generated),
  provider/tool counts (generated), and native capture provenance recorded in
  the media manifest and public surface facts.
- Absent, do not fabricate: testimonials, customer logos, benchmarks,
  pricing or session video. Published downloads and development-source
  captures are different evidence.

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
keyboard-only users; the site is also read at 390px on phones.
