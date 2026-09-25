#!/usr/bin/env python3
"""Runtime -> UI boundary ratchet for the runtime/TUI crate split.

Builds the top-level module graph of `crates/tui/src` and `crates/runtime/src`
with a lexer that masks comments and string literals and expands grouped
`use crate::{...}` imports, then counts every reference from the *runtime
closure* upward into UI code.

The runtime closure is every module reachable over production edges from the
eight seed modules (`core`, `tools`, `runtime_api`, `runtime_threads`,
`client`, `llm_client`, `config`, `session_manager`) without passing through a
UI module, plus every module that already lives in `crates/runtime`. It is
computed on every run; the member list is never written by hand.

Categories counted (keyed `from|to`):

* ``prod``  production references from the closure into UI modules
            (`tui`, `commands`, `remote_control`, `context_report`,
            `composer_*`) or into private crate-root (`lib.rs`) items;
* ``test``  the same, from `#[cfg(test)]` code and test files;
* ``late``  references (production or test) from the closure into modules
            outside it that are not UI either, i.e. modules that move after
            the strongly connected core (`exec_agent`, `route_preferences`,
            ...). A test edge constrains a crate move exactly like a
            production edge;
* ``uilib`` closure files that use a UI library (`ratatui`, `crossterm`,
            `codewhale_tui`, or `codewhale_palette` while it still pulls in
            ratatui), keyed `module|library`;
* ``doc``   intra-doc links in closure doc comments that point at UI code
            (`crate::tui::...`, `crate::commands::...`). rustdoc checks them
            once the module is public in `codewhale-runtime`.

The baseline is `scripts/runtime-boundary-baseline.json`. `--check` (default)
fails on any increased count or new key and prints `file:line` for it; it also
fails when a count dropped and the baseline was not lowered in the same change,
so the baseline stays honest. `--update` rewrites the baseline and refuses to
raise any count.

See docs/design/TUI_DECONSTRUCTION.md (runtime split, ratchet).
"""

from __future__ import annotations

import argparse
import collections
import json
import os
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
TUI_SRC = REPO_ROOT / "crates" / "tui" / "src"
RUNTIME_SRC = REPO_ROOT / "crates" / "runtime" / "src"
BASELINE = REPO_ROOT / "scripts" / "runtime-boundary-baseline.json"

SEEDS = (
    "core",
    "tools",
    "runtime_api",
    "runtime_threads",
    "client",
    "llm_client",
    "config",
    "session_manager",
)
UI_MODULES = {"tui", "commands", "remote_control", "context_report"}
UI_PREFIXES = ("composer_",)
ROOT_ITEMS = "lib.rs"
CATEGORIES = ("prod", "test", "late", "uilib", "doc")

HINTS = {
    "tui": "move the item down (e.g. into core::authority) or reach the UI "
    "through the host_terminal port",
    "commands": "read commands through the runtime CommandCatalog, or split "
    "the non-UI half of the command into a runtime module",
    ROOT_ITEMS: "move the crate-root helper into the runtime module that owns it",
}


def is_ui(module: str) -> bool:
    return (
        module in UI_MODULES
        or module.startswith(UI_PREFIXES)
        or module in (ROOT_ITEMS, "main.rs")
    )


# --------------------------------------------------------------------------
# Lexer: blank comments, string and char literals; keep offsets and newlines.
# --------------------------------------------------------------------------

_RAW_STR = re.compile(r'b?r(#*)"')
_CHAR = re.compile(r"'(\\.[^']*|[^\\'])'")


def lex_mask(src: str) -> str:
    out = list(src)
    i, n = 0, len(src)

    def blank(a: int, b: int) -> None:
        for k in range(a, min(b, n)):
            if out[k] != "\n":
                out[k] = " "

    while i < n:
        c = src[i]
        if src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            blank(i, j)
            i = j
            continue
        if src.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth += 1
                    j += 2
                elif src.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            blank(i, j)
            i = j
            continue
        prev_ident = i > 0 and (src[i - 1].isalnum() or src[i - 1] == "_")
        m = _RAW_STR.match(src, i, i + 10)
        if m and not prev_ident:
            end = '"' + m.group(1)
            j = src.find(end, i + len(m.group(0)))
            j = n if j < 0 else j + len(end)
            blank(i + 1, j - 1)
            i = j
            continue
        if c == '"' or (c == "b" and i + 1 < n and src[i + 1] == '"' and not prev_ident):
            j = i + (2 if c == "b" else 1)
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            blank(i + 1, j)
            i = j + 1
            continue
        if c == "'":
            m = _CHAR.match(src, i, i + 12)
            if m:
                blank(i + 1, i + len(m.group(0)) - 1)
                i += len(m.group(0))
                continue
        i += 1
    return "".join(out)


