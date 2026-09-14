---
name: cw-gates
description: "Use before claiming any Codewhale change is done, green, or ready to land: the focused-to-broad verification ladder, the budget checks CI enforces, and the rules for what counts as a passing test."
---

# cw-gates

Pick the smallest evidence that answers the actual risk, then quote the real
output. This repo has been burned twice by the alternative: an exit code
mistaken for a pass, and a harness whose scoring line silently reported
unevaluated rows as green. Assertions without command output are not evidence.

Stage 3 of the loop: [cw-orient](../cw-orient/SKILL.md) →
[cw-slice](../cw-slice/SKILL.md) → **gates** →
[cw-dogfood](../cw-dogfood/SKILL.md) → [cw-land](../cw-land/SKILL.md) →
[cw-handoff](../cw-handoff/SKILL.md).

## When to use

- Before saying "done", "green", "passing", "fixed", or "ready to land".
- Before a dogfood build — never install an ungated binary.
- When asked to "run the gates" or to prove a change is safe.

For release work specifically, use
[codew-release-qa-sweep](../codew-release-qa-sweep/SKILL.md) instead — it adds
the version-drift gate and the manual TUI QA targets on top of this ladder.

## Workflow

Climb only as far as the risk requires. Say where you stopped and what you
skipped.

### Rung 1 — always, and cheap

```bash
cargo fmt --all -- --check
git diff --check
```

### Rung 2 — the area that owns the change

`scripts/dev-test.sh` maps an area or a source path to the fastest correct
invocation, and applies the isolated build-dir topology
(`docs/BUILD_PERFORMANCE.md`):

```bash
scripts/dev-test.sh --list
scripts/dev-test.sh crates/tui/src/elapsed.rs      # path → area + filter
scripts/dev-test.sh tui tools::                    # area + filter
scripts/dev-test.sh config
```

It uses `cargo nextest run` when nextest is on PATH (`.config/nextest.toml`);
`CODEWHALE_DEV_NEXTEST=0` forces libtest. For the TUI crate, `--lib` and
`--tests` are disjoint — choose the target that owns the behavior rather than
running both by reflex.

`cargo test --no-run` answers a compile question without executing unrelated
cases. `cargo test --doc` covers doc examples, and is only worth running when
those examples changed.

### Rung 3 — the budgets and drift checks CI enforces

Run the ones your change can move. Each fails the build in CI:

```bash
python3 scripts/check-dead-code-budget.py          # #[allow(dead_code)] ceiling
python3 scripts/check-runtime-contract-budget.py
python3 scripts/check-persistence-backlog-budget.py
python3 scripts/check-provider-registry.py         # provider registry drift
python3 scripts/check-command-crate-boundaries.py  # command-contract boundary
python3 scripts/check-command-migration-manifest.py
python3 scripts/check-tui-locale-parity.py         # touched crates/tui/locales/
sh scripts/check-tui-product-vocabulary.sh
python3 scripts/check-readme-translations.py       # touched README*.md
./scripts/release/check-versions.sh                # touched a version anywhere
```

The dead-code budget may go **down** freely; raising it needs a reviewer to be
told why. Lock in a win with `python3 scripts/check-dead-code-budget.py --update`.

### Rung 4 — cross-cutting or release risk only

```bash
cargo clippy --workspace --all-targets --all-features --locked -- \
  -D warnings \
  -A clippy::uninlined_format_args \
  -A clippy::too_many_arguments \
  -A clippy::unnecessary_map_or
cargo nextest run --workspace --all-features --locked --profile ci
cargo test --workspace --all-features --locked --doc
git diff --exit-code -- Cargo.lock                 # lockfile drift guard
```

`--all-targets` matters: without it, clippy never lints test code, and the
v0.9.10 release gate opened with four clippy failures on a green `main`, three
of them in test targets.

### Rung 5 — website, when `web/` changed

```bash
cd web && npm ci && npm test && npm run check
```

## Claiming a test passed

- Quote the real `test result: N passed; M failed` line, and confirm `N > 0`
  **for the tests that cover your change**. `cargo test <filter>` exits 0 having
  run zero tests when the filter matches nothing; an exit code alone has already
  been mistaken for a pass here.
- Prefer proving a regression test fails without the fix. A test that passes
  either way pins the implementation, not the defect.
- Audit any harness before trusting its score. `ok = ok and X or True` parses as
  `(ok and X) or True` and once reported twelve unevaluated rows as passing.
- A focused rerun of a failing test distinguishes flake from regression in
  seconds. Do that before calling anything a flake, and root-cause anything that
  fails outside a known-flaky name — check for unisolated config-path reads or
  global timeout knobs first.

## Red flags / don't

- Don't say "tests pass" without the count line. Don't say "CI will catch it".
- Don't run the full workspace suite as ritual for a leaf change, and don't
  re-run an unchanged suite to feel more confident.
- Don't weaken a safety or data-integrity behavior to make a gate go green.
- Don't call a failure a flake without a focused rerun and a named cause.
- Don't skip the budget checks because they are "not really tests" — they are
  required CI contexts, and they encode migrations this repo has already paid for.
- Don't report a green gate as permission. A passing sweep is readiness
  evidence; landing, tagging, and publishing need their own approval.

## Output

A checklist: each command, pass/fail, and the salient line (test counts, budget
numbers, the `check-versions.sh` verdict). Name explicitly what you did **not**
run and why. If a step could not run in this environment, say so rather than
implying coverage you do not have.
