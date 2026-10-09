#!/usr/bin/env bash
# Install apt packages on a GitHub-hosted Ubuntu runner with every wait bounded.
#
# `apt-get update` has no overall deadline. When the regional Azure mirror was
# unreachable from one runner it sat silent until the 30-minute job limit
# cancelled "Mobile runtime smoke" twice on the same commit (runs 37673026751
# and 37673063356), while sibling jobs on other runners finished the same step
# in under a minute. A stall is now a failed attempt, the unreachable mirror is
# dropped in favour of the fallbacks the image already lists, and a package
# that still cannot be installed fails the step with apt's own error.
set -euo pipefail

if [ "$#" -eq 0 ]; then
  echo "usage: ci-apt-install.sh PACKAGE..." >&2
  exit 2
fi

mirrors=${CI_APT_MIRRORLIST:-/etc/apt/apt-mirrors.txt}
update_seconds=${CI_APT_UPDATE_SECONDS:-120}
install_seconds=${CI_APT_INSTALL_SECONDS:-600}
retry_sleep=${CI_APT_RETRY_SLEEP:-15}
apt_options=(
  -o Acquire::http::Timeout=30
  -o Acquire::https::Timeout=30
  -o Acquire::Retries=2
  -o DPkg::Lock::Timeout=120
)

updated=0
for attempt in 1 2 3 4 5; do
  if sudo timeout --kill-after=10 "$update_seconds" apt-get "${apt_options[@]}" update; then
    updated=1
    break
  fi
  echo "apt-get update failed or stalled (attempt $attempt of 5)"
  if [ -f "$mirrors" ] && grep -q 'azure\.archive\.ubuntu\.com' "$mirrors"; then
    kept=$(grep -v 'azure\.archive\.ubuntu\.com' "$mirrors" || true)
    if printf '%s\n' "$kept" | grep -Eq '^https?://'; then
      echo "Dropping the Azure mirror from $mirrors; remaining mirrors:"
      printf '%s\n' "$kept" | sudo tee "$mirrors"
    fi
  fi
  sleep "$retry_sleep"
done
if [ "$updated" != 1 ]; then
  echo "::warning::apt-get update never completed; installing from the package lists already on the image"
fi
sudo timeout --kill-after=10 "$install_seconds" apt-get "${apt_options[@]}" install -y "$@"
