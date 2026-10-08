"""Negative controls for all four FEAT-026 extraction findings."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

path = Path(__file__).with_name("check-command-session-proof.py")
spec = importlib.util.spec_from_file_location("session_proof", path)
proof = importlib.util.module_from_spec(spec)
spec.loader.exec_module(proof)


class SessionProofTests(unittest.TestCase):
    def fixture(self, root):
        group = root / proof.GROUP
        wrapper = root / proof.PROOF
        group.parent.mkdir(parents=True)
        wrapper.parent.mkdir(parents=True)
        group.write_text((proof.ROOT / proof.GROUP).read_text())
        wrapper.write_text((proof.ROOT / proof.PROOF).read_text())
        return group, wrapper

    def test_real_whole_group_is_included(self):
        self.assertEqual(proof.violations(proof.ROOT), [])

    def test_selected_leaf_or_stub_is_rejected(self):
        for replacement in ['pub mod session {}', '#[path = "structcopy.rs"] pub mod session;']:
            with self.subTest(replacement=replacement), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                _, wrapper = self.fixture(root)
                wrapper.write_text(replacement)
                self.assertTrue(proof.violations(root))

    def test_sibling_action_and_host_registration_are_rejected(self):
        for source in ['use crate::tui::app::AppAction;',
                       'use crate::commands::traits::ContextualCommand;',
                       'use crate::commands::CommandResult;',
                       'use codewhale_secrets::sanitize;']:
            with self.subTest(source=source), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                group, _ = self.fixture(root)
                (group.parent / 'new_helper.rs').write_text(source)
                self.assertTrue(proof.violations(root))

    def test_indirect_runtime_and_credential_dependencies_are_rejected(self):
        for package in ['codewhale-core', 'codewhale-state', 'codewhale-secrets',
                        'codewhale-tui', 'keyring', 'dbus', 'reqwest', 'tokio', 'rusqlite']:
            with self.subTest(package=package):
                self.assertEqual(len(proof.graph_violations(f'{package} v1.0.0')), 1)

    def test_shared_shapes_and_pure_libraries_are_allowed(self):
        self.assertEqual(proof.graph_violations('codewhale-command-contract v0.10.1\ncodewhale-protocol v0.10.1\ncodewhale-sanitize v0.10.1\nserde v1.0.0\nurl v2.5.0'), [])
