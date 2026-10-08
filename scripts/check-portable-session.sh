#!/bin/sh
# Compile the complete actual group; separate processes run strictly sequentially.
set -eu
cd "$(dirname "$0")/.."
graph=$(mktemp)
trap 'rm -f "$graph"' EXIT HUP INT TERM
cargo tree --locked -p codewhale-portable-session --edges normal --target all --prefix none > "$graph"
cat "$graph"
python3 scripts/check-command-session-proof.py --graph "$graph"
cargo check --locked -p codewhale-portable-session --lib --no-default-features
sh scripts/with-hermetic-test-home.sh cargo nextest run -p codewhale-portable-session --lib --all-features --locked --profile ci --no-tests=fail
