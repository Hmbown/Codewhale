#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT

fixture="${tmp_dir}/repo"
src_dir="${fixture}/target/release"
dest_dir="${tmp_dir}/installed"
receipt_dir="${tmp_dir}/receipts"
fake_bin="${tmp_dir}/bin"
marker="${tmp_dir}/invocations.log"

mkdir -p "${fixture}/scripts/release" "${src_dir}" "${dest_dir}" "${fake_bin}"
cp "${repo_root}/scripts/release/install-dogfood.sh" \
  "${fixture}/scripts/release/install-dogfood.sh"
printf 'target/\n' >"${fixture}/.gitignore"

git -C "${fixture}" init --quiet
git -C "${fixture}" config user.name "Dogfood Test"
git -C "${fixture}" config user.email "dogfood-test@example.invalid"
git -C "${fixture}" add .gitignore scripts/release/install-dogfood.sh
git -C "${fixture}" -c commit.gpgsign=false commit --quiet -m "fixture"
source_sha="$(git -C "${fixture}" rev-parse HEAD)"

make_binary() {
  cat >"${src_dir}/codewhale" <<EOF
#!/usr/bin/env bash
set -euo pipefail
if [[ "\${1:-}" == "--version" ]]; then
  printf '%s\n' 'codewhale 0.9.1 (${source_sha})'
  printf '%s\n' "\${0##*/}" >>"\${DOGFOOD_TEST_MARKER}"
  exit 0
fi
exit 2
EOF
  chmod +x "${src_dir}/codewhale"
}

make_binary

# Reproduce the old dogfood state: codew was a symlink to the dispatcher.
printf 'old dispatcher\n' >"${dest_dir}/codewhale"
ln -s "${dest_dir}/codewhale" "${dest_dir}/codew"

cat >"${fake_bin}/zsh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[[ "${1:-}" == "-lc" ]]
case "${2:-}" in
  "command -v codewhale") printf '%s\n' "${DOGFOOD_TEST_DEST}/codewhale" ;;
  "command -v codew") printf '%s\n' "${DOGFOOD_TEST_DEST}/codew" ;;
  "codewhale --version") exec "${DOGFOOD_TEST_DEST}/codewhale" --version ;;
  "codew --version") exec "${DOGFOOD_TEST_DEST}/codew" --version ;;
  *) exit 2 ;;
esac
EOF
chmod +x "${fake_bin}/zsh"

HOME="${tmp_dir}/home" \
PATH="${fake_bin}:${PATH}" \
DOGFOOD_TEST_DEST="${dest_dir}" \
DOGFOOD_TEST_MARKER="${marker}" \
CODEWHALE_INSTALL_DIRS="${dest_dir}" \
CODEWHALE_DOGFOOD_RECEIPT_DIR="${receipt_dir}" \
  "${fixture}/scripts/release/install-dogfood.sh" "${src_dir}" >/dev/null

for name in codewhale codew; do
  cmp -s "${src_dir}/codewhale" "${dest_dir}/${name}" || {
    echo "installed ${name} differs from the built fixture" >&2
    exit 1
  }
done

if [[ -L "${dest_dir}/codew" ]]; then
  echo "dogfood install left codew as a symlink" >&2
  exit 1
fi

[[ "$(grep -c '^codew$' "${marker}")" -ge 1 ]] || {
  echo "the installed codew command name was not exercised" >&2
  exit 1
}

receipt="$(find "${receipt_dir}" -type f -name '*.txt' -print -quit)"
[[ -n "${receipt}" ]]
grep -Fq "codew_sha256=" "${receipt}"
grep -Fq "fresh_shell_codew=${dest_dir}/codew" "${receipt}"
grep -Fq "installed_path=${dest_dir}/codew" "${receipt}"

echo "install-dogfood tests passed"

# A shell fixture cannot expose codesign's filename-derived identities. Run
# the actual installer on a native Mach-O and require one signed artifact for
# both command names, as the updater's sibling-ownership guard requires.
if [[ "$(uname -s)" == "Darwin" ]]; then
  cat >"${tmp_dir}/native.c" <<EOF
#include <stdio.h>
#include <string.h>
int main(int argc, char **argv) {
  if (argc == 2 && strcmp(argv[1], "--version") == 0) {
    puts("codewhale 0.9.1 (${source_sha})");
    return 0;
  }
  return 2;
}
EOF
  /usr/bin/cc "${tmp_dir}/native.c" -o "${src_dir}/codewhale"
  PATH="${fake_bin}:${PATH}" \
  DOGFOOD_TEST_DEST="${dest_dir}" \
  DOGFOOD_TEST_MARKER="${marker}" \
  CODEWHALE_INSTALL_DIRS="${dest_dir}" \
  CODEWHALE_DOGFOOD_RECEIPT_DIR="${receipt_dir}" \
    "${fixture}/scripts/release/install-dogfood.sh" "${src_dir}" >/dev/null
  cmp -s "${dest_dir}/codewhale" "${dest_dir}/codew" || {
    echo "native dogfood aliases acquired different code signatures" >&2
    exit 1
  }
  codesign --verify --strict "${dest_dir}/codewhale"
  codesign --verify --strict "${dest_dir}/codew"
  echo "native macOS dogfood identity test passed"
fi
