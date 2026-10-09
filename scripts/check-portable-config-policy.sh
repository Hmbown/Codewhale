#!/bin/sh
# Every Cargo process completes before the next; inspect all-target normal edges.
set -eu
cd "$(dirname "$0")/.."
graph=$(mktemp)
trap 'rm -f "$graph"' EXIT HUP INT TERM
cargo tree --locked -p codewhale-portable-config-policy --edges normal --target all --prefix none > "$graph"
cat "$graph"
python3 -B scripts/check-command-config-policy-proof.py --graph "$graph"
cargo check --locked -p codewhale-portable-config-policy --lib --no-default-features
env CARGO_TERM_COLOR=always CARGO_INCREMENTAL=0 RUSTFLAGS=-Dwarnings RUST_MIN_STACK=16777216 CODEWHALE_EXT_HOST_TESTS=1 sh scripts/with-hermetic-test-home.sh cargo nextest run -p codewhale-portable-config-policy --lib --all-features --locked --profile ci --no-tests=fail
