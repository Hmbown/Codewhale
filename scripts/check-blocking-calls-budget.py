#!/usr/bin/env python3
"""Ratchet for blocking calls that could land on Tokio workers (#6149).

Codewhale's convention: async code must not run blocking operations inline.
`std::fs`/`thread::sleep` (and friends) are fine inside `spawn_blocking`,
on dedicated `std::thread`s, and in synchronous entry points — but every
unprotected call site is one careless caller away from parking a runtime
worker. This check counts the sites that are NOT already inside a blocking
scope (`spawn_blocking`, `spawn_blocking_supervised`, `std::thread::spawn`,
`thread::Builder`) or test code, and fails if any file exceeds its recorded
budget in `check-blocking-calls-budget.json`.

Fix the call site — wrap the work in `spawn_blocking` (the established
pattern, ~80 sites) or switch to `tokio::fs`/`tokio::time` — or, if the site
is genuinely only reachable from synchronous code, acknowledge the debt by
raising the file's budget.

Run `python3 scripts/check-blocking-calls-budget.py --update` to regenerate
the budget after removing sites or after an intentional addition.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CRATES = ROOT / "crates"
BUDGET_PATH = Path(__file__).with_suffix(".json")

PATTERNS = {
    "thread_sleep": re.compile(r"\bthread::sleep\s*\("),
    "std_fs": re.compile(
        r"\bstd::fs::(?:read|read_to_string|write|create_dir|create_dir_all|"
        r"remove_file|remove_dir|remove_dir_all|copy|rename|metadata|"
        r"symlink_metadata|read_dir|canonicalize|exists|set_permissions|"
        r"hard_link|soft_link|symlink|File|OpenOptions|DirBuilder)\b"
    ),
    # Method form (`path.canonicalize()`) resolves the path on the calling
    # thread exactly like `std::fs::canonicalize` (#6522 review).
    "path_canonicalize": re.compile(r"\.canonicalize\s*\(\s*\)"),
}

ATTR_RE = re.compile(r"#\s*\[([^\]]*)\]")
FN_RE = re.compile(
    r"\b(?:pub(?:\([^)]*\))?\s+)?(?:unsafe\s+)?(?:extern\s+\"[^\"]*\"\s+)?"
    r"(async\s+)?fn\s+([A-Za-z_][\w]*)"
)
MOD_RE = re.compile(r"\bmod\s+([A-Za-z_][\w]*)")

TOKEN_RE = re.compile(
    r"#\s*\[[^\]]*\]"
    r"|\bmod\s+\w+"
    r"|\bimpl\b"
    r"|\basync\s+move\s*\{"
    r"|\basync\s*\{"
    r"|\b(?:pub(?:\([^)]*\))?\s+)?(?:unsafe\s+)?(?:async\s+)?fn\s+\w+"
    r"|spawn_blocking(?:_supervised)?"
    r"|thread::spawn"
    r"|thread::Builder::new"
    r"|[{}]"
)


def strip_comments_and_strings(text: str) -> str:
    """Blank out comments and string/char literal contents, keeping newlines."""
    out = list(text)
    i, n = 0, len(text)
    line_comment = block_comment = in_str = in_char = in_raw = False
    block_depth = 0
    raw_hashes = 0
    while i < n:
        c = text[i]
        if line_comment:
            if c == "\n":
                line_comment = False
            else:
                out[i] = " "
            i += 1
            continue
        if block_comment:
            if text[i : i + 2] == "/*":
                block_depth += 1
                out[i] = out[i + 1] = " "
                i += 2
                continue
            if text[i : i + 2] == "*/":
                block_depth -= 1
                out[i] = out[i + 1] = " "
                i += 2
                if block_depth == 0:
                    block_comment = False
                continue
            if c != "\n":
                out[i] = " "
            i += 1
            continue
        if in_str:
            if c == "\\":
                out[i] = out[i + 1] = " "
                i += 2
                continue
            if c == '"':
                in_str = False
            elif c != "\n":
                out[i] = " "
            i += 1
            continue
        if in_char:
            if c == "\\":
                out[i] = out[i + 1] = " "
                i += 2
                continue
            if c == "'":
                in_char = False
            elif c != "\n":
                out[i] = " "
            i += 1
            continue
        if in_raw:
            if c == '"' and text[i + 1 : i + 1 + raw_hashes] == "#" * raw_hashes:
                for j in range(1 + raw_hashes):
                    out[i + j] = " "
                i += 1 + raw_hashes
                in_raw = False
                continue
            if c != "\n":
                out[i] = " "
            i += 1
            continue
        if text[i : i + 2] == "//":
            line_comment = True
            out[i] = out[i + 1] = " "
            i += 2
            continue
        if text[i : i + 2] == "/*":
            block_comment = True
            block_depth = 1
            out[i] = out[i + 1] = " "
            i += 2
            continue
        if c == "r":
            m = re.match(r'r(#+)"', text[i:])
            if m:
                raw_hashes = len(m.group(1))
                in_raw = True
                for j in range(2 + raw_hashes):
                    out[i + j] = " "
                i += 2 + raw_hashes
                continue
        if c == '"':
            in_str = True
            out[i] = " "
            i += 1
            continue
        if c == "'" and re.match(r"'(?:\\.|[^'\\])'", text[i:]):
            in_char = True
            out[i] = " "
            i += 1
            continue
        i += 1
    return "".join(out)


def file_counts(path: Path) -> dict[str, int]:
    """Count unprotected blocking-call sites in one Rust source file."""
    text = path.read_text(encoding="utf-8", errors="replace")
    code = strip_comments_and_strings(text)
    counts = {name: 0 for name in PATTERNS}
    # Scope stack: entries are dicts {kind, open_depth} where kind is
    # 'test', 'blocking', 'fn', 'mod', or 'impl'. A hit counts only when the
    # innermost enclosing scope is neither test code nor a blocking pool /
    # dedicated-thread closure.
    stack: list[dict] = []
    pending_attr_test = False
    pending_blocking = False
    depth = 0
    for line in code.split("\n"):
        # Interleave pattern hits and scope tokens in column order so a
        # one-liner like `fn f() { thread::sleep(..) }` sees the fn scope.
        events: list[tuple[int, str, object]] = []
        for name, pat in PATTERNS.items():
            for m in pat.finditer(line):
                events.append((m.start(), "hit", name))
        for m in TOKEN_RE.finditer(line):
            events.append((m.start(), "tok", m.group(0)))
        events.sort(key=lambda e: e[0])
        for _col, kind, payload in events:
            if kind == "hit":
                if not any(s["kind"] in ("test", "blocking") for s in stack):
                    counts[payload] += 1  # type: ignore[index]
                continue
            tok = payload  # type: ignore[assignment]
            if tok.startswith("#"):
                inner = tok[tok.index("[") + 1 : -1]
                if "test" in inner:
                    pending_attr_test = True
                continue
            if tok == "{":
                depth += 1
                if pending_blocking:
                    stack.append({"kind": "blocking", "open": depth})
                elif stack and stack[-1]["open"] is None:
                    stack[-1]["open"] = depth
                pending_blocking = False
                continue
            if tok == "}":
                while stack and stack[-1]["open"] == depth:
                    stack.pop()
                depth -= 1
                continue
            if "spawn_blocking" in tok or tok in ("thread::spawn", "thread::Builder::new"):
                pending_blocking = True
                continue
            if tok.startswith("async") and tok.endswith("{"):
                stack.append({"kind": "fn", "open": depth + 1})
                depth += 1
                pending_attr_test = False
                pending_blocking = False
                continue
            fm = FN_RE.match(tok)
            if fm:
                kind = "test" if pending_attr_test else "fn"
                stack.append({"kind": kind, "open": None})
                pending_attr_test = False
                pending_blocking = False
                continue
            mm = MOD_RE.match(tok)
            if mm:
                name = mm.group(1)
                kind = "test" if (pending_attr_test or name.startswith("test")) else "mod"
                stack.append({"kind": kind, "open": None})
                pending_attr_test = False
                pending_blocking = False
                continue
            if tok == "impl":
                stack.append({"kind": "impl", "open": None})
                pending_attr_test = False
                pending_blocking = False
                continue
    return {k: v for k, v in counts.items() if v}


CFG_TEST_MOD = re.compile(
    r"#\[cfg\(test\)\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;"
)


def cfg_test_module_files() -> set[Path]:
    """Files that are entire `#[cfg(test)]` modules declared by another file.

    The per-file scanner recognises test scope it can see *inside* a file —
    `#[test]`, `mod tests`, `fn test_*`. It cannot see that a whole file is
    test-only, because that fact lives in the parent's `#[cfg(test)] mod
    foo;` declaration. Extracting a test suite into its own file therefore
    made an untouched call site look new (#6209 follow-up: PR #6096's
    `session_export_*_tests.rs`). Excluding these keeps the ratchet's stated
    contract — "not ... test code" — instead of taxing the extraction.
    """
    excluded: set[Path] = set()
    for path in CRATES.rglob("*.rs"):
        try:
            text = path.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        # `mod foo;` in `mod.rs`/`lib.rs`/`main.rs` resolves beside the file;
        # in any other `bar.rs` it resolves under `bar/` (non-mod-rs layout).
        base = (
            path.parent
            if path.name in ("mod.rs", "lib.rs", "main.rs")
            else path.parent / path.stem
        )
        for name in CFG_TEST_MOD.findall(text):
            for candidate in (base / f"{name}.rs", base / name / "mod.rs"):
                if candidate.is_file():
                    excluded.add(candidate.resolve())
    return excluded


def collect_current() -> dict[str, dict[str, int]]:
    budget: dict[str, dict[str, int]] = {}
    test_only = cfg_test_module_files()
    for path in sorted(CRATES.rglob("*.rs")):
        if path.resolve() in test_only:
            continue
        try:
            counts = file_counts(path)
        except OSError:
            continue
        if counts:
            rel = str(path.relative_to(ROOT))
            budget[rel] = counts
    return budget


def main() -> int:
    update = "--update" in sys.argv
    current = collect_current()
    if update:
        BUDGET_PATH.write_text(
            json.dumps(current, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        total = sum(sum(v.values()) for v in current.values())
        print(f"wrote {BUDGET_PATH.name}: {total} sites across {len(current)} files")
        return 0

    if not BUDGET_PATH.exists():
        print(f"missing {BUDGET_PATH.name}; run with --update to create it", file=sys.stderr)
        return 2
    budget = json.loads(BUDGET_PATH.read_text(encoding="utf-8"))

    failures: list[str] = []
    savings: list[str] = []
    for path, counts in sorted(current.items()):
        allowed = budget.get(path, {})
        for name, count in counts.items():
            limit = allowed.get(name, 0)
            if count > limit:
                failures.append(
                    f"{path}: {name} sites {count} > budget {limit}"
                )
            elif count < limit:
                savings.append(
                    f"{path}: {name} sites {count} < budget {limit} — tighten with --update"
                )
    for path, counts in sorted(budget.items()):
        if path not in current:
            savings.append(f"{path}: file clean — tighten with --update")
        else:
            for name in counts:
                if name not in current[path]:
                    savings.append(
                        f"{path}: {name} sites 0 < budget {counts[name]} — tighten with --update"
                    )

    for line in savings:
        print(line)
    if failures:
        print(
            "\nBlocking-call budget exceeded — new `thread::sleep`/`std::fs` call "
            "sites appeared outside spawn_blocking/dedicated-thread/test scopes:",
            file=sys.stderr,
        )
        for line in failures:
            print(f"  {line}", file=sys.stderr)
        print(
            "Move the work into `tokio::task::spawn_blocking` (or use tokio::fs "
            "/ tokio::time). If the site can only run on synchronous code, land "
            "the raised budget in this PR and say why in the PR description:\n"
            "  python3 scripts/check-blocking-calls-budget.py --update\n"
            "See #6149.",
            file=sys.stderr,
        )
        return 1
    total = sum(sum(v.values()) for v in current.values())
    print(f"blocking-call budget: {total} sites across {len(current)} files, within budget")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
