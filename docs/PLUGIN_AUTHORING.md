# Write your first Codewhale plugin

> 阅读简体中文版：[zh_hans/PLUGIN_AUTHORING.md](zh_hans/PLUGIN_AUTHORING.md)

Start with a skill: a Markdown instruction file inside a small plugin bundle.
The [hello-codewhale example](examples/plugins/hello-codewhale/plugin.json)
contains two files, declares no server or hook, and asks for no tool use.
This walkthrough takes it from source files to a reviewed, enabled skill.

## 1. Create the bundle

Use the checked-in example, or create this directory outside an installed
plugins directory:

```text
hello-codewhale/
├── plugin.json
└── skills/
    └── hello/
        └── SKILL.md
```

`plugin.json`:

```json
{
  "$schema": "https://agent-plugins.org/schemas/plugin.json",
  "name": "hello-codewhale",
  "version": "0.1.0",
  "description": "A minimal, explicitly invoked greeting skill."
}
```

`skills/hello/SKILL.md`:

```markdown
---
name: hello
description: Greet the user when they explicitly try the hello-codewhale example.
invocation: explicit-only
---

Respond with one short greeting in the user's language. Include the exact text
`hello-codewhale:hello` so they can identify the example they invoked.

Use only the conversation. Do not call tools, run commands, read or write files,
or contact external services.
```

