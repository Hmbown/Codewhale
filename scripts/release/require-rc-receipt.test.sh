#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
script="${repo_root}/scripts/release/require-rc-receipt.sh"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT

sha="0123456789abcdef0123456789abcdef01234567"

# Fake gh: answers the two API calls from fixture files and applies the
# caller's --jq filter with the real jq, so the filters themselves are tested.
fake_gh="${tmp_dir}/gh"
cat > "${fake_gh}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[[ "$1" == "api" ]] || { echo "unexpected: $*" >&2; exit 9; }
path="$2"
filter="$4"
case "${path}" in
  */workflows/release-candidate.yml/runs\?*) fixture="${FIXTURE_DIR}/runs.json" ;;
  */actions/runs/*/jobs\?*)
    run_id="${path#*/actions/runs/}"
    run_id="${run_id%%/*}"
    fixture="${FIXTURE_DIR}/jobs-${run_id}.json"
    ;;
  *) echo "unexpected path: ${path}" >&2; exit 9 ;;
esac
jq -r "${filter}" "${fixture}"
EOF
chmod +x "${fake_gh}"
export RC_RECEIPT_GH="${fake_gh}"
export FIXTURE_DIR="${tmp_dir}"

expect_pass() {
  local label="$1"
  if ! "${script}" owner/repo "${sha}" >"${tmp_dir}/out" 2>&1; then
    echo "FAIL (${label}): expected a receipt" >&2
    cat "${tmp_dir}/out" >&2
    exit 1
  fi
}
expect_fail() {
  local label="$1"
  if "${script}" owner/repo "${2:-${sha}}" >"${tmp_dir}/out" 2>&1; then
    echo "FAIL (${label}): expected refusal" >&2
    cat "${tmp_dir}/out" >&2
    exit 1
  fi
}

# 1. No RC run at all: refuse.
echo '{"workflow_runs":[]}' > "${tmp_dir}/runs.json"
expect_fail "no runs"
grep -q "No green release-candidate run" "${tmp_dir}/out"

# 2. Green RC run whose Parity job was skipped: refuse.
cat > "${tmp_dir}/runs.json" <<EOF
{"workflow_runs":[{"id":11,"head_sha":"${sha}","conclusion":"success","event":"workflow_dispatch","html_url":"https://example.invalid/11"}]}
EOF
echo '{"jobs":[{"name":"Parity / Workspace parity","conclusion":"skipped"},{"name":"Verify exact candidate web surface","conclusion":"success"}]}' > "${tmp_dir}/jobs-11.json"
expect_fail "parity skipped"

# 3. A run for a different SHA never counts, even if the API returned it.
cat > "${tmp_dir}/runs.json" <<'EOF'
{"workflow_runs":[{"id":12,"head_sha":"ffffffffffffffffffffffffffffffffffffffff","conclusion":"success","event":"workflow_dispatch","html_url":"https://example.invalid/12"}]}
EOF
echo '{"jobs":[{"name":"Parity / Workspace parity","conclusion":"success"}]}' > "${tmp_dir}/jobs-12.json"
expect_fail "other sha"

# 4. Green RC run with green Parity on the exact SHA: receipt.
cat > "${tmp_dir}/runs.json" <<EOF
{"workflow_runs":[
  {"id":11,"head_sha":"${sha}","conclusion":"success","event":"workflow_dispatch","html_url":"https://example.invalid/11"},
  {"id":13,"head_sha":"${sha}","conclusion":"success","event":"workflow_dispatch","html_url":"https://example.invalid/13"}
]}
EOF
echo '{"jobs":[{"name":"Parity / Workspace parity","conclusion":"success"}]}' > "${tmp_dir}/jobs-13.json"
expect_pass "green parity"
grep -q "https://example.invalid/13 (Parity green)" "${tmp_dir}/out"

# 5. Malformed SHA is rejected before any API call.
expect_fail "short sha" "abc123"
expect_fail "uppercase sha" "0123456789ABCDEF0123456789ABCDEF01234567"

echo "require-rc-receipt tests passed"
