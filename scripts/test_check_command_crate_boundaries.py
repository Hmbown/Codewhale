#!/usr/bin/env python3
"""Hermetic tests for the FEAT-014 command-contract boundary gate."""

from __future__ import annotations

import importlib.util
import sys
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


if __name__ == "__main__":
    unittest.main()
