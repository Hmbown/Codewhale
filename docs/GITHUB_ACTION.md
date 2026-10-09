# GitHub PR reviews

The root `action.yml` runs the existing `codewhale review` command with an
exact, checksummed release. It needs Node.js 22+, `gh`, Git, and a macOS or
Linux runner. It does not compile Codewhale or execute the PR's code.

This source adds review mode. Autonomous mention/fix mode remains on the
hosted GitHub App path; this Action does not yet implement SHA-6706's mention
acceptance. No release tag containing this Action is claimed here: pin the
reviewed Action commit until a release containing it exists. CLI version and
Action revision are separate pins.

## Account setup

Connect your provider and select the model in Codewhale. Create a dedicated
account machine key with `account:read`, `agent:run` and `models:infer`, and
save it as the repository Actions secret `CODEWHALE_API_KEY`.

Set repository variable `CODEWHALE_REVIEW_MODEL` to the exact `provider/model`
ID from the account's authenticated model catalog. This pins the account
selection explicitly: today's machine preflight exposes provider readiness,
not the selected model ID. Updating the account selection alone does not
update this variable. Do not substitute a guessed provider default or copy
the account key into a vendor key variable.

Add `.github/workflows/codewhale.yml`, replacing `ACTION_COMMIT_SHA` with the
reviewed 40-character commit containing this Action:

```yaml
name: Codewhale review
on:
  pull_request:
    types: [opened, synchronize, reopened, ready_for_review]
  workflow_dispatch:
    inputs:
      pr-number:
        description: Same-repository PR number
        required: true
        type: string
permissions:
  contents: read
  pull-requests: write
concurrency:
  group: codewhale-review-${{ github.event.pull_request.number || inputs.pr-number }}
  cancel-in-progress: true
jobs:
  review:
    runs-on: ubuntu-latest
    timeout-minutes: 25
    steps:
      - uses: actions/setup-node@v7
        with:
          node-version: '22'
      - uses: codewhale-hq/CodeWhale@ACTION_COMMIT_SHA
        id: review
        with:
          version: v0.10.0
          model: ${{ vars.CODEWHALE_REVIEW_MODEL }}
          pr-number: ${{ inputs.pr-number }}
        env:
          CODEWHALE_API_KEY: ${{ (github.event_name == 'workflow_dispatch' || github.event.pull_request.head.repo.full_name == github.repository) && secrets.CODEWHALE_API_KEY || '' }}
      - uses: actions/upload-artifact@v7
        if: always() && steps.review.outputs.receipt != ''
        with:
          name: codewhale-review-${{ github.run_id }}-${{ github.run_attempt }}
          path: ${{ steps.review.outputs.receipt }}
          retention-days: 14
```

No repository checkout is needed. To post as your own GitHub App, supply an
installation token as `github-token`; see [App identity setup](GITHUB_APP.md).
Never use `pull_request_target` with this Action. Fork PRs and drafts are
ineligible even on a manual run. A skipped event is not a clean review.

For BYOK, set `provider` explicitly (`deepseek`, `anthropic`, `openrouter`,
`zai`, or `modelstudio-token-plan`), set its exact `model`, and pass only the
matching provider secret in `env`. Omit `CODEWHALE_API_KEY`. The Action never
chooses a different credential after an authentication or payment failure.

## Bounds and evidence

| Input | Default | Meaning |
| --- | --- | --- |
| `version` | Required | Exact released CLI tag, never `latest` |
| `provider` | `codewhale` | Account relay or explicit BYOK |
| `model` | Required | Exact model ID; account mode uses `provider/model` |
| `max-chars` | 200000 | Complete diff characters per pass, maximum 8388608 |
| `max-passes` | 1 | Complete ordered passes, maximum 64 |
| `max-output-tokens` | CLI automatic | Optional per-pass ceiling, 8192–1000000 |
| `timeout-seconds` | 600 | Model/publication deadline, 30–1200 |
| `post` | `true` | `false` produces a receipt without publishing |

These are input, output, and time bounds, not a dollar guarantee. Model prices
and reasoning accounting vary. Raising the pass count authorizes more model
requests. The Action never retries inference automatically. Do not enable it
without setting the desired review scope and spend limits for the account.

Outputs are `outcome`, `receipt` (absolute JSON path), and `pr-url`. The
receipt contains the pinned revision, route, limits, publication state,
completion counts and reported token usage. It excludes raw model output,
provider errors, PR text and credentials. Provider token accounting can be
absent. A runner shutdown before the receipt is written has no completion
receipt and must not be counted as a clean review.

The CLI validates complete diff coverage and checks current revision before
publication. It reads bounded source excerpts from pinned Git blobs, without
running tests or investigating arbitrary unchanged callers. “Reviewed clean”
means the configured review completed with zero reported issues; it is not
proof that the PR contains no bugs.

## Outcomes and recovery

| Outcome | Meaning / next action |
| --- | --- |
| `reviewed_clean` | Complete model review with zero reported issues |
| `reviewed_with_findings` | Complete model review; findings remain advisory |
| `configuration_missing` | Check the exact release/model, key presence, route and input bounds |
| `failed` | No complete review receipt; check account readiness, provider balance, release assets and GitHub access |
| `incomplete` | Coverage was incomplete; inspect the PR and revise scope or limits |
| `publication_uncertain` | Check the PR before any retry; publication may have succeeded |
| `superseded` | PR revision changed; run against the current head |
| `not_eligible` | Fork, draft, closed PR or unsupported event; no model run |

Failures fail the optional Actions job; do not make it a required merge check
unless that is your repository policy. A provider failure is not a negative
verdict about the PR. No additional failure comment is posted.

After repairing setup, use “Run workflow” with a PR number. Enabling a disabled
workflow does not replay old events. Manual reruns can publish another review:
cross-run publication deduplication is not implemented in this Action. Check
GitHub first, especially after a timeout or cancellation. Recovery is proved
by a complete receipt for the current head plus the intended publication,
not by workflow enablement alone.