def match_brace(code: str, start: int) -> int:
    depth = 0
    for k in range(start, len(code)):
        if code[k] == "{":
            depth += 1
        elif code[k] == "}":
            depth -= 1
            if depth == 0:
                return k
    return len(code) - 1


CFG_TEST = re.compile(
    r"#\[cfg\((?:test|all\(test[^\]]*\)|any\(test[^\]]*\)|feature\s*=\s*\"test-support\")\)\]"
)


def strip_cfg_test(code: str, test_mod_decls: list[str]) -> str:
    """Blank items annotated `#[cfg(test)]`; collect `mod x;` declarations."""
    out = list(code)
    for m in CFG_TEST.finditer(code):
        j = m.end()
        while True:
            ws = re.match(r"\s*(#\[[^\]]*\])?", code[j:])
            if ws and ws.group(1):
                j += ws.end()
                continue
            j += len(code[j:]) - len(code[j:].lstrip())
            break
        md = re.match(r"(pub(\([^)]*\))?\s+)?mod\s+(\w+)\s*;", code[j:])
        if md:
            test_mod_decls.append(md.group(3))
        semi = code.find(";", j)
        brace = code.find("{", j)
        if brace >= 0 and (semi < 0 or brace < semi):
            end = match_brace(code, brace)
        else:
            end = semi if semi >= 0 else j
        for k in range(m.start(), end + 1):
            if out[k] != "\n":
                out[k] = " "
    return "".join(out)


def is_test_path(rel: str) -> bool:
    base = os.path.basename(rel)
    return (
        "/tests/" in "/" + rel
        or base in ("tests.rs", "test_support.rs", "test_env_lock.rs")
        or base.endswith(("_tests.rs", "_test.rs", "_acceptance.rs"))
        or base == "golden_harness.rs"
        or "goldens" in rel
        or "/fixtures/" in "/" + rel
    )


def top_module(rel: str) -> str:
    parts = rel.split("/")
    if len(parts) == 1:
        stem = parts[0][:-3]
        return {"lib": ROOT_ITEMS, "main": "main.rs"}.get(stem, stem)
    return parts[0]


def split_top(s: str) -> list[str]:
    parts, depth, cur = [], 0, ""
    for ch in s:
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append(cur)
            cur = ""
        else:
            cur += ch
    parts.append(cur)
    return [p.strip() for p in parts if p.strip()]


def use_leaves(tree: str, prefix: tuple[str, ...] = ()) -> list[tuple[str, tuple[str, ...]]]:
    """Flatten a use tree into (bound name, full path) pairs."""
    tree = tree.strip()
    if not tree:
        return []
    brace = tree.find("{")
    if brace >= 0:
        head = [p for p in tree[:brace].split("::") if p.strip()]
        inner = tree[brace + 1 : tree.rindex("}")]
        out = []
        for part in split_top(inner):
            out.extend(use_leaves(part, prefix + tuple(h.strip() for h in head)))
        return out
    alias = None
    m = re.match(r"(.*?)\s+as\s+(\w+)$", tree)
    if m:
        tree, alias = m.group(1), m.group(2)
    path = prefix + tuple(p.strip() for p in tree.split("::") if p.strip())
    if not path:
        return []
    name = alias or path[-1]
    if name == "self":
        name = path[-2] if len(path) > 1 else name
    return [(name, path)]


# --------------------------------------------------------------------------
# Graph construction
# --------------------------------------------------------------------------


@dataclass
class Ref:
    kind: str  # "prod" or "test"
    src: str  # module
    dst: str  # module, or lib.rs
    file: str
    line: int
    text: str


