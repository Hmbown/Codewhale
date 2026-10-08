#!/usr/bin/env python3
"""Report where Rust build output lives and how much of it nothing claims.

Read-only by design. This prints an inventory and a reclaim candidate list;
it never deletes, moves, or writes anything. Deleting hundreds of gigabytes
is a human decision, and the classification below is the evidence for it.

Why this exists: agents build far more often than people do, and each
worktree keeps its own build output. `scripts/dev-cache.sh` already isolates
per-workspace build dirs so parallel agents do not share one Cargo lock, but
nothing ever collects those directories when a worktree goes away.

Three places accumulate output, and they are easy to confuse:

  * `<worktree>/target`      — the ordinary Cargo target dir, one per checkout.
  * the isolated build cache — `dev-cache.sh` mode `isolated-build-dir`, keyed
                               by Cargo's `{workspace-path-hash}`, which
                               expands to a TWO-level `XX/YYYYYYYYYYYYYY` path.
                               Counting the first level alone undercounts
                               roots and makes every name look unmatched.
  * `<repo>/target`          — the main checkout's own, usually the largest
                               single object and easily forgotten.

Worktree classification is NOT reimplemented here: `scripts/workspace-status.sh`
already owns which worktrees exist, which are merged, and which are dirty.
Run that for the checkout side of the picture.

    scripts/measure-build-cache.py
    scripts/measure-build-cache.py --json
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]

# The signature every Cargo target dir carries. Used to identify a build root
# rather than guessing from the directory name, which is a hash.
CACHEDIR_SIGNATURE = "Signature: 8a477f597d28d172789f06886806bc55"


def disk_usage_bytes(path: pathlib.Path) -> int:
    """Apparent size via `du -sk`, which is what the operator sees."""
    try:
        out = subprocess.run(
            ["du", "-sk", str(path)],
            capture_output=True,
            text=True,
            check=True,
        ).stdout
    except (subprocess.CalledProcessError, FileNotFoundError):
        return 0
    try:
        return int(out.split("\t", 1)[0]) * 1024
    except (ValueError, IndexError):
        return 0


def gib(n: int) -> float:
    return n / (1024**3)


def cache_root() -> pathlib.Path | None:
    """The isolated build-dir root that `dev-cache.sh` would use.

    Asked of the script itself rather than re-derived, so this cannot drift
    from the thing that actually creates the directories.
    """
    script = ROOT / "scripts" / "dev-cache.sh"
    if not script.exists():
        return None
    try:
        out = subprocess.run(
            ["sh", "-c", f'. "{script}" && codewhale_dev_cache_apply && '
                         'printf "%s" "${CARGO_BUILD_BUILD_DIR:-}"'],
            capture_output=True,
            text=True,
            timeout=60,
        ).stdout.strip()
    except (subprocess.SubprocessError, OSError):
        return None
    if not out:
        return None
    # The value carries Cargo's `{workspace-path-hash}` placeholder; the root
    # is everything above it.
    marker = "{workspace-path-hash}"
    base = out.split(marker, 1)[0] if marker in out else out
    path = pathlib.Path(base.rstrip("/"))
    return path if path.is_dir() else None


def build_roots(base: pathlib.Path) -> list[pathlib.Path]:
    """Every Cargo build root under `base`, found by its CACHEDIR.TAG.

    Depth is not assumed: Cargo's hash expands two levels, and a future
    layout change should not silently return nothing.
    """
    roots: list[pathlib.Path] = []
    for dirpath, dirnames, filenames in os.walk(base):
        if "CACHEDIR.TAG" in filenames:
            tag = pathlib.Path(dirpath) / "CACHEDIR.TAG"
            try:
                first = tag.read_text(errors="replace").splitlines()[:1]
            except OSError:
                first = []
            if first and first[0].startswith(CACHEDIR_SIGNATURE):
                roots.append(pathlib.Path(dirpath))
                dirnames.clear()  # never descend into a build root
    return roots


def live_worktrees(repo: pathlib.Path) -> list[pathlib.Path]:
    try:
        out = subprocess.run(
            ["git", "-C", str(repo), "worktree", "list", "--porcelain"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout
    except (subprocess.CalledProcessError, FileNotFoundError):
        return []
    return [
        pathlib.Path(line.split(" ", 1)[1].strip())
        for line in out.splitlines()
        if line.startswith("worktree ")
    ]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--repo", type=pathlib.Path, default=ROOT)
    parser.add_argument("--json", action="store_true", help="machine-readable output")
    args = parser.parse_args()

    worktrees = live_worktrees(args.repo)
    checkout_targets = []
    for wt in worktrees:
        target = wt / "target"
        if target.is_dir():
            checkout_targets.append((wt, disk_usage_bytes(target)))
    checkout_targets.sort(key=lambda row: row[1], reverse=True)

    base = cache_root()
    roots = build_roots(base) if base else []
    cache_entries = sorted(
        ((r, disk_usage_bytes(r)) for r in roots),
        key=lambda row: row[1],
        reverse=True,
    )

    checkout_total = sum(size for _, size in checkout_targets)
    cache_total = sum(size for _, size in cache_entries)

    report = {
        "checkout_targets": [
            {"path": str(p), "bytes": b} for p, b in checkout_targets
        ],
        "checkout_total_bytes": checkout_total,
        "cache_root": str(base) if base else None,
        "cache_build_roots": len(cache_entries),
        "cache_total_bytes": cache_total,
        "grand_total_bytes": checkout_total + cache_total,
        "live_worktrees": len(worktrees),
    }

    if args.json:
        json.dump(report, sys.stdout, indent=2)
        sys.stdout.write("\n")
        return 0

    print("Rust build output")
    print("=================")
    print()
    print(f"Live worktrees (git): {len(worktrees)}")
    print()
    print(f"Per-checkout target dirs — {gib(checkout_total):.0f} GiB")
    for path, size in checkout_targets[:12]:
        print(f"  {gib(size):8.1f} GiB  {path.name}/target")
    if len(checkout_targets) > 12:
        print(f"  … {len(checkout_targets) - 12} more")
    print()
    if base is None:
        print("Isolated build cache: not configured (dev-cache.sh reported no build dir)")
    else:
        print(f"Isolated build cache — {gib(cache_total):.0f} GiB "
              f"across {len(cache_entries)} build roots")
        print(f"  root: {base}")
        for path, size in cache_entries[:12]:
            print(f"  {gib(size):8.1f} GiB  {path.relative_to(base)}")
        if len(cache_entries) > 12:
            print(f"  … {len(cache_entries) - 12} more")
    print()
    print(f"Total: {gib(checkout_total + cache_total):.0f} GiB")
    print()
    print("Nothing was deleted. A build root here is not proof of garbage: the")
    print("cache is keyed by Cargo's workspace-path hash, and only a build run")
    print("through scripts/dev-cargo.sh from that exact path can claim one.")
    print("Run scripts/workspace-status.sh for which worktrees are merged or")
    print("dirty before retiring anything.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
