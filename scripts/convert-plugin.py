#!/usr/bin/env python3
"""Convert selected OpenCode/DSH data — or a dsh bundle package — into a reviewable native plugin bundle.

This is an offline authoring tool, not a foreign plugin runtime or installer.
The existing Codewhale /plugin install and hash-bound review remain authoritative.
Requires Python 3.10+ and PyYAML 6+; never loads upstream code or configuration.
"""

import argparse
import hashlib
import ipaddress
import json
import os
from pathlib import Path
import re
import stat
import sys
from urllib.parse import urlsplit

import yaml


MAX_FILES = 4096
MAX_BYTES = 64 * 1024 * 1024
MAX_DOCUMENT = 1024 * 1024
# One dsh bundle may declare several ordered patch layers; they are applied over
# one empty profile exactly like the single-file form and share one byte budget.
MAX_PATCH_FILES = 64
# Recorded in every receipt so a reviewed conversion can be attributed to a converter revision.
CONVERTER_VERSION = "0.10.1"
NAME = re.compile(r"[a-z0-9](?:[a-z0-9.-]{0,62}[a-z0-9])?\Z")


class ConversionError(ValueError):
    pass


class JsExpr(str):
    """An unevaluated dsh `!!js` scalar captured for structural lowering; it is never executed."""


def require(condition, message):
    if not condition:
        raise ConversionError(message)


def mapping(value, allowed=None):
    require(isinstance(value, dict), "Expected a configuration object.")
    require(all(isinstance(k, str) for k in value), "Object keys must be strings.")
    if allowed is not None:
        require(not value.keys() - set(allowed), "Unsupported fields; select only documented portable declarations.")
    return value


def unique_pairs(pairs):
    result = {}
    for key, value in pairs:
        require(isinstance(key, str) and key not in result, "Duplicate or non-string object key.")
        result[key] = value
    return result


class DataLoader(yaml.SafeLoader):
    def construct_mapping(self, node, deep=False):
        return unique_pairs((self.construct_object(k, deep=deep), self.construct_object(v, deep=deep))
                            for k, v in node.value)


def js_scalar(loader, node):
    return JsExpr(loader.construct_scalar(node))


DataLoader.add_constructor("tag:yaml.org,2002:js", js_scalar)


def data(text, *, json_only=False, allow_js=False):
    """Closed data parsing: no YAML aliases/tags, duplicate keys or JS expressions."""
    try:
        if json_only:
            value = json.loads(text, object_pairs_hook=unique_pairs,
                               parse_constant=lambda _: (_ for _ in ()).throw(ConversionError("Non-finite JSON number.")))
        else:
            depth = 0
            for event in yaml.parse(text):
                tag = getattr(event, "tag", None)
                if isinstance(event, yaml.AliasEvent):
                    raise ConversionError("YAML aliases are unsupported.")
                if tag is not None and not (allow_js and isinstance(event, yaml.ScalarEvent)
                                            and tag == "tag:yaml.org,2002:js"):
                    raise ConversionError("YAML aliases and explicit tags (including !!js) are unsupported.")
                if isinstance(event, (yaml.MappingStartEvent, yaml.SequenceStartEvent)):
                    depth += 1
                    require(depth <= 32, "Configuration nesting exceeds 32 levels.")
                elif isinstance(event, (yaml.MappingEndEvent, yaml.SequenceEndEvent)):
                    depth -= 1
            value = yaml.load(text, Loader=DataLoader)
        check_data(value, allow_js=allow_js)
        return value
    except (yaml.YAMLError, json.JSONDecodeError, RecursionError, TypeError):
        # Parser errors can contain source lines and credentials. Do not echo them.
        raise ConversionError("Cannot parse portable data; use JSON for OpenCode or plain YAML/JSON for DSH.") from None


def check_data(value, depth=0, allow_js=False):
    require(depth <= 32, "Configuration nesting exceeds 32 levels.")
    if isinstance(value, dict):
        mapping(value)
        require("__jsExpr" not in value, "DSH executable expressions require a manual port.")
        for child in value.values():
            check_data(child, depth + 1, allow_js)
    elif isinstance(value, list):
        for child in value:
            check_data(child, depth + 1, allow_js)
    else:
        require(value is None or type(value) in (str, int, float, bool)
                or (allow_js and isinstance(value, JsExpr)), "Unsupported data type.")


def plain_path(path):
    """Reject links/reparse points in the supplied path, including ancestors."""
    path = Path(os.path.abspath(path))
    for entry in (path, *path.parents):
        try:
            info = entry.lstat()
        except FileNotFoundError:
            continue
        require(not stat.S_ISLNK(info.st_mode)
                and not (getattr(info, "st_file_attributes", 0) & 0x400),
                "Source and output paths must not contain links or reparse points.")
    return path


def read_file(path, limit=MAX_DOCUMENT):
    path = plain_path(path)
    info = path.stat()
    require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1, "Only regular, non-linked source files are supported.")
    require(info.st_size <= limit, "Source file exceeds the conversion size limit.")
    # O_NOFOLLOW protects the final component against replacement after lstat.
    fd = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    with os.fdopen(fd, "rb") as source:
        opened = os.fstat(source.fileno())
        require((info.st_dev, info.st_ino) == (opened.st_dev, opened.st_ino), "Source changed during conversion.")
        content = source.read(limit + 1)
    require(len(content) <= limit, "Source file exceeds the conversion size limit.")
    return content


def text_file(path):
    return decode_config(read_file(path))


def decode_config(content):
    try:
        return content.decode("utf-8")
    except UnicodeError:
        raise ConversionError("Configuration and skill entrypoints must be UTF-8.") from None


