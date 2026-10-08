#!/usr/bin/env bash
# Require issue-linked feature commits in a release range to leave a durable
# changelog receipt. This catches shipped user-visible features that otherwise
# disappear when the GitHub Release body is generated from CHANGELOG.md.
set -euo pipefail

base_ref="${1:?usage: $0 <base-ref> <head-ref> [notes-file ...]}"
head_ref="${2:?usage: $0 <base-ref> <head-ref> [notes-file ...]}"
shift 2

repo_root="${RELEASE_NOTE_REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
cd "${repo_root}"

if [[ "$#" -gt 0 ]]; then
  notes_files=("$@")
else
  notes_files=(CHANGELOG.md docs/CHANGELOG_ARCHIVE.md)
fi

for ref in "${base_ref}" "${head_ref}"; do
  if ! git rev-parse --verify --quiet "${ref}^{commit}" >/dev/null; then
    echo "error: release-note check cannot resolve ${ref}" >&2
    exit 2
  fi
done
for notes_file in "${notes_files[@]}"; do
  if [[ ! -f "${notes_file}" ]]; then
    echo "error: release-note check cannot read ${notes_file}" >&2
    exit 2
  fi
done

fail=0
checked=0
feature_subject_pattern='^feat(\([^)]*\))?!?:[[:space:]]'
while IFS= read -r -d '' sha &&
  IFS= read -r -d '' subject &&
  IFS= read -r -d '' body; do
  [[ "${subject}" =~ ${feature_subject_pattern} ]] || continue

  feature_text="${subject}"$'\n'"${body}"
  while IFS= read -r issue; do
    [[ -n "${issue}" ]] || continue
    case "${issue}" in
      000 | 1 | 24 | 26 | 1834 | 142352) continue ;;
    esac
    checked=$((checked + 1))
    found=0
    for notes_file in "${notes_files[@]}"; do
      if grep -Eq "#${issue}([^[:alnum:]_]|$)" "${notes_file}"; then
        found=1
        break
      fi
    done
    if [[ "${found}" -eq 0 ]]; then
      echo "::error::Feature commit ${sha:0:12} references #${issue}, but no release-note receipt exists in ${notes_files[*]}." >&2
      echo "  ${subject}" >&2
      fail=1
    fi
  done < <(
    printf '%s\n' "${feature_text}" \
      | grep -oE '#[0-9]+([^[:alnum:]_]|$)' \
      | sed -E 's/^#([0-9]+).*/\1/' \
      | sort -u \
      || true
  )
done < <(git log -z --no-merges --format='%H%x00%s%x00%b' "${base_ref}..${head_ref}")

if [[ "${fail}" -ne 0 ]]; then
  echo "Add the missing user-visible feature to the appropriate changelog section before release." >&2
  exit 1
fi

echo "Feature release-note receipts OK: ${checked} linked issue reference(s) checked in ${base_ref}..${head_ref}."
