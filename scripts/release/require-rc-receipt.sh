#!/usr/bin/env bash
# Refuse a release unless a green release-candidate run validated the exact
# commit, including its Parity job.
#
# Usage: require-rc-receipt.sh <owner/repo> <40-char sha>
#
# The receipt is a completed, successful `release-candidate.yml` run for
# <sha>, dispatched by hand, whose Parity job (the shared
# release-parity.yml gate) concluded success. A green run whose Parity job
# was skipped is not a receipt. Requires `gh` authenticated with actions:read.
#
# RC_RECEIPT_GH overrides the gh executable (tests only).
set -euo pipefail

repo="${1:-}"
sha="${2:-}"
gh_bin="${RC_RECEIPT_GH:-gh}"

if [[ -z "${repo}" || -z "${sha}" ]]; then
  echo "usage: $0 <owner/repo> <sha>" >&2
  exit 2
fi
if ! [[ "${repo}" =~ ^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$ ]]; then
  echo "::error::Repository '${repo}' must be owner/name." >&2
  exit 2
fi
if [[ "${#sha}" -ne 40 || "${sha}" =~ [^0-9a-f] ]]; then
  echo "::error::Release SHA must be a full 40-character lowercase commit SHA, got '${sha}'." >&2
  exit 2
fi

# sha is validated hex, so it is safe to place inside the jq program.
runs="$("${gh_bin}" api \
  "repos/${repo}/actions/workflows/release-candidate.yml/runs?head_sha=${sha}&status=success&per_page=100" \
  --jq ".workflow_runs[] | select(.head_sha == \"${sha}\" and .conclusion == \"success\" and .event == \"workflow_dispatch\") | [.id, .html_url] | @tsv")"

while IFS=$'\t' read -r run_id run_url; do
  [[ -n "${run_id}" ]] || continue
  if ! [[ "${run_id}" =~ ^[0-9]+$ ]]; then
    echo "::error::Unexpected run id '${run_id}' from the Actions API." >&2
    exit 1
  fi
  # Jobs from a reusable workflow are named "<caller name> / <job name>".
  parity_green="$("${gh_bin}" api \
    "repos/${repo}/actions/runs/${run_id}/jobs?filter=latest&per_page=100" \
    --jq '[.jobs[] | select((.name == "Parity" or (.name | startswith("Parity / "))) and .conclusion == "success")] | length')"
  if [[ "${parity_green}" =~ ^[0-9]+$ && "${parity_green}" -gt 0 ]]; then
    echo "Release-candidate receipt for ${sha}: ${run_url} (Parity green)"
    exit 0
  fi
  echo "::warning::Release-candidate run ${run_url} is green but has no successful Parity job; it is not a receipt." >&2
done <<< "${runs}"

echo "::error::No green release-candidate run with a passing Parity job for ${sha}. Validate that exact commit first: gh workflow run release-candidate.yml --ref main -f expected_sha=${sha} (use the release tag as --ref if main has moved past it), wait for it to go green, then re-run this Release. Never move a tag to a different SHA to get past this." >&2
exit 1
