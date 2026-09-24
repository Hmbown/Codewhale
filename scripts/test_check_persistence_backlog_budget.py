#!/usr/bin/env python3
"""Hermetic contract tests for the persistence backlog ratchet."""

from __future__ import annotations

import copy
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "check-persistence-backlog-budget.py"
SPEC = importlib.util.spec_from_file_location("check_persistence_backlog_budget", SCRIPT)
assert SPEC and SPEC.loader
mod = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = mod
SPEC.loader.exec_module(mod)


def receipt_fixture(*, rss_supported: bool = True) -> dict:
    receipt = {
        "document_kind": mod.RECEIPT_KIND,
        "schema_version": mod.SCHEMA_VERSION,
        **mod.FIXTURE,
        "source_sha": "0123456789abcdef0123456789abcdef01234567",
        "source_dirty": False,
        "rustc_version": "rustc test",
        "cargo_version": "cargo test",
        "build_profile": "test",
        "sample_count": 1,
        "platform": "macos" if rss_supported else "linux",
        "accepted_requests": 128,
        "retained_queued_requests": 128,
        "estimated_retained_payload_bytes": 8_500_000,
        "applied_version": 127,
        "final_version_applied": True,
        "enqueue_elapsed_ns": 500_000,
        "rss_supported": rss_supported,
        "rss_before_bytes": 100_000_000 if rss_supported else None,
        "rss_during_bytes": 112_000_000 if rss_supported else None,
        "rss_after_bytes": 103_000_000 if rss_supported else None,
        "rss_during_delta_bytes": 12_000_000 if rss_supported else None,
        "rss_after_delta_bytes": 3_000_000 if rss_supported else None,
        "limitations": ["macOS RSS only"],
    }
    return receipt


def budget_fixture(receipt: dict | None = None) -> dict:
    receipt = receipt or receipt_fixture()
    metrics = {
        field: receipt[field] if receipt[field] is not None else 0
        for field in mod.CEILING_FIELDS
    }
    return {
        "document_kind": mod.BUDGET_KIND,
        "schema_version": mod.SCHEMA_VERSION,
        "baseline_receipt": mod.BASELINE_RECEIPT_REFERENCE,
        "fixture": copy.deepcopy(mod.FIXTURE),
        "baseline_observation": {
            "accepted_requests": receipt["accepted_requests"],
            "applied_version": receipt["applied_version"],
            "provenance": {
                "platform": "macos",
                "source_sha": receipt["source_sha"],
                "source_dirty": False,
                "rustc_version": receipt["rustc_version"],
                "cargo_version": receipt["cargo_version"],
                "build_profile": "test",
                "sample_count": 1,
            },
            **copy.deepcopy(metrics),
        },
        "ceilings": copy.deepcopy(metrics),
    }


