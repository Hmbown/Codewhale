#!/bin/sh
# Exercise the real boundary using only synthetic homes and a fake toolchain.
# Quoted child programs and the literal argv probe expand only in the child.
# shellcheck disable=SC2016
set -eu
repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hermetic-home-proof.XXXXXX")
trap 'rm -rf -- "$fixture"' EXIT

mkdir -p "$fixture/bin" "$fixture/toolchain" "$fixture/tmp with spaces"
mkdir -p "$fixture/outer/home/.codewhale/fleets" "$fixture/cargo" "$fixture/rustup"
printf '%s\n' 'My fleet' > "$fixture/outer/home/.codewhale/fleets/selected"
printf '%s\n' '[invalid fixture' > "$fixture/outer/home/.codewhale/fleets/my-fleet.toml"
printf '%s\n' '#!/bin/sh' 'printf "%s\n" "$TEST_TOOLCHAIN/rustc"' > "$fixture/bin/rustup"
chmod +x "$fixture/bin/rustup"
fixture_stack=20971520

run_fixture() {
  env -i HOME="$fixture/outer/home" \
    CODEWHALE_HOME="$fixture/outer/home/.codewhale" \
    CODEWHALE_CONFIG_PATH="$fixture/outer/poison.toml" \
    DEEPSEEK_CONFIG_PATH="$fixture/outer/legacy-poison.toml" \
    DEEPSEEK_HOME="$fixture/outer/legacy-home" \
    CODEX_HOME="$fixture/outer/codex" \
    OPENAI_API_KEY=synthetic-outer-key \
    RUST_MIN_STACK="$fixture_stack" \
    CARGO_HOME="$fixture/cargo" RUSTUP_HOME="$fixture/rustup" \
    TEST_FIXTURE="$fixture" TEST_TOOLCHAIN="$fixture/toolchain" \
    TMPDIR="$fixture/tmp with spaces" \
    CODEWHALE_DEV_CACHE_QUIET=1 CODEWHALE_SCCACHE=0 \
    PATH="$fixture/bin:/usr/bin:/bin" "$@"
}

run_isolated() {
  run_fixture "$repo_root/scripts/with-hermetic-test-home.sh" "$@"
}

run_isolated sh -c '
  set -eu
  test "$HOME" != "$1/outer/home"
  test "$USERPROFILE" = "$HOME"
  test -d "$HOME/.codewhale"
  test -d "$HOME/AppData/Roaming"
  test -d "$HOME/AppData/Local"
  test "$APPDATA" = "$HOME/AppData/Roaming"
  test "$LOCALAPPDATA" = "$HOME/AppData/Local"
  test ! -e "$HOME/.codewhale/fleets/selected"
  test -z "${CODEWHALE_HOME+x}"
  test -z "${CODEWHALE_CONFIG_PATH+x}"
  test -z "${DEEPSEEK_CONFIG_PATH+x}"
  test -z "${DEEPSEEK_HOME+x}"
  test "$CODEX_HOME" != "$1/outer/codex"
  test -d "$XDG_CONFIG_HOME"
  test -d "$CODEX_HOME"
  test -z "$OPENAI_API_KEY"
  test "$RUST_MIN_STACK" = 20971520
  test "$CARGO_HOME" = "$1/cargo"
  test "$RUSTUP_HOME" = "$1/rustup"
  case "$PATH" in "$1/toolchain:"*) ;; *) exit 1 ;; esac
  test "$2" = '\''one argument $(not run)'\''
  printf "%s\n" "$HOME" > "$1/child-home"
' sh "$fixture" 'one argument $(not run)'
child_home=$(cat "$fixture/child-home")
test ! -d "${child_home%/*}"
test "$(cat "$fixture/outer/home/.codewhale/fleets/selected")" = 'My fleet'
test "$(cat "$fixture/outer/home/.codewhale/fleets/my-fleet.toml")" = '[invalid fixture'
printf '%s\n' 'ok 1 - isolated homes, credentials, argv, toolchain and outer state'

status=0
fixture_stack=
run_isolated sh -c 'set -e; test "$RUST_MIN_STACK" = 16777216; printf "%s\n" "$HOME" > "$1/failing-home"; exit 37' sh "$fixture" || status=$?
test "$status" -eq 37
child_home=$(cat "$fixture/failing-home")
test ! -d "${child_home%/*}"
test -f "$fixture/outer/home/.codewhale/fleets/selected"
printf '%s\n' 'ok 2 - child failure status and owned-home cleanup'

