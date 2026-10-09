---
name: cw-land
description: "Use when turning verified Codewhale work into commits, branches, or a merge: choosing direct-main vs. worktree vs. integration branch, preserving contributor credit, and honoring the gate artifact before merging."
---

# cw-land

Verified work still has to land without stepping on other writers, losing
contributor credit, or merging past a gate that has not actually passed. This
stage is about the boundary between "it works" and "it is in `main`" — and about
which of those steps you are allowed to take.

Stage 5 of the loop: [cw-orient](../cw-orient/SKILL.md) →
[cw-slice](../cw-slice/SKILL.md) → [cw-gates](../cw-gates/SKILL.md) →
[cw-dogfood](../cw-dogfood/SKILL.md) → **land** →
[cw-handoff](../cw-handoff/SKILL.md).

## When to use

- The change is verified and needs to become a commit on `main`.
- You are landing someone else's PR, harvesting a contributor's work, or
  resolving a conflict caused by `main` moving.
- You are about to merge something behind a required gate.

## Workflow

1. **Choose the landing shape.**
   - **The checkout that already exists** is the default. Direct to `main` is
     permitted for a small coherent change when that checkout is current and
     owns the affected files; when several agents share it, partition by file,
     stage only your paths, and retry a commit that fails on `index.lock`.
     Local commit permission never implies push, merge, tag, release, or
     deploy permission.
   - **No maintainer PR branches** (`AGENTS.md`): verified slices land on
     `main` directly and hosted CI runs there. PRs are for contributors.
   - **A worktree** is for a lane that genuinely conflicts with the dirt you
     found in [cw-orient](../cw-orient/SKILL.md), or an experiment you may
     throw away — not for parallel agents on the same lane.
   - **An integration branch** (`integration/<topic>-<pr>-<date>`) is for
     resolving a contributor PR that conflicts with a moving `main` when you
     cannot push to their fork. It leaves their branch untouched.

2. **Commit narrow and build-green.** One coherent change per commit; the tree
   builds at every commit. Put the real verification in the message — actual
   pass/fail counts, not "tests pass".

3. **Preserve credit mechanically, not just politely.** Commit authorship and
   `Co-authored-by:` trailers must use the contributor's own GitHub-linked
   address — GitHub reads neither `.github/AUTHOR_MAP` nor `.mailmap` for the
   contribution graph; those are project conventions on top. When a
   contributor's work lands as our commit, it carries both:
   ```text
   Harvested from PR #N by @handle

   Co-authored-by: Name <github-linked-email>
   ```
   That trailer is what lets `auto-close-harvested.yml` close their PR with
   credit. Canonical human identities live in `.github/AUTHOR_MAP`.

   Whether a bot or agent also appears in a trailer no longer matters — the CI
   check that policed trailer identities was removed because it rejected
   ordinary agent commits. Give humans their credit; don't spend time scrubbing
   tool trailers.

4. **Landing someone else's work: their time is more expensive than ours.**
   - Never make a contributor rebase around our churn. If their PR conflicts
     only because `main` moved, a maintainer resolves it.
   - Read their diff against the **merge base** first, so you know exactly what
     they added, then re-apply that — rather than hand-merging two large sides
     and hoping:
     ```bash
     git diff $(git merge-base main <pr-head>)..<pr-head>
     ```
   - **Conflicts that split mid-function do not resolve by keeping both sides.**
     Git's markers can land inside a body, so a both-sides resolution produces
     unbalanced braces that look plausible and do not compile. Default: take
     one side whole, then re-insert the other side's additions at their
     original anchor; when the conflict doesn't fit that pattern, resolve it
     however is correct and let the compiler judge.
   - `maintainerCanModify` does not guarantee push access to the fork. When the
     push is refused, land the resolved merge on an integration branch here.
   - **Check the contribution gate before assuming a PR is stalled.** An
     unlisted author's workflow runs sit at `action_required` and never start,
     so the PR looks abandoned when nobody has actually looked at it. Approve
     the runs, then fix the cause: add them to `.github/APPROVED_CONTRIBUTORS`
     (`all:username`), or comment `/lgtm` (PR scope) or `/lgtmi` (issue scope).

5. **Verify mergeability against the real head.** A PR that is clean against
   `main` can still conflict with a release branch:
   ```bash
   git merge-tree $(git merge-base <base> <pr-head>) <base> <pr-head>
   ```

6. **Merging under a gate.** `AGENTS.md` ("Merging under a gate") owns this:
   the artifact must literally say PASS, the review thread outranks the check
   rollup, and ambiguity blocks the merge — never the reverse.

7. **Clean up your own lane.** When a worktree you created lands on `main`,
   remove it (`git worktree remove <path>`) and delete its merged branch.
   Leave other writers' worktrees alone; `git worktree list` shows them.
   Worktree sprawl was a 560 GB problem here once.

## Red flags / don't

- Don't push, merge, tag, create a release, or deploy without explicit
  authorization. A local commit is not permission for any of those.
- Don't rewrite published history, retag a release, or force-push a shared ref.
- Don't commit `AGENTS.md` / `CLAUDE.md` operator controls that live outside the
  product repository into a public repo.
- Don't stage another writer's dirty files to get a clean commit.
- Don't merge on a green rollup alone when a review thread has open findings.
- Don't harvest or close from a PR title or label — review the code, tests,
  comments, and checks.
- Don't add another legacy call site for convenience once a replacement
  architecture is adopted. Declared migrations are one-way.
- Don't leave new enforcement live: keep it dry-run/advisory unless approved.

## Output

- The landing shape you chose and why (direct main / worktree / integration).
- Commit SHAs, branch name, and whether the branch is local-only or pushed.
- The credit trailers applied and to whom.
- The gate artifact's literal verdict at merge time, if a gate applies.
- Exactly which public actions you took, and which you deliberately did not.
