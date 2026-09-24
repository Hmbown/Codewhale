//! Conformance corpus for native DSH bundle import. The cases mirror the
//! bundle-mode contracts of `scripts/test_convert_plugin.py` (converter
//! 0.10.1); the pinned upstream package is shared with that suite.

use super::*;
use serde_json::json;
use std::path::PathBuf;

const MCP: &str = "@deepseek-ai/dsh-mcp-client";
const SKILLS: &str = "@deepseek-ai/dsh-skill-filesystem";

struct Fixture {
    _root: tempfile::TempDir,
    bundle: PathBuf,
    output: PathBuf,
}

fn fixture(patch: Option<&str>, manifest: Option<Json>) -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let bundle = root.path().join("dsh-bundle");
    fs::create_dir(&bundle).unwrap();
    let mut package = json!({
        "name": "@demo/tools-dsh",
        "version": "1.2.3",
        "dsh": {"bundle": {"patch": "./cordis.patch.yml"}},
    });
    if let Some(Json::Object(extra)) = manifest {
        for (key, value) in extra {
            package[key] = value;
        }
    }
    fs::write(bundle.join("package.json"), package.to_string()).unwrap();
    if let Some(patch) = patch {
        fs::write(bundle.join("cordis.patch.yml"), patch).unwrap();
    }
    let output = root.path().join("out");
    Fixture {
        _root: root,
        bundle,
        output,
    }
}

fn docs_row(extra: &str) -> String {
    format!(
        "  - id: docs-entry\n    name: '{MCP}'\n{extra}    config:\n      serverName: docs\n      transport: streamable-http\n      url: https://docs.example.invalid/mcp\n"
    )
}

fn servers(output: &Path) -> Json {
    let text = fs::read_to_string(output.join("mcp.json")).unwrap();
    serde_json::from_str::<Json>(&text).unwrap()["mcpServers"].clone()
}

fn receipt(output: &Path) -> String {
    fs::read_to_string(output.join("CONVERSION.md")).unwrap()
}

fn refused(fixture: &Fixture, needle: &str) {
    let error = convert_package(&fixture.bundle, &fixture.output)
        .unwrap_err()
        .to_string();
    assert!(error.contains(needle), "expected `{needle}` in: {error}");
    assert!(
        !fixture.output.exists(),
        "a refusal leaves no partial output"
    );
}

#[test]
fn evaluates_patches_and_skips_foreign_rows() {
    let f = fixture(
        Some(&format!(
            "- insert:\n  - id: docs-entry\n    name: '{MCP}'\n    config:\n      serverName: docs\n      transport: streamable-http\n      url: https://docs.example.invalid/mcp\n      toolCallTimeoutMs: 19000\n  - id: skin\n    name: '@deepseek-ai/dsh-client-ui-theme'\n    config: {{hue: 4}}\n- id: docs-entry\n  disabled: true\n- id: ghost\n  disabled: true\n"
        )),
        None,
    );
    let conversion = convert_package(&f.bundle, &f.output).unwrap();
    assert_eq!(conversion.plugin_name, "tools-dsh");
    assert_eq!(
        servers(&f.output)["docs"],
        json!({"type": "streamable-http", "url": "https://docs.example.invalid/mcp",
               "extensions": {"net.codewhale": {"execute_timeout": 19, "disabled": true}}})
    );
    let text = receipt(&f.output);
    for fact in ["@demo/tools-dsh@1.2.3", "skin", "ghost"] {
        assert!(text.contains(fact), "{fact}: {text}");
    }
    let manifest: Json =
        serde_json::from_str(&fs::read_to_string(f.output.join("plugin.json")).unwrap()).unwrap();
    assert_eq!(manifest["name"], "tools-dsh");
    assert_eq!(manifest["version"], "1.2.3");
    assert_eq!(
        manifest["extensions"]["net.codewhale"]["capabilities"]["network_hosts"],
        json!(["docs.example.invalid"])
    );
}