def skill_files(path, max_files=MAX_FILES, max_bytes=MAX_BYTES):
    source = plain_path(path)
    entry = source / "SKILL.md" if source.is_dir() else source
    require(entry.name == "SKILL.md" or entry.suffix == ".md", "Select a skill directory or Markdown skill file.")
    text = text_file(entry)
    parts = re.split(r"^---\s*$", text, maxsplit=2, flags=re.MULTILINE)
    require(len(parts) == 3 and not parts[0].strip(), "Skills need YAML frontmatter with name and description.")
    meta = mapping(data(parts[1]), {"name", "description", "license", "compatibility", "metadata",
                                  "disable-model-invocation", "user-invocable"})
    name = meta.get("name")
    require(isinstance(name, str) and re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", name)
            and len(name) <= 64, "Skill name must be a kebab-case identifier of at most 64 characters.")
    description = meta.get("description")
    require(isinstance(description, str) and description.strip(), "Skills need a non-empty description.")
    require("---" not in description, "Skill description contains a delimiter the native reader cannot preserve.")
    require(type(meta.get("user-invocable", True)) is bool and meta.get("user-invocable", True),
            "user-invocable:false has no equivalent in the native skill adapter; port it manually.")
    explicit = meta.get("disable-model-invocation", False)
    require(type(explicit) is bool, "disable-model-invocation must be a boolean.")
    # Native parsing is deliberately simpler than YAML. Emit unambiguous core fields.
    front = f"---\nname: {name}\ndescription: |-\n"
    front += "\n".join("  " + line for line in description.splitlines()) + "\n"
    if explicit:
        front += "invocation: explicit-only\n"
    generated = (front + "---" + parts[2]).encode("utf-8")
    files = {f"skills/{name}/SKILL.md": generated}
    extra = {key: meta[key] for key in ("license", "compatibility", "metadata") if key in meta}
    if extra:
        # Native frontmatter is a flat parser: nested metadata must not override
        # its name/description/invocation. Preserve attribution as companion data.
        files[f"skills/{name}/SOURCE_SKILL_METADATA.json"] = (json.dumps(extra, ensure_ascii=False, indent=2) + "\n").encode()
    used_bytes = sum(map(len, files.values()))
    require(len(files) <= max_files and used_bytes <= max_bytes, "Selected skills exceed the bundle budget.")
    if source.is_dir():
        visited = 0
        for current, dirs, children in os.walk(source, followlinks=False):
            for child in dirs + children:
                visited += 1
                require(visited <= MAX_FILES, "Selected skill contains too many filesystem entries.")
                candidate = plain_path(Path(current) / child)
                require(candidate.name not in {".git", ".env", ".installed-from"},
                        "Remove local secrets, repository metadata or install receipts from the selected skill.")
                if candidate.is_dir() or candidate == entry:
                    continue
                relative = f"skills/{name}/{candidate.relative_to(source).as_posix()}"
                require(relative not in files, "Skill companion collides with generated metadata.")
                require(len(files) < max_files, "Selected skills exceed the file budget.")
                files[relative] = read_file(candidate, max_bytes - used_bytes)
                used_bytes += len(files[relative])
    return name, files


def timeout(value):
    require(type(value) is int and 1000 <= value <= 3600000 and value % 1000 == 0,
            "Timeouts must be whole seconds expressed in milliseconds (1000–3600000); port other values manually.")
    return value // 1000


def server_options(config, dialect, defaults):
    extension = {}
    if dialect == "dsh":
        require(config.get("failOnStartupError", False) is False, "DSH startup-failure policy requires a manual port.")
        if "toolCallTimeoutMs" in config:
            extension["execute_timeout"] = timeout(config["toolCallTimeoutMs"])
    else:
        disabled = not config.get("enabled", True) if dialect == "opencode-v1" else config.get("disabled", False)
        flag = config.get("enabled", True) if dialect == "opencode-v1" else config.get("disabled", False)
        require(type(flag) is bool, "MCP enablement must be a boolean.")
        extension["disabled"] = disabled
        if dialect == "opencode-v1":
            if "timeout" in config:
                extension["connect_timeout"] = timeout(config["timeout"])
                extension["execute_timeout"] = timeout(config["timeout"])
        else:
            limits = {**mapping(defaults, {"startup", "request"}),
                      **mapping(config.get("timeout", {}), {"startup", "request"})}
            for old, new in (("startup", "connect_timeout"), ("request", "execute_timeout")):
                if old in limits:
                    extension[new] = timeout(limits[old])
    return extension


