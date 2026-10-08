#!/bin/sh
# Join the container's desktop session, then run the remote agent.
#
# `docker exec` starts with a fresh environment: it does not inherit the
# DISPLAY export or the dbus-run-session address the entrypoint created, and
# without the same session bus an exec'd agent cannot see the accessibility
# tree of apps already on the display. The entrypoint writes both to
# /tmp/cu-session.env; source it, then hand off.
set -eu

ENV_FILE=/tmp/cu-session.env
if [ -f "$ENV_FILE" ]; then
  # Sourced assignments are not exported by default; allexport marks them so
  # the agent process (and the backend tools it runs) inherit display and bus.
  set -a
  # shellcheck disable=SC1090
  . "$ENV_FILE"
  set +a
else
  export DISPLAY="${DISPLAY:-:0}"
fi

exec node /app/agent.mjs "$@"