Codewhale finds `skills/` automatically. `explicit-only` keeps this example out
of the model's automatic skill catalogue; you load it by name. See
[Skills](SKILLS.md#invocation-and-alias-metadata) for invocation metadata and
[Plugin bundles](PLUGIN_BUNDLES.md#manifest) for the authoritative manifest
contract. Keep new native bundles in `plugin.json`; no second manifest is
needed.

## 2. Install, inspect, and trust

Start Codewhale in the repository root. Enter these commands **inside the
Codewhale session**, one at a time:

```text
/plugin install ./docs/examples/plugins/hello-codewhale
/plugin validate hello-codewhale
/plugin show hello-codewhale
```

For your own bundle, replace the install path with its directory. Installation
copies it to `~/.codewhale/plugins/hello-codewhale/`, disabled and untrusted.
Review the installed source, skill inventory, and permissions. This example
should declare only Skills, with no MCP server, hook, or requested network host.

The install review prints a command containing two full hashes:

```text
/plugin trust hello-codewhale <full-content-sha256>.<full-capability-sha256>
```

Run the exact command printed by your review; the angle-bracket text above is
a placeholder. If you need a fresh review, use `/plugin trust hello-codewhale`
without a token. Trust records the reviewed content and capability hashes and
creates a runtime snapshot. It does not enable the plugin.

```text
/plugin enable hello-codewhale
/skills hello-codewhale:
/skills inspect
```

The skill is named `hello-codewhale:hello`: the bundle name qualifies the skill
name. `/skills inspect` identifies its reviewed plugin snapshot.

## 3. Invoke it and turn it off

```text
/skill hello-codewhale:hello
```

Codewhale confirms activation. Then send `Say hello.` as a normal message.
The reply should be a short greeting containing `hello-codewhale:hello`.
The example contributes instructions only; the reply still uses your selected
model and its normal provider connection. The local install, review, and
activation steps do not need a model call.

```text
/plugin disable hello-codewhale
```

Disabling removes the plugin's contributions while preserving its trust
receipt. A subsequent `/skill hello-codewhale:hello` must not activate it.
Enable it again when needed, provided its reviewed hashes still match.

## 4. Iterate and review changes

The installed bundle is a copy. Editing the example's original source does
not update that copy. To try a changed local source, disable and uninstall the
installed example, then install the source directory again:

```text
/plugin disable hello-codewhale
/plugin uninstall hello-codewhale
/plugin install ./docs/examples/plugins/hello-codewhale
/plugin validate hello-codewhale
```

Uninstall removes the installed copy; it leaves the original example source
alone. Review the new token, trust it, and enable it again. For bundles
installed from a remote source, use `/plugin update <name>`; see
[Installing plugins](PLUGINS.md#update-and-uninstall).

When files in a discovered bundle change directly, `/plugin reload` refreshes
the registry. Changed content invalidates the old receipt, even if you leave
the version unchanged. Reload does not grant trust. Use `/plugin revoke <name>`
to remove trust explicitly.

## Add only the components you need

All components use the same bundle review and existing Codewhale runtime:

| Component | Authoring surface |
| --- | --- |
| Skills | `skills/<name>/SKILL.md`; [instruction and invocation contract](SKILLS.md). |
| MCP | A sibling `mcp.json`; [bundle transport and credential rules](PLUGIN_BUNDLES.md#validation-both-formats). |
| Commands | Markdown command files; [command metadata](architecture/command-dispatch.md#user-commands). |
| Agent profiles | Fleet TOML profiles; [Fleet authoring](FLEET.md#authoring-agent-profiles-fleet-setup). |
| Hooks | `HooksConfig` TOML files; [events and process behavior](HOOKS.md). |

Declare Commands, Agents, and Hooks paths under
`extensions["net.codewhale"]` in `plugin.json`, as specified in
[Plugin bundles](PLUGIN_BUNDLES.md#active-and-inactive-component-surfaces).
Do not place MCP server fields or arbitrary runtime entrypoints at the manifest
root. LSP and native extensions can be inventoried but are not executable
plugin adapters.

Plugin trust is **not an OS sandbox**. A local MCP server or hook can launch a
process; review its code and authority before enabling it. Skills do not grant
permissions: repository instructions, permission rules, sandbox policy, and
tool approval still apply. Keep credentials out of bundles and command
arguments. Use the reviewed environment references documented in the
[bundle validation contract](PLUGIN_BUNDLES.md#validation-both-formats) for MCP;
read the separate [hook environment contract](HOOKS.md#the-hook-process-environment)
before adding a hook.

## Convert an existing plugin

[`scripts/convert-plugin.py`](../scripts/convert-plugin.py) converts explicitly
selected remote MCP declarations, packaged local Node MCP servers, and portable
Skills into a native bundle.
It requires Python 3.10+ and PyYAML 6+; install those separately if absent.
The converter installs no dependencies, scans no ambient configuration or
credentials, makes no network requests, and executes no source code.

### OpenCode

Save this plain JSON as `opencode-mcp.json`:

```json
{
  "mcp": {
    "docs": {
      "type": "remote",
      "url": "https://example.invalid/mcp",
      "oauth": false,
      "enabled": false
    }
  }
}
```

From the Codewhale repository root, run this in your shell:

```sh
python3 scripts/convert-plugin.py --format opencode-v1 \
  --config ./opencode-mcp.json --name migrated-tools --output ./migrated-opencode
```

Choose `--format opencode-v2` for the `mcp.servers.<name>` layout, whose server
flag is `disabled` instead of `enabled`. Select the format from the data;
filenames and upstream branch names do not determine its version. Both formats
require explicit `oauth: false` for remote servers. Remote MCP output uses **Streamable HTTP only**;
OpenCode's fallback to legacy SSE is not reproduced. For an SSE-only endpoint,
author native `mcp.json` with `type: "sse"` and use the same review flow.

When MCP servers are selected, configurations containing `tools`,
`permission`/`permissions`, `agent`/`agents`, legacy `mode`, or `default_agent`
are refused. These settings can restrict tool access beyond server enablement.
Manually preserve those restrictions in Codewhale before supplying an MCP-only
input; simply deleting the settings can widen access.

JSONC comments and trailing commas are not
accepted: provide a plain JSON copy containing the declarations you intend
to port.

### DeepSeek Harness (DSH)

Save this static Cordis entry list as `dsh-mcp.yml`:

```yaml
- name: '@deepseek-ai/dsh-mcp-client'
  disabled: true
  config:
    serverName: docs
    transport: streamable-http
    url: https://example.invalid/mcp
```

```sh
python3 scripts/convert-plugin.py --format dsh \
  --config ./dsh-mcp.yml --name migrated-dsh --output ./migrated-dsh
```

The DSH input may also be JSON, but must be the plain entry list, not a full
profile or patch composition. Each row must name `@deepseek-ai/dsh-mcp-client`.

A real dsh bundle package — an npm package whose `package.json` declares
`dsh.bundle.patch` — converts directly with `--bundle`:

```sh
python3 scripts/convert-plugin.py --format dsh \
  --bundle ./node_modules/@demo/tools-dsh --name migrated-dsh --output ./migrated-dsh
```

`dsh.bundle.patch` may name one patch file or an ordered list of files. The
converter reads only contained, non-linked package files (at most 64 files and
1 MiB of patch data combined), then applies `insert` and keyed overrides over
one empty profile. As in upstream `applyEntryPatches`, an override replaces a
whole field: `config` is **not** deep-merged. This is not a complete profile
resolver: other bundles, user overlays and deployment configuration are absent.

Disabled groups propagate their state to descendants. Disabled MCP declarations
stay disabled. Disabled skills are omitted with an explicit receipt because the
native skill format has no disabled state; `--skill` is an explicit selection,
not an automatic recovery of an omitted skill. A conditional/non-boolean
`disabled` value on a group or portable row refuses the conversion rather than
assuming it is enabled. Unsupported entry policy/dependency fields, including
`inject`, `intercept` and `isolate`, also refuse conversion on those rows or their
groups. Preserve their activation and authority rules in a manual port.

Foreign runtime plugins and `dsh.client` UI code are not executed or translated.
Other unrepresentable components are reported in `CONVERSION.md` and structured
`CONVERSION.json`, with source package/version, manifest and ordered-layer SHA-256
hashes, converter version, per-row outcomes and required manual ports. Unapplied
patch operations also appear in the structured manual-port list, with their source
layer and one-based operation index; they are not treated as successful overlays.
Intentionally omitted disabled skills are reported separately, not as manual ports.
A partial output is an authoring draft, not an equivalent DSH runtime: review every
skipped component, patch operation and dependency before installing anything.

The only lowered `!!js` expressions are `process.execPath` (becomes `node`) and
simple quoted/template literals without escapes or interpolation. Environment
expressions—including fallbacks—are never resolved against this machine.
Unrepresentable fields are reported without copying their values. For packaged
Node MCP, a relative entry may resolve inside the selected package and declared
relative working directory. External source roots require an explicit
`--stdio-root SERVER=DIRECTORY` and a reviewed relative entry; host paths are
never inferred or copied from expressions. Rows of
`@deepseek-ai/dsh-skill-filesystem` contribute their literal `customSkillDirs`
children only when those directories live inside the package. Default user and
project skill roots, watchers and foreign service dependencies are not imported.

### Local Node MCP servers

For an already packaged Node MCP server, select its original process working
directory explicitly. The converter copies that directory into `mcp/<server>`
and sets the native server's working directory to the reviewed copy. Relative
entrypoint imports and read-only resources keep the same layout.

```json
{
  "mcp": {
    "localdocs": {
      "type": "local",
      "command": ["node", "server.mjs"],
      "environment": {"API_TOKEN": "{env:LOCALDOCS_TOKEN}"},
      "enabled": false
    }
  }
}
```

```sh
python3 scripts/convert-plugin.py --format opencode-v1 \
  --config ./local-mcp.json --stdio-root localdocs=./packaged-localdocs \
  --name local-tools --output ./migrated-local
```

Repeat `--stdio-root SERVER=DIRECTORY` for every local server in the selected
configuration. OpenCode v2 uses `mcp.servers` and `disabled`. Static DSH entries
use `transport: stdio`, `command: node`, and `args: [server.mjs]`; DSH `env`
must be absent or empty because its literals/expressions are not OpenCode
environment references. Optional DSH/v2 `cwd` must be absent, empty, or `.`;
the selected root explicitly supplies the original working directory.

Use `node` plus one relative `.mjs`, `.js`, or `.cjs` entry. Package module
type and sibling imports are preserved by the native launch adapter. Compile
TypeScript to JavaScript before packaging; the converter does not run a compiler.
Package dependencies and read-only resources first, inside the selected root.
No package manager, install script, module loader or server runs during
conversion. Links/reparse points, hard-linked files, hidden files/directories
(including `.gitignore`, `.env*`, `.npmrc` and `node_modules/.bin`), common
credential filenames, and private-key containers are refused. Prepare a clean
package directory; ignore rules are not used to silently omit files. Inspect
every selected file for embedded credentials before conversion. The existing
4,096-file / 64 MiB aggregate bundle limit applies.

The converter rejects shell launchers, Node flags, extra arguments, non-Node
interpreters, literal environment values, and loader-changing environment
names. Stateful servers that write into their working directory, depend on the
live workspace, or import files outside the package need a manual native port.
Copying files does not statically verify JavaScript import closure or sandbox
arbitrary code. Local MCP processes run with host-user authority; their network
and filesystem access are not restricted by the remote endpoint host list.
The same native install, capability review, hash-bound trust, and enable steps
are required before Codewhale launches the server. This adds a packaged Node
MCP subset; it does not execute DSH/Cordis plugin modules.

### Review the result

Both examples preserve disabled servers and use a placeholder endpoint. Replace
the endpoint and change the source's enablement flag before reconverting when
you are ready to connect. The output directory must be new, with an existing
parent. Existing output is refused; rejected input leaves no output bundle.

Add `--skill ./my-skill` for an explicitly selected directory containing
`SKILL.md`, or `--skill ./my-skill.md` for a single file; repeat the option for
more skills. `--config` is optional for a skills-only conversion. Skills require
`name` and `description` frontmatter. `disable-model-invocation: true` becomes
native `invocation: explicit-only`. Informational `license`, `compatibility`,
and `metadata` fields are retained in `SOURCE_SKILL_METADATA.json` companion
data. Companion files from selected skill directories are copied as data;
review them and the instructions before loading the skill.

Only exact OpenCode header references such as `{env:MCP_TOKEN}` become native
`env_headers`; the converter never reads the variable's value. Literal headers,
DSH header expressions, and URL file/environment substitution are refused.
Configured timeouts must be whole seconds expressed in milliseconds, from
`1000` through `3600000`. Omitted timeouts use Codewhale's defaults.

Executable foreign plugins and hooks, other stdio launchers, automatic OAuth,
configuration JavaScript,
YAML aliases/tags, `__jsExpr`, and unsupported skill runtime fields (including
`user-invocable: false`) require a manual port. Conversion does not reproduce
another client's runtime or bypass Codewhale's credential and sandbox rules.

Read the generated `CONVERSION.md`, `CONVERSION.json` for bundles, `plugin.json`, `mcp.json` when present, and
all selected skill and MCP source files. Then use `/plugin install ./migrated-opencode` (or the
DSH output path), `/plugin validate <name>`, and the same hash-bound trust and
enable flow above. Conversion alone proves neither connectivity nor runtime
compatibility; the output is not installed, trusted, or enabled.

Source audit, 2026-09-08: OpenCode's [v1 MCP documentation](https://github.com/anomalyco/opencode/blob/d6855b6b47a8433462ac6aeeba882ccf734cb7f1/packages/web/src/content/docs/mcp-servers.mdx)
and [v2 MCP schema](https://github.com/anomalyco/opencode/blob/d6855b6b47a8433462ac6aeeba882ccf734cb7f1/packages/core/src/config/mcp.ts)
at `d6855b6b47`, and DSH's [MCP client reference](https://github.com/deepseek-ai/deepseek-harness/blob/c389f96bf3a9b6807cb71ed6bdad5849be0df6d8/packages/mcp/mcp-client/README.md)
at `c389f96bf3`. Bundle patch semantics were rechecked against DSH
[`00102833df`](https://github.com/deepseek-ai/deepseek-harness/tree/00102833dfaee1da9f48a3a8eae9d34005a75218)
(`0.1.7-alpha.2`); `scripts/fixtures/dsh-web-app` retains its real five-file package
as parser-only test data, not as an importable native plugin. CI runs the offline
converter corpus with Python/PyYAML and synthetic Node fixtures. Upstream supports
more than this deliberately bounded converter.

## Community context

This guide responds to [giancarlocp's request for plugin authoring guidance
and OpenCode conversion in discussion #5827](https://github.com/Hmbown/Codewhale/discussions/5827).
The Chinese companion follows the documentation work requested by
[SparkofSpike in issue #5482](https://github.com/Hmbown/Codewhale/issues/5482).