@dataclass
class Crate:
    name: str
    root: Path
    files: dict[str, tuple[str, str, str]] = field(default_factory=dict)
    modules: set[str] = field(default_factory=set)
    # names bound at the crate root by `use` -> resolving module (or "extern")
    root_names: dict[str, str] = field(default_factory=dict)
    # modules whose every file is test code (e.g. `#[cfg(test)] mod test_support;`)
    test_only: set[str] = field(default_factory=set)


USE_RE = re.compile(r"\buse\s+crate::")
PATH_RE = re.compile(r"(?<![\$\w])crate::(\w+)")
UILIB_RE = re.compile(r"(?<![\w:])(ratatui|crossterm|codewhale_tui|codewhale_palette)(?:::|\s*;|\s*\{)")
DOC_LINK_RE = re.compile(r"\[`?crate::(tui|commands)\b")


def load_crate(name: str, root: Path) -> Crate:
    crate = Crate(name, root)
    if not root.is_dir():
        return crate
    for dirpath, _, filenames in os.walk(root):
        for fname in filenames:
            if not fname.endswith(".rs"):
                continue
            rel = os.path.relpath(os.path.join(dirpath, fname), root).replace(os.sep, "/")
            src = Path(dirpath, fname).read_text(encoding="utf-8", errors="replace")
            code = lex_mask(src)
            crate.files[rel] = (src, code, "")
    crate.modules = {top_module(r) for r in crate.files} - {ROOT_ITEMS, "main.rs"}
    return crate


def resolve_root_names(crate: Crate, runtime_modules: set[str]) -> None:
    """Map names the crate root imports with `use` to the module they come from."""
    entry = crate.files.get("lib.rs")
    if not entry:
        return
    code = entry[1]
    depth = 0
    i = 0
    # Only top-level `use` items (brace depth 0) bind crate-root names.
    for m in re.finditer(r"[{}]|\b(?:pub(?:\([^)]*\))?\s+)?use\s+", code):
        tok = m.group(0)
        if tok == "{":
            depth += 1
            continue
        if tok == "}":
            depth -= 1
            continue
        if depth != 0 or m.start() < i:
            continue
        end = code.find(";", m.end())
        i = end
        for bound, path in use_leaves(code[m.end() : end]):
            head = path[0]
            if head == "crate" and len(path) > 1:
                target = path[1]
                crate.root_names[bound] = target if target in crate.modules else ROOT_ITEMS
            elif head in ("self", "super"):
                continue
            elif head in crate.modules:
                crate.root_names[bound] = head
            elif head == "codewhale_runtime" and len(path) > 1:
                crate.root_names[bound] = path[1]
            else:
                crate.root_names[bound] = "extern"


def test_file_set(crate: Crate) -> tuple[set[str], set[str]]:
    exact, prefixes = set(), set()
    for rel, (src, code, _) in list(crate.files.items()):
        decls: list[str] = []
        stripped = strip_cfg_test(code, decls)
        crate.files[rel] = (src, code, stripped)
        base_dir = os.path.dirname(rel)
        stem = os.path.basename(rel)[:-3]
        mod_dir = base_dir if stem in ("mod", "lib", "main") else os.path.join(base_dir, stem)
        for d in decls:
            exact.add(os.path.normpath(os.path.join(mod_dir, d + ".rs")))
            exact.add(os.path.normpath(os.path.join(mod_dir, d, "mod.rs")))
            prefixes.add(os.path.normpath(os.path.join(mod_dir, d)) + "/")
    return exact, prefixes


def file_is_test(rel: str, exact: set[str], prefixes: set[str]) -> bool:
    return (
        is_test_path(rel)
        or rel in exact
        or any(rel.startswith(p) for p in prefixes)
    )


