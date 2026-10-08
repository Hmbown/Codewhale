#!/usr/bin/env bash
# Start a local Codewhale runtime and the Weixin bridge in one terminal.
#
# Generates a shared CODEWHALE_RUNTIME_TOKEN, starts `codewhale serve --http` in
# the background, waits for it to answer /health, then runs the bridge in the
# foreground. Both processes share the generated token, so no copy/paste.
#
# Ctrl-C stops both. Override defaults with the env vars below:
#   CODEWHALE_RUNTIME_PORT   runtime port            (default 7878)
#   CODEWHALE_RUNTIME_TOKEN  reuse an existing token (default: generated)
#   WEIXIN_ALLOW_UNLISTED    first-pairing mode      (default true)
#   WEIXIN_STATE_DIR         state directory         (default: ./.state)

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bridge_dir="$(cd "$script_dir/.." && pwd)"

port="${CODEWHALE_RUNTIME_PORT:-7878}"
runtime_url="http://127.0.0.1:${port}"

if [[ -z "${CODEWHALE_RUNTIME_TOKEN:-}" ]]; then
  CODEWHALE_RUNTIME_TOKEN="$(openssl rand -hex 32)"
  echo "Generated CODEWHALE_RUNTIME_TOKEN for this session."
fi
export CODEWHALE_RUNTIME_TOKEN

export CODEWHALE_RUNTIME_URL="$runtime_url"
export WEIXIN_ALLOW_UNLISTED="${WEIXIN_ALLOW_UNLISTED:-true}"
export WEIXIN_STATE_DIR="${WEIXIN_STATE_DIR:-$bridge_dir/.state}"

runtime_pid=""

cleanup() {
  if [[ -n "$runtime_pid" ]] && kill -0 "$runtime_pid" 2>/dev/null; then
    echo ""
    echo "Stopping runtime (pid $runtime_pid)..."
    kill "$runtime_pid" 2>/dev/null || true
    wait "$runtime_pid" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

echo "Starting runtime on $runtime_url ..."
codewhale serve --http \
  --host 127.0.0.1 \
  --port "$port" \
  --auth-token "$CODEWHALE_RUNTIME_TOKEN" &
runtime_pid=$!

# Wait for /health before handing over to the bridge, so the first pairing
# message does not race a runtime that has not bound its port yet.
for _ in $(seq 1 60); do
  if curl -fsS "$runtime_url/health" >/dev/null 2>&1; then
    break
  fi
  if ! kill -0 "$runtime_pid" 2>/dev/null; then
    echo "Runtime exited before becoming healthy." >&2
    exit 1
  fi
  sleep 0.5
done

if ! curl -fsS "$runtime_url/health" >/dev/null 2>&1; then
  echo "Runtime did not become healthy at $runtime_url/health within 30s." >&2
  exit 1
fi

echo "Runtime is healthy. Starting Weixin bridge..."
echo "Allow-unlisted (first pairing): $WEIXIN_ALLOW_UNLISTED"
echo "State dir: $WEIXIN_STATE_DIR"
echo ""

cd "$bridge_dir"
node src/index.mjs
