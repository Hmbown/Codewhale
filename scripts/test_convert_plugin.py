#!/usr/bin/env python3
"""Offline conversion contracts; only the synthetic Node fixture is executed."""

import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

import yaml


SCRIPT = Path(__file__).resolve().with_name("convert-plugin.py")
SPEC = importlib.util.spec_from_file_location("convert_plugin", SCRIPT)
assert SPEC and SPEC.loader
converter = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(converter)
CANARY = "conversion-secret-canary-do-not-emit-7391"


class ConversionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="plugin-conversion-test-")
        self.addCleanup(self.temp.cleanup)
        # macOS /tmp and /var are links; the converter intentionally rejects them.
        self.root = Path(self.temp.name).resolve()
        self.sequence = 0

    def fresh(self, prefix):
        self.sequence += 1
        return self.root / f"{prefix}-{self.sequence}"

    def write(self, content, suffix=".json"):
        path = self.fresh("source").with_suffix(suffix)
        path.write_text(content, encoding="utf-8")
        return path

    def config(self, document):
        return self.write(json.dumps(document))

    def remote(self, **changes):
        return {"type": "remote", "url": "https://docs.example.invalid/mcp", "oauth": False, **changes}

    def v1(self, **changes):
        return {"mcp": {"docs": self.remote(**changes)}}

    def dsh(self, **changes):
        return [{"id": "docs-entry", "name": "@deepseek-ai/dsh-mcp-client", "config": {
            "serverName": "docs", "transport": "streamable-http",
            "url": "https://docs.example.invalid/mcp", **changes}}]

    def skill(self, name="safe-skill", *, metadata=None, body="Read the local reference.\n", directory=True):
        meta = {"name": name, "description": "Local reference guidance", **(metadata or {})}
        text = "---\n" + yaml.safe_dump(meta, sort_keys=False) + "---\n" + body
        if not directory:
            return self.write(text, ".md")
        path = self.fresh("skill")
        path.mkdir()
        (path / "SKILL.md").write_text(text, encoding="utf-8")
        return path

    def args(self, *, config=None, bundle=None, skills=(), dialect="opencode-v1", output=None,
             name="converted-demo", stdio_roots=()):
        return argparse.Namespace(config=config, bundle=bundle, skill=list(skills), format=dialect,
                                  output=output or self.fresh("output"), name=name, stdio_root=list(stdio_roots))

    def cli(self, args, *, env=None):
        command = [sys.executable, "-B", str(SCRIPT), "--format", args.format,
                   "--name", args.name, "--output", str(args.output)]
        if args.config:
            command += ["--config", str(args.config)]
        if args.bundle:
            command += ["--bundle", str(args.bundle)]
        for skill in args.skill:
            command += ["--skill", str(skill)]
        for root in args.stdio_root:
            command += ["--stdio-root", root]
        environment = dict(os.environ)
        environment.update(env or {})
        environment["PYTHONDONTWRITEBYTECODE"] = "1"
        return subprocess.run(command, capture_output=True, text=True, timeout=15,
                              cwd=self.root, env=environment, check=False)

    def refuse(self, args, *, message=None):
        with self.assertRaises(converter.ConversionError) as failure:
            converter.convert(args)
        if message:
            self.assertIn(message, str(failure.exception))
        self.assertFalse(args.output.exists(), "A rejected conversion must not publish a partial bundle")

    def servers(self, output):
        return json.loads((output / "mcp.json").read_text())["mcpServers"]

    def assert_no_canary(self, result, output):
        self.assertNotIn(CANARY, result.stdout)
        self.assertNotIn(CANARY, result.stderr)
        if output.exists():
            for path in output.rglob("*"):
                if path.is_file():
                    self.assertNotIn(CANARY.encode(), path.read_bytes())

    def test_cli_v1_preserves_disable_request_timeout_and_env_name(self):
        args = self.args(config=self.config(self.v1(
            enabled=False, timeout=7000, headers={"Authorization": "{env:CONVERSION_TEST_TOKEN}"})))
        result = self.cli(args, env={"CONVERSION_TEST_TOKEN": CANARY})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.servers(args.output), {"docs": {
            "type": "streamable-http", "url": "https://docs.example.invalid/mcp",
            "extensions": {"net.codewhale": {"disabled": True, "connect_timeout": 7,
                "execute_timeout": 7, "env_headers": {"Authorization": "CONVERSION_TEST_TOKEN"}}}}})
        manifest = json.loads((args.output / "plugin.json").read_text())
        self.assertEqual(manifest["extensions"]["net.codewhale"]["capabilities"]["network_hosts"],
                         ["docs.example.invalid"])
        self.assert_no_canary(result, args.output)

    def test_v2_preserves_global_timeout_and_per_server_override(self):
        document = {"mcp": {"timeout": {"startup": 4000, "request": 11000}, "servers": {
            "docs": self.remote(disabled=True, timeout={"request": 23000}),
            "other": self.remote(disabled=False)}}}
        args = self.args(config=self.config(document), dialect="opencode-v2")
        self.assertEqual(converter.convert(args), (0, 2, 0))
        servers = self.servers(args.output)
        self.assertEqual(servers["docs"]["extensions"]["net.codewhale"],
                         {"disabled": True, "connect_timeout": 4, "execute_timeout": 23})
        self.assertEqual(servers["other"]["extensions"]["net.codewhale"],
                         {"disabled": False, "connect_timeout": 4, "execute_timeout": 11})

    def test_cli_dsh_preserves_disabled_and_tool_timeout(self):
        document = self.dsh(toolCallTimeoutMs=19000)
        document[0]["disabled"] = True
        args = self.args(config=self.write(yaml.safe_dump(document), ".yml"), dialect="dsh")
        result = self.cli(args)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.servers(args.output)["docs"], {
            "type": "streamable-http", "url": "https://docs.example.invalid/mcp",
            "extensions": {"net.codewhale": {"disabled": True, "execute_timeout": 19}}})

    def node_source(self):
        root = self.fresh("packaged-node")
        root.mkdir()
        (root / "server.mjs").write_text("throw new Error('converter must never execute this');\n")
        return root

    def local(self, **changes):
        return {"type": "local", "command": ["node", "server.mjs"], **changes}

    def test_cli_local_node_copies_source_without_execution_or_credential_lookup(self):
        root = self.node_source()
        (root / "resource.json").write_text('{"answer":42}')
        args = self.args(config=self.config({"mcp": {"localdocs": self.local(
            enabled=False, timeout=7000, environment={"API_TOKEN": "{env:CONVERSION_TEST_TOKEN}"})}}),
            stdio_roots=[f"localdocs={root}"])
        result = self.cli(args, env={"CONVERSION_TEST_TOKEN": CANARY})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.servers(args.output)["localdocs"], {
            "type": "stdio", "command": "node", "args": ["server.mjs"], "cwd": "mcp/localdocs",
            "env": {"API_TOKEN": "${CONVERSION_TEST_TOKEN}"},
            "extensions": {"net.codewhale": {"disabled": True, "connect_timeout": 7, "execute_timeout": 7}}})
        self.assertEqual((args.output / "mcp/localdocs/server.mjs").read_bytes(), (root / "server.mjs").read_bytes())
        self.assertEqual((args.output / "mcp/localdocs/resource.json").read_bytes(), (root / "resource.json").read_bytes())
        extension = json.loads((args.output / "plugin.json").read_text())["extensions"]["net.codewhale"]
        self.assertEqual(extension, {"when": {"binaries": ["node"]}})
        self.assert_no_canary(result, args.output)

    def test_local_node_v2_and_dsh_preserve_disable_cwd_and_timeouts(self):
        root = self.node_source()
        cases = [("opencode-v2", {"mcp": {"timeout": {"startup": 4000, "request": 11000}, "servers": {
            "docs": self.local(cwd=".", disabled=True, timeout={"request": 23000})}}},
            {"disabled": True, "connect_timeout": 4, "execute_timeout": 23}),
            ("dsh", [{"name": "@deepseek-ai/dsh-mcp-client", "disabled": True, "config": {
                "serverName": "docs", "transport": "stdio", "command": "node", "args": ["server.mjs"],
                "cwd": ".", "env": {}, "toolCallTimeoutMs": 19000}}],
                {"disabled": True, "execute_timeout": 19})]
        for dialect, document, extension in cases:
            with self.subTest(dialect=dialect):
                args = self.args(config=self.config(document), dialect=dialect, stdio_roots=[f"docs={root}"])
                self.assertEqual(converter.convert(args), (0, 1, 0))
                server = self.servers(args.output)["docs"]
                self.assertEqual(server["cwd"], "mcp/docs")
                self.assertEqual(server["extensions"]["net.codewhale"], extension)

    @unittest.skipUnless(shutil.which("node"), "Node is needed for the synthetic MCP fixture")
    def test_packaged_node_mcp_discovers_and_calls_tool_with_sibling_and_cwd_resource(self):
        root = self.node_source()
        (root / "resource.json").write_text('{"answer":42}')
        (root / "sibling.mjs").write_text("export const name = 'fixture_answer';\n")
        (root / "server.mjs").write_text('''import readline from 'node:readline';
import { readFileSync } from 'node:fs';
import { name } from './sibling.mjs';
for await (const line of readline.createInterface({ input: process.stdin })) {
  const request = JSON.parse(line);
  if (request.id === undefined) continue;
  const result = request.method === 'initialize'
    ? { protocolVersion: '2024-11-05', capabilities: { tools: {} }, serverInfo: { name: 'fixture', version: '1' } }
    : request.method === 'tools/list'
      ? { tools: [{ name, description: 'Read the packaged answer', inputSchema: { type: 'object' } }] }
      : { content: [{ type: 'text', text: String(JSON.parse(readFileSync('resource.json', 'utf8')).answer) }] };
  process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: request.id, result }) + '\\n');
}
''')
        args = self.args(config=self.config({"mcp": {"docs": self.local()}}), stdio_roots=[f"docs={root}"])
        converter.convert(args)
        # Mutating original resources cannot change the converted package.
        (root / "resource.json").write_text('{"answer":99}')
        server = self.servers(args.output)["docs"]
        requests = [
            {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}},
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
            {"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "fixture_answer", "arguments": {}}},
        ]
        result = subprocess.run([shutil.which("node"), *server["args"]], cwd=args.output / server["cwd"],
            input="".join(json.dumps(request) + "\n" for request in requests), text=True, capture_output=True,
            timeout=10, check=False, env={"PATH": str(Path(shutil.which("node")).parent)})
        self.assertEqual(result.returncode, 0, result.stderr)
        responses = [json.loads(line) for line in result.stdout.splitlines()]
        self.assertEqual(responses[1]["result"]["tools"][0]["name"], "fixture_answer")
        self.assertEqual(responses[2]["result"]["content"], [{"type": "text", "text": "42"}])

    @unittest.skipUnless(shutil.which("node"), "Node is needed for the synthetic module fixture")
    def test_packaged_js_and_cjs_preserve_node_module_context(self):
        for entry, package_type, esm in (("server.js", "module", True),
                                         ("server.js", "commonjs", False),
                                         ("server.cjs", "module", False)):
            with self.subTest(entry=entry, package_type=package_type):
                root = self.fresh("module-context")
                root.mkdir()
                (root / "package.json").write_text(json.dumps({"type": package_type}))
                (root / "resource.json").write_text('{"answer":42}')
                (root / "helper.cjs").write_text("exports.answer = 7;\n")
                imports = ("import fs from 'node:fs'; import helper from './helper.cjs';\n" if esm else
                           "const fs = require('node:fs'); const helper = require('./helper.cjs');\n")
                (root / entry).write_text(imports +
                    "console.log(JSON.stringify([helper.answer, JSON.parse(fs.readFileSync('resource.json', 'utf8')).answer]));\n")
                args = self.args(config=self.config({"mcp": {"docs": self.local(command=["node", entry])}}),
                                 stdio_roots=[f"docs={root}"])
                self.assertEqual(converter.convert(args), (0, 1, 0))
                (root / "resource.json").write_text('{"answer":99}')
                server = self.servers(args.output)["docs"]
                result = subprocess.run([shutil.which("node"), *server["args"]],
                    cwd=args.output / server["cwd"], text=True, capture_output=True, timeout=10,
                    check=False, env={"PATH": str(Path(shutil.which("node")).parent)})
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(json.loads(result.stdout), [7, 42])

    def test_local_node_requires_matching_explicit_roots_and_safe_launcher(self):
        root = self.node_source()
        for command in (["node", "../server.mjs"], ["node", "/server.mjs"], ["node", "C:\\server.mjs"],
                        ["node", "server.ts"], ["node", "server.py"],
                        ["node", "--eval", "process.exit()"], ["node", "server.mjs", CANARY],
                        ["npx", "some-server"], ["sh", "server.mjs"], ["node", "https://example.invalid/server.mjs"],
                        ["node", "{env:ENTRY}.mjs"], ["node", 1]):
            with self.subTest(command=command):
                self.refuse(self.args(config=self.config({"mcp": {"docs": self.local(command=command)}}),
                                      stdio_roots=[f"docs={root}"]))
        config = self.config({"mcp": {"docs": self.local()}})
        for roots in ([], [f"other={root}"], [f"docs={root}", f"docs={root}"], [f"docs={root}", f"other={root}"]):
            self.refuse(self.args(config=config, stdio_roots=roots))
        self.refuse(self.args(skills=[self.skill()], stdio_roots=[f"docs={root}"]))
        self.refuse(self.args(config=self.config(self.v1()), stdio_roots=[f"docs={root}"]))
        self.refuse(self.args(config=config, stdio_roots=[f"docs={root}"], output=root / "output"))

    def test_local_node_rejects_literal_environment_loader_overrides_and_nonportable_cwd(self):
        root = self.node_source()
        for environment in ({"TOKEN": CANARY}, {"TOKEN": "{file:/private/key}"}, {"PLUGIN_ROOT": "{env:TOKEN}"},
                            {"node_options": "{env:OPTIONS}"}, {"NODE_PATH": "{env:IMPORTS}"}, {"PATH": "{env:PATH}"},
                            {"DYLD_INSERT_LIBRARIES": "{env:LIBRARY}"}, {"LD_PRELOAD": "{env:LIBRARY}"}):
            args = self.args(config=self.config({"mcp": {"docs": self.local(environment=environment)}}),
                             stdio_roots=[f"docs={root}"])
            result = self.cli(args)
            self.assertEqual(result.returncode, 1)
            self.assertFalse(args.output.exists())
            self.assert_no_canary(result, args.output)
        for cwd in ("..", "./workspace", "/workspace", ["."]):
            self.refuse(self.args(config=self.config({"mcp": {"servers": {"docs": self.local(cwd=cwd)}}}),
                                  dialect="opencode-v2", stdio_roots=[f"docs={root}"]))
        for extra in ({"env": {"TOKEN": "{env:TOKEN}"}}, {"failOnStartupError": True}, {"reconnect": {}}):
            config = {"serverName": "docs", "transport": "stdio", "command": "node", "args": ["server.mjs"], **extra}
            self.refuse(self.args(config=self.config([{"name": "@deepseek-ai/dsh-mcp-client", "config": config}]),
                                  dialect="dsh", stdio_roots=[f"docs={root}"]))

    def test_local_node_refuses_secret_and_linked_dependencies_atomically(self):
        for name in (".env.local", ".gitignore", ".npmrc", "credentials.json", "server.key", "identity.pem"):
            root = self.node_source()
            (root / name).write_text(CANARY)
            args = self.args(config=self.config({"mcp": {"docs": self.local()}}), stdio_roots=[f"docs={root}"])
            result = self.cli(args)
            self.assertEqual(result.returncode, 1)
            self.assert_no_canary(result, args.output)
            self.assertFalse(args.output.exists())
        for kind in ("symlink", "hardlink", "directory-link"):
            root = self.node_source()
            outside = self.write("outside", ".mjs")
            if kind == "hardlink":
                os.link(outside, root / "dependency.mjs")
            elif kind == "directory-link":
                (root / "node_modules").symlink_to(self.root, target_is_directory=True)
            else:
                (root / "dependency.mjs").symlink_to(outside)
            self.refuse(self.args(config=self.config({"mcp": {"docs": self.local()}}), stdio_roots=[f"docs={root}"]))
            self.assertEqual(outside.read_text(), "outside")

    def test_local_node_shares_aggregate_copy_budget_with_skills_and_servers(self):
        root = self.node_source()
        (root / "resource.bin").write_bytes(b"a" * 2300)
        config = self.config({"mcp": {"docs": self.local(), "second": self.local()}})
        with mock.patch.object(converter, "MAX_BYTES", 4000):
            self.refuse(self.args(config=config, stdio_roots=[f"docs={root}", f"second={root}"]))
        with mock.patch.object(converter, "MAX_FILES", 4):
            self.refuse(self.args(config=config, stdio_roots=[f"docs={root}", f"second={root}"]))

    def test_skill_keeps_explicit_invocation_and_metadata_out_of_frontmatter(self):
        extra = {"name": "wrong-name", "invocation": "automatic", "description": "wrong description"}
        skill = self.skill(metadata={"disable-model-invocation": True, "metadata": extra,
                                     "license": "MIT", "description": "Line one\ninvocation: automatic"})
        sentinel = self.root / "executed"
        code = f"from pathlib import Path\nPath({str(sentinel)!r}).write_text('executed')\n"
        (skill / "helper.py").write_text(code)
        args = self.args(skills=[skill])
        result = self.cli(args)
        self.assertEqual(result.returncode, 0, result.stderr)
        generated = args.output / "skills/safe-skill"
        text = (generated / "SKILL.md").read_text()
        front = yaml.safe_load(text.split("---", 2)[1])
        self.assertEqual(front, {"name": "safe-skill", "description": "Line one\ninvocation: automatic",
                                 "invocation": "explicit-only"})
        self.assertEqual(json.loads((generated / "SOURCE_SKILL_METADATA.json").read_text()),
                         {"metadata": extra, "license": "MIT"})
        self.assertEqual((generated / "helper.py").read_text(), code)
        self.assertFalse(sentinel.exists())
        self.assertIn("Read the local reference.", text)

    def test_flat_markdown_skill_uses_declared_name(self):
        skill = self.skill("flat-guide", directory=False)
        args = self.args(skills=[skill], dialect="opencode-v2")
        self.assertEqual(converter.convert(args), (1, 0, 0))
        self.assertTrue((args.output / "skills/flat-guide/SKILL.md").is_file())

    def test_skill_delimiter_and_unrepresentable_invocation_are_refused(self):
        for metadata in ({"description": "Review --- carefully", "disable-model-invocation": True},
                         {"user-invocable": False}, {"disable-model-invocation": "false"}):
            with self.subTest(metadata=metadata):
                self.refuse(self.args(skills=[self.skill(metadata=metadata)]))

    def test_executable_plugin_declarations_never_run(self):
        sentinel = self.root / "foreign-code-ran"
        plugin = self.root / "plugin.py"
        plugin.write_text(f"from pathlib import Path\nPath({str(sentinel)!r}).touch()\n")
        cases = [("opencode-v1", {"plugin": [str(plugin)], **self.v1()}),
                 ("opencode-v2", {"plugins": [{"package": str(plugin)}], "mcp": {"servers": {"docs": self.remote()}}}),
                 ("dsh", [{"id": "foreign", "name": str(plugin), "config": {}}])]
        for dialect, document in cases:
            with self.subTest(dialect=dialect):
                args = self.args(config=self.config(document), dialect=dialect)
                result = self.cli(args)
                self.assertEqual(result.returncode, 1)
                self.assertFalse(args.output.exists())
                self.assertFalse(sentinel.exists())

    def test_cli_global_tool_disable_is_not_dropped_during_mcp_conversion(self):
        document = {**self.v1(), "tools": {"docs*": False}}
        source = self.config(document)
        original = source.read_bytes()
        args = self.args(config=source)
        result = self.cli(args)
        self.assertEqual(result.returncode, 1)
        self.assertIn("preserve their restrictions in Codewhale", result.stderr)
        self.assertFalse(args.output.exists())
        self.assertEqual(source.read_bytes(), original)

    def test_opencode_policy_fields_require_a_manual_port_even_for_disabled_servers(self):
        rules = [{"action": "docs_*", "resource": "*", "effect": "deny"}]
        policies = {"tools": {"docs*": False}, "permission": {"docs_*": "deny"},
                    "permissions": rules,
                    "agent": {"reviewer": {"permission": {"docs_*": "deny"}}},
                    "agents": {"reviewer": {"permissions": rules}},
                    "mode": {"plan": {"tools": {"docs*": False}}}, "default_agent": "plan"}
        for dialect in ("opencode-v1", "opencode-v2"):
            for disabled in (False, True):
                for field, value in policies.items():
                    with self.subTest(dialect=dialect, disabled=disabled, field=field):
                        document = self.v1(enabled=not disabled) if dialect == "opencode-v1" else {
                            "mcp": {"servers": {"docs": self.remote(disabled=disabled)}}}
                        document[field] = value
                        self.refuse(self.args(config=self.config(document), dialect=dialect),
                                    message="require a manual port")

    def test_dsh_tag_and_plain_expression_are_rejected_even_when_disabled(self):
        sentinel = self.root / "expression-ran"
        expression = f"require('node:fs').writeFileSync({json.dumps(str(sentinel))}, 'ran')"
        document = self.dsh()
        document[0]["disabled"] = True
        document[0]["config"]["headers"] = {"Authorization": {"__jsExpr": expression}}
        tagged = yaml.safe_dump(self.dsh()).replace("transport: streamable-http", "transport: !!js " + expression)
        for text in (json.dumps(document), tagged):
            with self.subTest(text=text):
                self.refuse(self.args(config=self.write(text, ".yml"), dialect="dsh"))
                self.assertFalse(sentinel.exists())

    def test_dsh_patch_followed_by_disable_is_not_partially_imported(self):
        patches = [{"insert": self.dsh()}, {"id": "docs-entry", "disabled": True,
                    "config": {"url": "https://replacement.example.invalid/mcp"}}]
        self.refuse(self.args(config=self.config(patches), dialect="dsh"))

    def bundle(self, patch_text=None, manifest=None, patch_name="cordis.patch.yml"):
        directory = self.fresh("dsh-bundle")
        directory.mkdir()
        package = {"name": "@demo/tools-dsh", "version": "1.2.3",
                   "dsh": {"bundle": {"patch": f"./{patch_name}"}}, **(manifest or {})}
        (directory / "package.json").write_text(json.dumps(package))
        if patch_text is not None:
            (directory / patch_name).write_text(patch_text)
        return directory

    def test_dsh_bundle_evaluates_patches_and_skips_foreign_rows(self):
        bundle = self.bundle(yaml.safe_dump([
            {"insert": [
                {"id": "docs-entry", "name": "@deepseek-ai/dsh-mcp-client", "config": {
                    "serverName": "docs", "transport": "streamable-http",
                    "url": "https://docs.example.invalid/mcp", "toolCallTimeoutMs": 19000}},
                {"id": "skin", "name": "@deepseek-ai/dsh-client-ui-theme", "config": {"hue": 4}},
            ]},
            {"id": "docs-entry", "disabled": True},
            {"id": "ghost", "disabled": True},
        ]))
        args = self.args(bundle=bundle, dialect="dsh")
        result = self.cli(args)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.servers(args.output)["docs"], {
            "type": "streamable-http", "url": "https://docs.example.invalid/mcp",
            "extensions": {"net.codewhale": {"disabled": True, "execute_timeout": 19}}})
        receipt = (args.output / "CONVERSION.md").read_text()
        self.assertIn("@demo/tools-dsh@1.2.3", receipt)
        self.assertIn("skin", receipt)
        self.assertIn("ghost", receipt)

    def test_dsh_bundle_lowers_js_command_and_host_path_arg(self):
        bundle = self.bundle()
        server_dir = self.node_source()
        patch = ("- insert:\n  - id: local-entry\n    name: '@deepseek-ai/dsh-mcp-client'\n"
                 "    config:\n      serverName: localdocs\n      transport: stdio\n"
                 "      command: !!js process.execPath\n"
                 "      args:\n        - !!js process.env.CONVERT_TEST_UNSET_ENTRY_7391 || '"
                 + str(server_dir / "server.mjs") + "'\n")
        (bundle / "cordis.patch.yml").write_text(patch)
        args = self.args(bundle=bundle, dialect="dsh")
        result = self.cli(args)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.servers(args.output)["localdocs"], {
            "type": "stdio", "command": "node", "args": ["server.mjs"], "cwd": "mcp/localdocs",
            "env": {}, "extensions": {"net.codewhale": {}}})
        self.assertEqual((args.output / "mcp/localdocs/server.mjs").read_bytes(),
                         (server_dir / "server.mjs").read_bytes())

    def test_dsh_bundle_relative_entry_and_group_children_convert(self):
        bundle = self.bundle()
        (bundle / "mcp").mkdir()
        (bundle / "mcp" / "server.mjs").write_text("// fixture\n")
        patch = yaml.safe_dump([
            {"insert": [{"id": "grouped", "group": True, "config": []}]},
            {"id": "grouped", "insert": [
                {"id": "in-group", "name": "@deepseek-ai/dsh-mcp-client", "config": {
                    "serverName": "inner", "transport": "stdio", "command": "node",
                    "args": ["server.mjs"], "cwd": "mcp"}}]},
        ])
        (bundle / "cordis.patch.yml").write_text(patch)
        args = self.args(bundle=bundle, dialect="dsh")
        result = self.cli(args)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.servers(args.output)["inner"]["cwd"], "mcp/inner")
        self.assertTrue((args.output / "mcp/inner/server.mjs").is_file())

    def test_dsh_bundle_imports_custom_skill_dirs(self):
        bundle = self.bundle()
        skill = bundle / "pack-skills" / "guide"
        skill.mkdir(parents=True)
        (skill / "SKILL.md").write_text("---\nname: guide\ndescription: Bundled skill\n---\nBody.\n")
        patch = yaml.safe_dump([
            {"insert": [
                {"id": "skills-row", "name": "@deepseek-ai/dsh-skill-filesystem",
                 "config": {"customSkillDirs": ["pack-skills"]}},
                {"id": "mcp", "name": "@deepseek-ai/dsh-mcp-client", "config": {
                    "serverName": "docs", "transport": "streamable-http",
                    "url": "https://docs.example.invalid/mcp"}},
            ]},
        ])
        (bundle / "cordis.patch.yml").write_text(patch)
        args = self.args(bundle=bundle, dialect="dsh")
        self.assertEqual(converter.convert(args), (1, 1, 0))
        self.assertTrue((args.output / "skills/guide/SKILL.md").is_file())

    def test_dsh_bundle_never_executes_js_and_records_unlowerable_rows(self):
        sentinel = self.root / "expression-ran"
        bundle = self.bundle()
        patch = ("- insert:\n  - id: bad\n    name: '@deepseek-ai/dsh-mcp-client'\n"
                 "    config:\n      serverName: bad\n      transport: streamable-http\n"
                 "      url: !!js require('node:fs').writeFileSync('" + str(sentinel) + "', 'ran')\n"
                 "  - id: ok\n    name: '@deepseek-ai/dsh-mcp-client'\n"
                 "    config:\n      serverName: ok\n      transport: streamable-http\n"
                 "      url: https://ok.example.invalid/mcp\n")
        (bundle / "cordis.patch.yml").write_text(patch)
        args = self.args(bundle=bundle, dialect="dsh")
        result = self.cli(args)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(sentinel.exists())
        self.assertEqual(sorted(self.servers(args.output)), ["ok"])
        self.assertIn("bad", (args.output / "CONVERSION.md").read_text())

    def test_dsh_bundle_requires_manifest_patch_inside_package(self):
        bundle = self.bundle()
        self.refuse(self.args(bundle=bundle, dialect="dsh"), message="patch")
        escaping = self.bundle(manifest={"dsh": {"bundle": {"patch": "../outside.yml"}}})
        self.refuse(self.args(bundle=escaping, dialect="dsh"))
        plain = self.fresh("not-a-bundle")
        plain.mkdir()
        (plain / "package.json").write_text(json.dumps({"name": "plain"}))
        self.refuse(self.args(bundle=plain, dialect="dsh"), message="dsh.bundle.patch")
        self.refuse(self.args(bundle=self.bundle(), dialect="opencode-v1"), message="--format dsh")
        combined = self.bundle()
        (combined / "cordis.patch.yml").write_text("[]")
        self.refuse(self.args(bundle=combined, config=self.config(self.dsh()), dialect="dsh"),
                    message="not both")

    def test_cli_literal_credentials_and_interpolation_never_echo_or_publish(self):
        secret_file = self.write(CANARY, ".txt")
        cases = [self.v1(headers={"Authorization": "Bearer " + CANARY}),
                 self.v1(headers={"Authorization": "{file:" + str(secret_file) + "}"}),
                 self.v1(url="https://user:" + CANARY + "@docs.example.invalid/mcp"),
                 self.v1(url="https://docs.example.invalid/mcp?token=" + CANARY),
                 self.v1(url="https://docs.example.invalid/mcp#" + CANARY)]
        for document in cases:
            with self.subTest(document=document):
                args = self.args(config=self.config(document))
                result = self.cli(args)
                self.assertEqual(result.returncode, 1)
                self.assert_no_canary(result, args.output)
                self.assertFalse(args.output.exists())
        self.assertEqual(secret_file.read_text(), CANARY)

    def test_opposite_dialect_enablement_is_not_silently_ignored(self):
        cases = [("opencode-v1", self.v1(disabled=True)),
                 ("opencode-v2", {"mcp": {"servers": {"docs": self.remote(enabled=False)}}}),
                 ("opencode-v1", {"mcp": {"servers": {"docs": self.remote()}}}),
                 ("opencode-v2", self.v1())]
        for dialect, document in cases:
            with self.subTest(dialect=dialect, document=document):
                self.refuse(self.args(config=self.config(document), dialect=dialect))

    def test_duplicate_json_and_yaml_keys_are_rejected(self):
        cases = [("opencode-v1", '{"mcp": {}, "mcp": {}}'),
                 ("opencode-v1", '{"mcp":{"docs":{"type":"remote","url":"https://a.invalid","url":"https://b.invalid","oauth":false}}}'),
                 ("dsh", '- id: one\n  name: x\n  name: y\n  config: {}\n')]
        for dialect, text in cases:
            with self.subTest(dialect=dialect):
                self.refuse(self.args(config=self.write(text), dialect=dialect))

    def test_case_duplicate_and_reserved_headers_are_refused(self):
        for headers in ({"Authorization": "{env:A}", "authorization": "{env:B}"},
                        {"Accept": "{env:A}"}, {"content-TYPE": "{env:A}"}):
            with self.subTest(headers=headers):
                self.refuse(self.args(config=self.config(self.v1(headers=headers))))

    def test_duplicate_server_and_skill_names_do_not_overwrite(self):
        duplicate = self.dsh() + self.dsh()
        duplicate[1]["id"] = "other-entry"
        self.refuse(self.args(config=self.config(duplicate), dialect="dsh"))
        self.refuse(self.args(skills=[self.skill(), self.skill()]))

    def test_unsupported_oauth_stdio_and_lifecycle_fields_are_refused(self):
        cases = [("opencode-v1", self.v1(oauth={})),
                 ("opencode-v1", {"mcp": {"docs": {"type": "remote", "url": "https://docs.example.invalid/mcp"}}}),
                 ("opencode-v1", {"mcp": {"docs": {"type": "local", "command": ["python", "plugin.py"]}}}),
                 ("dsh", self.dsh(transport="stdio", command="python")),
                 ("dsh", self.dsh(reconnect={"enabled": False})),
                 ("dsh", self.dsh(failOnStartupError=True))]
        for dialect, document in cases:
            with self.subTest(dialect=dialect, document=document):
                self.refuse(self.args(config=self.config(document), dialect=dialect))

    def test_timeout_values_must_preserve_exact_native_units(self):
        for value in (True, 0, 999, 1500, 3600001, 4000000, "7000"):
            with self.subTest(value=value):
                self.refuse(self.args(config=self.config(self.v1(timeout=value))))

    def test_canonical_loopback_hosts_and_ambiguous_numeric_addresses(self):
        for url, host in (("http://127.0.0.1:4312/mcp", "127.0.0.1"),
                          ("http://[::1]:4312/mcp", "[::1]")):
            with self.subTest(url=url):
                args = self.args(config=self.config(self.v1(url=url)))
                converter.convert(args)
                self.assertEqual(self.servers(args.output)["docs"]["url"], url)
                manifest = json.loads((args.output / "plugin.json").read_text())
                self.assertEqual(manifest["extensions"]["net.codewhale"]["capabilities"]["network_hosts"], [host])
        for url in ("http://public.example.invalid/mcp", "https://127.1/mcp", "https://0x7f000001/mcp",
                    "https://127.000.000.001/mcp", "https://docs.example.invalid:0/mcp"):
            with self.subTest(url=url):
                self.refuse(self.args(config=self.config(self.v1(url=url))))

    def test_source_and_ancestor_symlinks_are_refused(self):
        source = self.skill()
        link = self.root / "linked-skill"
        link.symlink_to(source, target_is_directory=True)
        self.refuse(self.args(skills=[link]))
        parent_link = self.root / "linked-parent"
        parent_link.symlink_to(self.root, target_is_directory=True)
        self.refuse(self.args(skills=[parent_link / source.name]))
        (source / "linked-companion").symlink_to(self.write("private data", ".txt"))
        self.refuse(self.args(skills=[source]))

    def test_hardlinked_source_is_refused_without_altering_either_name(self):
        source = self.skill(directory=False)
        before = source.read_bytes()
        other = self.root / "hardlinked.md"
        os.link(source, other)
        self.refuse(self.args(skills=[source]))
        self.assertEqual(source.read_bytes(), before)
        self.assertEqual(other.read_bytes(), before)

    def test_output_link_is_refused_and_its_target_is_untouched(self):
        target = self.root / "existing-target"
        target.mkdir()
        marker = target / "keep"
        marker.write_text("keep")
        output = self.root / "output-link"
        output.symlink_to(target, target_is_directory=True)
        args = self.args(skills=[self.skill()], output=output)
        with self.assertRaises(converter.ConversionError):
            converter.convert(args)
        self.assertTrue(output.is_symlink())
        self.assertEqual(list(target.iterdir()), [marker])
        self.assertEqual(marker.read_text(), "keep")

    def test_traversal_names_and_output_inside_source_are_refused(self):
        skill = self.skill()
        self.refuse(self.args(skills=[skill], name="../escaped"))
        self.refuse(self.args(skills=[self.skill("../escaped")]))
        self.refuse(self.args(skills=[skill], output=skill / "generated"))
        self.assertFalse((self.root.parent / "escaped").exists())

    def test_generated_metadata_collision_never_overwrites_source(self):
        skill = self.skill(metadata={"metadata": {"license-owner": "fixture"}})
        companion = skill / "SOURCE_SKILL_METADATA.json"
        companion.write_text("original")
        self.refuse(self.args(skills=[skill]), message="collides")
        self.assertEqual(companion.read_text(), "original")

    def test_existing_output_is_unchanged_by_cli_refusal(self):
        output = self.fresh("existing-output")
        output.mkdir()
        (output / "plugin.json").write_text("original manifest")
        (output / "other.txt").write_bytes(b"untouched")
        result = self.cli(self.args(skills=[self.skill()], output=output))
        self.assertEqual(result.returncode, 1)
        self.assertEqual({p.name: p.read_bytes() for p in output.iterdir()},
                         {"plugin.json": b"original manifest", "other.txt": b"untouched"})

    def test_aggregate_byte_budget_rejects_before_opening_excess_companion(self):
        first, second = self.skill("first-guide"), self.skill("second-guide")
        (first / "asset.bin").write_bytes(b"a" * 2300)
        (second / "asset.bin").write_bytes(b"b" * 2300)
        with mock.patch.object(converter, "MAX_BYTES", 4000):
            # Either selection fits; their combination exceeds the same limit.
            converter.convert(self.args(skills=[first]))
            converter.convert(self.args(skills=[second]))
            with mock.patch.object(converter.os, "open", wraps=os.open) as opened:
                self.refuse(self.args(skills=[first, second]))
                paths = [Path(call.args[0]) for call in opened.call_args_list]
                self.assertIn(first / "asset.bin", paths)
                self.assertNotIn(second / "asset.bin", paths)

    def test_file_budget_has_a_success_control_and_preserves_atomic_rejection(self):
        skill = self.skill()
        (skill / "one.txt").write_text("one")
        (skill / "two.txt").write_text("two")
        with mock.patch.object(converter, "MAX_FILES", 5):
            converter.convert(self.args(skills=[skill]))
            (skill / "three.txt").write_text("three")
            self.refuse(self.args(skills=[skill]), message="budget")

    def test_oversize_document_is_rejected_before_open(self):
        source = self.write(" " * (1024 * 1024 + 1))
        with mock.patch.object(converter.os, "open", wraps=os.open) as opened:
            self.refuse(self.args(config=source), message="size limit")
            opened.assert_not_called()

    def test_depth_alias_and_server_count_bounds(self):
        for text, dialect in (("[" * 34 + "0" + "]" * 34, "dsh"),
                              ("- &entry {name: x}\n- *entry\n", "dsh"),
                              (json.dumps({"mcp": {f"server{i}": self.remote() for i in range(65)}}), "opencode-v1")):
            with self.subTest(dialect=dialect, text=text[:40]):
                self.refuse(self.args(config=self.write(text), dialect=dialect))

    def test_write_failure_cleans_only_new_output_and_preserves_source(self):
        skill = self.skill()
        original = (skill / "SKILL.md").read_bytes()
        unrelated = self.write("keep", ".txt")
        args = self.args(skills=[skill])
        real_open = Path.open

        def failing_open(path, *positional, **keywords):
            if path == args.output / "plugin.json":
                raise OSError("simulated disk write failure")
            return real_open(path, *positional, **keywords)

        with mock.patch.object(Path, "open", failing_open):
            with self.assertRaises(OSError):
                converter.convert(args)
        self.assertFalse(args.output.exists())
        self.assertEqual((skill / "SKILL.md").read_bytes(), original)
        self.assertEqual(unrelated.read_text(), "keep")

    def test_cli_jsonc_and_malformed_secret_input_are_safely_refused(self):
        for text in ('// comment\n' + json.dumps(self.v1()),
                     '{"mcp": {},}', '{"mcp": "' + CANARY):
            with self.subTest(text=text[:30]):
                args = self.args(config=self.write(text, ".jsonc"))
                result = self.cli(args)
                self.assertEqual(result.returncode, 1)
                self.assert_no_canary(result, args.output)
                self.assertFalse(args.output.exists())

    def test_local_secret_companion_is_refused_without_publication(self):
        skill = self.skill()
        (skill / ".env").write_text("TOKEN=" + CANARY)
        args = self.args(skills=[skill])
        result = self.cli(args)
        self.assertEqual(result.returncode, 1)
        self.assert_no_canary(result, args.output)
        self.assertFalse(args.output.exists())


if __name__ == "__main__":
    unittest.main()
