#!/usr/bin/env python3
"""Run one command while holding this machine's Codewhale build lock.

    scripts/build-lock.py <lock-file> -- <command> [args...]

Several agents share one checkout and one memory-constrained machine. Cargo's
own lock is per target directory, so builds into different target dirs still
run concurrently and exhaust memory. This holds an exclusive flock on a lock
file under the persistent cache root for the lifetime of the command, and
says who holds it while waiting.

The command runs as a child, never via exec: a daemon it spawns (an sccache
server, say) must not inherit the descriptor and pin the lock after the build
exits. Nested invocations see CODEWHALE_BUILD_LOCK_HELD and run unlocked.

Known limitation: advisory only. Cargo started outside scripts/dev-cargo.sh
does not take the lock, and on a platform without fcntl (Windows) the
command runs unlocked after a warning.
"""

import os
import signal
import subprocess
import sys
import time


def main() -> int:
    if len(sys.argv) < 4 or sys.argv[2] != "--":
        print("usage: build-lock.py <lock-file> -- <command> [args...]", file=sys.stderr)
        return 2
    lock_path, command = sys.argv[1], sys.argv[3:]
    env = dict(os.environ, CODEWHALE_BUILD_LOCK_HELD="1")
    try:
        import fcntl
    except ImportError:
        print("build-lock: fcntl unavailable; running without the machine build lock", file=sys.stderr)
        return subprocess.call(command, env=env)

    os.makedirs(os.path.dirname(lock_path) or ".", exist_ok=True)
    fd = os.open(lock_path, os.O_RDWR | os.O_CREAT, 0o600)
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        holder = os.pread(fd, 512, 0).decode(errors="replace").strip() or "holder unknown"
        print(
            f"build-lock: waiting for another Cargo build on this machine ({holder}); "
            "set CODEWHALE_BUILD_LOCK=0 to skip",
            file=sys.stderr,
            flush=True,
        )
        started = time.monotonic()
        fcntl.flock(fd, fcntl.LOCK_EX)
        print(
            f"build-lock: acquired after {time.monotonic() - started:.0f}s",
            file=sys.stderr,
            flush=True,
        )
    os.ftruncate(fd, 0)
    summary = " ".join(command[:5])
    os.pwrite(fd, f"pid {os.getpid()} in {os.getcwd()}: {summary}\n".encode(), 0)

    child = subprocess.Popen(command, env=env)
    for forwarded in (signal.SIGTERM, signal.SIGHUP):
        signal.signal(forwarded, lambda signum, _frame: child.send_signal(signum))
    # SIGINT reaches the whole foreground process group already; only keep
    # waiting so the lock outlives the child's own shutdown.
    signal.signal(signal.SIGINT, signal.SIG_IGN)
    status = child.wait()
    return 128 - status if status < 0 else status


if __name__ == "__main__":
    sys.exit(main())
