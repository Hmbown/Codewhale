#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
script="${repo_root}/scripts/release/prune-actions-caches.sh"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT

# Fake gh: serves the cache listing (filtered by &ref= like the real API),
# PR states, and records DELETE calls. The caller's --jq filter runs through
# the real jq so the timestamp parsing is exercised too.
fake_gh="${tmp_dir}/gh"
cat > "${fake_gh}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[[ "$1" == "api" ]] || { echo "unexpected: $*" >&2; exit 9; }
shift
if [[ "$1" == "-X" ]]; then
  [[ "$2" == "DELETE" ]] || exit 9
  echo "$3" >> "${FIXTURE_DIR}/deleted"
  exit 0
fi
[[ "$1" == "--paginate" ]] && shift
path="$1"
filter="$3"
case "${path}" in
  */actions/caches\?*)
    ref=""
    if [[ "${path}" == *"&ref="* ]]; then ref="${path#*&ref=}"; fi
    jq --arg ref "${ref}" '{actions_caches: [.actions_caches[] | select($ref == "" or .ref == $ref)]}' \
      "${FIXTURE_DIR}/caches.json" | jq -r "${filter}"
    ;;
  */pulls/*)
    pr="${path##*/}"
    jq ".\"${pr}\"" "${FIXTURE_DIR}/pulls.json" | jq -r "${filter}"
    ;;
  *) echo "unexpected path: ${path}" >&2; exit 9 ;;
esac
EOF
chmod +x "${fake_gh}"
export PRUNE_CACHES_GH="${fake_gh}"
export FIXTURE_DIR="${tmp_dir}"
export GH_REPO="owner/repo"
# 2026-09-22T12:00:00Z
export PRUNE_CACHES_NOW=1790078400

cat > "${tmp_dir}/caches.json" <<'EOF'
{"actions_caches":[
  {"id":1,"ref":"refs/pull/10/merge","size_in_bytes":100,"last_accessed_at":"2026-09-22T11:00:00.123Z"},
  {"id":2,"ref":"refs/pull/10/merge","size_in_bytes":200,"last_accessed_at":"2026-09-22T11:30:00Z"},
  {"id":3,"ref":"refs/pull/11/merge","size_in_bytes":400,"last_accessed_at":"2026-09-22T11:00:00Z"},
  {"id":4,"ref":"refs/heads/main","size_in_bytes":800,"last_accessed_at":"2026-09-01T00:00:00Z"},
  {"id":5,"ref":"refs/tags/v0.10.0","size_in_bytes":1600,"last_accessed_at":"2026-09-20T00:00:00.5Z"},
  {"id":6,"ref":"refs/tags/v0.10.1","size_in_bytes":3200,"last_accessed_at":"2026-09-22T10:00:00Z"}
]}
EOF
echo '{"10":{"state":"closed"},"11":{"state":"open"}}' > "${tmp_dir}/pulls.json"

deleted_ids() {
  if [[ -f "${tmp_dir}/deleted" ]]; then
    sed 's#.*/##' "${tmp_dir}/deleted" | sort -n | tr '\n' ' '
  fi
  rm -f "${tmp_dir}/deleted"
}

fail() { echo "FAIL: $*" >&2; exit 1; }

# 1. --ref deletes exactly that ref's caches.
"${script}" --ref refs/pull/10/merge > "${tmp_dir}/out"
[[ "$(deleted_ids)" == "1 2 " ]] || fail "--ref pull/10 deleted the wrong set"
grep -q "deleted 2 caches, 300 bytes" "${tmp_dir}/out" || fail "--ref summary"

# 2. --sweep: closed PR 10 and the day-old tag go; open PR 11, main and the
#    fresh tag stay.
"${script}" --sweep > "${tmp_dir}/out"
[[ "$(deleted_ids)" == "1 2 5 " ]] || fail "--sweep deleted the wrong set"

# 3. --dry-run deletes nothing but reports the same set.
"${script}" --dry-run --sweep > "${tmp_dir}/out"
[[ -z "$(deleted_ids)" ]] || fail "--dry-run deleted caches"
grep -q "would delete 3 caches, 1900 bytes" "${tmp_dir}/out" || fail "--dry-run summary"

# 4. Branch refs are refused outright.
if "${script}" --ref refs/heads/main > "${tmp_dir}/out" 2>&1; then
  fail "a branch ref was accepted"
fi
[[ -z "$(deleted_ids)" ]] || fail "a refused ref still deleted caches"

# 5. A tag ref deletes only that tag.
"${script}" --ref refs/tags/v0.10.1 > "${tmp_dir}/out"
[[ "$(deleted_ids)" == "6 " ]] || fail "--ref tag deleted the wrong set"

echo "prune-actions-caches tests passed"