#[test]
fn environment_expressions_are_never_evaluated_and_host_paths_never_copied() {
    let f = fixture(
        Some(&format!(
            "- insert:\n  - id: local-entry\n    name: '{MCP}'\n    config:\n      serverName: localdocs\n      transport: stdio\n      command: !!js process.execPath\n      args:\n        - !!js process.env.DSH_UNSET_7391 || '/tmp/server.mjs'\n  - id: env-url\n    name: '{MCP}'\n    config:\n      serverName: envdocs\n      transport: streamable-http\n      url: !!js '`https://example.invalid/${{process.env.DSH_CANARY}}/mcp`'\n{}",
            docs_row("")
        )),
        None,
    );
    convert_package(&f.bundle, &f.output).unwrap();
    let names: Vec<_> = servers(&f.output)
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(names, ["docs"]);
    assert!(!f.output.join("mcp").exists());
    let text = receipt(&f.output);
    assert!(
        text.contains("local-entry") && text.contains("reads an environment value"),
        "{text}"
    );
    assert!(text.contains("No environment variable values were resolved"));
}

#[test]
fn a_relative_entry_missing_from_the_package_is_skipped_not_searched_for() {
    let f = fixture(
        Some(&format!(
            "- insert:\n  - id: local-entry\n    name: '{MCP}'\n    config:\n      serverName: localdocs\n      transport: stdio\n      command: !!js process.execPath\n      args: ['./server.mjs']\n"
        )),
        None,
    );
    refused(&f, "No portable components");
}

#[test]
fn disabled_ancestry_disables_children_and_is_receipted() {
    let f = fixture(
        Some(&format!(
            "- insert:\n  - id: outer\n    group: true\n    disabled: true\n    config:\n      - id: inner\n        group: true\n        config:\n          - id: docs-entry\n            name: '{MCP}'\n            config: {{serverName: docs, transport: streamable-http, url: 'https://docs.example.invalid/mcp'}}\n      - id: skills-row\n        name: '{SKILLS}'\n        config: {{customSkillDirs: [pack-skills]}}\n  - id: live-entry\n    name: '{MCP}'\n    config: {{serverName: live, transport: streamable-http, url: 'https://live.example.invalid/mcp'}}\n"
        )),
        None,
    );
    let skill = f.bundle.join("pack-skills/guide");
    fs::create_dir_all(&skill).unwrap();
    fs::write(
        skill.join("SKILL.md"),
        "---\nname: guide\ndescription: Bundled skill\n---\nBody.\n",
    )
    .unwrap();
    convert_package(&f.bundle, &f.output).unwrap();
    let servers = servers(&f.output);
    assert_eq!(
        servers["docs"]["extensions"]["net.codewhale"],
        json!({"disabled": true})
    );
    assert_eq!(servers["live"]["extensions"]["net.codewhale"], json!({}));
    assert!(!f.output.join("skills").exists());
    let text = receipt(&f.output);
    assert!(text.contains("disabled by ancestor `outer`"), "{text}");
    assert!(text.contains("skipped-disabled"), "{text}");
}

#[test]
fn unresolved_conditional_gates_refuse_but_foreign_gates_are_skipped() {
    let gated_group = format!(
        "- insert:\n  - id: gate\n    name: '@deepseek-ai/cordis-plugin-group'\n    group: true\n    disabled: !!js process.env.DISABLE_THIS\n    config:\n    - id: docs-entry\n      name: '{MCP}'\n      config: {{serverName: docs, transport: streamable-http, url: 'https://docs.example.invalid/mcp'}}\n"
    );
    let gated_row = format!(
        "- insert:\n  - id: docs-entry\n    name: '{MCP}'\n    disabled: !!js process.env.DISABLE_THIS\n    config: {{serverName: docs, transport: streamable-http, url: 'https://docs.example.invalid/mcp'}}\n"
    );
    let string_gate = format!(
        "- insert:\n  - id: docs-entry\n    name: '{MCP}'\n    disabled: 'yes'\n    config: {{serverName: docs, transport: streamable-http, url: 'https://docs.example.invalid/mcp'}}\n"
    );
    for patch in [gated_group, gated_row, string_gate] {
        refused(&fixture(Some(&patch), None), "conditional or non-boolean");
    }
    let control = fixture(
        Some(&format!(
            "- insert:\n  - id: tool-bash\n    name: '@deepseek-ai/dsh-tool-bash'\n    disabled: !!js process.platform === 'win32'\n{}",
            docs_row("")
        )),
        None,
    );
    convert_package(&control.bundle, &control.output).unwrap();
    let text = receipt(&control.output);
    assert!(
        text.contains("tool-bash")
            && text.contains("conditional `disabled` gate was not evaluated"),
        "{text}"
    );
}

