#!/usr/bin/env python3
"""check-lexicon.py — warn when user-facing English copy drifts from the lexicon.

The product vocabulary is one word per concept across the TUI, the app, the
site and the docs: Agent and Fleet; Plan, Work and Operate; Permissions
(Ask, Auto-Review, Full Access); Tasks panel; Making room; Settings; the
presence words; Thinking. This script greps the English sources a person
actually reads for the words that decision retires, plus the engineering notes experience mark 5 keeps off product
surfaces (issue keys, HTTP routes, "Ctrl/Cmd").

Scanned:
  - crates/localization/locales/en.json            (values only, never keys)
  - web/lib/i18n/dictionaries/en/*.ts              (string literals)
  - web/lib/content/*.ts                           (string literals)

It is WARN-ONLY: it prints findings and exits 0 so it can run from
scripts/preflight.sh without turning a push red while the sweep finishes.
Pass --strict to exit 1 on any finding (for a local ratchet or a future CI
gate). Parser aliases, config keys and locale message identifiers are code,
not copy, and are out of scope.

    python3 scripts/check-lexicon.py            # warn
    python3 scripts/check-lexicon.py --strict   # fail on findings
    python3 scripts/check-lexicon.py --summary  # counts only
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EN_LOCALE = ROOT / "crates" / "localization" / "locales" / "en.json"
WEB_GLOBS = (
    "web/lib/i18n/dictionaries/en/*.ts",
    "web/lib/content/*.ts",
)

# (label, use-instead, compiled pattern). Case matters where the retired word
# is also an ordinary English word ("Act" the mode vs. "act" the verb).
RULES: list[tuple[str, str, re.Pattern[str]]] = [
    # §19 modes
    ("Act", "Work", re.compile(r"\bAct\b|\bACT\b")),
    ("read-only lane", "Plan", re.compile(r"read-only lane", re.I)),
    ("agent mode", "Work", re.compile(r"\bagent mode\b", re.I)),
    ("Fleet mode", "Operate", re.compile(r"\bfleet mode\b", re.I)),
    ("operator", "Coordinator (or Operate for the mode)", re.compile(r"\boperator\b", re.I)),
    # §19 permissions
    ("posture", "Permissions", re.compile(r"\bpostures?\b", re.I)),
    ("approval policy", "Permissions", re.compile(r"\b(approval|permission) policy\b", re.I)),
    # §16 / §19 Fleet and agents
    ("roster", "Fleet", re.compile(r"\broster\b", re.I)),
    ("worker", "agent", re.compile(r"(?<!Cloudflare )\bworkers?\b", re.I)),
    ("sub-agent", "agent", re.compile(r"\bsub-?agents?\b", re.I)),
    ("lane", "agent", re.compile(r"\blanes?\b", re.I)),
    ("leader", "Coordinator", re.compile(r"\bleader\b", re.I)),
    ("consultant", "Advisor", re.compile(r"\bconsultants?\b", re.I)),
    # §19 surfaces and states
    ("Work bar", "Tasks panel", re.compile(r"\bwork ?bar\b|\bwork dock\b|\brail panel\b", re.I)),
    ("Bash", "Command", re.compile(r"\bBash\b")),
    ("MCP Read/Action", "Connected app", re.compile(r"\bMCP (Read|Action)\b")),
    ("Deny this call", "Don't allow", re.compile(r"Deny this call|\(this kind\)")),
    ("abort", "Stop", re.compile(r"\babort(ed|s|ing)?\b", re.I)),
    ("compaction", "Making room", re.compile(r"\bauto-?compact\w*|\bcompaction\b", re.I)),
    ("waiting on you", "needs you", re.compile(r"waiting on you", re.I)),
    ("unobserved", "resting", re.compile(r"\bunobserved\b", re.I)),
    ("Reasoning", "Thinking", re.compile(r"\bReasoning\b")),
    ("charter", "Constitution", re.compile(r"\bcharter\b", re.I)),
    # Experience mark 5: no engineering notes on product surfaces
    ("issue key", "(remove)", re.compile(r"\bAPPS-\d+\b|\bSHA-(?!(1|256|384|512)\b)\d+\b|\(#\d{3,}\)")),
    ("HTTP route", "(describe what works)", re.compile(r"\b(GET|POST|PUT|DELETE|PATCH) /")),
    ("Ctrl/Cmd", "one key notation", re.compile(r"Ctrl/Cmd")),
]

# A settings screen titled "Config" (§19: Settings). Exact values only, so
# "Config file:" and "config.toml" stay legal.
CONFIG_TITLE = re.compile(r"^\s*Config\s*$")

# Deliberate, reviewed exceptions: {source-relative path: {key or literal: {labels}}}.
# Keep this short; every entry is a promise that the word is the right one.
ALLOW: dict[str, dict[str, set[str]]] = {
    "crates/localization/locales/en.json": {
        # Names the compatibility slash command the user typed.
        "CmdSubagentsDescription": {"worker", "sub-agent"},
        "HomeQuickSubagents": {"worker"},
    },
}

# Double-quoted, single-quoted or template string literals in TS sources.
TS_STRING = re.compile(r'"((?:[^"\\\n]|\\.)*)"|\'((?:[^\'\\\n]|\\.)*)\'|`((?:[^`\\]|\\.)*)`')


PLACEHOLDER = re.compile(r"\{[A-Za-z_][A-Za-z0-9_]*\}")


def findings_for(text: str) -> list[tuple[str, str]]:
    # `{posture}` is a substitution slot, not a word the reader sees.
    text = PLACEHOLDER.sub("", text)
    hits = [(label, instead) for label, instead, rx in RULES if rx.search(text)]
    if CONFIG_TITLE.match(text):
        hits.append(("Config", "Settings"))
    return hits


def scan_locale(path: Path):
    rel = str(path.relative_to(ROOT))
    allow = ALLOW.get(rel, {})
    data = json.loads(path.read_text(encoding="utf-8"))
    for key, value in data.items():
        if not isinstance(value, str):
            continue
        for label, instead in findings_for(value):
            if label in allow.get(key, set()):
                continue
            yield rel, key, label, instead, value


def scan_ts(path: Path):
    rel = str(path.relative_to(ROOT))
    allow = ALLOW.get(rel, {})
    text = path.read_text(encoding="utf-8")
    for lineno, line in enumerate(text.splitlines(), 1):
        stripped = line.lstrip()
        if stripped.startswith(("//", "*", "/*", "import ", "export type", "type ")):
            continue
        for m in TS_STRING.finditer(line):
            value = next(g for g in m.groups() if g is not None)
            # Skip identifiers, paths and URLs; copy has a space in it.
            if " " not in value:
                continue
            for label, instead in findings_for(value):
                if label in allow.get(value, set()):
                    continue
                yield rel, f"L{lineno}", label, instead, value


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--strict", action="store_true", help="exit 1 on any finding")
    parser.add_argument("--summary", action="store_true", help="print counts only")
    args = parser.parse_args()

    found = []
    if EN_LOCALE.exists():
        found.extend(scan_locale(EN_LOCALE))
    for pattern in WEB_GLOBS:
        for path in sorted(ROOT.glob(pattern)):
            if path.name.endswith(".test.ts"):
                continue
            found.extend(scan_ts(path))

    counts = Counter(label for _, _, label, _, _ in found)
    if not args.summary:
        for rel, where, label, instead, value in found:
            excerpt = value if len(value) <= 110 else value[:107] + "..."
            print(f"{rel}:{where}: '{label}' -> {instead}: {excerpt!r}")
    if found:
        tally = ", ".join(f"{label} {n}" for label, n in counts.most_common())
        print(f"[lexicon] {len(found)} finding(s): {tally}")
        if args.strict:
            return 1
        print("[lexicon] warn-only; pass --strict to fail")
    else:
        print("[lexicon] OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