def remote_server(config, dialect, defaults):
    mapping(config)
    if dialect == "dsh":
        require(config.get("transport") == "streamable-http", "Unsupported MCP transport.")
        mapping(config, {"serverName", "transport", "url", "headers", "toolCallTimeoutMs", "failOnStartupError"})
    else:
        require(config.get("type") == "remote", "Unsupported MCP transport.")
        mapping(config, {"type", "url", "headers", "oauth", "enabled" if dialect == "opencode-v1" else "disabled", "timeout"})
        require(config.get("oauth") is False, "Set oauth:false explicitly; plugin OAuth and upstream auto-OAuth cannot be converted.")
    extension = server_options(config, dialect, defaults)
    url = config.get("url")
    require(isinstance(url, str) and not re.search(r"[\s\\{}]", url), "MCP URL must be a literal endpoint without interpolation.")
    try:
        parsed = urlsplit(url)
        host = parsed.hostname
        require(bool(host) and parsed.port != 0, "MCP URL needs a valid host and port.")
    except ValueError:
        raise ConversionError("Invalid MCP endpoint URL.") from None
    require(parsed.scheme == "https" or (parsed.scheme == "http" and host in {"localhost", "127.0.0.1", "::1"}),
            "MCP endpoints need HTTPS (or explicit loopback HTTP).")
    require(parsed.username is None and parsed.password is None and not parsed.query and not parsed.fragment,
            "MCP URLs must not contain credentials, query strings or fragments.")
    require(host.isascii() and len(url) <= 4096, "Use an ASCII MCP hostname and URL of at most 4096 characters.")
    # URL implementations disagree on shorthand/hex IPv4 spellings. Emit only
    # canonical numeric hosts so the reviewed native host set is identical.
    if ":" in host or re.fullmatch(r"(?:[0-9]+|0x[0-9a-f]+)", host.rsplit(".", 1)[-1]):
        try:
            address = ipaddress.ip_address(host)
            require(str(address) == host, "Use a canonical numeric MCP address.")
            host = f"[{host}]" if address.version == 6 else host
        except ValueError:
            raise ConversionError("Use a canonical numeric MCP address.") from None
    headers = mapping(config.get("headers", {}))
    require(len(headers) <= 64, "At most 64 environment-backed headers are supported.")
    env_headers = {}
    seen_headers = set()
    for key, value in headers.items():
        require(re.fullmatch(r"[A-Za-z0-9!#$%&'*+.^_`|~-]+", key) is not None, "Invalid HTTP header name.")
        require(key.lower() not in seen_headers and key.lower() not in {"accept", "content-type"},
                "HTTP header names must be unique ignoring case; Accept and Content-Type belong to the native transport.")
        seen_headers.add(key.lower())
        require(isinstance(value, str), "HTTP headers must reference environment variable names.")
        reference = re.fullmatch(r"\{env:([A-Za-z_][A-Za-z0-9_]*)\}", value) if dialect != "dsh" else None
        require(reference is not None, "Literal headers, DSH expressions and file interpolation cannot be converted; author native env_headers manually.")
        env_headers[key] = reference[1]
    if env_headers:
        extension["env_headers"] = env_headers
    return {"type": "streamable-http", "url": url,
            "extensions": {"net.codewhale": extension}}, host


def stdio_server(config, dialect, defaults, name, root):
    require(root is not None, "Local MCP needs an explicit --stdio-root SERVER=DIRECTORY containing its packaged Node source.")
    if dialect == "dsh":
        mapping(config, {"serverName", "transport", "command", "args", "env", "cwd", "toolCallTimeoutMs", "failOnStartupError"})
        command, arguments = config.get("command"), config.get("args", [])
        require(not mapping(config.get("env", {})), "DSH stdio env values and expressions require a manual native port.")
        environment = {}
    else:
        allowed = {"type", "command", "environment", "timeout", "enabled" if dialect == "opencode-v1" else "disabled"}
        if dialect == "opencode-v2":
            allowed.add("cwd")
        mapping(config, allowed)
        argv = config.get("command")
        require(isinstance(argv, list) and len(argv) == 2, "Local MCP command must be exactly [\"node\", \"relative-entry.js\"].")
        command, arguments = argv[0], argv[1:]
        environment = mapping(config.get("environment", {}))
    require(command == "node" and isinstance(arguments, list) and len(arguments) == 1,
            "Only node with one packaged .mjs, .js or .cjs entry is supported; no launcher flags, package managers or shell commands.")
    entry = arguments[0]
    require(isinstance(entry, str) and re.fullmatch(r"(?:\./)?[A-Za-z0-9_][A-Za-z0-9_./-]*\.(?:mjs|js|cjs)", entry)
            and ".." not in entry.split("/"), "Node entry must be a contained relative .mjs, .js or .cjs file; compile other entry formats before packaging.")
    require(config.get("cwd", "") in ("", "."), "Select the original process working directory with --stdio-root; other cwd values require a manual port.")
    require(plain_path(root / entry).is_file(), "Packaged Node entry does not exist.")
    require(len(environment) <= 64, "At most 64 environment mappings are supported.")
    env = {}
    for key, value in environment.items():
        require(re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", key) is not None
                and key.upper() not in {"PLUGIN_ROOT", "PLUGIN_DATA", "NODE_OPTIONS", "NODE_PATH", "PATH",
                                        "LD_PRELOAD", "LD_LIBRARY_PATH", "DYLD_INSERT_LIBRARIES", "DYLD_LIBRARY_PATH", "DYLD_FRAMEWORK_PATH"},
                "Invalid, reserved or loader-changing stdio environment name.")
        reference = re.fullmatch(r"\{env:([A-Za-z_][A-Za-z0-9_]*)\}", value) if isinstance(value, str) else None
        require(reference is not None, "Local MCP environment values must be exact OpenCode {env:NAME} references; literals are not copied.")
        env[key] = "${" + reference[1] + "}"
    return {"type": "stdio", "command": "node", "args": [entry], "cwd": f"mcp/{name}",
            "env": env, "extensions": {"net.codewhale": server_options(config, dialect, defaults)}}


def stdio_files(root, name, max_files, max_bytes):
    """Copy an explicitly packaged directory, never resolve/install dependencies."""
    def fail_walk(error):
        raise error

    files, visited, used_bytes = {}, 0, 0
    for current, dirs, children in os.walk(root, followlinks=False, onerror=fail_walk):
        for child in dirs + children:
            visited += 1
            require(visited <= MAX_FILES, "Packaged MCP source has too many filesystem entries.")
            candidate = plain_path(Path(current) / child)
            lower = candidate.name.lower()
            require(not lower.startswith(".") and lower not in {"credentials", "credentials.json", "secrets.json", "id_rsa", "id_ed25519"}
                    and candidate.suffix.lower() not in {".pem", ".key", ".p12", ".pfx"},
                    "Package MCP source without hidden files, repository metadata or credential files; nothing was copied.")
            if candidate.is_dir():
                continue
            require(len(files) < max_files, "Selected components exceed the file budget.")
            content = read_file(candidate, max_bytes - used_bytes)
            files[f"mcp/{name}/{candidate.relative_to(root).as_posix()}"] = content
            used_bytes += len(content)
    return files


