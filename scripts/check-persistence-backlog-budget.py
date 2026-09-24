#!/usr/bin/env python3
"""Check the paused persistence backlog against one-way local ceilings.

Usage:
    python3 scripts/check-persistence-backlog-budget.py
    python3 scripts/check-persistence-backlog-budget.py --receipt receipt.json
    python3 scripts/check-persistence-backlog-budget.py --update

``--update`` is the receipt command a failing PR runs to land an intended
increase in the same PR: it raises only the exceeded ceilings to the measured
values and never lowers one, because the ceilings carry deliberate measurement
noise headroom. Tighten by hand, with the reason recorded in the budget.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parent.parent
MEASURE_SCRIPT = ROOT / "scripts" / "measure-persistence-backlog.py"
BUDGET_PATH = ROOT / "scripts" / "persistence-backlog-budget.json"
BASELINE_RECEIPT_PATH = ROOT / "scripts" / "persistence-backlog-baseline-receipt.json"
BASELINE_RECEIPT_REFERENCE = "scripts/persistence-backlog-baseline-receipt.json"
RECEIPT_KIND = "codewhale.persistence_backlog_receipt"
BUDGET_KIND = "codewhale.persistence_backlog_budget"
SCHEMA_VERSION = 2

FIXTURE = {
    "fixture_id": "paused-production-channel-session-snapshot-v1",
    "request_variant": "session_snapshot",
    "payload_estimator": "retained-saved-session-json-bytes-v1",
    "paused_consumer": True,
    "requests_attempted": 128,
    "content_bytes_per_request": 64 * 1024,
    "single_session_id": True,
    "expected_applied_version": 127,
}

REQUIRED_RECEIPT_FIELDS = (
    "document_kind",
    "schema_version",
    "source_sha",
    "source_dirty",
    "rustc_version",
    "cargo_version",
    "build_profile",
    "sample_count",
    "fixture_id",
    "platform",
    "request_variant",
    "payload_estimator",
    "paused_consumer",
    "requests_attempted",
    "content_bytes_per_request",
    "single_session_id",
    "expected_applied_version",
    "accepted_requests",
    "retained_queued_requests",
    "estimated_retained_payload_bytes",
    "applied_version",
    "final_version_applied",
    "enqueue_elapsed_ns",
    "rss_supported",
    "rss_before_bytes",
    "rss_during_bytes",
    "rss_after_bytes",
    "rss_during_delta_bytes",
    "rss_after_delta_bytes",
    "limitations",
)

CEILING_FIELDS = (
    "retained_queued_requests",
    "estimated_retained_payload_bytes",
    "enqueue_elapsed_ns",
    "rss_during_delta_bytes",
    "rss_after_delta_bytes",
)
RSS_SAMPLE_FIELDS = ("rss_before_bytes", "rss_during_bytes", "rss_after_bytes")
RSS_DELTA_FIELDS = ("rss_during_delta_bytes", "rss_after_delta_bytes")
SUPPORTED_PLATFORMS = {"linux", "macos", "windows"}
SOURCE_SHA_PATTERN = re.compile(r"[0-9a-f]{40}")


class PersistenceBacklogError(ValueError):
    """A receipt or budget broke the measurement contract."""


def load_json(path: Path, label: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise PersistenceBacklogError(f"invalid {label} {path}: {error}") from error
    if not isinstance(value, dict):
        raise PersistenceBacklogError(f"{label} must be a JSON object")
    return value


def non_negative_integer(value: Any, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise PersistenceBacklogError(f"{field} must be a non-negative integer")
    return value


def validate_frozen_field(field: str, value: Any, expected: Any) -> None:
    if type(value) is not type(expected) or value != expected:
        raise PersistenceBacklogError(
            f"receipt {field} must remain {expected!r}, got {value!r}"
        )


def current_source_identity() -> dict[str, Any]:
    def run(command: list[str]) -> str:
        result = subprocess.run(
            command,
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=False,
        )
        if result.returncode != 0:
            raise PersistenceBacklogError(
                f"source provenance command failed: {' '.join(command)}"
            )
        return result.stdout.strip()

    return {
        "source_sha": run(["git", "rev-parse", "HEAD"]),
        "source_dirty": bool(
            run(["git", "status", "--porcelain", "--untracked-files=normal"])
        ),
        "rustc_version": run(["rustc", "--version"]),
        "cargo_version": run(["cargo", "--version"]),
        "build_profile": "test",
        "sample_count": 1,
    }


def validate_receipt(
    receipt: dict[str, Any],
    *,
    expected_source: dict[str, Any] | None = None,
    require_clean_source: bool = False,
) -> None:
    missing = [field for field in REQUIRED_RECEIPT_FIELDS if field not in receipt]
    if missing:
        raise PersistenceBacklogError(
            "receipt missing required field(s): " + ", ".join(missing)
        )
    if receipt["document_kind"] != RECEIPT_KIND:
        raise PersistenceBacklogError(f"receipt document_kind must be {RECEIPT_KIND}")
    if receipt["schema_version"] != SCHEMA_VERSION:
        raise PersistenceBacklogError("receipt schema_version changed")
    for field, expected in FIXTURE.items():
        validate_frozen_field(field, receipt[field], expected)
    if not isinstance(receipt["source_sha"], str) or not SOURCE_SHA_PATTERN.fullmatch(
        receipt["source_sha"]
    ):
        raise PersistenceBacklogError("receipt source_sha must be an exact lowercase Git SHA")
    if type(receipt["source_dirty"]) is not bool:
        raise PersistenceBacklogError("receipt source_dirty must be boolean")
    for field, prefix in (("rustc_version", "rustc "), ("cargo_version", "cargo ")):
        if not isinstance(receipt[field], str) or not receipt[field].startswith(prefix):
            raise PersistenceBacklogError(f"receipt {field} must be a version string")
    validate_frozen_field("build_profile", receipt["build_profile"], "test")
    validate_frozen_field("sample_count", receipt["sample_count"], 1)
    if expected_source is not None:
        for field in (
            "source_sha",
            "source_dirty",
            "rustc_version",
            "cargo_version",
            "build_profile",
            "sample_count",
        ):
            if receipt[field] != expected_source[field]:
                raise PersistenceBacklogError(
                    f"receipt {field} does not match the checked source"
                )
    if require_clean_source and receipt["source_dirty"]:
        raise PersistenceBacklogError("persistence measurement source tree is dirty")
    platform = receipt["platform"]
    if not isinstance(platform, str) or platform not in SUPPORTED_PLATFORMS:
        raise PersistenceBacklogError("receipt platform is unsupported")

    attempted = non_negative_integer(receipt["requests_attempted"], "requests_attempted")
    accepted = non_negative_integer(receipt["accepted_requests"], "accepted_requests")
    if accepted != attempted:
        raise PersistenceBacklogError(
            "accepted_requests must equal requests_attempted; sender rejection is not backlog improvement"
        )
    retained = non_negative_integer(
        receipt["retained_queued_requests"], "retained_queued_requests"
    )
    if retained > accepted:
        raise PersistenceBacklogError("retained_queued_requests exceeds accepted_requests")
    for field in ("estimated_retained_payload_bytes", "enqueue_elapsed_ns"):
        non_negative_integer(receipt[field], field)
    if retained == 0 or receipt["estimated_retained_payload_bytes"] == 0:
        raise PersistenceBacklogError(
            "the paused channel must retain the newest request and its payload"
        )
    minimum_payload_bytes = retained * FIXTURE["content_bytes_per_request"]
    if receipt["estimated_retained_payload_bytes"] < minimum_payload_bytes:
        raise PersistenceBacklogError(
            "estimated_retained_payload_bytes is smaller than the frozen retained content"
        )
    applied = non_negative_integer(
        receipt["applied_version"], "applied_version"
    )
    if applied != FIXTURE["expected_applied_version"]:
        raise PersistenceBacklogError("applied_version is not the final sent version")
    if receipt["final_version_applied"] is not True:
        raise PersistenceBacklogError("final_version_applied must be true")

    limitations = receipt["limitations"]
    if not isinstance(limitations, list) or not limitations or not all(
        isinstance(item, str) and item for item in limitations
    ):
        raise PersistenceBacklogError("limitations must be a non-empty string array")

    if not isinstance(receipt["rss_supported"], bool):
        raise PersistenceBacklogError("rss_supported must be boolean")
    if receipt["rss_supported"] != (platform == "macos"):
        raise PersistenceBacklogError(
            "rss_supported must be true exactly on the macOS measurement lane"
        )
    rss_fields = RSS_SAMPLE_FIELDS + RSS_DELTA_FIELDS
    if receipt["rss_supported"]:
        for field in rss_fields:
            non_negative_integer(receipt[field], field)
        before = receipt["rss_before_bytes"]
        if receipt["rss_during_delta_bytes"] != max(
            0, receipt["rss_during_bytes"] - before
        ):
            raise PersistenceBacklogError("rss_during_delta_bytes is inconsistent")
        if receipt["rss_after_delta_bytes"] != max(
            0, receipt["rss_after_bytes"] - before
        ):
            raise PersistenceBacklogError("rss_after_delta_bytes is inconsistent")
    elif any(receipt[field] is not None for field in rss_fields):
        raise PersistenceBacklogError("unsupported RSS fields must be null")


def validate_budget(budget: dict[str, Any]) -> None:
    if budget.get("document_kind") != BUDGET_KIND:
        raise PersistenceBacklogError(f"budget document_kind must be {BUDGET_KIND}")
    if budget.get("schema_version") != SCHEMA_VERSION:
        raise PersistenceBacklogError("budget schema_version changed")
    fixture = budget.get("fixture")
    if not isinstance(fixture, dict) or set(fixture) != set(FIXTURE):
        raise PersistenceBacklogError("budget fixture no longer matches the frozen workload")
    for field, expected in FIXTURE.items():
        if type(fixture[field]) is not type(expected) or fixture[field] != expected:
            raise PersistenceBacklogError(
                f"budget fixture.{field} must remain {expected!r}"
            )
    if budget.get("baseline_receipt") != BASELINE_RECEIPT_REFERENCE:
        raise PersistenceBacklogError("budget baseline_receipt path changed")
    ceilings = budget.get("ceilings")
    baseline = budget.get("baseline_observation")
    if not isinstance(ceilings, dict) or not isinstance(baseline, dict):
        raise PersistenceBacklogError("budget needs ceilings and baseline_observation objects")
    for field in CEILING_FIELDS:
        ceiling = non_negative_integer(ceilings.get(field), f"ceilings.{field}")
        observed = non_negative_integer(
            baseline.get(field), f"baseline_observation.{field}"
        )
        if observed > ceiling:
            raise PersistenceBacklogError(
                f"baseline_observation.{field} exceeds its ceiling"
            )
    baseline_accepted = non_negative_integer(
        baseline.get("accepted_requests"), "baseline_observation.accepted_requests"
    )
    if baseline_accepted != FIXTURE["requests_attempted"]:
        raise PersistenceBacklogError(
            "baseline_observation.accepted_requests must equal requests_attempted"
        )
    baseline_applied = non_negative_integer(
        baseline.get("applied_version"), "baseline_observation.applied_version"
    )
    if baseline_applied != FIXTURE["expected_applied_version"]:
        raise PersistenceBacklogError(
            "baseline_observation.applied_version must be the final sent version"
        )
    baseline_retained = baseline["retained_queued_requests"]
    baseline_payload = baseline["estimated_retained_payload_bytes"]
    if baseline_retained == 0 or baseline_payload == 0:
        raise PersistenceBacklogError(
            "baseline_observation must retain the final request and payload"
        )
    if baseline_retained > baseline_accepted:
        raise PersistenceBacklogError(
            "baseline_observation.retained_queued_requests exceeds accepted_requests"
        )
    if baseline_payload < baseline_retained * FIXTURE["content_bytes_per_request"]:
        raise PersistenceBacklogError(
            "baseline_observation payload is smaller than frozen retained content"
        )
    provenance = baseline.get("provenance")
    if not isinstance(provenance, dict):
        raise PersistenceBacklogError("baseline_observation needs provenance")
    if provenance.get("platform") != "macos":
        raise PersistenceBacklogError("baseline provenance platform must be macos")
    if not isinstance(provenance.get("source_sha"), str) or not SOURCE_SHA_PATTERN.fullmatch(
        provenance["source_sha"]
    ):
        raise PersistenceBacklogError("baseline provenance needs an exact source SHA")
    if provenance.get("source_dirty") is not False:
        raise PersistenceBacklogError("baseline provenance must identify a clean source tree")
    for field, prefix in (("rustc_version", "rustc "), ("cargo_version", "cargo ")):
        if not isinstance(provenance.get(field), str) or not provenance[field].startswith(prefix):
            raise PersistenceBacklogError(f"baseline provenance needs {field}")
    if provenance.get("build_profile") != "test" or not (
        type(provenance.get("sample_count")) is int
        and provenance["sample_count"] == 1
    ):
        raise PersistenceBacklogError("baseline provenance build profile/sample count changed")


def validate_baseline_receipt(
    budget: dict[str, Any], baseline_receipt: dict[str, Any]
) -> None:
    validate_receipt(baseline_receipt, require_clean_source=True)
    baseline = budget["baseline_observation"]
    for field in ("accepted_requests", "applied_version", *CEILING_FIELDS):
        if baseline_receipt[field] != baseline[field]:
            raise PersistenceBacklogError(
                f"baseline receipt {field} does not match baseline_observation"
            )
    provenance = baseline["provenance"]
    for field in (
        "platform",
        "source_sha",
        "source_dirty",
        "rustc_version",
        "cargo_version",
        "build_profile",
        "sample_count",
    ):
        if baseline_receipt[field] != provenance[field]:
            raise PersistenceBacklogError(
                f"baseline receipt {field} does not match baseline provenance"
            )


def compare(
    receipt: dict[str, Any],
    budget: dict[str, Any],
    *,
    expected_source: dict[str, Any] | None = None,
    require_clean_source: bool = False,
) -> tuple[list[tuple[str, int, int]], list[tuple[str, int, int]]]:
    validate_receipt(
        receipt,
        expected_source=expected_source,
        require_clean_source=require_clean_source,
    )
    validate_budget(budget)
    increases: list[tuple[str, int, int]] = []
    decreases: list[tuple[str, int, int]] = []
    for field in CEILING_FIELDS:
        if field in RSS_DELTA_FIELDS and not receipt["rss_supported"]:
            continue
        current = receipt[field]
        ceiling = budget["ceilings"][field]
        if current > ceiling:
            increases.append((field, current, ceiling))
        elif current < ceiling:
            decreases.append((field, current, ceiling))
    return increases, decreases


def measure() -> dict[str, Any]:
    env = os.environ.copy()
    env["CARGO_NET_OFFLINE"] = "true"
    result = subprocess.run(
        [sys.executable, str(MEASURE_SCRIPT)],
        cwd=ROOT,
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )
    sys.stderr.write(result.stderr)
    if result.returncode != 0:
        sys.stdout.write(result.stdout)
        raise PersistenceBacklogError("measurement command failed")
    try:
        receipt = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise PersistenceBacklogError(f"measurement emitted invalid JSON: {error}") from error
    if not isinstance(receipt, dict):
        raise PersistenceBacklogError("measurement receipt must be an object")
    return receipt


def update_command(receipt_path: Path | None, budget_path: Path) -> str:
    parts = ["python3", "scripts/check-persistence-backlog-budget.py"]
    if receipt_path is not None:
        parts.extend(["--receipt", str(receipt_path)])
    if budget_path != BUDGET_PATH:
        parts.extend(["--budget", str(budget_path)])
    parts.append("--update")
    return " ".join(parts)


def raise_ceilings(
    budget: dict[str, Any], increases: list[tuple[str, int, int]]
) -> dict[str, Any]:
    """Return a copy of ``budget`` with each exceeded ceiling set to its measurement."""
    updated = json.loads(json.dumps(budget))
    for field, current, _ceiling in increases:
        updated["ceilings"][field] = current
    validate_budget(updated)
    return updated


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--receipt", type=Path, help="check an existing receipt")
    parser.add_argument("--budget", type=Path, default=BUDGET_PATH)
    parser.add_argument(
        "--update",
        action="store_true",
        help="raise exceeded ceilings to the measured values (never lowers one)",
    )
    args = parser.parse_args()
    try:
        expected_source = current_source_identity()
        receipt = load_json(args.receipt, "receipt") if args.receipt else measure()
        budget = load_json(args.budget, "budget")
        baseline_receipt = load_json(BASELINE_RECEIPT_PATH, "baseline receipt")
        validate_baseline_receipt(budget, baseline_receipt)
        increases, decreases = compare(
            receipt,
            budget,
            expected_source=expected_source,
            # An update runs while the author is mid-change; the measurement
            # still names its exact SHA and dirty bit, so only the enforcing
            # check insists on a clean tree.
            require_clean_source=not args.update,
        )
    except PersistenceBacklogError as error:
        print(f"[persistence-backlog-budget] ERROR: {error}", file=sys.stderr)
        return 2
    if args.update:
        if not increases:
            print(
                "[persistence-backlog-budget] --update: no ceiling exceeded; "
                f"{args.budget} left unchanged"
            )
            return 0
        try:
            updated = raise_ceilings(budget, increases)
            args.budget.write_text(json.dumps(updated, indent=2) + "\n", encoding="utf-8")
        except (OSError, PersistenceBacklogError) as error:
            print(
                f"[persistence-backlog-budget] ERROR: failed to update budget: {error}",
                file=sys.stderr,
            )
            return 2
        for field, current, ceiling in increases:
            print(f"[persistence-backlog-budget] raised {field}: {ceiling} -> {current}")
        print(
            f"[persistence-backlog-budget] wrote {args.budget}; say why in the PR "
            "description, or add a dated _rebaseline note to the budget."
        )
        return 0
    if increases:
        for field, current, ceiling in increases:
            print(
                f"[persistence-backlog-budget] FAIL: {field}={current} exceeds {ceiling}",
                file=sys.stderr,
            )
        print(
            "\nShrink the retained backlog, or if the growth is intended land the new "
            f"ceiling in this PR:\n  {update_command(args.receipt, args.budget)}",
            file=sys.stderr,
        )
        return 1
    print("[persistence-backlog-budget] PASS: one-way ceilings respected")
    for field, current, ceiling in decreases:
        print(f"  can tighten {field}: {current} < {ceiling}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
