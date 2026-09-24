#!/usr/bin/env bash
# Delete GitHub Actions caches that no future run can use.
#
# Usage:
#   prune-actions-caches.sh [--dry-run] --ref <git-ref> [--ref <git-ref> ...]
#   prune-actions-caches.sh [--dry-run] --sweep
#
# --ref deletes every cache saved under that exact ref (for example
# refs/pull/123/merge or refs/tags/v1.2.3).
#
# --sweep deletes caches under refs/pull/N/* whose PR is closed, and caches
# under refs/tags/* last used more than a day ago (a finished release run
# never reads them again; the day keeps an in-flight release's own cache).
# Branch caches, including main, are never touched.
#
# A cache is only readable from its own ref and the default branch, so a
# closed PR's or a released tag's entries are dead weight that pushes live
# main entries out once the repo passes its 10 GiB cap.
#
# Needs GH_REPO=owner/name and gh authenticated with actions:write.
# PRUNE_CACHES_GH overrides the gh executable and PRUNE_CACHES_NOW the epoch
# clock (tests only).
set -euo pipefail

gh_bin="${PRUNE_CACHES_GH:-gh}"
now="${PRUNE_CACHES_NOW:-$(date +%s)}"
tag_min_age_seconds=86400
dry_run=false
sweep=false
refs=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) dry_run=true ;;
    --sweep) sweep=true ;;
    --ref)
      [[ $# -ge 2 ]] || { echo "--ref needs a value" >&2; exit 2; }
      refs+=("$2")
      shift
      ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
  shift
done

repo="${GH_REPO:-}"
if ! [[ "${repo}" =~ ^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$ ]]; then
  echo "GH_REPO must be owner/name, got '${repo}'" >&2
  exit 2
fi
if [[ "${sweep}" == false && ${#refs[@]} -eq 0 ]]; then
  echo "usage: $0 [--dry-run] (--ref <ref>... | --sweep)" >&2
  exit 2
fi
for ref in ${refs[@]+"${refs[@]}"}; do
  # Only PR and tag refs are prunable by name; never a branch.
  if ! [[ "${ref}" =~ ^refs/pull/[0-9]+/(merge|head)$ || "${ref}" =~ ^refs/tags/[A-Za-z0-9._-]+$ ]]; then
    echo "refusing to prune caches for '${ref}': only refs/pull/N/{merge,head} and refs/tags/<tag> are allowed" >&2
    exit 2
  fi
done

deleted=0
bytes=0

delete_cache() {
  local id="$1" ref="$2" size="$3"
  if [[ "${dry_run}" == true ]]; then
    echo "would delete cache ${id} (${ref}, ${size} bytes)"
  else
    "${gh_bin}" api -X DELETE "repos/${repo}/actions/caches/${id}" >/dev/null
    echo "deleted cache ${id} (${ref}, ${size} bytes)"
  fi
  deleted=$((deleted + 1))
  bytes=$((bytes + size))
}

# id, ref, size, last-accessed epoch (fractional seconds stripped for jq).
list_caches() {
  local query="$1"
  "${gh_bin}" api --paginate "repos/${repo}/actions/caches?per_page=100${query}" \
    --jq '.actions_caches[] | [.id, .ref, .size_in_bytes, (.last_accessed_at | sub("\\.[0-9]+"; "") | fromdateiso8601)] | @tsv'
}

# Each listing is read in full before deleting, so deletes never shift the
# pages still to be fetched, and a failed listing stops the script (set -e).
for ref in ${refs[@]+"${refs[@]}"}; do
  listing="$(list_caches "&ref=${ref}")"
  while IFS=$'\t' read -r id cache_ref size _; do
    [[ -n "${id}" ]] || continue
    [[ "${cache_ref}" == "${ref}" ]] || continue
    delete_cache "${id}" "${cache_ref}" "${size}"
  done <<< "${listing}"
done

if [[ "${sweep}" == true ]]; then
  # "N=state" lines; bash 3.2 (macOS) has no associative arrays.
  pr_states=""
  listing="$(list_caches "")"
  while IFS=$'\t' read -r id cache_ref size accessed; do
    [[ -n "${id}" ]] || continue
    if [[ "${cache_ref}" =~ ^refs/pull/([0-9]+)/(merge|head)$ ]]; then
      pr="${BASH_REMATCH[1]}"
      state="$(printf '%s' "${pr_states}" | sed -n "s/^${pr}=//p")"
      if [[ -z "${state}" ]]; then
        state="$("${gh_bin}" api "repos/${repo}/pulls/${pr}" --jq '.state')"
        pr_states="${pr_states}${pr}=${state}"$'\n'
      fi
      if [[ "${state}" == "closed" ]]; then
        delete_cache "${id}" "${cache_ref}" "${size}"
      fi
    elif [[ "${cache_ref}" =~ ^refs/tags/ ]]; then
      if (( now - accessed > tag_min_age_seconds )); then
        delete_cache "${id}" "${cache_ref}" "${size}"
      fi
    fi
  done <<< "${listing}"
fi

verb="deleted"
[[ "${dry_run}" == true ]] && verb="would delete"
echo "${verb} ${deleted} caches, ${bytes} bytes"