def mcp_config(path, dialect, stdio_roots=None):
    stdio_roots = stdio_roots or {}
    document = data(text_file(path), json_only=dialect != "dsh")
    ignored = 0
    if dialect == "dsh":
        require(isinstance(document, list), "DSH input must be a plain Cordis entry list, not a profile or patch composition.")
        entries = []
        for row in document:
            mapping(row, {"name", "id", "config", "disabled"})
            require(row.get("name") == "@deepseek-ai/dsh-mcp-client", "DSH runtime plugins and patch operations require a manual port.")
            require(type(row.get("disabled", False)) is bool, "DSH disabled must be a boolean.")
            config = mapping(row.get("config"))
            entries.append((config.get("serverName"), config, row.get("disabled", False)))
        defaults = {}
    else:
        mapping(document)
        require(not document.get("plugin") and not document.get("plugins"), "OpenCode executable plugins require a manual port; select portable data only.")
        ignored = len(document.keys() - {"$schema", "mcp", "plugin", "plugins"})
        servers = mapping(document.get("mcp", {}))
        defaults = {}
        if dialect == "opencode-v2":
            mapping(servers, {"servers", "timeout"})
            defaults = servers.get("timeout", {})
            servers = mapping(servers.get("servers", {}))
        # These application fields govern MCP tool access, including legacy
        # per-agent overrides. A server-level enabled flag cannot preserve them.
        if servers:
            require(not document.keys() & {"tools", "permission", "permissions", "agent", "agents", "mode", "default_agent"},
                    "OpenCode tool permissions and agent policies require a manual port; preserve their restrictions in Codewhale before supplying MCP-only input.")
        entries = [(key, value, False) for key, value in servers.items()]
    require(len(entries) <= 64, "At most 64 MCP servers can be converted at once.")
    result, hosts = {}, set()
    local_names = set()
    for name, config, disabled in entries:
        require(isinstance(name, str) and re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]{0,31}", name), "Invalid or missing MCP server name.")
        require(name not in result, "Duplicate MCP server name; no entries were written.")
        mapping(config)
        local = config.get("transport") == "stdio" if dialect == "dsh" else config.get("type") == "local"
        if local:
            converted = stdio_server(config, dialect, defaults, name, stdio_roots.get(name))
            local_names.add(name)
        else:
            converted, host = remote_server(config, dialect, defaults)
            hosts.add(host)
        if disabled:
            converted["extensions"]["net.codewhale"]["disabled"] = True
        result[name] = converted
    require(set(stdio_roots) == local_names, "Every --stdio-root must name a selected local MCP server.")
    return result, sorted(hosts), ignored


DSH_MCP_CLIENT = "@deepseek-ai/dsh-mcp-client"
DSH_SKILL_FILESYSTEM = "@deepseek-ai/dsh-skill-filesystem"
DSH_ENTRY = re.compile(r"(?:\./)?[A-Za-z0-9_][A-Za-z0-9_./-]*\.(?:mjs|js|cjs)")
def js_literal(text, label):
    """Accept a quoted string or a template literal with no interpolation.

    Conversion never reads this machine's environment or any other ambient state, so a
    literal that would resolve from it is refused for a manual port instead of being
    serialized into generated bundle data."""
    text = text.strip()
    if len(text) >= 2 and text[0] in "\"'" and text[-1] == text[0] and text[0] not in text[1:-1]:
        require("\\" not in text[1:-1] and "\n" not in text[1:-1],
                f"{label} contains JavaScript escapes; author the literal explicitly.")
        return text[1:-1]
    if text.startswith("`") and text.endswith("`") and len(text) >= 2:
        body = text[1:-1]
        require("${" not in body and "\\" not in body and "`" not in body,
                f"{label} interpolates a value that conversion never evaluates; author the literal explicitly.")
        return body
    raise ConversionError(f"{label} is not a quoted or template literal; author the value explicitly.")


def lower_js(value, label):
    """Lower only the `!!js` idioms that need no ambient state; every other expression refuses."""
    text = str(value).strip()
    if text == "process.execPath":
        return "node"
    if "process.env" in text:
        raise ConversionError(
            f"{label} reads an environment value, and conversion never evaluates this machine's environment; "
            "author the literal explicitly, or select a packaged directory with --stdio-root")
    if text.startswith("`") or text[:1] in "\"'":
        return js_literal(text, label)
    raise ConversionError(f"{label} uses a `!!js` expression with no portable lowering; author the value explicitly.")


def free_of_js(value):
    """True when no unevaluated `!!js` scalar survives inside the value."""
    if isinstance(value, JsExpr):
        return False
    if isinstance(value, dict):
        return all(free_of_js(key) and free_of_js(child) for key, child in value.items())
    if isinstance(value, list):
        return all(free_of_js(child) for child in value)
    return True


