#!/usr/bin/env python3
"""Check that config.example.toml keeps root-level keys at the root.

TOML assigns every key after a `[table]` header to that table, so a root key
written below `[update]` silently becomes `update.skills_dir`. A user who
copies the example then gets a config whose keys the loader never reads.
This parses the example with the stdlib TOML parser and asserts the keys the
loader reads from the root are really root keys.
"""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CONFIG_EXAMPLE_TOML = ROOT / "config.example.toml"

ROOT_KEYS = (
    "allow_shell",
    "approval_policy",
    "sandbox_mode",
    "max_subagents",
    "skills_dir",
    "mcp_config_path",
    "notes_path",
    "memory_path",
)


def main() -> int:
    try:
        with CONFIG_EXAMPLE_TOML.open("rb") as handle:
            config = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        print(f"config.example.toml: cannot parse: {error}", file=sys.stderr)
        return 1

    errors = []
    for key in ROOT_KEYS:
        if key in config:
            continue
        owners = sorted(
            name
            for name, value in config.items()
            if isinstance(value, dict) and key in value
        )
        where = f" (parsed under [{', '.join(owners)}])" if owners else ""
        errors.append(
            f"config.example.toml: `{key}` is not a root-level key{where}; "
            "move it above the first [table] header"
        )

    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print(f"config.example.toml: {len(ROOT_KEYS)} root-level keys OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
