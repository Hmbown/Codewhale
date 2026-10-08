<!-- Thanks for the PR! English or 中文 are both fine. Short is good. -->

## What and why

<!-- What does this change, and why? A few sentences is enough. -->

## Issue

<!-- CI needs one line here. Replace N with a number:
  - Closes #N    this PR finishes the issue (Fixes / Resolves also work)
  - Refs #N      related or partial work; the issue stays open
  - No-Issue: <one-line reason>   for typos, chores, dependency bumps
Only write a closing word when you mean it. GitHub closes the issue even in
"this does not close ..." and CI rejects that wording.
Put issue numbers here rather than in `feat:` commit messages: CI requires a
`feat:` commit that mentions #N to also add #N to CHANGELOG.md. -->

## How I tested it

<!-- Commands you ran and what they showed. Test what you changed, for example
`scripts/dev-test.sh tui <filter>`; CI runs the full suite for you.
For visible UI changes, add a screenshot or recording. -->

## Checklist

- [ ] One focused change, rebased on current `main`
- [ ] `cargo fmt --all` passes, and tests cover new or changed behavior
- [ ] If this adds or changes a user-facing feature, I updated its docs and its row in `docs/features.toml` (`cargo test -p codewhale-tui --test feature_registry` checks it)
