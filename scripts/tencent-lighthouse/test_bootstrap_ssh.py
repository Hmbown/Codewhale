#!/usr/bin/env python3
"""Exercise the bootstrap's real SSH preflight and firewall block with a fake UFW."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


class BootstrapSshTests(unittest.TestCase):
    def run_policy(self, cidrs=None, allow_any=None):
        source = Path(__file__).with_name("bootstrap-ubuntu.sh").read_text()
        preflight = source.split("# SSH_ALLOWED_CIDRS accepts", 1)[1].split("apt-get update", 1)[0]
        firewall = source.split("# Apply the already validated narrow rules", 1)[1].split("ufw --force enable", 1)[0]
        with tempfile.TemporaryDirectory() as directory:
            receipt = Path(directory) / "ufw.txt"
            env = dict(os.environ, UFW_RECEIPT=str(receipt))
            env.pop("SSH_ALLOWED_CIDRS", None)
            env.pop("SSH_ALLOW_ANY_SOURCE", None)
            if cidrs is not None:
                env["SSH_ALLOWED_CIDRS"] = cidrs
            if allow_any is not None:
                env["SSH_ALLOW_ANY_SOURCE"] = allow_any
            # Only the actual preflight and firewall policy execute. Package
            # installation, user creation, cloning and host files never run.
            script = "set -euo pipefail\nufw() { printf '%s\\n' \"$*\" >>\"$UFW_RECEIPT\"; }\n"
            script += "# SSH_ALLOWED_CIDRS accepts" + preflight
            script += "# Apply the already validated narrow rules" + firewall
            result = subprocess.run(["bash", "-c", script], env=env, capture_output=True, text=True)
            return result, receipt.read_text().splitlines() if receipt.exists() else []

    def test_default_rejects_before_any_firewall_change(self):
        result, calls = self.run_policy()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("No host changes", result.stderr)
        self.assertEqual(calls, [])

    def test_whitespace_and_commas_do_not_opt_in(self):
        result, calls = self.run_policy(" ,\t,\n ")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(calls, [])

    def test_explicit_public_ssh_is_warned(self):
        result, calls = self.run_policy(allow_any="1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("reachable from every source", result.stderr)
        self.assertEqual(calls, ["allow OpenSSH"])

    def test_mistyped_public_opt_in_is_denied(self):
        result, calls = self.run_policy(allow_any="true")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(calls, [])

    def test_narrow_ipv4_and_ipv6_are_added_before_broad_rule_removal(self):
        result, calls = self.run_policy("203.0.113.4/32, 2001:db8::/64")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls, ["allow from 203.0.113.4/32 to any app OpenSSH", "allow from 2001:db8::/64 to any app OpenSSH", "delete allow OpenSSH"])

    def test_explicit_cidrs_take_precedence_over_public_opt_in(self):
        result, calls = self.run_policy("203.0.113.0/24", "1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("allow OpenSSH", calls)

    def test_invalid_later_cidr_changes_no_rules(self):
        result, calls = self.run_policy("203.0.113.4/32, ::::::/64")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(calls, [])

    def test_unbounded_or_invalid_networks_are_denied(self):
        for cidr in ["0.0.0.0/0", "::/0", "203.0.113.4", "300.1.2.3/32", "2001:db8::/129", "fe80::%en0/64"]:
            with self.subTest(cidr=cidr):
                result, calls = self.run_policy(cidr)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(calls, [])


if __name__ == "__main__":
    unittest.main()
