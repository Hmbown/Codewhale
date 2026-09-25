#!/usr/bin/env python3
"""Hermetic tests for the FEAT-014 command-contract boundary gate."""

from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "check-command-crate-boundaries.py"
SPEC = importlib.util.spec_from_file_location("command_boundary", SCRIPT)
assert SPEC and SPEC.loader
mod = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = mod
SPEC.loader.exec_module(mod)


def valid_graph() -> dict[str, set[str]]:
    return {
        "codewhale-command-contract": {"codewhale-core"},
        "codewhale-core": set(),
        "codewhale-secrets": {"codewhale-paths"},
        "codewhale-paths": set(),
        "codewhale-tui": set(),
    }


class DependencyTests(unittest.TestCase):
    def test_leaf_graph_passes(self) -> None:
        self.assertEqual(mod.check_dependency_graph(valid_graph()), [])

    def test_direct_tui_edge_fails(self) -> None:
        graph = valid_graph()
        graph["codewhale-command-contract"].add("codewhale-tui")
        self.assertEqual(len(mod.check_dependency_graph(graph)), 1)

    def test_transitive_tui_edge_fails(self) -> None:
        graph = valid_graph()
        graph["codewhale-core"].add("codewhale-tui")
        self.assertEqual(len(mod.check_dependency_graph(graph)), 1)

    def test_missing_contract_fails(self) -> None:
        graph = valid_graph()
        del graph["codewhale-command-contract"]
        violations = mod.check_dependency_graph(graph)
        self.assertEqual(len(violations), 1)
        self.assertIn("missing", str(violations[0]))

    def test_missing_sanitizer_fails(self) -> None:
        graph = valid_graph()
        del graph["codewhale-secrets"]
        violations = mod.check_dependency_graph(graph)
        self.assertEqual(len(violations), 1)
        self.assertIn("codewhale-secrets", str(violations[0]))

    def test_sanitizer_reaching_tui_fails(self) -> None:
        # The shared sanitizer is consumed by portable command helpers, so a TUI
        # edge would pull the whole TUI into the extracted command crate.
        graph = valid_graph()
        graph["codewhale-paths"].add("codewhale-tui")
        violations = mod.check_dependency_graph(graph)
        self.assertEqual(len(violations), 1)
        self.assertIn("codewhale-secrets", str(violations[0]))

    def test_both_packages_reaching_tui_fails_twice(self) -> None:
        graph = valid_graph()
        graph["codewhale-core"].add("codewhale-tui")
        graph["codewhale-paths"].add("codewhale-tui")
        self.assertEqual(len(mod.check_dependency_graph(graph)), 2)

    def test_dev_dependency_is_not_a_normal_edge(self) -> None:
        metadata = {"packages": [
            {"name": "codewhale-command-contract", "dependencies": [
                {"name": "codewhale-tui", "kind": "dev"},
                {"name": "codewhale-core", "kind": None},
            ]},
            {"name": "codewhale-core", "dependencies": []},
            {"name": "codewhale-secrets", "dependencies": []},
            {"name": "codewhale-tui", "dependencies": []},
        ]}
        graph = mod.dependency_graph(metadata)
        self.assertEqual(graph["codewhale-command-contract"], {"codewhale-core"})
        self.assertEqual(mod.check_dependency_graph(graph), [])


class SourceTests(unittest.TestCase):
    def test_clean_shapes_pass(self) -> None:
        source = "pub struct CommandContexts<'a> {}\npub trait CommandModelContext {}\n"
        self.assertEqual(mod.check_contract_source_text(source, "clean.rs"), [])

    def test_forbidden_edges_fail(self) -> None:
        cases = [
            "use codewhale_tui::tui::app::App;",
            "use ratatui::widgets::Paragraph;",
            "use crate::tui::App;",
            "pub struct CommandContext {}",
            "let handler: Box<dyn Fn()> = value;",
        ]
        for source in cases:
            with self.subTest(source=source):
                self.assertTrue(mod.check_contract_source_text(source, "sample.rs"))

    def test_comments_and_plural_envelope_pass(self) -> None:
        source = (
            "// Never import codewhale_tui or define CommandContext here.\n"
            "pub struct CommandContexts<'a> { marker: &'a str }\n"
        )
        self.assertEqual(mod.check_contract_source_text(source, "safe.rs"), [])