def collect_refs(crate: Crate, all_modules: set[str], prefix: str) -> list[Ref]:
    exact, prefixes = test_file_set(crate)
    refs: list[Ref] = []
    by_module: dict[str, list[bool]] = collections.defaultdict(list)
    for rel in crate.files:
        by_module[top_module(rel)].append(file_is_test(rel, exact, prefixes))
    crate.test_only = {m for m, flags in by_module.items() if all(flags)}

    def target_of(name: str) -> str:
        if name in all_modules:
            return name
        return crate.root_names.get(name, ROOT_ITEMS)

    for rel, (src, code, stripped) in crate.files.items():
        mod = top_module(rel)
        whole_test = file_is_test(rel, exact, prefixes)
        lines = src.split("\n")

        def kind_at(pos: int) -> str:
            if whole_test:
                return "test"
            return "prod" if stripped[pos] == code[pos] and not stripped[pos].isspace() else "test"

        covered: set[int] = set()
        for m in USE_RE.finditer(code):
            end = code.find(";", m.end())
            if end < 0:
                continue
            covered.update(range(m.start(), end))
            body = code[m.end() : end]
            line = code.count("\n", 0, m.start()) + 1
            for _, path in use_leaves(body):
                t = path[0]
                if t in ("self", "super"):
                    continue
                refs.append(Ref(kind_at(m.start()), mod, target_of(t), f"{prefix}/{rel}", line, lines[line - 1].strip()[:160]))
        for m in PATH_RE.finditer(code):
            if m.start() in covered:
                continue
            line = code.count("\n", 0, m.start()) + 1
            refs.append(Ref(kind_at(m.start()), mod, target_of(m.group(1)), f"{prefix}/{rel}", line, lines[line - 1].strip()[:160]))
    return refs


@dataclass
class Report:
    closure: list[str]
    counts: dict[str, dict[str, int]]
    locations: dict[str, dict[str, list[str]]]


def build_report(tui_src: Path = TUI_SRC, runtime_src: Path = RUNTIME_SRC) -> Report:
    tui = load_crate("codewhale-tui", tui_src)
    runtime = load_crate("codewhale-runtime", runtime_src)
    all_modules = tui.modules | runtime.modules
    resolve_root_names(tui, runtime.modules)
    resolve_root_names(runtime, runtime.modules)
    tui_refs = collect_refs(tui, all_modules, "crates/tui/src")
    rt_refs = collect_refs(runtime, runtime.modules, "crates/runtime/src")
    refs = tui_refs + rt_refs

    prod_edges: dict[str, set[str]] = collections.defaultdict(set)
    for r in refs:
        if r.kind == "prod" and r.src != r.dst:
            prod_edges[r.src].add(r.dst)

    # A test-only module that closure tests use (`test_support`) compiles in
    # the same crate as those tests, so it belongs to the closure too and its
    # own upward references count as test references.
    test_only = tui.test_only | runtime.test_only
    test_edges: dict[str, set[str]] = collections.defaultdict(set)
    for r in refs:
        if r.src != r.dst and r.dst in test_only:
            test_edges[r.src].add(r.dst)

    seen: set[str] = set()
    stack = [s for s in SEEDS if s in all_modules] + sorted(runtime.modules)
    while stack:
        m = stack.pop()
        if m in seen or is_ui(m) or m == "extern":
            continue
        seen.add(m)
        stack.extend(prod_edges.get(m, ()))
        stack.extend(test_edges.get(m, ()))

    counts = {c: collections.Counter() for c in CATEGORIES}
    where: dict[str, dict[str, list[str]]] = {c: collections.defaultdict(list) for c in CATEGORIES}
    for r in refs:
        if r.src not in seen or r.src == r.dst or r.dst == "extern":
            continue
        if is_ui(r.dst):
            cat = r.kind
        elif r.dst not in seen:
            cat = "late"
        else:
            continue
        key = f"{r.src}|{r.dst}"
        counts[cat][key] += 1
        where[cat][key].append(f"{r.file}:{r.line}: {r.text}")

    for crate, prefix in ((tui, "crates/tui/src"), (runtime, "crates/runtime/src")):
        for rel, (src, code, _) in crate.files.items():
            mod = top_module(rel)
            if mod not in seen:
                continue
            lines = src.split("\n")
            for m in UILIB_RE.finditer(code):
                lib = m.group(1)
                if crate is runtime and lib == "codewhale_palette":
                    # The runtime crate links palette without its ratatui
                    # feature; the cargo-tree rule proves that side.
                    continue
                line = code.count("\n", 0, m.start()) + 1
                key = f"{mod}|{lib}"
                counts["uilib"][key] += 1
                where["uilib"][key].append(f"{prefix}/{rel}:{line}: {lines[line - 1].strip()[:160]}")
            for line_no, raw in enumerate(lines, start=1):
                stripped = raw.lstrip()
                if not (stripped.startswith("///") or stripped.startswith("//!")):
                    continue
                for m in DOC_LINK_RE.finditer(stripped):
                    key = f"{mod}|{m.group(1)}"
                    counts["doc"][key] += 1
                    where["doc"][key].append(f"{prefix}/{rel}:{line_no}: {stripped[:160]}")

    return Report(
        sorted(seen),
        {c: dict(sorted(counts[c].items())) for c in CATEGORIES},
        {c: dict(where[c]) for c in CATEGORIES},
    )


