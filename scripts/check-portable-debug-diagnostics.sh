#!/bin/sh
# FEAT-029: compile the complete portable debug closure without TUI.
set -eu
cd "$(dirname "$0")/.."

# Normal graph (not a source-token approximation). A forbidden transitive
# dependency fails even if the compiler would otherwise accept the facade.
graph=$(cargo tree --locked -p codewhale-commands --edges normal --prefix none)
printf '%s\n' "$graph"
if printf '%s\n' "$graph" | grep -Eq '^codewhale-tui([[:space:]]|$)'; then
    echo '[portable-debug-diagnostics] FAIL: TUI is a normal dependency' >&2
    exit 1
fi
printf '%s\n' '[portable-debug-diagnostics] normal dependency graph has no codewhale-tui'

# The production crate owns the complete group and its diagnostics helpers.
# It uses actual contract-owned result/action shapes.
cargo check --locked -p codewhale-commands --lib --no-default-features
sh scripts/with-hermetic-test-home.sh cargo test -p codewhale-commands --lib --locked -- --format=pretty