status=0
run_isolated > "$fixture/usage" 2>&1 || status=$?
test "$status" -eq 2
printf '%s\n' 'ok 3 - missing command is rejected'

# Exercise the actual developer entry point without invoking a Rust tool.
# The cache remains outside the disposable HOME, including Cargo's literal
# build-dir template and --config argument needed for template expansion.
cat > "$fixture/toolchain/cargo" <<'EOF'
#!/bin/sh
set -eu
if [ "${1:-}" = --version ]; then
  printf '%s\n' 'cargo 1.97.0 (synthetic)'
  exit 0
fi
test "$HOME" != "$TEST_FIXTURE/outer/home" || {
  printf '%s\n' 'dev-test left ambient HOME visible' >&2
  exit 1
}
test "$USERPROFILE" = "$HOME"
test "$APPDATA" = "$HOME/AppData/Roaming"
test "$LOCALAPPDATA" = "$HOME/AppData/Local"
test -d "$APPDATA"
test -d "$LOCALAPPDATA"
test ! -e "$HOME/.codewhale/fleets/selected"
test -z "${CODEWHALE_HOME+x}"
test -z "${CODEWHALE_CONFIG_PATH+x}"
test -z "${DEEPSEEK_CONFIG_PATH+x}"
test -z "${DEEPSEEK_HOME+x}"
test -z "$OPENAI_API_KEY"
test "$CARGO_HOME" = "$TEST_FIXTURE/cargo"
test "$RUSTUP_HOME" = "$TEST_FIXTURE/rustup"
test "$RUST_MIN_STACK" = 16777216
test "$CARGO_BUILD_BUILD_DIR" = "$TEST_FIXTURE/outer/home/.cache/codewhale/build/{workspace-path-hash}"
test "$CARGO_BUILD_BUILD_DIR" = "$CODEWHALE_CACHE_ROOT/build/{workspace-path-hash}"
printf '%s\n' "$HOME" > "$TEST_FIXTURE/dev-home"
printf '%s\n' "$@" > "$TEST_FIXTURE/dev-argv"
# A real libtest run prints this; dev-test.sh refuses a filtered green without
# it. TEST_CARGO_SILENT drops it so the refusal itself can be exercised.
if [ -z "${TEST_CARGO_SILENT:-}" ]; then
  printf '%s\n' 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s'
fi
exit "${TEST_CARGO_STATUS:-0}"
EOF
printf '%s\n' '#!/bin/sh' 'printf "%s\n" "commit-hash: synthetic"' > "$fixture/toolchain/rustc"
printf '%s\n' '#!/bin/sh' 'exit 0' > "$fixture/bin/cargo-nextest"
chmod +x "$fixture/toolchain/cargo" "$fixture/toolchain/rustc" "$fixture/bin/cargo-nextest"
ln -s "$fixture/toolchain/cargo" "$fixture/bin/cargo"
ln -s "$fixture/toolchain/rustc" "$fixture/bin/rustc"

count=3
for runner in 0 1; do
  for area in config tui-integration; do
    run_fixture env CODEWHALE_DEV_NEXTEST="$runner" \
      "$repo_root/scripts/dev-test.sh" "$area" 'one argument $(not run)' > "$fixture/dev-output"
    {
      printf '%s\n' --config "build.build-dir = \"$fixture/outer/home/.cache/codewhale/build/{workspace-path-hash}\""
      if [ "$runner" -eq 1 ]; then
        printf '%s\n' nextest run
      else
        printf '%s\n' test
      fi
      if [ "$area" = config ]; then
        printf '%s\n' -p codewhale-config --lib
      else
        printf '%s\n' -p codewhale-tui --test integration
      fi
      printf '%s\n' --locked 'one argument $(not run)'
    } > "$fixture/expected-argv"
    cmp "$fixture/expected-argv" "$fixture/dev-argv"
    child_home=$(cat "$fixture/dev-home")
    test ! -d "${child_home%/*}"
    test -d "$fixture/outer/home/.cache/codewhale/build"
    count=$((count + 1))
    printf 'ok %s - dev-test runner=%s area=%s isolates config and preserves persistent cache and argv\n' "$count" "$runner" "$area"
  done
done

status=0
run_fixture env CODEWHALE_DEV_NEXTEST=0 TEST_CARGO_STATUS=37 \
  "$repo_root/scripts/dev-test.sh" config > "$fixture/dev-output" || status=$?