#[test]
fn ordered_layers_replace_config_and_record_provenance() {
    let f = fixture(
        Some(&format!(
            "- insert:\n  - id: docs-entry\n    name: '{MCP}'\n    config: {{serverName: docs, transport: streamable-http, url: 'https://first.example.invalid/mcp', toolCallTimeoutMs: 19000}}\n"
        )),
        Some(
            json!({"dsh": {"bundle": {"patch": ["./cordis.patch.yml", "./presets/overlay.patch.yml"]}}}),
        ),
    );
    fs::create_dir(f.bundle.join("presets")).unwrap();
    fs::write(
        f.bundle.join("presets/overlay.patch.yml"),
        "- id: docs-entry\n  disabled: true\n  config: {serverName: docs, transport: streamable-http, url: 'https://second.example.invalid/mcp'}\n",
    )
    .unwrap();
    convert_package(&f.bundle, &f.output).unwrap();
    assert_eq!(
        servers(&f.output)["docs"],
        json!({"type": "streamable-http", "url": "https://second.example.invalid/mcp",
               "extensions": {"net.codewhale": {"disabled": true}}})
    );
    let text = receipt(&f.output);
    assert!(text.contains("applied in declaration order"));
    assert!(text.contains(&format!("Converter version {CONVERTER_VERSION}")));
    assert!(text.contains("Source package: @demo/tools-dsh@1.2.3"));
    assert!(text.contains(&sha256_hex(
        &fs::read(f.bundle.join("package.json")).unwrap()
    )));
    for relative in ["./cordis.patch.yml", "./presets/overlay.patch.yml"] {
        let content = fs::read(f.bundle.join(relative.trim_start_matches("./"))).unwrap();
        assert!(
            text.contains(&format!(
                "{relative} (sha256 {}, {} bytes)",
                sha256_hex(&content),
                content.len()
            )),
            "{relative}: {text}"
        );
    }
    let structured: Json =
        serde_json::from_str(&fs::read_to_string(f.output.join("CONVERSION.json")).unwrap())
            .unwrap();
    assert_eq!(structured["schema"], "codewhale.plugin-conversion.v1");
}

#[test]
fn skipped_patch_operations_are_structured_manual_ports() {
    let f = fixture(
        Some(&format!("- insert:\n{}", docs_row(""))),
        Some(json!({"dsh": {"bundle": {"patch": ["./cordis.patch.yml", "./overlay.yml"]}}})),
    );
    fs::write(
        f.bundle.join("overlay.yml"),
        "- {id: missing-group, insert: []}\n- {disabled: true}\n- {id: missing-row, disabled: true}\n- {id: docs-entry, name: wrong-package, disabled: true}\n",
    )
    .unwrap();
    convert_package(&f.bundle, &f.output).unwrap();
    let structured: Json =
        serde_json::from_str(&fs::read_to_string(f.output.join("CONVERSION.json")).unwrap())
            .unwrap();
    let skipped = structured["required_manual_ports"].as_array().unwrap();
    assert_eq!(
        skipped
            .iter()
            .map(|row| row["row"].clone())
            .collect::<Vec<_>>(),
        [
            json!("missing-group"),
            Json::Null,
            json!("missing-row"),
            json!("docs-entry")
        ]
    );
    assert_eq!(
        skipped
            .iter()
            .map(|row| row["patch"].clone())
            .collect::<Vec<_>>(),
        [json!(1), json!(2), json!(3), json!(4)]
    );
    for row in skipped {
        assert_eq!(
            (row["kind"].as_str(), row["layer"].as_str()),
            (Some("patch"), Some("./overlay.yml"))
        );
        assert!(receipt(&f.output).contains(row["reason"].as_str().unwrap()));
    }
    assert!(
        servers(&f.output)["docs"]["extensions"]["net.codewhale"]
            .get("disabled")
            .is_none()
    );
}

