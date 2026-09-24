#!/usr/bin/env bash
# One local preflight for the generated files and ratchets that otherwise turn
# main red after a green PR: run it before you push.
#
#   scripts/preflight.sh            regenerate what can be regenerated, check the rest
#   scripts/preflight.sh --check    change nothing; fail if anything is stale
#   scripts/preflight.sh --full     also run the cargo-backed runtime-contract and
#                                   persistence-backlog ratchets (minutes, offline)
#   scripts/preflight.sh --base REF compare feature receipts against REF
#                                   (default: merge base with origin/main)
#
# As a pre-push hook it runs in --check mode:
#   ln -s ../../scripts/preflight.sh "$(git rev-parse --git-path hooks)/pre-push"
#
# What it covers, and the command that fixes each one:
#   - crates/tui/CHANGELOG.md slice    scripts/sync-changelog.sh (written here)
#   - README locale stamps and links    retranslate; check-readme-translations.py
#                                       prints the new sha256 stamp to use
#   - product lexicon (warn-only)      python3 scripts/check-lexicon.py lists each hit
#   - dead-code / blocking-calls         each ratchet's --update, committed in the
#     (and with --full runtime-contract, same PR with the reason in the PR body
#     persistence-backlog)
#   - feature release-note receipts      add each feat commit's #issue to
#     for this branch's own commits      CHANGELOG.md in the same PR
set -uo pipefail

mode="write"
full=0
base=""
if [[ "$(basename "$0")" == "pre-push" ]]; then
  # git passes <remote> <url>; the hook only ever checks.
  mode="check"
  set --
fi
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --check) mode="check"; shift ;;
    --full) full=1; shift ;;
    --base) base="${2:?--base needs a ref}"; shift 2 ;;
    -h|--help) sed -n '2,26p' "$0"; exit 0 ;;
    *) echo "usage: $0 [--check] [--full] [--base REF]" >&2; exit 2 ;;
  esac
done

root="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}" 2>/dev/null || echo "${BASH_SOURCE[0]}")")/.." && pwd)"
cd "${root}" || exit 2

failed=()
step() {
  local label="$1" fix="$2"
  shift 2
  echo "== ${label}"
  if "$@"; then
    return 0
  fi
  failed+=("${label}: ${fix}")
}

if [[ "${mode}" == "write" ]]; then
  step "TUI changelog slice" "scripts/sync-changelog.sh" ./scripts/sync-changelog.sh
else
  step "TUI changelog slice" "scripts/sync-changelog.sh" ./scripts/sync-changelog.sh --check
fi
step "README translations in sync" \
  "retranslate the changed README sections, then update each stamp to the sha256 printed above" \
  python3 scripts/check-readme-translations.py
step "README locale link symmetry" "link every README.<locale>.md from README.md" \
  bash scripts/check-readme-locales.sh
# Warn-only: lists retired product words and engineering notes in English
# copy; never fails the preflight.
step "product lexicon (warn-only)" "python3 scripts/check-lexicon.py" \
  python3 scripts/check-lexicon.py --summary
step "dead-code budget" "python3 scripts/check-dead-code-budget.py --update" \
  python3 scripts/check-dead-code-budget.py
step "blocking-calls budget" "python3 scripts/check-blocking-calls-budget.py --update" \
  python3 scripts/check-blocking-calls-budget.py
if [[ "${full}" == "1" ]]; then
  step "runtime-contract budget" \
    "python3 scripts/check-runtime-contract-budget.py --update --allow-increase" \
    python3 scripts/check-runtime-contract-budget.py
  step "persistence-backlog budget" \
    "python3 scripts/check-persistence-backlog-budget.py --update" \
    python3 scripts/check-persistence-backlog-budget.py
fi

if [[ -z "${base}" ]]; then
  base="$(git merge-base HEAD origin/main 2>/dev/null || true)"
fi
if [[ -n "${base}" ]]; then
  step "feature release-note receipts (${base:0:12}..HEAD)" \
    "add each feat commit's #issue to CHANGELOG.md in this branch" \
    ./scripts/release/check-feature-release-notes.sh "${base}" HEAD
else
  echo "== feature release-note receipts: skipped (no origin/main; pass --base REF)"
fi

echo
if [[ "${#failed[@]}" -gt 0 ]]; then
  echo "preflight: ${#failed[@]} check(s) failed. Fix, commit the result, and re-run:" >&2
  for line in "${failed[@]}"; do
    echo "  - ${line}" >&2
  done
  exit 1
fi
if [[ "${mode}" == "write" ]] && ! git diff --quiet -- crates/tui/CHANGELOG.md; then
  echo "preflight: OK, and crates/tui/CHANGELOG.md was regenerated -- commit it."
else
  echo "preflight: OK"
fi