test "$status" -eq 37
child_home=$(cat "$fixture/dev-home")
test ! -d "${child_home%/*}"
test "$(cat "$fixture/outer/home/.codewhale/fleets/selected")" = 'My fleet'
printf '%s\n' 'ok 8 - dev-test preserves failure status and cleans only its temporary home'

# Exercise Windows cygpath path normalization, backslash toolchain resolution,
# and AppData provisioning required by sccache and Windows known-folder lookups.
mkdir -p "$fixture/win-bin" "$fixture/win-toolchain/bin"
cat > "$fixture/win-bin/cygpath" <<'EOF'
#!/bin/sh
set -eu
case "${1:-}" in
  -m|-u) shift ;;
  *) exit 2 ;;
esac
printf '%s\n' "$1" | tr '\\' '/'
EOF
cat > "$fixture/win-bin/rustup" <<'EOF'
#!/bin/sh
set -eu
printf '%s\n' "$TEST_TOOLCHAIN\\bin\\rustc.exe"
EOF
chmod +x "$fixture/win-bin/cygpath" "$fixture/win-bin/rustup"

run_win_fixture() {
  env -i HOME="$fixture/outer/home" \
    USERPROFILE="$fixture/outer/home" \
    APPDATA="$fixture/outer/home/AppData/Roaming" \
    LOCALAPPDATA="$fixture/outer/home/AppData/Local" \
    CARGO_HOME="$fixture/cargo" RUSTUP_HOME="$fixture/rustup" \
    TEST_FIXTURE="$fixture" TEST_TOOLCHAIN="$fixture/win-toolchain" \
    TMPDIR="$fixture/tmp with spaces" \
    PATH="$fixture/win-bin:/usr/bin:/bin" "$@"
}

run_win_fixture "$repo_root/scripts/with-hermetic-test-home.sh" sh -c '
  set -eu
  test "$HOME" != "$1/outer/home"
  test "$USERPROFILE" = "$HOME"
  test "$APPDATA" = "$HOME/AppData/Roaming"
  test "$LOCALAPPDATA" = "$HOME/AppData/Local"
  test -d "$HOME/AppData/Roaming"
  test -d "$HOME/AppData/Local"
  test -d "$HOME/.codewhale"
  test "$APPDATA" != "$1/outer/home/AppData/Roaming"
  test "$LOCALAPPDATA" != "$1/outer/home/AppData/Local"
  # Verify toolchain_bin stripped the backslash executable properly
  case "$PATH" in "$1/win-toolchain/bin:"*) ;; *) exit 1 ;; esac
  # Verify sccache config-dir resolution target (RoamingAppData/Mozilla/sccache)
  sccache_cfg_parent="$APPDATA/Mozilla/sccache"
  mkdir -p "$sccache_cfg_parent"
  printf "%s\n" "test-config = true" > "$sccache_cfg_parent/config"
  test -f "$HOME/AppData/Roaming/Mozilla/sccache/config"
  test ! -e "$1/outer/home/AppData/Roaming/Mozilla/sccache/config"
  printf "%s\n" "$HOME" > "$1/win-child-home"
' sh "$fixture"
win_child_home=$(cat "$fixture/win-child-home")
test ! -d "${win_child_home%/*}"
printf '%s\n' 'ok 9 - cygpath path normalization, backslash toolchain resolution and hermetic AppData'

# The guard 8b6dad20e introduced: libtest exits 0 when a filter matches
# nothing, which has been mistaken for a pass here before. With an explicit
# filter and no `test result:` line, dev-test.sh must refuse that green.
status=0
run_fixture env CODEWHALE_DEV_NEXTEST=0 TEST_CARGO_SILENT=1 \
  "$repo_root/scripts/dev-test.sh" config 'filter $(matching nothing)' \
  > "$fixture/dev-output" 2>&1 || status=$?
test "$status" -ne 0
grep -q 'refusing green' "$fixture/dev-output"
# nextest fails loud on an empty selection, so the guard must not wrap it.
run_fixture env CODEWHALE_DEV_NEXTEST=1 TEST_CARGO_SILENT=1 \
  "$repo_root/scripts/dev-test.sh" config 'filter $(matching nothing)' \
  > "$fixture/dev-output" 2>&1
printf '%s\n' 'ok 10 - a filtered libtest run with no test result refuses green'

printf '%s\n' 'test result: 10 passed; 0 failed'