def evaluate_patches(patches, notes, locations):
    """Apply a dsh bundle patch list over an empty entry list (applyEntryPatches parity):
    `insert` appends rows or appends into a group entry's config, keyed overrides
    replace fields on an earlier inserted row. Skipped patches need a manual port;
    retain their layer and operation index in both human and structured receipts."""
    entries, index, outcomes = [], {}, []
    def skipped(order, patch, reason):
        notes.append(f"patch {order + 1}: {reason}; skipped")
        identifier, package = patch.get("id"), patch.get("name")
        outcomes.append({"row": identifier if isinstance(identifier, str) else None,
                         "package": package if isinstance(package, str) else None,
                         "kind": "patch", "outcome": "skipped", "reason": reason,
                         **locations[order]})
    def build_map(rows):
        for row in rows:
            if not isinstance(row, dict):
                continue
            identifier = row.get("id")
            if isinstance(identifier, str):
                index[identifier] = row
            config = row.get("config")
            if row.get("group") is True and isinstance(config, list):
                build_map(config)
    for order, patch in enumerate(patches):
        require(isinstance(patch, dict), "Each dsh patch must be an object.")
        insert, identifier = patch.get("insert"), patch.get("id")
        if insert is not None:
            require(isinstance(insert, list) and all(isinstance(row, dict) for row in insert),
                    "A dsh patch `insert` must be a list of entries.")
            if identifier is None:
                entries.extend(insert)
            else:
                target = index.get(identifier)
                if target is None or target.get("group") is not True:
                    skipped(order, patch, f"insert target `{identifier}` is missing or not a group")
                    continue
                if not isinstance(target.get("config"), list):
                    target["config"] = []
                target["config"].extend(insert)
            build_map(insert)
            continue
        if not isinstance(identifier, str):
            skipped(order, patch, "non-insert patch without an `id`")
            continue
        target = index.get(identifier)
        if target is None:
            skipped(order, patch, f"entry `{identifier}` was not inserted by an earlier layer")
            continue
        name = patch.get("name")
        if name is not None and name != target.get("name"):
            skipped(order, patch, f"`name` does not match entry `{identifier}`")
            continue
        for key, value in patch.items():
            if key not in ("id", "name"):
                target[key] = value
    return entries, outcomes


def load_dsh_bundle(path):
    """Read a dsh bundle package: package.json → `dsh.bundle.patch` (one path or an ordered
    list of paths) → rows evaluated over one empty profile.

    Returns (manifest, entries, notes, layers, manifest_hash, patch_outcomes). Every layer is contained in the
    package, read once, and bounded in aggregate before any output directory is created."""
    bundle = plain_path(path)
    require(bundle.is_dir(), "Select a dsh bundle package directory (a directory containing package.json).")
    manifest_content = read_file(bundle / "package.json")
    manifest = data(decode_config(manifest_content), json_only=True)
    mapping(manifest)
    dsh = manifest.get("dsh")
    require(isinstance(dsh, dict) and isinstance(dsh.get("bundle"), dict),
            "Not a dsh bundle package: package.json lacks `dsh.bundle.patch`.")
    notes = []
    if dsh.get("client") is not None:
        notes.append("package declares `dsh.client`; the client UI half has no Codewhale equivalent and was not converted")
    declared = dsh["bundle"].get("patch")
    if isinstance(declared, str):
        declared = [declared]
    require(isinstance(declared, list) and bool(declared),
            "`dsh.bundle.patch` must name a patch file or a non-empty ordered list of patch files.")
    require(all(isinstance(item, str) and bool(item) for item in declared),
            "Every `dsh.bundle.patch` entry must be a non-empty relative path.")
    require(len(declared) <= MAX_PATCH_FILES, "At most 64 `dsh.bundle.patch` files are supported.")
    layers, patches, locations, total = [], [], [], 0
    seen_paths = set()
    for relative in declared:
        require(not Path(relative).is_absolute() and ".." not in Path(relative).parts
                and not re.search(r"[\\\\:\x00-\x1f]", relative),
                "Every `dsh.bundle.patch` entry must be a relative path inside the bundle directory.")
        patch_path = plain_path(bundle / relative)
        require(patch_path.is_relative_to(bundle) and patch_path.is_file(),
                "Every `dsh.bundle.patch` entry must resolve to a file inside the bundle directory.")
        require(patch_path not in seen_paths,
                "Each `dsh.bundle.patch` file may be listed once; a duplicate would apply its layer twice.")
        seen_paths.add(patch_path)
        require(total < MAX_DOCUMENT, "Selected patch files exceed the 1 MiB aggregate patch limit.")
        content = read_file(patch_path, MAX_DOCUMENT - total)
        total += len(content)
        layer = data(decode_config(content), allow_js=True)
        require(isinstance(layer, list), "A dsh bundle patch must be a patch list.")
        layers.append({"path": relative, "sha256": hashlib.sha256(content).hexdigest(), "bytes": len(content)})
        patches.extend(layer)
        locations.extend({"layer": relative, "patch": order + 1} for order in range(len(layer)))
    entries, patch_outcomes = evaluate_patches(patches, notes, locations)
    return manifest, entries, notes, layers, hashlib.sha256(manifest_content).hexdigest(), patch_outcomes


