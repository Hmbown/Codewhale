#!/usr/bin/env python3
"""Hermetic tests for the bare-reqwest-constructor gate (#6153)."""

from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "check-reqwest-builders.py"
SPEC = importlib.util.spec_from_file_location("reqwest_builders", SCRIPT)
assert SPEC and SPEC.loader
mod = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = mod
SPEC.loader.exec_module(mod)


class ReqwestBuilderTests(unittest.TestCase):
    def test_qualified_constructors_fail(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            victim = root / "tui.rs"
            victim.write_text(
                "let a = reqwest::Client::builder();\n"
                "let b = reqwest::Client::new();\n"
                "let c = reqwest::blocking::Client::builder();\n"
                "let d = reqwest::blocking::Client::new();\n",
                encoding="utf-8",
            )
            self.assertEqual(len(mod.find_violations(root)), 4)

    def test_bare_client_spelling_fails_with_reqwest_import(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            victim = root / "remote.rs"
            victim.write_text(
                "use reqwest::{Client, Method};\nlet client = Client::builder();\n",
                encoding="utf-8",
            )
            violations = mod.find_violations(root)
            self.assertEqual(len(violations), 1)
            self.assertIn("remote.rs:2", violations[0])

    def test_bare_client_without_reqwest_import_passes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            victim = root / "custom.rs"
            victim.write_text(
                "struct Client;\nimpl Client {\n    fn new() -> Self {\n        Client\n    }\n}\n"
                "let c = Client::new();\n",
                encoding="utf-8",
            )
            # No `use reqwest...Client` import, so this is a custom type.
            self.assertEqual(mod.find_violations(root), [])

    def test_tls_builders_and_comments_pass(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            victim = root / "ok.rs"
            victim.write_text(
                "// `reqwest::Client::builder()` panics under `rustls-no-provider`.\n"
                "let a = crate::tls::reqwest_client_builder();\n"
                "let b = codewhale_release::tls::reqwest_client();\n"
                "let c = codewhale_release::platform_blocking_http_client_builder();\n"
                "let d = FixedSummaryClient::default();\n",
                encoding="utf-8",
            )
            self.assertEqual(mod.find_violations(root), [])

    def test_release_crate_is_exempt(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            allowed_dir = root / "release" / "src"
            allowed_dir.mkdir(parents=True)
            sanctioned = allowed_dir / "lib.rs"
            sanctioned.write_text(
                "let builder = reqwest::Client::builder();\n", encoding="utf-8"
            )
            self.assertEqual(mod.find_violations(root, allowed_dir), [])


if __name__ == "__main__":
    unittest.main()
