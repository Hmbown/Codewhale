# Codewhale messaging brief

Owner of English home copy: `lib/i18n/dictionaries/en/home.ts`. Style rules: `COPY_STYLE.md`.
Facts: `docs/features.toml`, `docs/*.md`. Research date: 2026-10-04.

## 1. How the field advertises (condensed)

| Tool | Hero (short quote) | Leads with | CTA | Any-model / open source |
| --- | --- | --- | --- | --- |
| OpenCode | "The open source AI coding agent" | LSP, multi-session, share links; 208K stars, 950 contributors | install command; "Read docs" | "connect any model from any provider" |
| Pi | "this one is yours" | minimal harness, 15+ providers, tree history | npm command; "Documentation" | MIT; switch models mid-session |
| Codex CLI | "a coding agent from OpenAI that runs locally" | terminal, IDE, app, cloud surfaces | curl installer | Apache-2.0 repo; OpenAI models |
| Claude Code | "Hand Claude a bug fix, test, or multi-day migration" | issues to PRs, long refactors; customer logos | "Download for macOS"; curl | single vendor; plan pricing |
| Cline | "The Open Coding Agent" | multi-file diffs, bash, Plan then Act; 11M+ installs | "Get Started" | "Every model, your choice"; Apache-2.0 |
| Zed | "Your last next editor" | Rust speed, parallel agents; named quotes | "Download now" / "Clone source" | "doesn't lock you into one model" |
| Cursor | "your coding agent for building ambitious software" | parallel agents; Fortune 500, CEO quotes | "Download for macOS" | model picker incl. Auto |
| Amp | "built for the frontier" | multi-model routing, cloud machines | sign up | "uses them all" |
| Warp | "Open Platform for Automating Development" | any model or harness; 800k+ devs, logos | "Request Early Access" | bring your own model |
| Aider | "AI pair programming in your terminal" | codebase map, git; 44K stars, 15B tokens/week | pip command | "almost any LLM, including local" |
| Factory | "Build Your Software Factory" | plan, build, review, ship | docs links | "multiple model providers" |
| Kilo Code | "One open agent for every developer workflow" | 500+ models, zero markup, MIT; 5M+ users | "Code for Free" | BYO keys, local, "No silent model switching" |
| Roomote (ex-Roo) | "The cloud coding agent you actually own." | verify, cross-model review | "Try now free" | source-available |
| Crush | "Your new coding bestie" | multi-model, LSP, MCP; 28.5k stars | brew / npm / winget | OpenAI- or Anthropic-compatible APIs |
| Gemini CLI | "Build, debug & deploy with AI" | large codebases, workflows | npm command | Gemini only |
| Hermes, OpenHands, Deep Agents, MiniMax, Kimi | category sentence + table of 5-7 capabilities | install first | "Use any model you want" / "Model-agnostic" | |

openai.com/codex and ampcode.com blocked fetches; Codex and Amp rows use their README and manual.

## 2. Patterns that persuade

1. **Category in the first five words.** "The open source AI coding agent" (OpenCode). The reader knows what it is before reading on.
2. **One concrete differentiator in the subhead.** "connect any model from any provider" (OpenCode). "500+ models, zero markup" (Kilo).
3. **Install command above the fold.** OpenCode, Pi, Aider, Crush show a copyable command, not only a button.
4. **Social proof as plain numbers.** "208K GitHub Stars · 950 Contributors" (OpenCode); "44K stars, 6.8M installs" (Aider).
5. **Capabilities as verbs on real objects.** "Edits across your project", "Runs bash commands" (Cline).
6. **Control stated as a mechanism.** "Plan, then Act" with approval steps (Cline); "No silent model switching" (Kilo).
7. Weaker patterns to avoid: manifesto subheads (Cursor), clever slogans (Zed, Crush), enterprise logos we cannot claim.

## 3. Positioning