#[test]
fn patch_layer_list_is_bounded_and_contained() {
    let portable = format!("- insert:\n{}", docs_row(""));
    let attempt = |declared: Json, files: &[(&str, String)]| {
        let f = fixture(None, Some(json!({"dsh": {"bundle": {"patch": declared}}})));
        for (name, text) in files {
            let destination = f.bundle.join(name);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::write(destination, text).unwrap();
        }
        f
    };
    let ok = attempt(json!(["./a.yml"]), &[("a.yml", portable.clone())]);
    convert_package(&ok.bundle, &ok.output).unwrap();
    refused(
        &attempt(
            json!(["./a.yml", "./a.yml"]),
            &[("a.yml", portable.clone())],
        ),
        "listed once",
    );
    refused(
        &attempt(json!(["a.yml", "./a.yml"]), &[("a.yml", portable.clone())]),
        "listed once",
    );
    refused(&attempt(json!(["/tmp/a.yml"]), &[]), "relative path");
    refused(&attempt(json!(["C:\\a.yml"]), &[]), "relative path");
    refused(
        &attempt(
            json!(["./a.yml", "../outside.yml"]),
            &[("a.yml", portable.clone())],
        ),
        "relative path",
    );
    refused(&attempt(json!(["./missing.yml"]), &[]), "inside the bundle");
    refused(&attempt(json!([]), &[]), "non-empty ordered list");
    let many: Vec<String> = (0..65).map(|i| format!("./layer-{i}.yml")).collect();
    refused(&attempt(json!(many), &[]), "At most 64");
    refused(
        &attempt(
            json!(["./a.yml", "./b.yml"]),
            &[
                ("a.yml", format!("[]\n{}", " ".repeat(1024 * 1024 - 3))),
                ("b.yml", "[]\n".into()),
            ],
        ),
        "aggregate patch limit",
    );
}

#[test]
fn pinned_upstream_multifile_package_parses_without_promoting_rows() {
    let bundle = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/fixtures/dsh-web-app");
    let upstream: Json =
        serde_json::from_str(&fs::read_to_string(bundle.join("UPSTREAM.json")).unwrap()).unwrap();
    assert_eq!(
        upstream["commit"],
        "00102833dfaee1da9f48a3a8eae9d34005a75218"
    );
    for (relative, digest) in upstream["files"].as_object().unwrap() {
        assert_eq!(
            &json!(sha256_hex(&fs::read(bundle.join(relative)).unwrap())),
            digest,
            "{relative}"
        );
    }
    let package = Package::open(&bundle).unwrap();
    let loaded = load_bundle(&package).unwrap();
    assert_eq!(loaded.layers.len(), 5);
    assert!(!loaded.entries.is_empty());
    let mut components = Components::default();
    components.walk(&package, &loaded.entries, None).unwrap();
    assert!(
        components.servers.is_empty()
            && components.hosts.is_empty()
            && components.skill_dirs.is_empty()
    );
    assert!(
        components
            .outcomes
            .iter()
            .all(|o| !o.outcome.starts_with("converted"))
    );
    assert!(
        components
            .notes
            .iter()
            .any(|note| note.contains("conditional"))
    );
}