def dsh_bundle_components(entries, bundle, explicit_roots):
    """Convert evaluated dsh entries into Codewhale servers + skill sources.
    Unconvertible rows are recorded as skipped outcomes, never silently dropped."""
    servers, hosts, notes, outcomes = {}, [], [], []
    implicit_roots = {}
    skill_dirs = []

    def label(row):
        identifier = row.get("id")
        return f"`{identifier}`" if isinstance(identifier, str) else "an unlabeled row"

    def record(row, kind, result, reason=""):
        """One structured per-row outcome, also rendered into the diagnostics list."""
        identifier = row.get("id") if isinstance(row, dict) else None
        package = row.get("name") if isinstance(row, dict) else None
        outcomes.append({"row": identifier if isinstance(identifier, str) else None,
                         "package": package if isinstance(package, str) else None,
                         "kind": kind, "outcome": result, "reason": reason})
        notes.append(f"{label(row)} [{kind}] {result}" + (f": {reason}" if reason else ""))

    def mcp_row(row, disabled, disabled_reason):
        config = dict(mapping(row.get("config")))
        name = config.get("serverName")
        require(isinstance(name, str) and re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]{0,31}", name),
                "dsh-mcp-client config needs a literal `serverName` of 1–32 letters/digits/_/-")
        require(name not in servers, f"Duplicate MCP server name `{name}`; nothing was written for it.")
        for field in ("command", "cwd", "url", "serverName"):
            if isinstance(config.get(field), JsExpr):
                config[field] = lower_js(config[field], f"`{field}` in `{name}`")
        if isinstance(config.get("args"), list):
            config["args"] = [lower_js(item, f"`args` in `{name}`") if isinstance(item, JsExpr) else item
                              for item in config["args"]]
        arguments = config.get("args") or []
        root = explicit_roots.get(name)
        if config.get("transport") == "stdio" and root is None and isinstance(arguments, list) and len(arguments) == 1:
            arg = arguments[0]
            if isinstance(arg, str) and not DSH_ENTRY.fullmatch(arg):
                # A host path is never copied implicitly: the operator selects a source root.
                raise ConversionError(
                    f"`args` in `{name}` names a host path that is outside the selected package; conversion never "
                    f"copies an ambient path. Select that directory explicitly with --stdio-root {name}=DIRECTORY")
            if isinstance(arg, str) and ".." not in arg.split("/"):
                candidate = bundle / arg
                cwd = config.get("cwd", "")
                if cwd in ("", ".") and candidate.is_file():
                    root = bundle
                elif (isinstance(cwd, str) and cwd not in ("", ".") and ".." not in Path(cwd).parts
                      and not re.search(r"[\\\\:\x00-\x1f]", cwd)
                      and not Path(cwd).is_absolute() and (bundle / cwd / arg).is_file()):
                    root = plain_path(bundle / cwd)
                    config["cwd"] = "."
        require(free_of_js(config), f"an unevaluated `!!js` remains in `{name}`; author that field explicitly")
        local = config.get("transport") == "stdio"
        if local:
            converted = stdio_server(config, "dsh", {}, name, root)
            # Commit source copying only after every field of this server validated.
            if name not in explicit_roots:
                implicit_roots[name] = root
                notes.append(f"`{name}`: stdio source root resolved inside the selected package at "
                             f"`{root.relative_to(bundle).as_posix() or '.'}`")
        else:
            converted, host = remote_server(config, "dsh", {})
            hosts.append(host)
        if disabled:
            converted["extensions"]["net.codewhale"]["disabled"] = True
        servers[name] = converted
        record(row, "mcp", "converted-disabled" if disabled else "converted", disabled_reason)

    def walk(rows, disabled_ancestor=None):
        for row in rows:
            require(isinstance(row, dict), "Each dsh group child must be an entry object.")
            config = row.get("config")
            disabled = row.get("disabled", False)
            name = row.get("name")
            group = row.get("group") is True
            if group or name in (DSH_MCP_CLIENT, DSH_SKILL_FILESYSTEM):
                # Dependency, interception and isolation affect activation/authority, including
                # inherited group context. Unknown fields must not turn a gated row into a tool.
                require(not row.keys() - {"id", "name", "config", "group", "disabled"},
                        f"{label(row)} has unsupported entry policy or dependency fields; port its "
                        "activation and authority semantics manually before conversion.")
                require(type(row.get("group", False)) is bool,
                        f"{label(row)} has a non-boolean group flag; resolve it explicitly.")
            if group:
                require(isinstance(config, list), f"{label(row)} needs a list of group entries.")
            if type(disabled) is not bool:
                # dsh's loader propagates `disabled` to descendants, so an unresolved gate on a
                # group or on a convertible row is authority-relevant: refusing is the only
                # behaviour that cannot enable content its author disables at runtime.
                if group or name in (DSH_MCP_CLIENT, DSH_SKILL_FILESYSTEM):
                    raise ConversionError(
                        f"{label(row)} gates activation with a conditional or non-boolean `disabled` value that "
                        "cannot be resolved offline; conversion refuses to assume the row is enabled. Pre-resolve "
                        "the gate in a reviewed copy of the patch layer, then convert that copy.")
                # A foreign row contributes nothing either way, so record it and keep going.
                record(row, "foreign", "skipped",
                       "no portable representation; its conditional `disabled` gate was not evaluated")
                continue
            if group:
                walk(config, disabled_ancestor or (label(row) if disabled else None))
                continue
            if disabled:
                disabled_reason = "the source row sets `disabled: true`"
            elif disabled_ancestor:
                disabled_reason = f"the row is disabled by ancestor {disabled_ancestor}"
            else:
                disabled_reason = ""
            effective = disabled or bool(disabled_ancestor)
            if name == DSH_MCP_CLIENT:
                if isinstance(config, dict):
                    server_name = config.get("serverName")
                    require(not isinstance(server_name, str) or server_name not in servers,
                            "Duplicate MCP server name; no entries were written.")
                try:
                    mcp_row(row, effective, disabled_reason)
                except ConversionError as error:
                    record(row, "mcp", "skipped", str(error))
            elif name == DSH_SKILL_FILESYSTEM:
                if effective:
                    record(row, "skill", "skipped-disabled",
                           f"{disabled_reason}; native skills have no disabled state, so the intent is preserved "
                           "by omission — pass --skill explicitly to import it")
                    continue
                dirs = config.get("customSkillDirs") if isinstance(config, dict) else None
                if not isinstance(dirs, list) or not dirs:
                    record(row, "skill", "skipped", "skill row has no `customSkillDirs` to import")
                    continue
                imported = 0
                for entry in dirs:
                    if isinstance(entry, JsExpr) or not isinstance(entry, str):
                        notes.append(f"{label(row)}: a `customSkillDirs` entry is not a literal path; skipped")
                        continue
                    candidate = Path(entry)
                    if candidate.is_absolute() or ".." in candidate.parts:
                        notes.append(f"{label(row)}: `customSkillDirs` entry `{entry}` is outside the bundle; "
                                     "pass it explicitly with --skill")
                        continue
                    resolved = plain_path(bundle / entry)
                    if not resolved.is_dir():
                        notes.append(f"{label(row)}: `customSkillDirs` entry `{entry}` does not exist in the bundle; skipped")
                        continue
                    skill_dirs.append(resolved)
                    imported += 1
                record(row, "skill", "converted" if imported else "skipped",
                       f"imported {imported} `customSkillDirs` entries inside the package" if imported
                       else "no `customSkillDirs` entry inside the package could be imported")
            else:
                shown = name if isinstance(name, str) else "unlabeled"
                record(row, "foreign", "skipped",
                       f"only dsh-mcp-client and dsh-skill-filesystem rows convert; `{shown}` has no portable "
                       "representation")

    walk(entries)
    require(len(servers) <= 64, "At most 64 MCP servers can be converted at once.")
    skills = []
    for directory in skill_dirs:
        for child in sorted(directory.iterdir()):
            if child.name.startswith("."):
                continue
            if (child / "SKILL.md").is_file() or child.suffix == ".md":
                skills.append(child)
    return servers, hosts, skills, implicit_roots, notes, outcomes