For developers who want a coding agent they can inspect and point at any model, Codewhale is an
open-source (MIT) coding agent for the terminal, scripts, and CI. Unlike single-vendor agents, it
works with over 40 built-in provider routes, any OpenAI-compatible endpoint, and local models.
Proof: Plan/Work/Operate modes, Ask/Auto-Review/Full Access approvals, an OS sandbox, `/receipts`,
`/preview-request`, and a community of 41k GitHub stars and 244 contributors.

## 4. Hero candidates

**A. Recommended.**
Headline: "The open-source coding agent for any model"
Subhead: "{brand} reads your project, edits files, and runs your tests from your terminal. Connect a hosted or local model, and choose which actions need your approval."
Why: category and differentiator in seven words; matches the README tagline, so docs and site agree;
noun phrase translates without English word order tricks; subhead adds the action list and control.

**B.** Headline: "A coding agent that works with your models"
Subhead: same as A.
Why: warmer and reader-centred, but "open source" moves out of the headline and the README tagline diverges.

**C.** Headline: "Describe the change, then review the diff"
Subhead: "{brand} is an open-source coding agent for your terminal. It works with hosted and local models, and it asks before it acts in Ask mode."
Why: shows the workflow. Rejected as the lead because it hides the category and depends on English imperative rhythm.

## 5. Proof points, in priority order

1. Any model: built-in provider routes, any OpenAI-compatible endpoint, local models (Ollama, vLLM, SGLang).
2. Control you can check: modes, approval postures, sandbox, `/undo`, `/receipts`, `/preview-request`.
3. Open source under MIT; no Codewhale account needed for the terminal or local browser.
4. Runs where you work: `codewhale`, `codewhale exec`, `codewhale web`, `codewhale review --pr N`, Runtime API.
5. Long work: `/goal`, sub-agents, Fleet with a pre-spend check, checked-in workflows.
6. Extend: MCP, skills, plugins, hooks, Claude Code plugin compatibility.
7. Community: GitHub stars and contributors, rendered from live data, never typed into copy.

## 6. Translation-safety rules (18 locales)

1. Write literal declarative sentences. One claim per sentence.
2. Keep each sentence at 20 words or fewer. Headlines at 8 words or fewer.
3. No idioms, puns, wordplay, slang, or cultural references ("bestie", "your last next editor").
4. No paired imperative slogans ("Describe X. Review Y.") or verbless fragments that rely on English rhythm.
   Prefer a full noun phrase or one complete sentence.
5. Use a single verb where one exists: "stop", not "shut down"; "divide", not "split up"; "check", not "look over".
6. Use the glossary terms below exactly. Do not translate them, pluralize them, or replace them with synonyms.
7. Commands, flags, paths, slash commands, and numbers are untranslated tokens: `codewhale exec`, `/receipts`.
8. Keep `{brand}`, `{version}`, `{tag}` placeholders verbatim; one `{brand}` per hero lede.
9. Avoid counts that drift (provider totals, star counts) in dictionary strings; render them from data.
10. Avoid "it", "this", and "that" when the referent is in another sentence.
11. Same claim, same words: reuse the docs sentence rather than paraphrasing it.
12. No em-dash asides; use a period, comma, or colon.

## 7. Glossary (fixed terms, from `lib/content/vocabulary.ts` and `docs/features.toml`)

| Term | Kind | Rule |
| --- | --- | --- |
| Codewhale | brand | never translated; `{brand}` in hero lede |
| Plan, Work, Operate | modes | keep English, capitalized |
| Ask, Auto-Review, Full Access | approval postures | keep English, capitalized |
| Fleet, Workflow, Lane, Runtime | product terms | keep English; definitions in vocabulary.ts |
| Provider, Model, Advisor | route identity | translate the common noun, keep the definition |
| Receipts (`/receipts`) | command | command token stays English |
| Request preview (`/preview-request`) | command | command token stays English |
| Goal (`/goal`), sub-agents, skills, plugins, hooks, MCP | features | MCP stays English |
| `codewhale`, `codewhale exec`, `codewhale web`, `codewhale review --pr N` | commands | never translated |
| Computer Use | plugin | English name, marked preview |
| CodeWhale GUI | community VS Code extension | proper noun |