#[test]
fn group_children_and_relative_stdio_entries_convert_from_the_declared_cwd() {
    let f = fixture(
        Some(&format!(
            "- insert:\n  - {{id: grouped, group: true, config: []}}\n- id: grouped\n  insert:\n  - id: in-group\n    name: '{MCP}'\n    config: {{serverName: inner, transport: stdio, command: node, args: [server.mjs], cwd: packaged}}\n"
        )),
        None,
    );
    fs::write(f.bundle.join("server.mjs"), "wrong source").unwrap();
    fs::create_dir(f.bundle.join("packaged")).unwrap();
    fs::write(f.bundle.join("packaged/server.mjs"), "correct source").unwrap();
    let conversion = convert_package(&f.bundle, &f.output).unwrap();
    assert!(conversion.requires_node);
    assert_eq!(conversion.local_servers, ["inner"]);
    assert_eq!(servers(&f.output)["inner"]["cwd"], "mcp/inner");
    assert_eq!(
        fs::read_to_string(f.output.join("mcp/inner/server.mjs")).unwrap(),
        "correct source"
    );
}

#[test]
fn custom_skill_dirs_import_and_disabled_skills_are_omitted() {
    let build = |disabled: &str| {
        let f = fixture(
            Some(&format!(
                "- insert:\n  - id: skills-row\n    name: '{SKILLS}'\n{disabled}    config: {{customSkillDirs: [pack-skills]}}\n{}",
                docs_row("")
            )),
            None,
        );
        let skill = f.bundle.join("pack-skills/guide");
        fs::create_dir_all(&skill).unwrap();
        fs::write(
            skill.join("SKILL.md"),
            "---\nname: guide\ndescription: Bundled skill\n---\nBody.\n",
        )
        .unwrap();
        fs::write(skill.join("notes.txt"), "companion").unwrap();
        f
    };
    let enabled = build("");
    let conversion = convert_package(&enabled.bundle, &enabled.output).unwrap();
    assert_eq!(conversion.skills, ["guide"]);
    let skill = fs::read_to_string(enabled.output.join("skills/guide/SKILL.md")).unwrap();
    assert!(
        skill.starts_with("---\nname: guide\ndescription: |-\n  Bundled skill\n---"),
        "{skill}"
    );
    assert_eq!(
        fs::read_to_string(enabled.output.join("skills/guide/notes.txt")).unwrap(),
        "companion"
    );
    let disabled = build("    disabled: true\n");
    assert!(
        convert_package(&disabled.bundle, &disabled.output)
            .unwrap()
            .skills
            .is_empty()
    );
    let text = receipt(&disabled.output);
    assert!(
        text.contains("skipped-disabled") && text.contains("preserved by omission"),
        "{text}"
    );
}

#[test]
fn js_is_never_executed_and_unlowerable_rows_are_recorded() {
    let sentinel = tempfile::tempdir().unwrap().path().join("expression-ran");
    let f = fixture(
        Some(&format!(
            "- insert:\n  - id: bad\n    name: '{MCP}'\n    config:\n      serverName: bad\n      transport: streamable-http\n      url: !!js require('node:fs').writeFileSync('{}', 'ran')\n  - id: ok\n    name: '{MCP}'\n    config: {{serverName: ok, transport: streamable-http, url: 'https://ok.example.invalid/mcp'}}\n",
            sentinel.display()
        )),
        None,
    );
    convert_package(&f.bundle, &f.output).unwrap();
    assert!(!sentinel.exists());
    let names: Vec<_> = servers(&f.output)
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(names, ["ok"]);
    assert!(receipt(&f.output).contains("bad"));
}

#[test]
fn manifests_must_declare_contained_patches() {
    refused(&fixture(None, None), "inside the bundle");
    refused(
        &fixture(
            None,
            Some(json!({"dsh": {"bundle": {"patch": "../outside.yml"}}})),
        ),
        "relative path",
    );
    let plain = fixture(None, None);
    fs::write(
        plain.bundle.join("package.json"),
        json!({"name": "plain"}).to_string(),
    )
    .unwrap();
    refused(&plain, "dsh.bundle.patch");
    assert!(!is_dsh_package(&plain.bundle));
    assert!(is_dsh_package(&fixture(None, None).bundle));
}