def convert(args):
    require(NAME.fullmatch(args.name) is not None and ".." not in args.name and "--" not in args.name,
            "Choose a native plugin name: 1–64 lowercase letters/digits with single internal dots or hyphens.")
    bundle_arg = getattr(args, "bundle", None)
    require(bundle_arg is None or args.format == "dsh", "--bundle reads a DeepSeek Harness bundle package; use --format dsh.")
    require(not (bundle_arg is not None and args.config), "Select --bundle or --config, not both.")
    output = Path(os.path.abspath(args.output))
    plain_path(output.parent)
    require(not os.path.lexists(output), "Output already exists; choose a fresh directory. Nothing was overwritten.")
    files, skill_names, notes = {}, set(), []
    layers, manifest_hash, outcomes = [], None, []
    skill_sources = [plain_path(path) for path in args.skill]
    servers, hosts, ignored = {}, [], 0
    roots = {}
    for specification in getattr(args, "stdio_root", []):
        name, separator, directory = specification.partition("=")
        require(separator and directory and name not in roots, "Use unique --stdio-root SERVER=DIRECTORY selections.")
        source = plain_path(directory)
        require(source.is_dir(), "The selected stdio root must be a directory.")
        require(source != output and source not in output.parents, "Output must be outside the selected MCP source.")
        roots[name] = source
    bundle_manifest = None
    if bundle_arg is not None:
        bundle_manifest, entries, bundle_notes, layers, manifest_hash, patch_outcomes = load_dsh_bundle(bundle_arg)
        notes += bundle_notes
        bundle = plain_path(bundle_arg)
        require(bundle != output and bundle not in output.parents, "Output must be outside the selected bundle.")
        servers, hosts, bundled_skills, implicit_roots, row_notes, outcomes = dsh_bundle_components(entries, bundle, roots)
        outcomes = patch_outcomes + outcomes
        notes += row_notes
        skill_sources += bundled_skills
        implicit_roots.update(roots)
        explicit_names = set(roots)
        roots = implicit_roots
        local_names = {name for name, server in servers.items() if server["type"] == "stdio"}
        require(explicit_names <= local_names, "Every --stdio-root must name a selected local MCP server.")
    else:
        require(not roots or args.config, "--stdio-root requires a selected MCP configuration.")
        servers, hosts, ignored = mcp_config(args.config, args.format, roots) if args.config else ({}, [], 0)
    for source in skill_sources:
        require(source != output and source not in output.parents, "Output must be outside the selected skill.")
        name, additions = skill_files(source, MAX_FILES - len(files), MAX_BYTES - sum(map(len, files.values())))
        require(name not in skill_names, "Duplicate skill name; no files were written.")
        skill_names.add(name)
        files.update(additions)
    for name, source in roots.items():
        files.update(stdio_files(source, name, MAX_FILES - len(files), MAX_BYTES - sum(map(len, files.values()))))
    require(files or servers, "No portable components selected. Use --skill, --config or --bundle.")
    manifest = {"$schema": "https://agent-plugins.org/schemas/plugin.json", "name": args.name}
    if bundle_manifest is not None:
        for field in ("version", "description"):
            value = bundle_manifest.get(field)
            if isinstance(value, str) and value.strip():
                manifest[field] = value
        notes.insert(0, f"source package: {bundle_manifest.get('name', 'unnamed')}"
                        + (f"@{bundle_manifest['version']}" if isinstance(bundle_manifest.get('version'), str) else ""))
    extension = {}
    if hosts:
        extension["capabilities"] = {"network_hosts": sorted(set(hosts))}
    if roots:
        extension["when"] = {"binaries": ["node"]}
    if extension:
        manifest["extensions"] = {"net.codewhale": extension}
    files["plugin.json"] = (json.dumps(manifest, indent=2) + "\n").encode()
    if servers:
        files["mcp.json"] = (json.dumps({"mcpServers": servers}, indent=2) + "\n").encode()
    provenance = [f"Converter version {CONVERTER_VERSION}. Source dialect: {args.format}."]
    if bundle_manifest is not None:
        source_version = bundle_manifest.get("version")
        provenance.append("Source package: " + str(bundle_manifest.get("name", "unnamed"))
                          + (f"@{source_version}" if isinstance(source_version, str) and source_version.strip() else ""))
        provenance.append(f"Source manifest sha256: {manifest_hash}")
        provenance.append("Selected patch layers, applied in declaration order:")
        provenance += [f"- {layer['path']} (sha256 {layer['sha256']}, {layer['bytes']} bytes)" for layer in layers]
    outcome_lines = ["## Component outcomes", "",
                     "Skipped rows and patch operations need a manual port; skipped-disabled skills are intentionally omitted.", ""] + [
        f"- {outcome['row'] or 'unlabeled'} ({outcome['package'] or 'unlabeled'}) {outcome['kind']}: {outcome['outcome']}"
        + (f" — {outcome['reason']}" if outcome["reason"] else "") for outcome in outcomes]
    lines = ["# Conversion receipt", "", *provenance, "",
             f"Converted {len(skill_names)} selected Skills, {len(servers) - len(roots)} remote and "
             f"{len(roots)} local MCP declarations.",
             f"Ignored {ignored} unrelated top-level application settings.", ""]
    if outcomes:
        lines += [*outcome_lines, ""]
    if notes:
        lines += ["Bundle diagnostics:", *[f"- {note}" for note in notes], ""]
    lines += [
        "No source code, package manager, install hook, network request or credential lookup ran.",
        "No environment variable values were resolved. Only the selected input roots were read;",
        "host paths are never inferred from expressions or copied without an explicit source selection.",
        "Companion skill files were copied as data; review them before loading a skill.",
        "Selected Node source, dependencies and resources were copied as data into mcp/<server>.",
        "Each --stdio-root is the original process working directory; the staged copy becomes its cwd.",
        "Review all copied files and environment names. Node runs only through the native trusted MCP lifecycle.",
        "Local MCP runs with host-user process authority, not an OS sandbox; stdio does not confine its network or files.",
        "Mutable workspace state, writable package resources and external module dependencies require a manual port.",
        "This output is not installed, trusted or enabled. Run `/plugin install <directory>`,",
        "then `/plugin validate <name>` and review the exact trust token before enabling.",
        "Remote MCP output uses Streamable HTTP only; OpenCode's legacy SSE fallback is not reproduced.",
        "Conversion does not prove server connectivity or foreign runtime compatibility."]
    files["CONVERSION.md"] = ("\n".join(lines) + "\n").encode()
    if bundle_manifest is not None:
        receipt = {
            "schema": "codewhale.plugin-conversion.v1", "converter_version": CONVERTER_VERSION,
            "source": {"dialect": args.format, "package": bundle_manifest.get("name"),
                       "version": bundle_manifest.get("version"), "manifest_sha256": manifest_hash,
                       "patch_layers": layers},
            "counts": {"skills": len(skill_names), "mcp_servers": len(servers)},
            "outcomes": outcomes, "diagnostics": notes,
            "required_manual_ports": [outcome for outcome in outcomes if outcome["outcome"] == "skipped"],
            "installed": False, "trusted": False, "enabled": False,
        }
        files["CONVERSION.json"] = (json.dumps(receipt, indent=2) + "\n").encode()
    require(len(files) <= MAX_FILES and sum(map(len, files.values())) <= MAX_BYTES,
            "Output exceeds the 4096-file / 64 MiB bundle budget.")
    # Complete all parsing before exclusive creation. Never install or alter a source tree.
    output.mkdir(mode=0o700)
    written = []
    try:
        for relative, content in files.items():
            destination = output / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            with destination.open("xb") as handle:
                written.append(destination)
                handle.write(content)
    except OSError:
        # Remove only files this operation created, never a pre-existing tree.
        for destination in reversed(written):
            destination.unlink(missing_ok=True)
        for current, dirs, _ in os.walk(output, topdown=False):
            for directory in dirs:
                (Path(current) / directory).rmdir()
        output.rmdir()
        raise
    return len(skill_names), len(servers), ignored


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--format", choices=("opencode-v1", "opencode-v2", "dsh"), required=True)
    parser.add_argument("--config", type=Path, help="OpenCode JSON or static DSH Cordis YAML/JSON (optional)")
    parser.add_argument("--bundle", type=Path,
                        help="dsh bundle package directory (package.json with dsh.bundle.patch, one path or an "
                             "ordered list); evaluates the patch layers in order")
    parser.add_argument("--skill", type=Path, action="append", default=[], help="Explicit skill directory or Markdown file; repeatable")
    parser.add_argument("--stdio-root", action="append", default=[], metavar="SERVER=DIRECTORY",
                        help="Explicit packaged Node MCP working directory; repeat for each selected local server")
    parser.add_argument("--name", required=True, help="Name for the new native bundle")
    parser.add_argument("--output", type=Path, required=True, help="Fresh directory whose parent already exists")
    args = parser.parse_args()
    try:
        skills, servers, ignored = convert(args)
    except (ConversionError, OSError, UnicodeError) as error:
        message = str(error) if isinstance(error, ConversionError) else "File operation failed; source and output must be accessible regular paths."
        print(f"Conversion refused: {message}", file=sys.stderr)
        return 1
    print(f"Prepared {skills} Skills and {servers} MCP declarations; {ignored} unrelated settings omitted.")
    print("Review CONVERSION.md, then use the existing /plugin install, validate, trust and enable commands.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
