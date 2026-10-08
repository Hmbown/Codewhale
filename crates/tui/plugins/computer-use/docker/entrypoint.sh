#!/bin/sh
# Stand up the parts a Linux login session would have provided, then hand off.
#
# Display :0 is the "host" desktop. The parity engine samples it through
# hostProbe() to measure whether a run disturbed the user's session, and the
# isolated route's claim ("0px, no active-window change") is only worth
# anything if something real is being sampled — a missing :0 would report
# zeroes because the probe failed, not because nothing moved. The isolated
# route's own Xvfb :99 is started by the driver, not here.
set -eu

HOST_DISPLAY="${CU_HOST_DISPLAY:-:0}"
HOST_GEOMETRY="${CU_HOST_GEOMETRY:-1600x1200x24}"

# Reap the display before PID 1 exits, so a normal container restart does
# not inherit an Xvfb lock for the previous container's process IDs.
xvfb_pid= wm_pid= session_pid=
cleanup() {
  trap - EXIT INT TERM
  for child_pid in $session_pid $wm_pid $xvfb_pid; do
    kill -TERM "$child_pid" 2>/dev/null || true
  done
  for child_pid in $session_pid $wm_pid $xvfb_pid; do
    wait "$child_pid" 2>/dev/null || true
  done
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

Xvfb "$HOST_DISPLAY" -screen 0 "$HOST_GEOMETRY" -nolisten tcp >/tmp/xvfb-host.log 2>&1 &
xvfb_pid=$!
i=0
until DISPLAY="$HOST_DISPLAY" xdotool getdisplaygeometry >/dev/null 2>&1; do
  i=$((i + 1))
  if [ "$i" -gt 60 ]; then
    echo "entrypoint: Xvfb $HOST_DISPLAY did not come up" >&2
    cat /tmp/xvfb-host.log >&2 || true
    exit 1
  fi
  sleep 0.25
done

DISPLAY="$HOST_DISPLAY" openbox >/tmp/openbox-host.log 2>&1 &
wm_pid=$!
sleep 0.5

export DISPLAY="$HOST_DISPLAY"

# AT-SPI rides the session bus, and at-spi-bus-launcher is activated from it on
# demand. Without a session bus every semantic tool fails at the accessibility
# tree, so wrap the whole command rather than starting a daemon and hoping the
# address is inherited. The inner sh also records the session env for
# docker/agent-exec.sh, so `docker exec`'d agents join this same display+bus
# instead of starting blind.
dbus-run-session -- sh -c 'printf "DISPLAY=%s\nDBUS_SESSION_BUS_ADDRESS=%s\n" "$DISPLAY" "$DBUS_SESSION_BUS_ADDRESS" > /tmp/cu-session.env; exec "$@"' sh "$@" &
session_pid=$!
wait "$session_pid"
