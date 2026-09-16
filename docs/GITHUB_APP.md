# GitHub App Setup (Codewhale Agent reviews)

`codewhale review --pr N` writes an advisory code review of a pull request. With
`--post` (or from CI) the review is published to GitHub. Published reviews can
appear under two identities:

- the default token the CI job already has (`github.token`), or
- a dedicated **GitHub App** so the review shows as a bot — e.g.
  `codewhale-agent[bot]` — instead of a personal account.

The App identity is optional. Nothing below is needed to run
`codewhale review --pr N` locally and print the report to your terminal.

Related docs:

- [Automatic Workflows](AUTOMATIC_WORKFLOWS.md) — the review workflow in context
- [Providers](PROVIDERS.md) — the model/key used to write the review
- [Receipts](RECEIPTS.md) — how posted reviews are anchored to a head SHA

## Account keys and provider keys

`CODEWHALE_API_KEY` is a Codewhale account machine key (`cwc_key_…`), not a
vendor credential. In account mode the workflow first runs
`codewhale --no-project-config account agent` to check authentication and the
account's configured agent. It then selects the existing `codewhale` provider,
which sends that key to the Codewhale model relay. It never copies an account
key into a vendor environment variable.

Connect the underlying provider in your Codewhale account and create a machine
key with `agent:run` and `models:infer` scopes. The default scopes from
`codewhale account api-keys create --name github-review` also include
`account:read`, which permits identity checks. Set `CODEWHALE_REVIEW_MODEL` to
an exact `provider/model` id returned by your account's authenticated
`GET /v1/models` catalog. The offline model defaults are bootstrap values, not
proof of account access; see [Providers](PROVIDERS.md).

Bring your own provider key by setting its secret instead of
`CODEWHALE_API_KEY`:

| Secret | Route |
| --- | --- |
| `CODEWHALE_API_KEY` | Codewhale account relay; requires an explicit account catalog model |
| `ZAI_API_KEY` | z.ai Coding Plan |
| `MODELSTUDIO_API_KEY` | Model Studio Token Plan |
| `DEEPSEEK_API_KEY` | DeepSeek |
| `OPENROUTER_API_KEY` | OpenRouter |
| `ANTHROPIC_API_KEY` | Anthropic |

If both account and vendor secrets exist, the workflow selects account mode
and leaves every vendor variable unchanged. In this mode
`CODEWHALE_REVIEW_PROVIDER` must be unset or `codewhale`; a conflicting value
fails before review. To select BYOK, remove the account secret from this
workflow's configuration.

## Choosing the review route and model

Configure repository variables under Settings → Secrets and variables →
Actions → Variables:

| Variable | Account mode | BYOK mode |
| --- | --- | --- |
| `CODEWHALE_REVIEW_PROVIDER` | Unset or `codewhale` | Explicit provider, such as `deepseek` |
| `CODEWHALE_REVIEW_MODEL` | Required exact account catalog `provider/model` id | Optional exact model id; otherwise the provider's default |

For BYOK without an explicit provider, the workflow chooses the first available
key in this order: z.ai, Model Studio Token Plan, DeepSeek, OpenRouter,
Anthropic. Set the provider explicitly when several keys are present.

The workflow passes provider and model as global CLI flags before `review`,
with `--no-project-config`. Account mode deliberately pins the relay route:
the account agent precondition reports a configured provider, but does not
supply an exact model id or a vendor credential to the runner.

For release PR **#6002** only, the workflow supplies an explicitly approved
`deepseek` / `deepseek-v4-pro` route and ceilings of **500000** characters per
pass, **16** complete passes, and **65536** output tokens per request. Existing
repository variables override these values. Other PRs retain the defaults
below. This exception changes no credentials or coverage rules: a diff that
requires more than 16 passes still fails before model review, and a provider
non-run is never completed-review evidence. Keep the release head frozen
during review to avoid cancellation and repeated provider cost.

## Complete diffs and input limits

