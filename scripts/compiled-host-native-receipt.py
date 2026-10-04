#!/usr/bin/env python3
"""Export exact-image Native results from the existing full nextest invocation.

This consumes JUnit output, never runs Rust or fabricates a runtime pass. The
completion markers are emitted by Rust only after their complete Native scenarios. Release
staging also requires the official CI run to be green at this exact source SHA.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import sys
import xml.etree.ElementTree as ET

COMPILED = "extension_host::tests::compiled_native_host_cannot_read_secrets_or_write_outside_its_data_dir"
MEMORY = "extension_host::tests::compiled_native_host_memory_cap_is_enforced"
WINDOWS = [
    "extension_host::windows::tests::windows_native_lpac_node_owner_boundary",
    "extension_host::windows::tests::windows_native_lpac_bun_owner_boundary",
    "extension_host::windows::tests::windows_native_lpac_compiled_owner_boundary",
    "extension_host::windows::tests::windows_native_profiles_are_distinct_and_acl_grant_refuses_junctions",
    "extension_host::windows::tests::windows_argv_and_environment_keep_exact_values_and_reject_nul",
    "extension_host::windows::tests::windows_profile_retirement_removes_only_its_grants_and_inherited_data_on_restarts",
    "extension_host::windows::tests::windows_profile_directory_budget_and_recorded_identity_refuse_before_overwrite",
]
RUST_OS = {"linux": "linux", "darwin": "macos", "win32": "windows"}
RUST_ARCH = {"x64": "x86_64", "arm64": "aarch64"}
MAX_REPORT = 64 * 1024 * 1024
MAX_LOG = 64 * 1024


def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            value.update(chunk)
    return value.hexdigest()


def native_identity() -> tuple[str, str]:
    native_platform = {"linux": "linux", "darwin": "darwin", "win32": "win32"}.get(sys.platform)
    native_arch = {"x86_64": "x64", "amd64": "x64", "arm64": "arm64", "aarch64": "arm64"}.get(platform.machine().lower())
    if native_platform is None or native_arch is None:
        raise ValueError("unsupported actual native runner")
    return native_platform, native_arch


def extract(report: bytes, target_platform: str, target_arch: str) -> tuple[int, bytes]:
    if len(report) > MAX_REPORT or b"<!DOCTYPE" in report or b"<!ENTITY" in report:
        raise ValueError("invalid or oversized nextest report")
    root = ET.fromstring(report)
    required = [COMPILED, MEMORY, *(WINDOWS if target_platform == "win32" else [])]
    records = {}
    for case in root.iter("testcase"):
        name = case.get("name", "")
        if name not in required:
            continue
        if name in records:
            raise ValueError(f"duplicate/retried Native testcase: {name}")
        # quick-junit represents earlier attempt errors separately. No hidden
        # retries, ignored cases, or successful return without the marker pass.
        if any(child.tag in {"failure", "error", "skipped", "flakyFailure", "flakyError", "rerunFailure", "rerunError"} for child in case):
            raise ValueError(f"Native testcase did not pass once: {name}")
        output = "".join((child.text or "") for child in case if child.tag in {"system-out", "system-err"})
        records[name] = (case.get("time", "unknown"), output)
    if set(records) != set(required):
        raise ValueError("required Native testcase is missing: " + ", ".join(set(required) - set(records)))
    for case, scenario in [(COMPILED, "containment"), (MEMORY, "memory")]:
        marker = f"compiled-native-{scenario}=passed platform={RUST_OS[target_platform]} arch={RUST_ARCH[target_arch]}"
        if marker not in records[case][1].splitlines():
            raise ValueError(f"compiled Native {scenario} completion marker is missing or belongs to another native target")
    lines = ["scope=native-compiled-host", f"platform={target_platform} arch={target_arch}", f"junit_sha256={hashlib.sha256(report).hexdigest()}"]
    for name in required:
        time, output = records[name]
        lines.extend([f"test={name} result=passed attempts=1 seconds={time}", output.rstrip()])
    log = ("\n".join(lines) + "\n").encode()
    if len(log) > MAX_LOG:
        raise ValueError("Native receipt log exceeds 64 KiB")
    return len(required), log


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--junit", type=Path, required=True)
    parser.add_argument("--compiled-image", type=Path, required=True)
    parser.add_argument("--bundle", type=Path, required=True)
    parser.add_argument("--bun", type=Path, required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--image-platform", required=True)
    parser.add_argument("--image-arch", required=True)
    parser.add_argument("--host-sha256", required=True)
    parser.add_argument("--bundle-sha256", required=True)
    parser.add_argument("--runtime-sha256", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    native_platform, native_arch = native_identity()
    if (args.image_platform, args.image_arch) != (native_platform, native_arch):
        raise ValueError("actual image/compiler identity differs from native runner; emulation/cross-target proof does not transfer")
    if len(args.source_sha) != 40 or any(char not in "0123456789abcdef" for char in args.source_sha):
        raise ValueError("source SHA must identify the exact 40-hex CI checkout")
    canonical = "codewhale-extension-host" + (".exe" if native_platform == "win32" else "")
    if args.compiled_image.name != canonical:
        raise ValueError("compiled Native receipt requires the canonical image basename")
    for path, expected in [(args.compiled_image, args.host_sha256), (args.bundle, args.bundle_sha256), (args.bun, args.runtime_sha256)]:
        if not path.is_file() or path.is_symlink() or digest(path) != expected:
            raise ValueError(f"qualified input changed: {path.name}")
    if args.junit.stat().st_size > MAX_REPORT:
        raise ValueError("oversized nextest report")
    passed, log = extract(args.junit.read_bytes(), native_platform, native_arch)
    libc = None
    if native_platform == "linux":
        value = os.confstr("CS_GNU_LIBC_VERSION")
        if not value or not value.startswith("glibc "):
            raise ValueError("Linux native receipt requires a measured glibc runner; musl is unqualified")
        libc = {"family": "glibc", "version": value.split(" ", 1)[1], "scope": "native-runner-observed"}
    # A preexisting output must not be confused with this attempt's receipt.
    args.output.mkdir(parents=True, exist_ok=False)
    copied = args.output / canonical
    shutil.copy2(args.compiled_image, copied)
    if digest(copied) != args.host_sha256:
        raise ValueError("copied compiled image differs from tested bytes")
    (args.output / "native-containment.log").write_bytes(log)
    receipt = {
        "scope": "native-compiled-host", "source_sha": args.source_sha,
        "bundle_sha256": args.bundle_sha256, "runtime_sha256": args.runtime_sha256,
        "host_sha256": args.host_sha256, "platform": native_platform, "arch": native_arch,
        "passed": passed, "failed": 0, "skipped": 0,
        "log": "native-containment.log", "log_sha256": hashlib.sha256(log).hexdigest(),
        "junit_sha256": digest(args.junit), "libc": libc,
    }
    (args.output / "native-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(receipt))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, ET.ParseError) as error:
        print(f"Native compiled-host receipt refused: {error}", file=sys.stderr)
        sys.exit(1)
