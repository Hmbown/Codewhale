#!/bin/sh
# Shared test-home boundary for local and CI workspace tests.
# Rust toolchain homes stay real.
set -eu

if [ "$#" -eq 0 ]; then
  printf '%s\n' 'usage: with-hermetic-test-home.sh command [args...]' >&2
  exit 2
fi

real_cargo_home=${CARGO_HOME:-${HOME}/.cargo}
real_rustup_home=${RUSTUP_HOME:-${HOME}/.rustup}
# Git Bash hands MSYS paths to native Windows processes, which cannot
# resolve them. `-m` keeps forward slashes, so MSYS tools still work.
if command -v cygpath >/dev/null 2>&1; then
  real_cargo_home=$(cygpath -m "$real_cargo_home")
  real_rustup_home=$(cygpath -m "$real_rustup_home")
fi

rustc_bin=$(RUSTUP_HOME="$real_rustup_home" rustup which rustc)
if command -v cygpath >/dev/null 2>&1; then
  # PATH is a POSIX list in Git Bash: a C:/ drive prefix would add a
  # spurious separator. Only exported native home values use mixed paths.
  rustc_bin=$(cygpath -u "$rustc_bin")
fi
rustc_bin_norm=$(printf '%s' "$rustc_bin" | tr "\\\\" '/')
toolchain_bin=${rustc_bin_norm%/*}

test_home_root=$(mktemp -d "${TMPDIR:-/tmp}/codewhale-test-home.XXXXXX")
test_home_root_raw=$test_home_root
if command -v cygpath >/dev/null 2>&1; then
  test_home_root=$(cygpath -m "$test_home_root")
fi

# Materialized builtin plugins leave read-only runtime trees behind; make the
# tree writable first so cleanup never masks the command's own exit status.
trap 'chmod -R u+w -- "$test_home_root_raw" 2>/dev/null || true; rm -rf -- "$test_home_root_raw" 2>/dev/null || true' EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

mkdir -p "$test_home_root/home/.codewhale" "$test_home_root/xdg"
# Windows tools and known-folder resolvers (e.g. sccache / directories crate)
# expand %USERPROFILE%\AppData\Roaming and Local, and SHGetKnownFolderPath
# verifies directory existence before returning success.
mkdir -p "$test_home_root/home/AppData/Roaming" "$test_home_root/home/AppData/Local"
mkdir -p "$test_home_root/codex" "$test_home_root/grok" "$test_home_root/kimi-code"
mkdir -p "$test_home_root/kimi-share" "$test_home_root/claude"

# Use the isolated HOME default, while allowing each test to choose its own
# home or config fixture. Canonical overrides would shadow legacy fixtures.
unset CODEWHALE_HOME CODEWHALE_CONFIG_PATH DEEPSEEK_CONFIG_PATH DEEPSEEK_HOME

env \
  HOME="$test_home_root/home" \
  USERPROFILE="$test_home_root/home" \
  APPDATA="$test_home_root/home/AppData/Roaming" \
  LOCALAPPDATA="$test_home_root/home/AppData/Local" \
  XDG_CONFIG_HOME="$test_home_root/xdg" \
  CODEX_HOME="$test_home_root/codex" \
  GROK_HOME="$test_home_root/grok" \
  GROK_AUTH_PATH="$test_home_root/grok/auth.json" \
  KIMI_CODE_HOME="$test_home_root/kimi-code" \
  KIMI_SHARE_DIR="$test_home_root/kimi-share" \
  CLAUDE_CONFIG_DIR="$test_home_root/claude" \
  DEEPSEEK_API_KEY= \
  OPENAI_API_KEY= \
  ANTHROPIC_API_KEY= \
  XAI_API_KEY= \
  GROK_API_KEY= \
  MOONSHOT_API_KEY= \
  KIMI_API_KEY= \
  XIAOMI_MIMO_API_KEY= \
  XIAOMI_MIMO_TOKEN_PLAN_API_KEY= \
  MIMO_API_KEY= \
  MIMO_TOKEN_PLAN_API_KEY= \
  RUST_MIN_STACK="${RUST_MIN_STACK:-16777216}" \
  CARGO_HOME="$real_cargo_home" \
  RUSTUP_HOME="$real_rustup_home" \
  PATH="$toolchain_bin:$PATH" \
  "$@"