class PersistenceBacklogBudgetTests(unittest.TestCase):
    def test_equal_baseline_passes(self) -> None:
        self.assertEqual(
            mod.compare(receipt_fixture(), budget_fixture()),
            ([], []),
        )

    def test_every_receipt_field_is_required(self) -> None:
        budget = budget_fixture()
        for field in mod.REQUIRED_RECEIPT_FIELDS:
            with self.subTest(field=field):
                receipt = receipt_fixture()
                del receipt[field]
                with self.assertRaisesRegex(
                    mod.PersistenceBacklogError, "missing required field"
                ):
                    mod.compare(receipt, budget)

    def test_frozen_workload_cannot_be_weakened_to_fake_an_improvement(self) -> None:
        budget = budget_fixture()
        for field, replacement in [
            ("paused_consumer", False),
            ("requests_attempted", 64),
            ("content_bytes_per_request", 32 * 1024),
            ("single_session_id", False),
            ("expected_applied_version", 63),
            ("request_variant", "clear_checkpoint"),
            ("payload_estimator", "shallow-size"),
        ]:
            with self.subTest(field=field):
                receipt = receipt_fixture()
                receipt[field] = replacement
                with self.assertRaisesRegex(mod.PersistenceBacklogError, field):
                    mod.compare(receipt, budget)

    def test_boolean_fixture_fields_reject_integer_aliases(self) -> None:
        budget = budget_fixture()
        for field in ("paused_consumer", "single_session_id"):
            with self.subTest(field=field):
                receipt = receipt_fixture()
                receipt[field] = 1
                with self.assertRaisesRegex(mod.PersistenceBacklogError, field):
                    mod.compare(receipt, budget)

    def test_budget_boolean_and_sample_count_aliases_are_rejected(self) -> None:
        receipt = receipt_fixture()
        for field in ("paused_consumer", "single_session_id"):
            with self.subTest(field=field):
                budget = budget_fixture()
                budget["fixture"][field] = 1
                with self.assertRaisesRegex(mod.PersistenceBacklogError, field):
                    mod.compare(receipt, budget)

        budget = budget_fixture()
        budget["baseline_observation"]["provenance"]["sample_count"] = True
        with self.assertRaisesRegex(mod.PersistenceBacklogError, "sample count"):
            mod.compare(receipt, budget)

    def test_every_ceiling_rejects_growth_and_accepts_tightening(self) -> None:
        baseline = receipt_fixture()
        baseline["retained_queued_requests"] = 64
        # Leave enough valid payload headroom for the retained-count subtest
        # to change that one metric without making the receipt impossible.
        baseline["estimated_retained_payload_bytes"] = 8_500_000
        budget = budget_fixture(baseline)
        for field in mod.CEILING_FIELDS:
            with self.subTest(field=field):
                grown = copy.deepcopy(baseline)
                grown[field] += 1
                if field == "rss_during_delta_bytes":
                    grown["rss_during_bytes"] += 1
                elif field == "rss_after_delta_bytes":
                    grown["rss_after_bytes"] += 1
                increases, _ = mod.compare(grown, budget)
                self.assertEqual([item[0] for item in increases], [field])

                reduced = copy.deepcopy(baseline)
                reduced[field] -= 1
                if field == "rss_during_delta_bytes":
                    reduced["rss_during_bytes"] -= 1
                elif field == "rss_after_delta_bytes":
                    reduced["rss_after_bytes"] -= 1
                increases, decreases = mod.compare(reduced, budget)
                self.assertEqual(increases, [])
                self.assertIn(field, [item[0] for item in decreases])

    def test_non_macos_receipt_keeps_rss_shape_but_skips_rss_ceilings(self) -> None:
        receipt = receipt_fixture(rss_supported=False)
        budget = budget_fixture()
        increases, decreases = mod.compare(receipt, budget)
        self.assertEqual(increases, [])
        self.assertNotIn(
            "rss_during_delta_bytes", [item[0] for item in decreases]
        )
        self.assertNotIn("rss_after_delta_bytes", [item[0] for item in decreases])

    def test_rss_delta_must_match_samples(self) -> None:
        receipt = receipt_fixture()
        receipt["rss_during_delta_bytes"] += 1
        with self.assertRaisesRegex(mod.PersistenceBacklogError, "inconsistent"):
            mod.compare(receipt, budget_fixture())

    def test_sender_rejection_cannot_masquerade_as_backlog_improvement(self) -> None:
        receipt = receipt_fixture()
        receipt["accepted_requests"] = 1
        receipt["retained_queued_requests"] = 1
        receipt["estimated_retained_payload_bytes"] = 66_000
        with self.assertRaisesRegex(
            mod.PersistenceBacklogError, "sender rejection is not backlog improvement"
        ):
            mod.compare(receipt, budget_fixture())

    def test_stale_applied_version_cannot_pass_as_coalescing(self) -> None:
        receipt = receipt_fixture()
        receipt["applied_version"] = 126
        receipt["final_version_applied"] = False
        with self.assertRaisesRegex(mod.PersistenceBacklogError, "final sent version"):
            mod.compare(receipt, budget_fixture())

    def test_impossible_one_byte_retained_payload_is_rejected(self) -> None:
        receipt = receipt_fixture()
        receipt["retained_queued_requests"] = 1
        receipt["estimated_retained_payload_bytes"] = 1
        with self.assertRaisesRegex(mod.PersistenceBacklogError, "frozen retained content"):
            mod.compare(receipt, budget_fixture())

    def test_rss_support_is_required_exactly_on_macos(self) -> None:
        macos_without_rss = receipt_fixture(rss_supported=False)
        macos_without_rss["platform"] = "macos"
        with self.assertRaisesRegex(mod.PersistenceBacklogError, "exactly on the macOS"):
            mod.compare(macos_without_rss, budget_fixture())

        linux_with_rss = receipt_fixture()
        linux_with_rss["platform"] = "linux"
        with self.assertRaisesRegex(mod.PersistenceBacklogError, "exactly on the macOS"):
            mod.compare(linux_with_rss, budget_fixture())

        unknown = receipt_fixture(rss_supported=False)
        unknown["platform"] = "unknown"
        with self.assertRaisesRegex(mod.PersistenceBacklogError, "unsupported"):
            mod.compare(unknown, budget_fixture())

        malformed = receipt_fixture(rss_supported=False)
        malformed["platform"] = []
        with self.assertRaisesRegex(mod.PersistenceBacklogError, "unsupported"):
            mod.compare(malformed, budget_fixture())

    def test_cli_source_identity_rejects_historical_or_dirty_receipts(self) -> None:
        receipt = receipt_fixture()
        expected = {
            field: receipt[field]
            for field in (
                "source_sha",
                "source_dirty",
                "rustc_version",
                "cargo_version",
                "build_profile",
                "sample_count",
            )
        }
        historical = copy.deepcopy(receipt)
        historical["source_sha"] = "f" * 40
        with self.assertRaisesRegex(mod.PersistenceBacklogError, "checked source"):
            mod.compare(historical, budget_fixture(), expected_source=expected)

        dirty = copy.deepcopy(receipt)
        dirty["source_dirty"] = True
        expected_dirty = copy.deepcopy(expected)
        expected_dirty["source_dirty"] = True
        with self.assertRaisesRegex(mod.PersistenceBacklogError, "source tree is dirty"):
            mod.compare(
                dirty,
                budget_fixture(),
                expected_source=expected_dirty,
                require_clean_source=True,
            )

    def test_budget_cannot_hide_a_baseline_above_its_ceiling(self) -> None:
        budget = budget_fixture()
        budget["baseline_observation"]["retained_queued_requests"] += 1
        with self.assertRaisesRegex(mod.PersistenceBacklogError, "exceeds its ceiling"):
            mod.compare(receipt_fixture(), budget)

    def test_budget_cannot_claim_an_empty_retained_baseline(self) -> None:
        budget = budget_fixture()
        budget["baseline_observation"]["retained_queued_requests"] = 0
        budget["baseline_observation"]["estimated_retained_payload_bytes"] = 0
        with self.assertRaisesRegex(mod.PersistenceBacklogError, "retain the final"):
            mod.compare(receipt_fixture(), budget)

    def test_raw_baseline_receipt_must_match_budget_metrics_and_provenance(self) -> None:
        receipt = receipt_fixture()
        budget = budget_fixture(receipt)
        mod.validate_baseline_receipt(budget, receipt)

        stale_metric = copy.deepcopy(receipt)
        stale_metric["enqueue_elapsed_ns"] += 1
        with self.assertRaisesRegex(mod.PersistenceBacklogError, "does not match"):
            mod.validate_baseline_receipt(budget, stale_metric)

        stale_source = copy.deepcopy(receipt)
        stale_source["source_sha"] = "f" * 40
        with self.assertRaisesRegex(mod.PersistenceBacklogError, "does not match"):
            mod.validate_baseline_receipt(budget, stale_source)

    def _run_cli(self, receipt: dict, budget: dict, *extra: str) -> tuple[int, str, dict]:
        """Run main() against temp files with the source identity pinned to ``receipt``."""
        source = {
            field: receipt[field]
            for field in (
                "source_sha",
                "source_dirty",
                "rustc_version",
                "cargo_version",
                "build_profile",
                "sample_count",
            )
        }
        with tempfile.TemporaryDirectory() as tmp:
            receipt_path = Path(tmp) / "receipt.json"
            budget_path = Path(tmp) / "budget.json"
            baseline_path = Path(tmp) / "baseline.json"
            receipt_path.write_text(json.dumps(receipt), encoding="utf-8")
            budget_path.write_text(json.dumps(budget, indent=2) + "\n", encoding="utf-8")
            baseline_path.write_text(json.dumps(receipt_fixture()), encoding="utf-8")
            output = io.StringIO()
            argv = [
                "check",
                "--receipt",
                str(receipt_path),
                "--budget",
                str(budget_path),
                *extra,
            ]
            with (
                mock.patch.object(mod, "current_source_identity", return_value=source),
                mock.patch.object(mod, "BASELINE_RECEIPT_PATH", baseline_path),
                mock.patch.object(sys, "argv", argv),
                redirect_stdout(output),
                redirect_stderr(output),
            ):
                result = mod.main()
            written = json.loads(budget_path.read_text(encoding="utf-8"))
        return result, output.getvalue(), written

    def test_failure_prints_update_receipt_and_update_raises_only_exceeded(self) -> None:
        budget = budget_fixture()
        receipt = receipt_fixture()
        receipt["enqueue_elapsed_ns"] = budget["ceilings"]["enqueue_elapsed_ns"] + 7
        receipt["retained_queued_requests"] -= 1

        result, output, unchanged = self._run_cli(receipt, budget)
        self.assertEqual(result, 1)
        self.assertIn("check-persistence-backlog-budget.py", output)
        self.assertIn("--update", output)
        self.assertEqual(unchanged, budget)

        result, output, updated = self._run_cli(receipt, budget, "--update")
        self.assertEqual(result, 0, output)
        self.assertEqual(
            updated["ceilings"]["enqueue_elapsed_ns"], receipt["enqueue_elapsed_ns"]
        )
        # Decreases keep their noise headroom: --update never lowers a ceiling.
        self.assertEqual(
            updated["ceilings"]["retained_queued_requests"],
            budget["ceilings"]["retained_queued_requests"],
        )
        self.assertEqual(updated["baseline_observation"], budget["baseline_observation"])

    def test_update_without_growth_leaves_budget_untouched(self) -> None:
        budget = budget_fixture()
        result, output, written = self._run_cli(receipt_fixture(), budget, "--update")
        self.assertEqual(result, 0, output)
        self.assertEqual(written, budget)


if __name__ == "__main__":
    unittest.main()