class TreeModeTests(unittest.TestCase):
    RULE = mod.BoundaryRule(
        "codewhale-runtime",
        "tree",
        ("codewhale-tui", "ratatui", "crossterm"),
        "the runtime must stay UI-free",
    )

    def test_parse_cargo_tree_keeps_names(self) -> None:
        text = "codewhale-runtime v0.10.0 (/x)\nanyhow v1.0.100\nratatui v0.30.2 (*)\n"
        self.assertEqual(mod.parse_cargo_tree(text), {"codewhale-runtime", "anyhow", "ratatui"})

    def test_clean_tree_passes(self) -> None:
        self.assertEqual(mod.check_tree_packages(self.RULE, {"anyhow", "serde"}), [])

    def test_ui_library_in_tree_fails(self) -> None:
        violations = mod.check_tree_packages(self.RULE, {"anyhow", "ratatui", "crossterm"})
        self.assertEqual(len(violations), 2)
        self.assertIn("ratatui", str(violations[1]) + str(violations[0]))


def write_tree(root: Path, files: dict[str, str]) -> None:
    for rel, text in files.items():
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")


class RatchetTests(unittest.TestCase):
    def setUp(self) -> None:
        self.graph = mod.load_runtime_ratchet()

    def report(self, tui: dict[str, str], runtime: dict[str, str] | None = None):
        with tempfile.TemporaryDirectory() as tmp:
            write_tree(Path(tmp, "tui"), tui)
            write_tree(Path(tmp, "runtime"), runtime or {})
            return self.graph.build_report(Path(tmp, "tui"), Path(tmp, "runtime"))

    def test_counts_grouped_imports_and_masks_comments_and_strings(self) -> None:
        report = self.report({
            "lib.rs": "mod core; mod tui;\n",
            "core.rs": (
                "use crate::{tui::App, tui::views::{A, B}};\n"
                "// crate::tui::ignored\n"
                "const S: &str = \"crate::tui::ignored\";\n"
                "fn f() { crate::tui::draw(); }\n"
                "#[cfg(test)]\nmod tests { fn t() { crate::tui::fixture(); } }\n"
            ),
            "tui.rs": "",
        })
        self.assertEqual(report.counts["prod"], {"core|tui": 4})
        self.assertEqual(report.counts["test"], {"core|tui": 1})

    def test_ui_library_and_late_edges_are_counted(self) -> None:
        report = self.report({
            "lib.rs": "mod core; mod exec_agent;\n",
            "core.rs": "fn f() { crossterm::terminal::enable_raw_mode(); }\n"
            "#[cfg(test)]\nmod tests { fn t() { crate::exec_agent::run(); } }\n",
            "exec_agent.rs": "",
        })
        self.assertEqual(report.counts["uilib"], {"core|crossterm": 1})
        self.assertEqual(report.counts["late"], {"core|exec_agent": 1})

    def test_runtime_crate_modules_join_the_closure(self) -> None:
        report = self.report(
            {"lib.rs": "mod core;\nuse codewhale_runtime::{elapsed};\n", "core.rs": ""},
            {"lib.rs": "pub mod elapsed;\n", "elapsed.rs": "fn f() { ratatui::x(); }\n"},
        )
        self.assertIn("elapsed", report.closure)
        self.assertEqual(report.counts["uilib"], {"elapsed|ratatui": 1})

    def test_rise_and_unrecorded_drop_both_fail(self) -> None:
        report = self.report({
            "lib.rs": "mod core; mod tui;\n",
            "core.rs": "fn f() { crate::tui::a(); crate::tui::b(); }\n",
            "tui.rs": "",
        })
        rises, drops = self.graph.compare({"counts": {"prod": {"core|tui": 1}}}, report)
        self.assertTrue(rises and rises[0].startswith("prod core|tui: 1 -> 2"))
        self.assertEqual(drops, [])
        rises, drops = self.graph.compare({"counts": {"prod": {"core|tui": 3}}}, report)
        self.assertEqual((rises, drops), ([], ["prod core|tui: 3 -> 2"]))

    def test_checked_in_baseline_holds(self) -> None:
        self.assertEqual(self.graph.check(), [])


if __name__ == "__main__":
    unittest.main()
