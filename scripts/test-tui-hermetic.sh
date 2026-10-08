#!/bin/sh
# Run the TUI library tests through the shared isolated test-home boundary.
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
runs=${TUI_HERMETIC_RUNS:-2}
filter=${TUI_HERMETIC_FILTER:-}

"$repo_root/scripts/check-tui-product-vocabulary.sh"

run=1
while [ "$run" -le "$runs" ]; do
  printf '%s\n' "hermetic TUI run $run/$runs"
  (
    cd "$repo_root"
    # The filter and cargo command are expanded by the isolated child shell.
    # shellcheck disable=SC2016
    "$repo_root/scripts/with-hermetic-test-home.sh" \
      sh -c '
        if [ -n "$1" ]; then
          exec cargo test --quiet -p codewhale-tui --lib --locked "$1"
        fi
        exec cargo test --quiet -p codewhale-tui --lib --locked
      ' sh "$filter"
  )
  run=$((run + 1))
done
