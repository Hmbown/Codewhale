---
name: cw-orient
description: "Use at the start of any Codewhale work session, or when unsure which checkout, branch, or worktree is authoritative: establish live repo truth before reading a plan or editing a file."
---

# cw-orient

Open every session on live truth, not on a handoff note, a directory name, or
what was true last time. This repo runs many worktrees at once; the cheapest bug
to avoid is editing the wrong one. Five commands, then you know where you are.

This is stage 1 of the loop: **orient → [cw-slice](../cw-slice/SKILL.md) →
[cw-gates](../cw-gates/SKILL.md) → [cw-dogfood](../cw-dogfood/SKILL.md) →
[cw-land](../cw-land/SKILL.md) → [cw-handoff](../cw-handoff/SKILL.md)**.

## When to use

- Starting a session, resuming one, or taking over from another agent.
- A handoff, issue, or plan tells you the state of the repo — verify before
  trusting it.
- You are about to edit and are not certain this checkout owns the files.

## Workflow

1. **Locate yourself.** Checkout, branch, dirt, and how far you are from the
   remote:
   ```bash
   git rev-parse --show-toplevel && git branch --show-current
   git status --short --branch
   git log --oneline --decorate -10
   ```
   A detached HEAD or a branch with no upstream is normal here — note it, don't
   "fix" it silently.

2. **See the other lanes.** Worktree sprawl is the standing hazard: several
   checkouts of this repo are usually live, each with its own dirty state.
   ```bash
   git worktree list
   git branch --sort=-committerdate --format='%(committerdate:short) %(refname:short)' | head -20
   ```
   If the work you were asked to do already has a worktree, use it. Do not
   start a second copy of the same lane.

3. **Read the dirt before you touch it.** Modified files you did not write
   belong to someone else — another agent, another lane, an in-flight slice:
   ```bash
   git status --porcelain
   git diff --stat
   ```
   Preserve them. Leave them unstaged, and do not `git checkout --` or stash
   another writer's work to get a clean tree. If your change genuinely conflicts
   with the dirt, work in a fresh worktree instead.

4. **Fix which guidance applies.** The nearest scoped `AGENTS.md` wins over the
   root one, and it is where the per-area rules actually live:
   ```bash
   find . -name AGENTS.md -not -path './tmp/*' -not -path '*/node_modules/*' -not -path './target/*'
   ```
   Today: root `AGENTS.md`, `crates/tui/AGENTS.md`,
   `crates/localization/locales/AGENTS.md`, `web/AGENTS.md`. Read the one that owns the
   files you are about to touch.

5. **Establish version truth from source, not memory.**
   ```bash
   grep -m1 '^version' Cargo.toml
   ./scripts/release/check-versions.sh
   ```
   `check-versions.sh` is the drift gate across the workspace version, npm,
   `Cargo.lock`, the changelog, and the README. If it disagrees with what you
   were told, believe the script.

6. **Only if the task is about the community queue** — issues, PRs, harvesting,
   credit — pull the live queue with the `gh-*` skills in this directory rather
   than assuming from memory. When the task is local-only, stay offline and
   record the missing external receipt instead.

## Red flags / don't

- Don't infer the active lane from a directory name, a stale handoff, or a
  `.md` file's confident prose. All three have been wrong here.
- Don't treat a plan document as current state. Plans describe intent; `git`
  describes reality.
- Don't clean, stash, reset, or `git checkout --` files you did not modify.
  Worktree dirt is usually another writer, not garbage.
- Don't start work in a worktree whose branch is already merged to `main` —
  that lane is retirement material, not a base.
- Don't skip step 4 because the root `AGENTS.md` "probably covers it". The
  scoped files carry the rules that actually get violated.
- Don't fetch, browse, or hit GitHub when the task was scoped local-only.

## Output

Open with a short status block. Skip fields that are empty or normal — a
clean tree on `main` with no related worktrees is one line, not five:

- checkout path, branch (or detached HEAD), base commit;
- dirty files and whose they appear to be;
- other worktrees that own related work;
- workspace version and whether `check-versions.sh` agrees;
- which `AGENTS.md` files govern this change.