#[test]
fn policy_and_dependency_fields_never_widen_activation() {
    for field in [
        "inject: [approvals]",
        "intercept: {tools: true}",
        "isolate: {tools: private}",
        "unknownGate: true",
    ] {
        for disabled in ["false", "true"] {
            let rows = [
                format!(
                    "  - {{id: docs-entry, name: '{MCP}', {field}, disabled: {disabled}, config: {{serverName: docs, transport: streamable-http, url: 'https://docs.example.invalid/mcp'}}}}\n"
                ),
                format!(
                    "  - {{id: skills, name: '{SKILLS}', {field}, disabled: {disabled}, config: {{customSkillDirs: [skills]}}}}\n"
                ),
                format!(
                    "  - {{id: group, group: true, {field}, disabled: {disabled}, config: []}}\n"
                ),
            ];
            for row in rows {
                refused(
                    &fixture(Some(&format!("- insert:\n{row}")), None),
                    "unsupported entry policy or dependency",
                );
            }
        }
    }
}

#[test]
fn skipped_stdio_rows_never_copy_their_source() {
    let f = fixture(
        Some(&format!(
            "- insert:\n{}  - id: unportable\n    name: '{MCP}'\n    config: {{serverName: local, transport: stdio, command: node, args: [server.mjs], cwd: packaged, env: {{TOKEN: canary-9f2}}}}\n",
            docs_row("")
        )),
        None,
    );
    fs::create_dir(f.bundle.join("packaged")).unwrap();
    fs::write(
        f.bundle.join("packaged/server.mjs"),
        "throw new Error('never run');\n",
    )
    .unwrap();
    convert_package(&f.bundle, &f.output).unwrap();
    assert!(!f.output.join("mcp").exists());
    let structured = fs::read_to_string(f.output.join("CONVERSION.json")).unwrap();
    let parsed: Json = serde_json::from_str(&structured).unwrap();
    assert_eq!(parsed["required_manual_ports"][0]["row"], "unportable");
    assert!(!structured.contains("canary-9f2"));
}

#[test]
fn a_duplicate_server_cannot_discard_a_disabled_row() {
    let f = fixture(
        Some(&format!(
            "- insert:\n{}{}",
            docs_row(""),
            docs_row("    disabled: true\n").replace("id: docs-entry", "id: disabled-copy")
        )),
        None,
    );
    refused(&f, "Duplicate MCP server");
}

#[test]
fn literal_lowering_never_evaluates_or_changes_escaped_strings() {
    for expression in [
        "'https://example.invalid/mcp'",
        "`https://example.invalid/mcp`",
    ] {
        assert_eq!(
            lower_js(expression, "url").unwrap(),
            "https://example.invalid/mcp"
        );
    }
    assert_eq!(lower_js("process.execPath", "command").unwrap(), "node");
    for expression in [
        r"'https://example.invalid/\\x41'",
        "`x` + `y`",
        "process.env.TOKEN",
        "process.env.TOKEN || 'literal'",
        "`${process.env.TOKEN}`",
        "process.env['TOKEN']",
    ] {
        assert!(lower_js(expression, "url").is_err(), "{expression}");
    }
}

