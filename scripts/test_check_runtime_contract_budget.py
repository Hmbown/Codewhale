#!/usr/bin/env python3
"""Hermetic tests for scripts/check-runtime-contract-budget.py."""

from __future__ import annotations

import importlib.util
import io
import json
import os
import stat
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "check-runtime-contract-budget.py"

SPEC = importlib.util.spec_from_file_location("check_runtime_contract_budget", SCRIPT)
assert SPEC and SPEC.loader
mod = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = mod
SPEC.loader.exec_module(mod)


def receipt_fixture() -> dict:
    def stage(stage_id: str, byte_count: int, delta: int | None = None) -> dict:
        value = {
            "bytes": byte_count,
            "identity_sha256": mod.hashlib.sha256(stage_id.encode()).hexdigest(),
        }
        if delta is not None:
            value["delta_bytes"] = delta
        return value

    def tool_surface(names: list[str], byte_count: int) -> dict:
        names = sorted(names)
        return {
            "bytes": byte_count,
            "identity_sha256": mod.tool_identity_digest(names),
            "tokens_est": (byte_count + 3) // 4,
            "tool_names": names,
            "tools": len(names),
        }

    return {
        "document_kind": mod.RECEIPT_KIND,
        "schema_version": mod.SCHEMA_VERSION,
        "representative_context": {
            "fixture_id": mod.REPRESENTATIVE_FIXTURE_ID,
            "prompts_byte_identical": True,
            "stages": {
                "base": stage("base", 24000),
                "project": stage("project", 24100, 100),
                "instructions": stage("instructions", 24220, 120),
                "skill": stage("skill", 24360, 140),
                "memory": stage("memory", 24450, 90),
                "goal": stage("goal", 24540, 90),
                "handoff": stage("handoff", 24780, 240),
            },
            "system_prompt_blocks": 6,
            "total_bytes": 24780,
            "total_tokens_est": 6195,
        },
        "skill_discovery": {
            "first_delta": {
                "directories_visited": 1,
                "root_discovery_calls": 1,
                "skill_md_read_attempts": 1,
            },
            "prompts_byte_identical": True,
            "second_delta": {
                "directories_visited": 1,
                "root_discovery_calls": 1,
                "skill_md_read_attempts": 1,
            },
        },
        "system_prompt": {
            "modes": {
                "plan": {
                    "system_prompt_bytes": 24000,
                    "system_prompt_tokens_est": 6000,
                    "system_prompt_blocks": 4,
                    "mode_instructions_bytes": 700,
                    "mode_instructions_tokens_est": 175,
                },
                "act": {
                    "system_prompt_bytes": 25040,
                    "system_prompt_tokens_est": 6260,
                    "system_prompt_blocks": 4,
                    "mode_instructions_bytes": 805,
                    "mode_instructions_tokens_est": 202,
                },
                "operate": {
                    "system_prompt_bytes": 24500,
                    "system_prompt_tokens_est": 6125,
                    "system_prompt_blocks": 4,
                    "mode_instructions_bytes": 750,
                    "mode_instructions_tokens_est": 188,
                },
            }
        },
        "tool_catalog": {
            "surface_profile": mod.TOOL_SURFACE_PROFILE,
            "execution_shell": "bash",
            "modes": {
                "plan": {
                    "full": tool_surface(["File", "Git", "create_goal"], 18000),
                    "active": tool_surface(["File", "Git"], 12000),
                },
                "act": {
                    "full": tool_surface(["Bash", "File", "verify"], 20000),
                    "active": tool_surface(["Bash", "File"], 13000),
                },
                "operate": {
                    "full": tool_surface(["File", "Run", "verify"], 21000),
                    "active": tool_surface(["File", "Run"], 14000),
                },
            },
        },
    }


def set_path(document: dict, path: tuple[str, ...], value: int) -> None:
    target = document
    for part in path[:-1]:
        target = target[part]
    target[path[-1]] = value


def write_documents(tmp: str, receipt: dict, budget: dict) -> tuple[Path, Path]:
    receipt_path = Path(tmp) / "receipt.json"
    budget_path = Path(tmp) / "budget.json"
    receipt_path.write_text(json.dumps(receipt), encoding="utf-8")
    budget_path.write_text(json.dumps(budget), encoding="utf-8")
    return receipt_path, budget_path


