#!/usr/bin/env python3
"""Hermetic tests for the blocking-calls budget gate (#6149)."""

from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "check-blocking-calls-budget.py"
SPEC = importlib.util.spec_from_file_location("blocking_calls", SCRIPT)
assert SPEC and SPEC.loader
mod = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = mod
SPEC.loader.exec_module(mod)


def counts(source: str) -> dict[str, int]:
    with tempfile.TemporaryDirectory() as tmp:
        victim = Path(tmp) / "victim.rs"
        victim.write_text(source, encoding="utf-8")
        return mod.file_counts(victim)


class BlockingCallScopeTests(unittest.TestCase):
    def test_sleep_in_plain_fn_counts(self) -> None:
        self.assertEqual(
            counts("fn wait() {\n    std::thread::sleep(std::time::Duration::from_millis(1));\n}\n"),
            {"thread_sleep": 1},
        )

    def test_sleep_in_async_fn_counts(self) -> None:
        self.assertEqual(
            counts("async fn run() {\n    std::thread::sleep(std::time::Duration::from_millis(1));\n}\n"),
            {"thread_sleep": 1},
        )

    def test_sleep_in_spawn_blocking_is_exempt(self) -> None:
        self.assertEqual(
            counts(
                "async fn run() {\n"
                "    tokio::task::spawn_blocking(move || {\n"
                "        std::thread::sleep(std::time::Duration::from_millis(1));\n"
                "    });\n"
                "}\n"
            ),
            {},
        )

    def test_sleep_in_dedicated_thread_is_exempt(self) -> None:
        self.assertEqual(
            counts(
                "fn pump() {\n"
                "    std::thread::Builder::new().spawn(move || {\n"
                "        std::thread::sleep(std::time::Duration::from_millis(5));\n"
                "    });\n"
                "}\n"
            ),
            {},
        )

    def test_sleep_in_tests_mod_is_exempt(self) -> None:
        self.assertEqual(
            counts(
                "fn prod() {}\n"
                "#[cfg(test)]\n"
                "mod tests {\n"
                "    fn probe() { std::thread::sleep(std::time::Duration::from_millis(1)); }\n"
                "}\n"
            ),
            {},
        )

    def test_sleep_in_cfg_test_fn_is_exempt(self) -> None:
        self.assertEqual(
            counts(
                "#[cfg(test)]\n"
                "fn helper() { std::thread::sleep(std::time::Duration::from_millis(1)); }\n"
            ),
            {},
        )

    def test_std_fs_call_counts(self) -> None:
        self.assertEqual(
            counts("async fn go() {\n    let _ = std::fs::read_to_string(p).unwrap();\n}\n"),
            {"std_fs": 1},
        )

    def test_std_fs_imports_and_signature_types_do_not_count(self) -> None:
        self.assertEqual(
            counts(
                "use std::fs::File;\n"
                "use std::fs::OpenOptions;\n"
                "use std::fs::DirBuilder;\n"
                "type Handle = std::fs::File;\n"
                "fn types(_: &std::fs::File, _: std::fs::OpenOptions, "
                "_: std::fs::DirBuilder) -> Option<std::fs::File> { None }\n"
            ),
            {},
        )

    def test_std_fs_qualified_member_operations_count(self) -> None:
        for operation in (
            "std::fs::File::open(path)",
            "std::fs::File::create(path)",
            "std::fs::File::options()",
            "std::fs::OpenOptions::new()",
            "std::fs::DirBuilder::new()",
            "std::fs::File :: open(path)",
        ):
            with self.subTest(operation=operation):
                self.assertEqual(
                    counts(f"async fn run() {{ let _ = {operation}; }}\n"),
                    {"std_fs": 1},
                )

    def test_std_fs_qualified_member_in_spawn_blocking_is_exempt(self) -> None:
        self.assertEqual(
            counts(
                "async fn run() {\n"
                "    tokio::task::spawn_blocking(move || {\n"
                "        let _ = std::fs::File::open(path);\n"
                "    });\n"
                "}\n"
            ),
            {},
        )

    def test_comment_and_string_literals_do_not_count(self) -> None:
        self.assertEqual(
            counts(
                "fn doc() {\n"
                "    // std::thread::sleep(std::time::Duration::from_millis(1));\n"
                '    let s = "std::fs::read_to_string(p)";\n'
                "    let t = r#\"std::fs::write(a, b)\"#;\n"
                "}\n"
            ),
            {},
        )

    def test_tokio_equivalents_do_not_count(self) -> None:
        self.assertEqual(
            counts(
                "async fn go() {\n"
                "    tokio::time::sleep(std::time::Duration::from_millis(1)).await;\n"
                "    let _ = tokio::fs::read_to_string(p).await;\n"
                "}\n"
            ),
            {},
        )


