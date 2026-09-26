#!/usr/bin/env python3
"""Widen exactly the runtime items the compiler asks for (runtime/TUI split).

After `move-modules.py` moves code into `crates/runtime`, items that were
`pub(crate)` inside the TUI are private to the new crate. This loop runs
`cargo check --message-format=json` and rewrites only the definitions the
compiler names:

* E0603 / E0624 / E0616 / E0451 (private item, method, field, field in a
  struct literal) whose definition is in `crates/runtime/src`: the
  definition's `pub(crate)` / `pub(super)` / missing visibility becomes `pub`;
  a private `mod x;` becomes `pub mod x;`;
* a runtime `dead_code` warning on a `pub(crate)` item: the item is used only
  from the TUI, so it is widened the same way;
* `unfulfilled_lint_expectations` in the runtime: an `#[expect(dead_code)]`
  (or `cfg_attr(.., expect(..))`) the export made unfulfilled is deleted, but
  only when the attribute is alone on its line.

It never bulk-rewrites `pub(crate)`. Every widened item is printed; items in
security-relevant modules are flagged for human review.

While the loop runs, the runtime manifest's `[lints] workspace = true` is
commented out (lints capped to warnings), because an item that is dead inside
the runtime would otherwise fail the runtime build under `warnings = "deny"`
before cargo ever reached the TUI's privacy errors. The stanza is restored
when the loop ends, and a final check runs with it.

Usage:
    python3 scripts/split/widen.py [--cargo "scripts/dev-cargo.sh"] [--max-rounds N]
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
RUNTIME_SRC = REPO_ROOT / "crates" / "runtime" / "src"
RUNTIME_MANIFEST = REPO_ROOT / "crates" / "runtime" / "Cargo.toml"
LINTS_ON = "[lints]\nworkspace = true\n"
LINTS_CAPPED = "# [lints] capped by scripts/split/widen.py\n# workspace = true\n"
PRIVACY_CODES = {"E0603", "E0624", "E0616", "E0451"}
SECURITY_MODULES = (
    "sandbox",
    "core/authority",
    "network_policy",
    "repo_law",
    "workspace_trust",
    "oauth",
    "credentials",
    "mcp",
    "runtime_api",
    "execpolicy",
)
ITEM_KW = r"(?:async\s+|unsafe\s+|const\s+|extern\s+\"[^\"]*\"\s+)*(?:fn|struct|enum|union|trait|type|const|static|mod|use)\b"


def run_cargo(cargo: list[str], packages: list[str], extra: list[str]) -> list[dict]:
    cmd = [*cargo, "check", "--locked", "--message-format=json", *extra]
    for p in packages:
        cmd += ["-p", p]
    proc = subprocess.run(cmd, cwd=REPO_ROOT, capture_output=True, text=True)
    messages = []
    for line in proc.stdout.splitlines():
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue
        if msg.get("reason") == "compiler-message":
            messages.append(msg["message"])
    if proc.returncode != 0 and not messages:
        sys.stderr.write(proc.stderr[-4000:])
    return messages


def runtime_spans(message: dict) -> list[dict]:
    spans = list(message.get("spans", []))
    for child in message.get("children", []):
        spans.extend(child.get("spans", []))
    out = []
    for span in spans:
        path = (REPO_ROOT / span["file_name"]).resolve()
        if str(path).startswith(str(RUNTIME_SRC)):
            out.append(span)
    return out


def widen_line(line: str, name: str | None) -> str | None:
    """Rewrite one definition line to `pub`; None when it cannot be done safely."""
    m = re.match(r"^(\s*)pub\s*\((?:crate|super|in [^)]*)\)\s+", line)
    if m:
        return m.group(1) + "pub " + line[m.end() :]
    if re.match(r"^\s*pub\s", line):
        return None  # already public: the error is about a parent module
    m = re.match(rf"^(\s*)(?={ITEM_KW})", line)
    if m:
        return m.group(1) + "pub " + line[m.end() :]
    if name:
        m = re.match(rf"^(\s*)(?={re.escape(name)}\s*:)", line)
        if m:
            return m.group(1) + "pub " + line[m.end() :]
    return None


def find_field(struct: str, field: str) -> tuple[Path, int] | None:
    pat = re.compile(rf"\bstruct\s+{re.escape(struct)}\b")
    for path in RUNTIME_SRC.rglob("*.rs"):
        lines = path.read_text(encoding="utf-8").split("\n")
        for i, line in enumerate(lines):
            if pat.search(line):
                for j in range(i + 1, min(i + 400, len(lines))):
                    if re.match(rf"^\s*(pub(\([^)]*\))?\s+)?{re.escape(field)}\s*:", lines[j]):
                        return path, j + 1
                    if lines[j].startswith("}"):
                        break
    return None


def item_line(path: Path, line_no: int) -> int:
    """Walk down from an attribute or doc line to the item line itself."""
    lines = path.read_text(encoding="utf-8").split("\n")
    i = line_no - 1
    while i < len(lines) and re.match(r"^\s*(#\[|///|//!)", lines[i]):
        i += 1
    return i + 1


def apply(edits: dict[tuple[Path, int], str | None]) -> list[str]:
    """Apply per-line edits: a string replaces the line, None deletes it."""
    by_file: dict[Path, list[tuple[int, str | None]]] = {}
    for (path, line_no), new in edits.items():
        by_file.setdefault(path, []).append((line_no, new))
    report = []
    for path, changes in by_file.items():
        lines = path.read_text(encoding="utf-8").split("\n")
        for line_no, new in sorted(changes, reverse=True):
            old = lines[line_no - 1]
            rel = path.relative_to(REPO_ROOT)
            flag = " [security review]" if any(f"/{m}" in f"/{rel}" for m in SECURITY_MODULES) else ""
            if new is None:
                del lines[line_no - 1]
                report.append(f"removed {rel}:{line_no}: {old.strip()}")
            else:
                lines[line_no - 1] = new
                report.append(f"widened {rel}:{line_no}: {new.strip()}{flag}")
        path.write_text("\n".join(lines), encoding="utf-8")
    return report


def plan_edits(messages: list[dict]) -> tuple[dict, list[str]]:
    edits: dict[tuple[Path, int], str | None] = {}
    unresolved: list[str] = []
    for msg in messages:
        code = (msg.get("code") or {}).get("code") or ""
        level = msg.get("level")
        text = msg.get("message", "")
        if code in PRIVACY_CODES:
            name_m = re.search(r"`([^`]+)`", text)
            name = name_m.group(1).split("::")[-1] if name_m else None
            targets = [s for s in runtime_spans(msg) if not s.get("is_primary") or code == "E0603"]
            done = False
            for span in targets:
                path = (REPO_ROOT / span["file_name"]).resolve()
                line_no = item_line(path, span["line_start"])
                line = path.read_text(encoding="utf-8").split("\n")[line_no - 1]
                new = widen_line(line, name)
                if new is not None:
                    edits[(path, line_no)] = new
                    done = True
            if not done and code == "E0616":
                m = re.search(r"field `(\w+)` of struct `(?:[\w:]+::)?(\w+)", text)
                hit = m and find_field(m.group(2), m.group(1))
                if hit:
                    path, line_no = hit
                    line = path.read_text(encoding="utf-8").split("\n")[line_no - 1]
                    new = widen_line(line, m.group(1))
                    if new is not None:
                        edits[(path, line_no)] = new
                        done = True
            if not done:
                unresolved.append(msg.get("rendered") or text)
            continue
        if code == "dead_code" and level in ("warning", "error"):
            for span in runtime_spans(msg):
                path = (REPO_ROOT / span["file_name"]).resolve()
                line_no = span["line_start"]
                line = path.read_text(encoding="utf-8").split("\n")[line_no - 1]
                if re.match(r"^\s*pub\s*\(", line):
                    new = widen_line(line, None)
                    if new is not None:
                        edits[(path, line_no)] = new
                        continue
                unresolved.append(msg.get("rendered") or text)
            continue
        if code == "unfulfilled_lint_expectations":
            for span in runtime_spans(msg):
                path = (REPO_ROOT / span["file_name"]).resolve()
                line_no = span["line_start"]
                line = path.read_text(encoding="utf-8").split("\n")[line_no - 1].strip()
                if line.startswith("#[") and line.endswith("]") and "expect" in line:
                    edits[(path, line_no)] = None
                else:
                    unresolved.append(msg.get("rendered") or text)
            continue
        if level == "error":
            unresolved.append(msg.get("rendered") or text)
    return edits, unresolved


def set_lints(capped: bool) -> None:
    text = RUNTIME_MANIFEST.read_text(encoding="utf-8")
    old, new = (LINTS_ON, LINTS_CAPPED) if capped else (LINTS_CAPPED, LINTS_ON)
    if old in text:
        RUNTIME_MANIFEST.write_text(text.replace(old, new), encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--cargo", default="scripts/dev-cargo.sh")
    parser.add_argument("--max-rounds", type=int, default=12)
    parser.add_argument("--packages", default="codewhale-runtime,codewhale-tui")
    parser.add_argument("--extra", default="--lib --tests")
    args = parser.parse_args(argv)
    cargo = shlex.split(args.cargo)
    packages = args.packages.split(",")
    extra = shlex.split(args.extra)

    log: list[str] = []
    set_lints(capped=True)
    try:
        for round_no in range(1, args.max_rounds + 1):
            messages = run_cargo(cargo, packages, extra)
            edits, unresolved = plan_edits(messages)
            if not edits:
                break
            changes = apply(edits)
            log.extend(changes)
            print(f"round {round_no}: {len(changes)} edit(s)", flush=True)
    finally:
        set_lints(capped=False)
    for line in log:
        print(line)
    messages = run_cargo(cargo, packages, extra)
    edits, unresolved = plan_edits(messages)
    remaining = [m for m in messages if m.get("level") == "error"]
    if edits or remaining:
        print(f"\n{len(remaining)} error(s) and {len(edits)} further edit(s) remain with lints on:")
        for m in remaining[:40]:
            print(m.get("rendered") or m.get("message"))
        for (path, line_no), new in list(edits.items())[:40]:
            print(f"pending {path.relative_to(REPO_ROOT)}:{line_no}: {new}")
        return 1
    print(f"clean: {len(log)} edit(s)")
    return 0


if __name__ == "__main__":
    os.environ.setdefault("CARGO_TERM_COLOR", "never")
    sys.exit(main())
