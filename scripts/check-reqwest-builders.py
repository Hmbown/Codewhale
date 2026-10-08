#!/usr/bin/env python3
"""Forbid bare reqwest client constructors outside crates/release.

The workspace builds reqwest with `rustls-no-provider`, so a bare
`reqwest::Client::builder()` (or `::new()`, or the blocking counterparts)
panics in `default_rustls_crypto_provider` whenever it runs before any other
client has installed the crypto provider. Every client must therefore go
through `codewhale_release::tls` (`reqwest_client_builder()`,
`reqwest_blocking_client_builder()`, `reqwest_client()`), which installs the
provider exactly once before the first build. The only sanctioned bare
constructors are the platform builders in `crates/release/src/`.

See #6153 (0.9.13 shipped this panic on the first-run Ollama probe).
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CRATES = ROOT / "crates"
# The TLS-owner crate: its platform builders are the sanctioned constructors.
ALLOWED_PREFIX = CRATES / "release" / "src"

# Fully-qualified bare constructors. Always a violation outside ALLOWED_PREFIX.
QUALIFIED_RE = re.compile(
    r"reqwest::(?:blocking::)?Client::(?:builder|new)\s*\("
)
# Bare `Client::builder()` / `Client::new()` spellings. Only a violation in a
# file that imports reqwest's Client, so custom `FooClient` types never match
# (`(?<![\w:])` also keeps `FixedSummaryClient::default()`-style hits out).
BARE_RE = re.compile(r"(?<![\w:])Client::(?:builder|new)\s*\(")
REQWEST_CLIENT_IMPORT_RE = re.compile(r"use\s+reqwest::[^\n;]*\bClient\b")


def _is_comment_only(line: str) -> bool:
    stripped = line.lstrip()
    return stripped.startswith("//")


def file_violations(path: Path, allowed_prefix: Path = ALLOWED_PREFIX) -> list[str]:
    """Return `path:line: <match>` entries for bare constructors in one file."""
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return []
    try:
        rel = path.relative_to(ROOT)
    except ValueError:
        rel = path
    if allowed_prefix in path.parents or path == allowed_prefix:
        return []
    hits: list[str] = []
    bare_allowed = REQWEST_CLIENT_IMPORT_RE.search(text) is None
    for lineno, line in enumerate(text.splitlines(), start=1):
        if _is_comment_only(line):
            continue
        match = QUALIFIED_RE.search(line)
        if match is None and not bare_allowed:
            match = BARE_RE.search(line)
        if match is not None:
            hits.append(f"{rel}:{lineno}: {match.group(0)}")
    return hits


def find_violations(
    root: Path = CRATES, allowed_prefix: Path = ALLOWED_PREFIX
) -> list[str]:
    """Walk `root/**/*.rs` and collect every bare-constructor violation."""
    violations: list[str] = []
    for path in sorted(root.rglob("*.rs")):
        violations.extend(file_violations(path, allowed_prefix))
    return violations


def main() -> int:
    violations = find_violations()
    if violations:
        print(
            "bare reqwest client constructor(s) outside crates/release/src "
            "(use codewhale_release::tls instead; see #6153):",
            file=sys.stderr,
        )
        for violation in violations:
            print(f"  {violation}", file=sys.stderr)
        return 1
    print("reqwest builder check OK: no bare Client::builder()/new() outside crates/release/src.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