class CfgTestModuleExclusion(unittest.TestCase):
    """A file that is wholly a `#[cfg(test)]` module is test code (#6149).

    The per-file scanner only sees test scope declared *inside* a file, so an
    extracted test suite looked like brand-new unprotected call sites even
    though nothing moved onto an async path. PR #6096's
    `session_export_*_tests.rs` reddened `main` this way.
    """

    def _crates(self, tmp: Path, files: dict[str, str]) -> Path:
        crates = tmp / "crates" / "demo" / "src"
        crates.mkdir(parents=True)
        for name, body in files.items():
            target = crates / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(body, encoding="utf-8")
        return tmp / "crates"

    def test_whole_file_cfg_test_module_is_excluded(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            crates = self._crates(
                root,
                {
                    "lib.rs": "#[cfg(test)]\nmod suite;\n",
                    "suite.rs": "fn helper() { let _ = std::fs::read_to_string(p); }\n",
                },
            )
            original = mod.CRATES
            try:
                mod.CRATES = crates
                excluded = mod.cfg_test_module_files()
            finally:
                mod.CRATES = original
            self.assertIn((crates / "demo" / "src" / "suite.rs").resolve(), excluded)

    def test_plain_mod_declaration_is_not_excluded(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            crates = self._crates(
                root,
                {
                    "lib.rs": "mod production;\n",
                    "production.rs": "fn helper() { let _ = std::fs::read_to_string(p); }\n",
                },
            )
            original = mod.CRATES
            try:
                mod.CRATES = crates
                excluded = mod.cfg_test_module_files()
            finally:
                mod.CRATES = original
            self.assertNotIn(
                (crates / "demo" / "src" / "production.rs").resolve(), excluded
            )

    def _excluded(self, root: Path, files: dict[str, str]) -> tuple[Path, set[Path]]:
        crates = self._crates(root, files)
        original = mod.CRATES
        try:
            mod.CRATES = crates
            return crates / "demo" / "src", mod.cfg_test_module_files()
        finally:
            mod.CRATES = original

    def test_inline_cfg_test_item_include_is_excluded(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            source, excluded = self._excluded(Path(tmp), {
                "lib.rs": '#[cfg(test)] mod verification { include!("part.rs"); }',
                "part.rs": 'fn helper() { std::fs::read("fixture"); }',
            })
            self.assertIn((source / "part.rs").resolve(), excluded)

    def test_path_module_and_recursive_includes_inherit_test_only_scope(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            source, excluded = self._excluded(Path(tmp), {
                "lib.rs": '#[cfg(test)] #[path = "suite.rs"] mod verification;',
                "suite.rs": 'include!("pieces/first.rs");',
                "pieces/first.rs": 'include!(r"second.rs");',
                "pieces/second.rs": 'fn helper() { std::fs::read("fixture"); }',
            })
            for name in ("suite.rs", "pieces/first.rs", "pieces/second.rs"):
                self.assertIn((source / name).resolve(), excluded)

    def test_test_looking_plain_module_and_filename_are_not_exempt(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            source, excluded = self._excluded(Path(tmp), {
                "lib.rs": 'mod tests { include!("test_cases.rs"); }',
                "test_cases.rs": 'fn helper() { std::fs::read("fixture"); }',
            })
            self.assertNotIn((source / "test_cases.rs").resolve(), excluded)

    def test_comments_and_string_include_or_cfg_lookalikes_are_not_exempt(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            source, excluded = self._excluded(Path(tmp), {
                "lib.rs": '''// #[cfg(test)] mod commented;
/* #[cfg(test)] mod blocked { include!("part.rs"); } */
const A: &str = "#[cfg(test)] mod stringed; include!(\\\"part.rs\\\");";
const B: &str = r#"#[cfg(test)] mod raw; include!("part.rs");"#;
const C: &str = r"backslash \\";
include!("production.rs");
''',
                "commented.rs": '', "stringed.rs": '', "raw.rs": '',
                "part.rs": '', "production.rs": '',
            })
            self.assertFalse(excluded)
            self.assertNotIn((source / "production.rs").resolve(), excluded)

    def test_production_include_blocks_shared_fragment_and_descendants(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            source, excluded = self._excluded(Path(tmp), {
                "lib.rs": 'include!("shared.rs"); #[cfg(test)] mod suite { include!("shared.rs"); }',
                "shared.rs": 'include!("child.rs");',
                "child.rs": 'fn helper() { std::fs::read("fixture"); }',
            })
            for name in ("shared.rs", "child.rs"):
                self.assertNotIn((source / name).resolve(), excluded)

    def test_production_path_module_blocks_test_alias(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            source, excluded = self._excluded(Path(tmp), {
                "lib.rs": '#[path="shared.rs"] mod production; #[cfg(test)] #[path="shared.rs"] mod suite;',
                "shared.rs": 'include!("child.rs");',
                "child.rs": 'fn helper() { std::fs::read("fixture"); }',
            })
            self.assertNotIn((source / "shared.rs").resolve(), excluded)
            self.assertNotIn((source / "child.rs").resolve(), excluded)

    def test_cfg_test_function_include_is_not_a_module_exemption(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            source, excluded = self._excluded(Path(tmp), {
                "lib.rs": '#[cfg(test)] fn probe() { include!("part.rs"); }',
                "part.rs": 'fn helper() { std::fs::read("fixture"); }',
            })
            self.assertNotIn((source / "part.rs").resolve(), excluded)

    def test_dynamic_include_and_mixed_cfg_are_not_inferred_as_test_only(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            source, excluded = self._excluded(Path(tmp), {
                "lib.rs": '#[cfg(test)] mod suite { include!(concat!("part", ".rs")); }\n#[cfg(any(test, feature="shipping"))] mod shipped { include!("shipping.rs"); }',
                "part.rs": '', "shipping.rs": '',
            })
            self.assertFalse(excluded)
            self.assertNotIn((source / "part.rs").resolve(), excluded)

    def test_default_crate_roots_and_production_children_stay_counted_when_included_by_tests(self) -> None:
        for entry in ("lib.rs", "main.rs"):
            with self.subTest(entry=entry), tempfile.TemporaryDirectory() as tmp:
                source, excluded = self._excluded(Path(tmp), {
                    "lib.rs": '#[cfg(test)] mod suite { include!("' + entry + '"); }',
                    entry: 'mod production; #[cfg(test)] mod suite { include!("' + entry + '"); }',
                    "production.rs": 'fn run() { std::fs::read("real"); }',
                })
                self.assertNotIn((source / entry).resolve(), excluded)
                self.assertNotIn((source / "production.rs").resolve(), excluded)

    def test_cargo_custom_lib_root_keeps_unguarded_module_and_include_descendants_counted(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            source, excluded = self._excluded(Path(tmp), {
                "../Cargo.toml": '[package]\nname="demo"\nversion="0.1.0"\n[lib]\npath="src/custom.rs"\n',
                "lib.rs": '#[cfg(test)] mod suite { include!("custom.rs"); }',
                "custom.rs": 'mod production; include!("fragment.rs");',
                "custom/production.rs": 'fn run() { std::fs::read("real"); }',
                "fragment.rs": 'fn run() { std::thread::sleep(delay); }',
            })
            for name in ("custom.rs", "custom/production.rs", "fragment.rs"):
                self.assertNotIn((source / name).resolve(), excluded)

    def test_cargo_custom_binary_root_outside_src_stays_counted_when_test_reachable(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            source, excluded = self._excluded(Path(tmp), {
                "../Cargo.toml": '[package]\nname="demo"\nversion="0.1.0"\n[[bin]]\nname="launcher"\npath="front/launch.rs"\n',
                "lib.rs": '#[cfg(test)] mod suite { include!("../front/launch.rs"); }',
                "../front/launch.rs": 'include!("production.rs");',
                "../front/production.rs": 'fn run() { std::fs::read("real"); }',
            })
            for name in ("../front/launch.rs", "../front/production.rs"):
                self.assertNotIn((source / name).resolve(), excluded)

    def test_automatic_binary_roots_remain_counted(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            source, excluded = self._excluded(Path(tmp), {
                "lib.rs": '#[cfg(test)] mod suite { include!("bin/one.rs"); include!("bin/two/main.rs"); }',
                "bin/one.rs": 'include!("child.rs");',
                "bin/two/main.rs": 'include!("../child.rs");',
                "bin/child.rs": 'fn run() { std::fs::read("real"); }',
            })
            for name in ("bin/one.rs", "bin/two/main.rs", "bin/child.rs"):
                self.assertNotIn((source / name).resolve(), excluded)

    def test_recursive_test_include_cycle_terminates(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            source, excluded = self._excluded(Path(tmp), {
                "lib.rs": '#[cfg(test)] mod suite { include!("first.rs"); }',
                "first.rs": 'include!("second.rs");',
                "second.rs": 'include!("first.rs");',
            })
            self.assertEqual(excluded, {(source / name).resolve() for name in ("first.rs", "second.rs")})


class ExplicitModuleGraphScopeTests(unittest.TestCase):
    _crates = CfgTestModuleExclusion._crates

    def test_cargo_integration_target_and_declared_helpers_are_test_only(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            crates = self._crates(Path(tmp), {
                "../Cargo.toml": '[package]\nname="demo"\nversion="0.1.0"\n',
                "lib.rs": 'fn shipping() {}',
                "../tests/scenario.rs": 'mod support; fn probe() { std::fs::read("fixture"); }',
                "../tests/support/mod.rs": 'fn helper() { std::fs::read("fixture"); }',
            })
            package = crates / "demo"
            test_only, _ = mod.module_file_scopes(package)
            self.assertIn((package / "tests/scenario.rs").resolve(), test_only)
            self.assertIn((package / "tests/support/mod.rs").resolve(), test_only)
            self.assertNotIn((package / "src/lib.rs").resolve(), test_only)

    def test_disabled_automatic_tests_do_not_exempt_an_unclaimed_path(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            crates = self._crates(Path(tmp), {
                "../Cargo.toml": '[package]\nname="demo"\nversion="0.1.0"\nautotests=false\n',
                "lib.rs": 'fn shipping() {}',
                "../tests/scenario.rs": 'fn run() { std::fs::read("real"); }',
            })
            package = crates / "demo"
            self.assertNotIn((package / "tests/scenario.rs").resolve(), mod.cfg_test_module_files(package))

    def test_explicit_test_target_never_exempts_a_shipping_binary(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            crates = self._crates(Path(tmp), {
                "../Cargo.toml": '[package]\nname="demo"\nversion="0.1.0"\n[[bin]]\nname="launch"\npath="front/launch.rs"\n[[test]]\nname="probe"\npath="front/launch.rs"\n',
                "lib.rs": 'fn shipping() {}',
                "../front/launch.rs": 'include!("child.rs");',
                "../front/child.rs": 'fn run() { std::fs::read("real"); }',
            })
            package = crates / "demo"
            test_only, production = mod.module_file_scopes(package)
            for name in ("front/launch.rs", "front/child.rs"):
                self.assertNotIn((package / name).resolve(), test_only)
                self.assertIn((package / name).resolve(), production)

    def test_explicit_test_path_overrides_automatic_name_without_exempting_both(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            crates = self._crates(Path(tmp), {
                "../Cargo.toml": '[package]\nname="demo"\nversion="0.1.0"\n[[test]]\nname="scenario"\npath="probes/actual.rs"\n',
                "lib.rs": 'fn shipping() {}',
                "../tests/scenario.rs": 'fn unclaimed() {}',
                "../probes/actual.rs": 'fn actual() {}',
            })
            package = crates / "demo"
            test_only, _ = mod.module_file_scopes(package)
            self.assertIn((package / "probes/actual.rs").resolve(), test_only)
            self.assertNotIn((package / "tests/scenario.rs").resolve(), test_only)

    def test_explicit_source_root_does_not_mutate_or_consult_global_crates(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            source = self._crates(Path(tmp), {
                "lib.rs": '#[cfg(test)] mod suite { include!("piece.rs"); }',
                "piece.rs": 'fn probe() {}',
            }) / "demo" / "src"
            original = mod.CRATES
            self.assertEqual(mod.cfg_test_module_files(source), {(source / "piece.rs").resolve()})
            self.assertEqual(mod.CRATES, original)

    def test_production_conflicts_include_descendants_and_override_test_names(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            source = self._crates(Path(tmp), {
                "lib.rs": 'include!("shared_tests.rs"); #[cfg(test)] mod suite { include!("shared_tests.rs"); }',
                "shared_tests.rs": 'include!("child_test.rs");',
                "child_test.rs": 'fn run() {}',
            }) / "demo" / "src"
            test_only, production = mod.module_file_scopes(source)
            self.assertEqual(test_only, set())
            for name in ("lib.rs", "shared_tests.rs", "child_test.rs"):
                self.assertIn((source / name).resolve(), production)

    def test_custom_root_outside_source_preserves_production_descendants(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            crates = self._crates(Path(tmp), {
                "../Cargo.toml": '[package]\nname="demo"\nversion="0.1.0"\n[lib]\npath="entry.rs"\n',
                "lib.rs": '#[cfg(test)] mod suite { include!("../entry.rs"); }',
                "../entry.rs": 'include!("src/child_tests.rs");',
                "child_tests.rs": 'fn run() {}',
            })
            source = crates / "demo" / "src"
            test_only, production = mod.module_file_scopes(source.parent)
            self.assertNotIn((source / "child_tests.rs").resolve(), test_only)
            self.assertIn((source / "child_tests.rs").resolve(), production)


if __name__ == "__main__":
    unittest.main()