# --------------------------------------------------------------------------
# Ratchet
# --------------------------------------------------------------------------


def totals(counts: dict[str, dict[str, int]]) -> dict[str, int]:
    return {c: sum(counts.get(c, {}).values()) for c in CATEGORIES}


def compare(baseline: dict, report: Report) -> tuple[list[str], list[str]]:
    """Return (increases, unrecorded decreases) as human-readable lines."""
    rises: list[str] = []
    drops: list[str] = []
    base_counts = baseline.get("counts", {})
    for cat in CATEGORIES:
        base = base_counts.get(cat, {})
        cur = report.counts.get(cat, {})
        for key, n in cur.items():
            b = base.get(key, 0)
            if n > b:
                dst = key.split("|", 1)[1]
                hint = HINTS.get(dst, "see docs/design/TUI_DECONSTRUCTION.md (runtime split blockers)")
                head = f"{cat} {key}: {b} -> {n}" + (" (new pair)" if key not in base else "")
                rises.append(f"{head}; {hint}")
                for loc in report.locations.get(cat, {}).get(key, [])[:20]:
                    rises.append(f"    {loc}")
        for key, b in base.items():
            n = cur.get(key, 0)
            if n < b:
                drops.append(f"{cat} {key}: {b} -> {n}")
    return rises, drops


def load_baseline(path: Path = BASELINE) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def baseline_document(report: Report) -> dict:
    return {
        "_comment": (
            "Runtime -> UI boundary ratchet (docs/design/TUI_DECONSTRUCTION.md, runtime split). Counts may "
            "only go down. Regenerate with python3 scripts/split/module_graph.py "
            "--update after removing references; never raise a count by hand."
        ),
        "totals": totals(report.counts),
        "counts": report.counts,
    }


def check(path: Path = BASELINE, report: Report | None = None) -> list[str]:
    """Return violation lines (empty when the ratchet holds)."""
    report = report or build_report()
    if not path.is_file():
        return [f"missing baseline {path.relative_to(REPO_ROOT)}; run with --update"]
    rises, drops = compare(load_baseline(path), report)
    problems = []
    if rises:
        problems.append("runtime -> UI references rose (the ratchet only goes down):")
        problems.extend(f"  {r}" for r in rises)
    if drops:
        problems.append(
            "runtime -> UI references dropped but the baseline was not lowered; "
            "run python3 scripts/split/module_graph.py --update and commit it:"
        )
        problems.extend(f"  {d}" for d in drops)
    return problems


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="enforce the ratchet (default)")
    mode.add_argument("--update", action="store_true", help="lower the baseline to the current counts")
    mode.add_argument("--report", action="store_true", help="print counts and closure as JSON")
    args = parser.parse_args(argv)

    report = build_report()
    if args.report:
        json.dump(
            {"closure": report.closure, "totals": totals(report.counts), "counts": report.counts,
             "locations": report.locations},
            sys.stdout,
            indent=1,
        )
        print()
        return 0
    if args.update:
        if BASELINE.is_file():
            rises, _ = compare(load_baseline(), report)
            if rises:
                print("[runtime-boundary] refusing to raise the baseline:", file=sys.stderr)
                for r in rises:
                    print(f"  {r}", file=sys.stderr)
                return 1
        BASELINE.write_text(json.dumps(baseline_document(report), indent=2) + "\n", encoding="utf-8")
        print(f"[runtime-boundary] baseline written: {totals(report.counts)}")
        return 0
    problems = check(report=report)
    if problems:
        print("[runtime-boundary] FAIL", file=sys.stderr)
        for p in problems:
            print(p, file=sys.stderr)
        return 1
    print(f"[runtime-boundary] PASS: {totals(report.counts)} ({len(report.closure)} closure modules)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
