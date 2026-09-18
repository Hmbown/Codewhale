---
name: cw-handoff
description: "Use when writing a Codewhale takeover prompt, continuation note, or end-of-session summary for another agent or a later session: a paste-ready handoff grounded in live state, with done/suspected/blocked kept separate."
---

# cw-handoff

A handoff is read by an agent with none of your context and every incentive to
believe you. That makes an optimistic handoff worse than no handoff: it converts
your guesses into the next session's premises. Write it so the reader can
re-derive the state instead of trusting the prose.

Stage 6 of the loop, and the one that makes the loop a loop: the next session
starts at [cw-orient](../cw-orient/SKILL.md) with what you leave here.

## When to use

- Ending a session with work in flight.
- Asked for "a prompt for another agent", branch takeover instructions, a
  continuation note, or a summary of current state for async work.
- Handing a lane to a different model, a fleet worker, or a remote session.

## Workflow

1. **Write it as a prompt the next agent can paste directly.** Not a report
   about the work — instructions for continuing it.

2. **Open with the refresh block, not with your summary.** The first thing the
   reader should do is verify you:
   ```bash
   cd <repo path>
   git status --short --branch && git branch --show-current
   git log --oneline --decorate -20
   git worktree list
   ./scripts/release/check-versions.sh
   ```
   Tell them to trust that output over anything below it.

3. **Cover these, in whatever order fits; drop sections that are empty:**
   - Repository path and expected branch or worktree.
   - The authority line: what they may and may not do without asking. Default to
     local-only — no push, merge, tag, publish, GitHub Release, or destructive
     cleanup without explicit approval.
   - Durable files to read, ordered by importance: the scoped `AGENTS.md` for
     the area, then the specific docs (`docs/CACHE.md`,
     `docs/MOTION_CONTRACT.md`, `docs/ARCHITECTURE.md`, `crates/tui/AGENTS.md`)
     the task actually touches.
   - Commits already landed, with SHAs.
   - **Dirty worktree caveats, naming uncommitted files explicitly**, and whose
     they are. This is the single most useful line in most handoffs.
   - The next slices, in priority order, each bounded the way
     [cw-slice](../cw-slice/SKILL.md) bounds one.
   - The verification gate for those slices — the smallest correct one from
     [cw-gates](../cw-gates/SKILL.md), plus any known-flaky names.
   - Open decisions that genuinely belong to Hunter.

4. **Separate three states, and never blur them:** *done and verified* (with the
   command output that proves it), *suspected* (a hypothesis, labeled), and
   *blocked* (with what unblocks it). If you did not run it, it is not done.

5. **Say what the branch is for.** Local-only, pushed for backup, or intended to
   stay unpushed — the next agent cannot tell from `git` alone, and guessing
   wrong is how work gets force-pushed away.

6. **Record missing external receipts.** If the task was local-only, say which
   evidence you could not gather (CI state, registry state, review threads)
   rather than inferring it. A named gap is useful; a confident guess is not.

## Red flags / don't

- Don't imply publication happened unless you verified registry, tag, or release
  state live.
- Don't hand off a narrative. Prefer concrete paths, commands, and SHAs.
- Don't summarize away the dirt. Unnamed uncommitted files get destroyed.
- Don't hand the next agent decisions you could have made. Reserve Hunter-facing
  choices for product direction, irreversible actions, and visual judgments that
  need eyes.
- Don't copy a previous handoff's state forward. Re-derive it; that is what
  step 2 is for.
- Don't include secrets, tokens, or provider credentials in a handoff file.

## Output

A single paste-ready block with the default shape — refresh commands,
authority line, files to read, landed SHAs, named dirty files, prioritized
next slices, the verification gate, and the open decisions — with done /
suspected / blocked visibly separated. Compress when trivial, but never
compress away the state separation: that is what keeps guesses from
becoming the next session's premises.