The workflow checks out the event's pinned head SHA for same-repository PRs,
and the pinned base SHA for fork PRs. It uses full history, fetches the base
repository's PR head ref, and verifies both event commits and a single merge
base. Fetching fork objects does not check out or execute their files, hooks,
submodules, or filters. Checkout credentials are not persisted.
[GitHub's checkout documentation](https://github.com/actions/checkout) describes
`fetch-depth: 0` and `persist-credentials: false`.

The shared collector uses the complete GitHub diff when available and a
verified local Git diff when the API cannot provide it, including large PRs.
It rejects a changed snapshot, unavailable history, or incomplete diff before
review. Repository variable `CODEWHALE_REVIEW_MAX_CHARS` sets the input limit
per pass (default **200000**, allowed range **1–8388608**). The collector also has
an **8 MiB output** and **60-second command** bound; a character limit does not
bypass those transport bounds.

A complete diff requiring more than one configured-limit pass fails the job by
default. It is never silently truncated or treated as a provider funding
problem. Repository variable `CODEWHALE_REVIEW_MAX_PASSES` (default **1**,
allowed range **1–64**) passes `--max-passes N` to the CLI. Raising it explicitly
authorizes the workflow to run up to N ordered passes for a complete review,
with additional provider cost and run time. Set it only after reviewing that
budget; leaving it unset retains one pass. If any pass fails, no partial
review is posted.
Increasing the character limit is a separate input-budget choice and still
requires a model with sufficient context.

When the complete PR cannot fit the allowed pass count, keep the failed
advisory check and record that the model review did not run. Do not turn an
input-limit failure into a clean review. Maintainers can explicitly authorize
bounded whole-PR passes, or review bounded paths with a trusted build using
`review --base <base-sha> --path <path>` from a checkout pinned to the PR head.
Local diff reviews also reject oversized input. Path scopes cannot use `--pr`
or `--post`; their receipts cover only the selected paths.
Record the exact base/head, included paths and diff fingerprints, findings,
checks actually run, and remaining coverage. Separately review interactions
across paths and inspect changed media. A file inventory or a passing test
suite is not evidence that those source reviews completed. This fallback
does not change repository rules or satisfy a required whole-PR review.

## Review evidence and precision

The Actions-backed GitHub App and the `review` tool use the same PR review
contract. Findings must explain an introduced defect's trigger, source evidence,
impact and a useful fix. Generic requests for more tests, style preferences and
unsupported compiler claims do not qualify as findings. An empty findings list
is valid; unresolved assumptions belong in the assessment.

When the exact PR head is available locally, each pass also receives numbered
source excerpts around its changed hunks and nearby module declarations. These
come from regular Git blobs at the pinned head, never from dirty checkout files
or symlink targets. Source is not executed and no additional model call is made.
The excerpts use only the unused portion of `CODEWHALE_REVIEW_MAX_CHARS`, capped
at 50000 characters and 32 files per pass; individual blobs above 128 KiB are
omitted. The complete diff remains intact and remains the inline-comment scope.

The request explicitly records unavailable files and omitted context. It does
not inspect unchanged caller files or run builds/tests, and a completed review
does not establish either. These source and local-fixture guarantees do not
establish a model's bug-detection rate or parity with another review product.

## Output budget

`CODEWHALE_REVIEW_MAX_OUTPUT_TOKENS` optionally sets the CLI's output budget
through `CODEWHALE_MAX_OUTPUT_TOKENS`. Without it, the CLI chooses its automatic
cap. The workflow rejects values below **8192** to leave room for reasoning
and the final review. Provider accounting and supported limits vary; an empty
response is not proof of any one cause. A zero-exit review with empty output
fails the job.

## One-time setup, five steps

You need owner access to the GitHub repository once. After setup, eligible non-draft
same-repository pull requests can post reviews as the App.

1. **Create the App.** GitHub → *Settings → Developer settings → GitHub Apps →
   New GitHub App*. Name it (e.g. `Codewhale Agent`), set a homepage URL, and
   **uncheck Webhook → Active** — the review is pulled on PR events by Actions,
   so no webhook is needed.
2. **Grant two repository permissions.**
   - *Pull requests* → **Read & write** (to post the review and inline comments)
   - *Contents* → **Read-only** (to read the diff; read-only is enough — avoid
     write unless you have another reason)
   Choose *Only on this account*, then **Create GitHub App**.
3. **Download the private key.** On the App's page, *Private keys → Generate a
   private key*. Keep the `.pem` file secret; it is the App's credential.
4. **Install the App** on your account (*Install App* on the same page) and
   select the repositories reviews should cover.
5. **Add repository settings.** GitHub → *Settings → Secrets and
   variables → Actions*:

   | Kind     | Name                        | Value                               |
   |----------|-----------------------------|-------------------------------------|
   | Variable | `CODEWHALE_APP_ID`          | the App ID shown on the App's page  |
   | Secret   | `CODEWHALE_APP_PRIVATE_KEY` | the full `.pem` file contents       |
   | Secret   | `CODEWHALE_API_KEY`         | a Codewhale machine key; for BYOK use the provider's own secret name instead |
   | Variable | `CODEWHALE_REVIEW_MODEL`    | exact account catalog `provider/model` id (required for account mode) |

   App settings control identity. Model access separately requires a review
   key and, for account mode, the catalog model. Optional budget variables are
   `CODEWHALE_REVIEW_MAX_CHARS`, `CODEWHALE_REVIEW_MAX_PASSES`, and
   `CODEWHALE_REVIEW_MAX_OUTPUT_TOKENS`.

## How the pieces connect

[The review workflow](../.github/workflows/codewhale-review.yml) uses
`pull_request` for non-draft PRs targeting `main`. Only same-repository PRs
receive review secrets and build the candidate CLI. Fork PRs keep the trusted
base checkout and run only the diff-object checks; they receive no model or
App secrets and no model review. GitHub also
[withholds ordinary secrets from fork pull requests](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#workflows-in-forked-repositories).
Review a fork separately with a trusted build and deliberately provided
credentials. This workflow does not execute a fetched fork merely to obtain
a large diff.

For eligible reviews, when
`CODEWHALE_APP_ID` **and** `CODEWHALE_APP_PRIVATE_KEY` are both present, the
job mints a short-lived installation token for the App
(`actions/create-github-app-token`) and hands it to the CLI as `GH_TOKEN`.
Otherwise it falls back to the workflow's own `github.token`. The CLI never
stores the token; each run mints a fresh one.

The key-presence test lives in the job's `env:` block rather than its `if:`
because the `secrets` context is not available in a job-level `if:`. Job-level
`env` can read `secrets`, and step-level `if:` can read `env`, so build and review steps
gate on the non-secret string `env.HAS_ANY_KEY`. Diff preparation needs only
the workflow token with repository read access. Only booleans about presence
live at job scope; the key values are injected into the one step that runs the
review.

Missing review credentials and provider HTTP failures keep the existing
advisory policy: the job can be green while the step summary explicitly says
**not run**. Provider failures also leave an idempotent non-run PR comment.
These are not clean-review results. Input-limit, snapshot, build, and other
review failures still fail the job. A successful later review removes a stale
non-run comment.

The review itself is one **COMMENT** review — a summary body plus inline line
comments anchored to the PR head SHA. It never approves or requests changes;
CODEOWNERS stays the human authority.

## Running a review yourself

```sh
# print a report locally (uses your configured provider key)
codewhale review --pr 1234

# pin the route when a model is reachable through more than one provider
codewhale --provider deepseek --model MODEL_ID review --pr 1234

# account mode: check the agent, then use an exact id from the account catalog
codewhale --no-project-config account agent
codewhale --no-project-config --provider codewhale --model PROVIDER/MODEL_ID review --pr 1234

# explicitly increase a complete-diff input limit when needed
codewhale review --pr 1234 --repo OWNER/REPO --max-chars 6000000

# explicitly authorize at most 8 complete ordered model passes
codewhale review --pr 1234 --repo OWNER/REPO --max-passes 8

# publish it to GitHub as whichever identity GH_TOKEN carries
codewhale review --pr 1234 --post
```

`GH_TOKEN` may be your `gh` CLI token (posts as you) or an App installation
token (posts as the App). The `--post` flag is always opt-in.

## Troubleshooting

- **Review posts as you, not the bot.** The variable or the private-key secret
  is missing/empty; the job silently falls back to `github.token`. Check both
  names character-for-character.
- **Step summary says "not run".** No model review completed. Check whether
  this is a fork, review credentials are missing, or the provider failed.
- **Account model or provider error.** Set the provider to `codewhale` (or
  unset it), choose the exact model from the account catalog, and check that
  the machine key has the required scopes and the account has a configured
  agent. A vendor key belongs in its own secret, never `CODEWHALE_API_KEY`.
- **Complete diff exceeds the input limit.** Inspect the reported size and
  model context capacity before raising `CODEWHALE_REVIEW_MAX_CHARS`. An 8 MiB
  transport-bound failure cannot be bypassed with that variable.
- **PR head changed or history is unavailable.** Rerun for the current
  revision. The workflow refuses to review an unverified snapshot.
- **"available from configured provider route(s): ...".** Two provider keys are
  configured and the model is reachable from both. Set repository variable
  `CODEWHALE_REVIEW_PROVIDER`.
- **Empty review.** The job fails. Inspect provider errors and output-budget
  receipts; increasing `CODEWHALE_REVIEW_MAX_OUTPUT_TOKENS` may help when
  reasoning exhausted the budget, but does not diagnose the cause by itself.
- **App token step fails.** The `.pem` was regenerated after the secret was
  set — paste the newest key into `CODEWHALE_APP_PRIVATE_KEY` again, and
  confirm the App is actually installed on the repository.
- **Name already taken.** GitHub App names are global; pick another name. The
  bot's display login is `<slug>[bot]`, derived from the name.