class RuntimeContractBudgetTests(unittest.TestCase):
    def test_equal_fixture_passes_all_ceiling_checks(self) -> None:
        receipt = receipt_fixture()
        budget = mod.budget_from_receipt(receipt)
        self.assertEqual(len(mod.METRICS), 55)
        self.assertEqual(mod.compare(receipt, budget), ([], []))

    def test_unpinned_shell_receipt_is_rejected(self) -> None:
        receipt = receipt_fixture()
        budget = mod.budget_from_receipt(receipt)
        del receipt["tool_catalog"]["execution_shell"]
        with self.assertRaisesRegex(mod.RuntimeContractError, "execution_shell"):
            mod.compare(receipt, budget)

    def test_wrong_shell_is_rejected_even_with_matching_budget(self) -> None:
        receipt = receipt_fixture()
        budget = mod.budget_from_receipt(receipt)
        for shell in ["/bin/bash", "/bin/zsh", "pwsh", None]:
            with self.subTest(shell=shell):
                receipt["tool_catalog"]["execution_shell"] = shell
                budget["tool_catalog"]["execution_shell"] = shell
                with self.assertRaisesRegex(mod.RuntimeContractError, "execution_shell"):
                    mod.compare(receipt, budget)

    def test_every_owned_metric_rejects_an_increase(self) -> None:
        budget = mod.budget_from_receipt(receipt_fixture())
        for path, _label in mod.METRICS:
            with self.subTest(metric=".".join(path)):
                receipt = receipt_fixture()
                current = mod.metric_value(receipt, path, "receipt")
                set_path(receipt, path, current + 1)
                if path[-1] == "tools":
                    with self.assertRaisesRegex(
                        mod.RuntimeContractError, "tool_names length"
                    ):
                        mod.compare(receipt, budget)
                    continue
                increases, decreases = mod.compare(receipt, budget)
                self.assertEqual([item[0] for item in increases], [".".join(path)])
                self.assertEqual(decreases, [])

    def test_decrease_passes_and_is_reported_for_tightening(self) -> None:
        receipt = receipt_fixture()
        budget = mod.budget_from_receipt(receipt)
        path = ("system_prompt", "modes", "operate", "system_prompt_bytes")
        set_path(receipt, path, 24400)
        increases, decreases = mod.compare(receipt, budget)
        self.assertEqual(increases, [])
        self.assertEqual([item[0] for item in decreases], [".".join(path)])

    def test_missing_and_non_integer_metrics_are_rejected(self) -> None:
        receipt = receipt_fixture()
        budget = mod.budget_from_receipt(receipt)
        del receipt["tool_catalog"]["modes"]["act"]["active"]["bytes"]
        with self.assertRaisesRegex(mod.RuntimeContractError, "active.bytes"):
            mod.compare(receipt, budget)

        receipt = receipt_fixture()
        receipt["tool_catalog"]["modes"]["act"]["active"]["bytes"] = True
        with self.assertRaisesRegex(mod.RuntimeContractError, "non-negative integer"):
            mod.compare(receipt, budget)

    def test_document_kinds_and_schema_versions_are_distinct_and_required(self) -> None:
        receipt = receipt_fixture()
        budget = mod.budget_from_receipt(receipt)
        with self.assertRaisesRegex(mod.RuntimeContractError, "document_kind"):
            mod.compare(budget, budget)

        receipt.pop("schema_version")
        with self.assertRaisesRegex(mod.RuntimeContractError, "schema_version"):
            mod.compare(receipt, budget)

        receipt = receipt_fixture()
        budget["schema_version"] = mod.SCHEMA_VERSION + 1
        with self.assertRaisesRegex(mod.RuntimeContractError, "schema_version"):
            mod.compare(receipt, budget)

    def test_saved_receipt_requires_byte_identical_prompts(self) -> None:
        receipt = receipt_fixture()
        budget = mod.budget_from_receipt(receipt)
        receipt["skill_discovery"]["prompts_byte_identical"] = False
        with self.assertRaisesRegex(mod.RuntimeContractError, "must be true"):
            mod.compare(receipt, budget)

    def test_representative_fixture_identity_and_stability_are_required(self) -> None:
        receipt = receipt_fixture()
        budget = mod.budget_from_receipt(receipt)
        receipt["representative_context"]["fixture_id"] = "ambient-developer-state"
        with self.assertRaisesRegex(mod.RuntimeContractError, "fixture_id"):
            mod.compare(receipt, budget)

        receipt = receipt_fixture()
        receipt["representative_context"]["prompts_byte_identical"] = False
        with self.assertRaisesRegex(mod.RuntimeContractError, "must be true"):
            mod.compare(receipt, budget)

    def test_tool_identity_rejects_equal_size_substitution_and_removal(self) -> None:
        budget = mod.budget_from_receipt(receipt_fixture())

        receipt = receipt_fixture()
        active = receipt["tool_catalog"]["modes"]["act"]["active"]
        active["tool_names"] = sorted(["File", "Hash"])
        active["identity_sha256"] = mod.tool_identity_digest(active["tool_names"])
        with self.assertRaisesRegex(mod.RuntimeContractError, "identity changed"):
            mod.compare(receipt, budget)

        receipt = receipt_fixture()
        full = receipt["tool_catalog"]["modes"]["plan"]["full"]
        full["tool_names"].remove("Git")
        full["tools"] -= 1
        full["identity_sha256"] = mod.tool_identity_digest(full["tool_names"])
        with self.assertRaisesRegex(mod.RuntimeContractError, "identity changed"):
            mod.compare(receipt, budget)

    def test_tool_identity_rejects_missing_structure_and_wrong_mode_swap(self) -> None:
        budget = mod.budget_from_receipt(receipt_fixture())

        receipt = receipt_fixture()
        del receipt["tool_catalog"]["modes"]["operate"]["full"]["tool_names"]
        with self.assertRaisesRegex(mod.RuntimeContractError, "missing required field"):
            mod.compare(receipt, budget)

        receipt = receipt_fixture()
        plan = receipt["tool_catalog"]["modes"]["plan"]["active"]
        act = receipt["tool_catalog"]["modes"]["act"]["active"]
        for field in ["tool_names", "identity_sha256"]:
            plan[field], act[field] = act[field], plan[field]
        with self.assertRaisesRegex(mod.RuntimeContractError, "identity changed"):
            mod.compare(receipt, budget)

    def test_representative_stage_identity_rejects_same_length_substitution(self) -> None:
        receipt = receipt_fixture()
        budget = mod.budget_from_receipt(receipt)
        digest = receipt["representative_context"]["stages"]["project"][
            "identity_sha256"
        ]
        receipt["representative_context"]["stages"]["project"][
            "identity_sha256"
        ] = ("0" if digest[0] != "0" else "1") + digest[1:]
        with self.assertRaisesRegex(mod.RuntimeContractError, "identity changed"):
            mod.compare(receipt, budget)

    def test_same_resolved_receipt_and_budget_path_is_rejected(self) -> None:
        budget = mod.budget_from_receipt(receipt_fixture())
        with tempfile.TemporaryDirectory() as tmp:
            budget_path = Path(tmp) / "budget.json"
            nested = Path(tmp) / "nested"
            nested.mkdir()
            budget_path.write_text(json.dumps(budget), encoding="utf-8")
            alias = nested / ".." / "budget.json"
            errors = io.StringIO()
            with redirect_stderr(errors):
                result = mod.main(
                    ["--receipt", str(alias), "--budget", str(budget_path)]
                )
        self.assertEqual(result, 2)
        self.assertIn("distinct filesystem paths", errors.getvalue())

    def test_receipt_path_avoids_running_measurement(self) -> None:
        receipt = receipt_fixture()
        budget = mod.budget_from_receipt(receipt)
        with tempfile.TemporaryDirectory() as tmp:
            receipt_path, budget_path = write_documents(tmp, receipt, budget)
            output = io.StringIO()
            with (
                mock.patch.object(
                    mod, "run_measurement", side_effect=AssertionError("must stay hermetic")
                ),
                redirect_stdout(output),
            ):
                result = mod.main(
                    ["--receipt", str(receipt_path), "--budget", str(budget_path)]
                )
        self.assertEqual(result, 0)
        self.assertIn("all 55 metrics are exactly at budget", output.getvalue())

    def test_default_measurement_forces_cargo_offline(self) -> None:
        receipt = receipt_fixture()
        completed = mock.Mock(returncode=0, stdout=json.dumps(receipt), stderr="")
        with mock.patch.object(mod.subprocess, "run", return_value=completed) as run:
            measured = mod.run_measurement()
        self.assertEqual(measured, receipt)
        command = run.call_args.args[0]
        environment = run.call_args.kwargs["env"]
        self.assertEqual(command, [sys.executable, str(mod.MEASURE_SCRIPT)])
        self.assertEqual(environment["CARGO_NET_OFFLINE"], "true")

    def test_cli_decrease_prints_a_tightening_command(self) -> None:
        receipt = receipt_fixture()
        budget = mod.budget_from_receipt(receipt)
        set_path(receipt, ("system_prompt", "modes", "plan", "system_prompt_bytes"), 23000)
        with tempfile.TemporaryDirectory() as tmp:
            receipt_path, budget_path = write_documents(tmp, receipt, budget)
            output = io.StringIO()
            with redirect_stdout(output):
                result = mod.main(
                    ["--receipt", str(receipt_path), "--budget", str(budget_path)]
                )
        self.assertEqual(result, 0)
        self.assertIn("1 can be tightened", output.getvalue())
        self.assertIn("--update", output.getvalue())

    def test_update_tightens_decreases_and_preserves_permissions(self) -> None:
        receipt = receipt_fixture()
        budget = mod.budget_from_receipt(receipt)
        path = ("tool_catalog", "modes", "plan", "active", "bytes")
        set_path(receipt, path, 11000)
        with tempfile.TemporaryDirectory() as tmp:
            receipt_path, budget_path = write_documents(tmp, receipt, budget)
            os.chmod(budget_path, 0o640)
            with redirect_stdout(io.StringIO()):
                result = mod.main(
                    [
                        "--receipt",
                        str(receipt_path),
                        "--budget",
                        str(budget_path),
                        "--update",
                    ]
                )
            updated = json.loads(budget_path.read_text(encoding="utf-8"))
            updated_mode = stat.S_IMODE(budget_path.stat().st_mode)
        self.assertEqual(result, 0)
        self.assertEqual(mod.metric_value(updated, path, "budget"), 11000)
        self.assertEqual(updated_mode, 0o640)

    def test_atomic_update_failure_leaves_original_and_removes_temporary_file(self) -> None:
        budget = mod.budget_from_receipt(receipt_fixture())
        replacement = mod.budget_from_receipt(receipt_fixture())
        replacement["_comment"] = "replacement"
        with tempfile.TemporaryDirectory() as tmp:
            budget_path = Path(tmp) / "budget.json"
            original = json.dumps(budget)
            budget_path.write_text(original, encoding="utf-8")
            with (
                mock.patch.object(mod.os, "replace", side_effect=OSError("stop")),
                self.assertRaisesRegex(OSError, "stop"),
            ):
                mod.write_budget_atomic(budget_path, replacement)
            after = budget_path.read_text(encoding="utf-8")
            temporary_files = list(Path(tmp).glob(".budget.json.*.tmp"))
        self.assertEqual(after, original)
        self.assertEqual(temporary_files, [])

    def test_update_refuses_an_increase_without_rewriting_budget(self) -> None:
        receipt = receipt_fixture()
        budget = mod.budget_from_receipt(receipt)
        path = ("skill_discovery", "second_delta", "directories_visited")
        set_path(receipt, path, 2)
        with tempfile.TemporaryDirectory() as tmp:
            receipt_path, budget_path = write_documents(tmp, receipt, budget)
            original = budget_path.read_text(encoding="utf-8")
            with redirect_stderr(io.StringIO()):
                result = mod.main(
                    [
                        "--receipt",
                        str(receipt_path),
                        "--budget",
                        str(budget_path),
                        "--update",
                    ]
                )
            after = budget_path.read_text(encoding="utf-8")
        self.assertEqual(result, 1)
        self.assertEqual(after, original)

    def test_failure_prints_the_same_pr_receipt_command(self) -> None:
        receipt = receipt_fixture()
        budget = mod.budget_from_receipt(receipt)
        set_path(receipt, ("skill_discovery", "second_delta", "directories_visited"), 2)
        with tempfile.TemporaryDirectory() as tmp:
            receipt_path, budget_path = write_documents(tmp, receipt, budget)
            errors = io.StringIO()
            with redirect_stderr(errors):
                result = mod.main(
                    ["--receipt", str(receipt_path), "--budget", str(budget_path)]
                )
        self.assertEqual(result, 1)
        self.assertIn("--update --allow-increase", errors.getvalue())

    def test_allow_increase_lands_growth_and_identity_change_keeping_history(self) -> None:
        receipt = receipt_fixture()
        budget = mod.budget_from_receipt(receipt)
        budget["_comment"] = "history that must survive a rebase"
        grown = ("skill_discovery", "second_delta", "directories_visited")
        set_path(receipt, grown, 2)
        active = receipt["tool_catalog"]["modes"]["act"]["active"]
        active["tool_names"] = sorted(["File", "Hash"])
        active["identity_sha256"] = mod.tool_identity_digest(active["tool_names"])
        with tempfile.TemporaryDirectory() as tmp:
            receipt_path, budget_path = write_documents(tmp, receipt, budget)
            errors = io.StringIO()
            with redirect_stderr(errors):
                refused = mod.main(
                    ["--receipt", str(receipt_path), "--budget", str(budget_path)]
                )
            self.assertEqual(refused, 2)
            self.assertIn("--update --allow-increase", errors.getvalue())
            with redirect_stdout(io.StringIO()):
                result = mod.main(
                    [
                        "--receipt",
                        str(receipt_path),
                        "--budget",
                        str(budget_path),
                        "--update",
                        "--allow-increase",
                    ]
                )
            updated = json.loads(budget_path.read_text(encoding="utf-8"))
            with redirect_stdout(io.StringIO()):
                recheck = mod.main(
                    ["--receipt", str(receipt_path), "--budget", str(budget_path)]
                )
        self.assertEqual(result, 0)
        self.assertEqual(recheck, 0)
        self.assertEqual(mod.metric_value(updated, grown, "budget"), 2)
        self.assertEqual(updated["_comment"], "history that must survive a rebase")

    def test_allow_increase_requires_update(self) -> None:
        with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            mod.main(["--allow-increase"])


if __name__ == "__main__":
    raise SystemExit(unittest.main())