#[test]
fn data_parsing_is_closed() {
    assert!(
        parse_json(r#"{"a": 1, "a": 2}"#).is_err(),
        "duplicate JSON keys"
    );
    assert!(
        parse_yaml("a: 1\na: 2\n", false).is_err(),
        "duplicate YAML keys"
    );
    assert!(parse_yaml("a: &x 1\nb: *x\n", false).is_err(), "aliases");
    assert!(parse_yaml("a: !!str 1\n", false).is_err(), "explicit tags");
    assert!(
        parse_yaml("a: !!js process.execPath\n", false).is_err(),
        "js without permission"
    );
    assert_eq!(
        parse_yaml("a: !!js process.execPath\n", true)
            .unwrap()
            .get("a"),
        Some(&Value::Js("process.execPath".into()))
    );
    assert!(parse_yaml("- !!js x\n", true).unwrap() == Value::Seq(vec![Value::Js("x".into())]));
    assert!(
        parse_yaml("!!js [a]\n", true).is_err(),
        "tagged collections"
    );
    assert!(
        parse_yaml("a: 1\n---\nb: 2\n", false).is_err(),
        "multiple documents"
    );
    assert!(parse_yaml("{__jsExpr: x}\n", false).is_err());
    assert!(
        parse_yaml(&format!("{}1{}", "[".repeat(33), "]".repeat(33)), false).is_err(),
        "depth"
    );
    assert_eq!(
        parse_yaml("{a: yes, b: true, c: 0x10, d: 1.5, e: ~, f: '1'}\n", false).unwrap(),
        Value::Map(vec![
            ("a".into(), Value::Str("yes".into())),
            ("b".into(), Value::Bool(true)),
            ("c".into(), Value::Int(16)),
            ("d".into(), Value::Float(1.5)),
            ("e".into(), Value::Null),
            ("f".into(), Value::Str("1".into())),
        ])
    );
}

#[test]
fn endpoints_are_literal_https_or_loopback_with_canonical_hosts() {
    for (url, host) in [
        ("https://docs.example.invalid/mcp", "docs.example.invalid"),
        (
            "https://Docs.Example.Invalid:8443/mcp",
            "docs.example.invalid",
        ),
        ("http://127.0.0.1:9000/mcp", "127.0.0.1"),
        ("http://[::1]:9000/mcp", "[::1]"),
        ("https://10.0.0.1/mcp", "10.0.0.1"),
    ] {
        assert_eq!(endpoint_host(url).unwrap(), host, "{url}");
    }
    for url in [
        "http://docs.example.invalid/mcp",
        "https://user:secret@docs.example.invalid/mcp",
        "https://docs.example.invalid/mcp?token=x",
        "https://docs.example.invalid/mcp#x",
        "https://127.1/mcp",
        "https://0x7f.0.0.1/mcp",
        "https://docs.example.invalid:0/mcp",
        "https://docs.example.invalid/${x}",
        "ftp://docs.example.invalid/mcp",
    ] {
        assert!(endpoint_host(url).is_err(), "{url}");
    }
}

#[test]
fn links_inside_the_package_are_refused() {
    #[cfg(unix)]
    {
        let f = fixture(Some(&format!("- insert:\n{}", docs_row(""))), None);
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("layer.yml"), "[]\n").unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("layer.yml"),
            f.bundle.join("linked.yml"),
        )
        .unwrap();
        fs::write(
            f.bundle.join("package.json"),
            json!({"name": "@demo/tools-dsh", "dsh": {"bundle": {"patch": ["./linked.yml"]}}})
                .to_string(),
        )
        .unwrap();
        refused(&f, "links or reparse points");
    }
}

#[test]
fn the_converted_bundle_is_a_valid_native_plugin() {
    let f = fixture(Some(&format!("- insert:\n{}", docs_row(""))), None);
    convert_package(&f.bundle, &f.output).unwrap();
    let manifest =
        crate::plugins::agent_plugin::resolve_manifest_path(&f.output).expect("manifest");
    let validated =
        crate::plugins::manifest::PluginManifest::validate_from_path(&manifest).unwrap();
    assert_eq!(validated.manifest.plugin.name, "tools-dsh");
}

#[test]
fn derived_names_are_native_plugin_names() {
    assert_eq!(
        derived_plugin_name("@deepseek-ai/bundle-web-app").as_deref(),
        Some("bundle-web-app")
    );
    assert_eq!(
        derived_plugin_name("Tools_DSH").as_deref(),
        Some("tools-dsh")
    );
    assert_eq!(derived_plugin_name("@x/--"), None);
}
