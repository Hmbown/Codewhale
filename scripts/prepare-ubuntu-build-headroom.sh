#!/usr/bin/env bash
# Fixed provisioning for ephemeral GitHub-hosted Ubuntu jobs only.
# Earlier Safety/Lint exit 143 and runner shutdown do not prove OOM or timeout.
# Keep the existing SDK removal, swap size, warning fallback, and diagnostics.
set -euo pipefail

if [ "${GITHUB_ACTIONS:-}" != true ] || [ "${RUNNER_OS:-}" != Linux ] \
  || [ "${RUNNER_ENVIRONMENT:-}" != github-hosted ]; then
  echo "Refusing build-headroom cleanup outside GitHub-hosted Linux CI" >&2
  exit 1
fi

df -h / /mnt; free -m
sudo rm -rf /usr/share/dotnet /usr/local/lib/android /opt/ghc /usr/local/.ghcup /opt/hostedtoolcache/CodeQL
if sudo fallocate -l 8G /mnt/cw-swapfile && sudo chmod 600 /mnt/cw-swapfile \
  && sudo mkswap /mnt/cw-swapfile >/dev/null && sudo swapon /mnt/cw-swapfile; then
  echo "Added 8 GiB swap at /mnt/cw-swapfile"
else
  echo "::warning::Could not add swap; continuing with the runner default"
fi
df -h / /mnt; free -m
