#!/usr/bin/env python3
"""Deterministic crate-boundary gate for the command extraction (FEAT-014).

Enforces the EPIC-006 boundary contract:

1. `codewhale-command-contract` and `codewhale-secrets` may not transitively
   depend on `codewhale-tui` (normal edges, via `cargo metadata`).
   `codewhale-secrets` owns the shared output sanitizer that portable command
   helpers consume (FEAT-025 D4), so it must stay TUI-free before
   `codewhale-commands` depends on it.
2. `codewhale-command-contract` source may not import the concrete `App`,
   widget/renderer/view/event-loop surfaces, or `ratatui`/`crossterm`.
3. No composite `CommandContext` symbol (supertrait/struct/enum) may exist in
   the contract — the deep-dive D2 "no super-context" rule.
4. No boxed handler storage (`Box<`) in the contract — the D1/D4 fn-pointer
   transport rule.

The guard is hermetic: it reads `cargo metadata` and the contract source only;
it never starts the TUI and makes no network calls.

Usage:
    python3 scripts/check-command-crate-boundaries.py           # enforce
    python3 scripts/check-command-crate-boundaries.py --check   # enforce (default)
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
CONTRACT_DIR = REPO_ROOT / "crates" / "command-contract" / "src"
CONTRACT_PACKAGE = "codewhale-command-contract"
FORBIDDEN_TUI_PACKAGE = "codewhale-tui"

# Workspace packages that must stay free of any (normal) path to the TUI. The
# contract carries the portable shapes; `codewhale-secrets` owns the shared pure
# sanitizer those shapes' handlers consume. Both are prerequisites for
# `codewhale-commands` (FEAT-016/043), so a TUI edge here would silently drag
# the whole TUI into the extracted command crate.
TUI_FREE_PACKAGES = (CONTRACT_PACKAGE, "codewhale-secrets")

# Import lines that must never appear in the contract (narrowly scoped: real
# imports only, comments never match because they do not start with `use`).
FORBIDDEN_IMPORT_PATTERNS = [
    (re.compile(r"^\s*(pub\s+)?use\s+codewhale_tui\b"), "codewhale-tui import"),
    (re.compile(r"^\s*(pub\s+)?use\s+ratatui\b"), "ratatui (widget) import"),
    (re.compile(r"^\s*(pub\s+)?use\s+crossterm\b"), "crossterm (terminal) import"),
    (re.compile(r"^\s*(pub\s+)?use\s+.*\bApp\b"), "concrete App import"),
    (re.compile(r"^\s*(pub\s+)?use\s+.*\bBuffer\b"), "render buffer import"),
    (re.compile(r"^\s*(pub\s+)?use\s+.*\bWidget\b"), "widget import"),
    (re.compile(r"^\s*(pub\s+)?use\s+.*\bViewStack\b"), "view-stack import"),
    (re.compile(r"^\s*(pub\s+)?use\s+.*\bEventLoop\b"), "event-loop import"),
]

# Composite super-context symbols (D2: exactly `CommandContext`, not the
# plural envelope `CommandContexts` nor facet names like `CommandModelContext`).
COMPOSITE_SYMBOL_PATTERN = re.compile(
    r"^\s*(pub\s+)?(trait|struct|enum)\s+CommandContext\b"
)
# Boxed handler/closure storage (D1: fn pointers only).
BOXED_STORAGE_PATTERN = re.compile(r"\bBox\s*<")


class BoundaryViolation:
    """One deterministic boundary violation with an actionable diagnostic."""

    def __init__(self, category: str, location: str, detail: str) -> None:
        self.category = category
        self.location = location
        self.detail = detail

    def __str__(self) -> str:
        return f"{self.category}: {self.location}: {self.detail}"


def load_workspace_metadata() -> dict:
    """Load the locked workspace dependency graph via cargo metadata."""
    result = subprocess.run(
        [
            "cargo",
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--no-deps",
        ],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    return json.loads(result.stdout)


def dependency_graph(metadata: dict) -> dict[str, set[str]]:
    """Map package name -> set of direct NORMAL dependency package names.

    Dev- and build-dependencies are excluded: the gate contract checks normal
    transitive edges (a dev-dependency on the TUI, e.g. for acceptance
    harnesses, must not trip the boundary).
    """
    graph: dict[str, set[str]] = {}
    for package in metadata["packages"]:
        deps = set()
        for dep in package.get("dependencies", []):
            # kind is None for normal dependencies, "dev" or "build" otherwise.
            if dep.get("kind") is not None:
                continue
            name = dep.get("name")
            if name:
                deps.add(name)
        graph[package["name"]] = deps
    return graph


def reaches_tui(package: str, graph: dict[str, set[str]]) -> bool:
    """Whether `package` transitively reaches the forbidden TUI package."""
    seen: set[str] = set()
    stack = list(graph.get(package, set()))
    while stack:
        name = stack.pop()
        if name == FORBIDDEN_TUI_PACKAGE:
            return True
        if name in seen:
            continue
        seen.add(name)
        stack.extend(graph.get(name, set()))
    return False


def check_dependency_graph(graph: dict[str, set[str]]) -> list[BoundaryViolation]:
    """No TUI-free package may reach codewhale-tui through normal edges."""
    violations: list[BoundaryViolation] = []
    for package in TUI_FREE_PACKAGES:
        if package not in graph:
            violations.append(
                BoundaryViolation(
                    "dependency-graph",
                    package,
                    "workspace package missing from the cargo metadata graph",
                )
            )
            continue
        if reaches_tui(package, graph):
            violations.append(
                BoundaryViolation(
                    "dependency-graph",
                    package,
                    f"transitively depends on {FORBIDDEN_TUI_PACKAGE}",
                )
            )
    return violations


def check_contract_source_text(text: str, display_path: str) -> list[BoundaryViolation]:
    """Scan one source text for forbidden imports/symbols (hermetic test hook)."""
    violations: list[BoundaryViolation] = []
    for line_no, line in enumerate(text.splitlines(), start=1):
        stripped = line.strip()
        for pattern, label in FORBIDDEN_IMPORT_PATTERNS:
            if pattern.match(stripped):
                violations.append(
                    BoundaryViolation(
                        "source-scan",
                        f"{display_path}:{line_no}",
                        f"forbidden {label}: {stripped}",
                    )
                )
        if COMPOSITE_SYMBOL_PATTERN.match(stripped):
            violations.append(
                BoundaryViolation(
                    "source-scan",
                    f"{display_path}:{line_no}",
                    f"composite CommandContext symbol (D2 forbids super-contexts): {stripped}",
                )
            )
        if BOXED_STORAGE_PATTERN.search(stripped):
            violations.append(
                BoundaryViolation(
                    "source-scan",
                    f"{display_path}:{line_no}",
                    f"boxed storage in the contract (D1 requires fn pointers): {stripped}",
                )
            )
    return violations


def check_contract_source() -> list[BoundaryViolation]:
    """Scan contract production source for forbidden imports and symbols."""
    violations: list[BoundaryViolation] = []
    if not CONTRACT_DIR.is_dir():
        return [
            BoundaryViolation(
                "source-scan",
                str(CONTRACT_DIR),
                "command-contract src directory missing",
            )
        ]
    for path in sorted(CONTRACT_DIR.rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        rel = path.relative_to(REPO_ROOT)
        violations.extend(check_contract_source_text(text, str(rel)))
    return violations


def run_checks(metadata: dict | None = None) -> list[BoundaryViolation]:
    """Run all boundary checks; return the collected violations."""
    graph = dependency_graph(metadata) if metadata is not None else dependency_graph(
        load_workspace_metadata()
    )
    return check_dependency_graph(graph) + check_contract_source()


def main(argv: list[str] | None = None) -> int:
    del argv  # reserved for future flags (e.g. --update); check is the default
    violations = run_checks()
    if violations:
        print("[command-crate-boundaries] FAIL", file=sys.stderr)
        for violation in violations:
            print(f"  {violation}", file=sys.stderr)
        return 1
    print(
        f"[command-crate-boundaries] PASS: "
        f"{', '.join(TUI_FREE_PACKAGES)} have no {FORBIDDEN_TUI_PACKAGE} edge; "
        "no forbidden import, composite context, or boxed handler in the contract"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
