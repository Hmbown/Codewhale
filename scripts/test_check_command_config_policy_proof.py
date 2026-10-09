"""Executable negative controls for the complete config-policy extraction proof."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

path = Path(__file__).with_name("check-command-config-policy-proof.py")
spec = importlib.util.spec_from_file_location("policy_proof", path)
proof = importlib.util.module_from_spec(spec)
spec.loader.exec_module(proof)


class PolicyProofTests(unittest.TestCase):
    def fixture(self, root):
        for relative in proof.REQUIRED | {proof.PROOF, proof.MANIFEST}:
            target = root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text((proof.ROOT / relative).read_text())

    def test_actual_complete_slice_is_included(self):
        self.assertEqual(proof.violations(proof.ROOT), [])

    def test_selected_leaf_stub_and_test_only_root_are_rejected(self):
        for source in ['pub mod policy {}', '#[path = "permissions.rs"] pub mod policy;',
                       '#[cfg(test)]\n' + (proof.ROOT / proof.PROOF).read_text()]:
            with self.subTest(source=source), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                self.fixture(root)
                (root / proof.PROOF).write_text(source)
                self.assertTrue(proof.violations(root))

    def test_omitted_status_or_helper_is_rejected(self):
        for name in ["status", "policy_messages", "money", "tests"]:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                self.fixture(root)
                group = root / proof.GROUP
                import re
                if name == 'money':
                    group.write_text(group.read_text().replace('use codewhale_command_contract::money;', ''))
                else:
                    group.write_text(re.sub(r'#\[path = "[^"]+"\]\s*(?:pub(?:\([^)]*\))?\s+)?mod ' + name + ';', '', group.read_text()))
                self.assertTrue(proof.violations(root))

    def test_new_transitive_helper_cannot_import_host_services(self):
        for source in ['use crate::tui::app::App;', 'use codewhale_config::Config;',
                       'use crate::commands::CommandResult;', 'use std::fs;', 'use codewhale_secrets::Store;']:
            with self.subTest(source=source), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                self.fixture(root)
                group = root / proof.GROUP
                group.write_text(group.read_text() + '\n#[path = "new_helper.rs"] mod helper;\n')
                group.with_name('new_helper.rs').write_text(source)
                self.assertTrue(proof.violations(root))

    def test_shapes_cannot_move_to_dev_dependencies(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.fixture(root)
            manifest = root / proof.MANIFEST
            manifest.write_text(manifest.read_text().replace('[dev-dependencies]', '[build-dependencies]').replace('[dependencies]', '[dev-dependencies]'))
            self.assertTrue(proof.violations(root))

    def test_direct_and_transitive_runtime_or_storage_edges_are_rejected(self):
        allowed = 'codewhale-portable-config-policy v0.10.1\ncodewhale-command-contract v0.10.1\ncodewhale-protocol v0.10.1\nserde v1.0.0\n'
        self.assertEqual(proof.graph_violations(allowed), [])
        for package in ['codewhale-tui', 'codewhale-config', 'codewhale-core', 'codewhale-runtime',
                        'codewhale-execpolicy', 'codewhale-secrets', 'codewhale-state', 'codewhale-localization',
                        'tokio', 'reqwest', 'rusqlite', 'keyring', 'ratatui', 'crossterm']:
            with self.subTest(package=package):
                self.assertEqual(len(proof.graph_violations(allowed + f'{package} v1.0.0\n')), 1)


if __name__ == '__main__':
    unittest.main(verbosity=2)
