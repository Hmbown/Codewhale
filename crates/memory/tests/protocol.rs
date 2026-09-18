use codewhale_memory::{protocol::ToolServer, *};
use serde_json::{Value, json};
fn server(readonly: bool) -> ToolServer {
    let scope = Scope::user("local", "u").workspace("alpha");
    let access = if readonly {
        Access::readonly(vec![scope.clone()])
    } else {
        Access::operator(vec![scope.clone()])
    }
    .unwrap();
    ToolServer::new(
        Store::in_memory_with_clock(|| 1_750_000_000).unwrap(),
        access,
        scope,
        None,
        None,
    )
    .unwrap()
}
fn proposal() -> Value {
    json!({"request_id":"p1","kind":"fact","title":"cache","body":"Cache reuse is a design goal.","source_uri":"codewhale://session/1/message/1"})
}
#[test]
fn review_and_forget_are_never_exposed_to_the_model() {
    let s = server(false);
    let names: Vec<_> = s
        .tools()
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(
        !names
            .iter()
            .any(|s| s.contains("approve") || s.contains("forget") || s.contains("review"))
    );
}
#[test]
fn readonly_schema_removes_mutation_tools() {
    let mut s = server(true);
    assert!(!s.tools().to_string().contains("memory_propose"));
    assert!(matches!(
        s.invoke("memory_propose", proposal()),
        Err(Error::Denied)
    ));
}
#[test]
fn proposal_cannot_choose_its_authority_or_scope() {
    let mut s = server(false);
    let mut args = proposal();
    args["scope"] = json!({"tenant":"other"});
    assert!(s.invoke("memory_propose", args).is_err());
}
#[test]
fn proposed_memory_stays_out_of_recall() {
    let mut s = server(false);
    let result = s.invoke("memory_propose", proposal()).unwrap();
    assert_eq!(result["memory"]["status"], "candidate");
    let result = s.invoke("memory_search", json!({"query":"cache"})).unwrap();
    assert_eq!(result["hits"].as_array().unwrap().len(), 0);
}
#[test]
fn protocol_initializes_supported_compatibility_version() {
    let mut s = server(false);
    let r=s.handle(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}})).unwrap();
    assert_eq!(r["result"]["protocolVersion"], "2025-11-25");
}
#[test]
fn unsupported_new_version_is_not_silently_claimed() {
    let mut s = server(false);
    let r=s.handle(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2026-07-28"}})).unwrap();
    assert_eq!(r["result"]["protocolVersion"], "2025-11-25");
}
#[test]
fn mutation_notifications_are_ignored() {
    let mut s = server(false);
    let r=s.handle(json!({"jsonrpc":"2.0","method":"tools/call","params":{"name":"memory_propose","arguments":proposal()}}));
    assert!(r.is_none());
    let r = s.invoke("memory_propose", proposal()).unwrap();
    assert_eq!(r["created"], true);
}
#[test]
fn get_cannot_read_an_unavailable_id() {
    let mut s = server(false);
    assert!(matches!(
        s.invoke("memory_get", json!({"id":"other-project-memory"})),
        Err(Error::NotFound)
    ));
}
#[test]
fn oversized_frame_is_rejected_without_processing() {
    let mut s = server(false);
    let bytes = vec![b'x'; policy::MAX_FRAME_BYTES + 1];
    let mut out = Vec::new();
    assert!(s.serve(std::io::Cursor::new(bytes), &mut out).is_err());
    assert!(out.is_empty());
}
#[test]
fn invalid_json_does_not_crash_stream() {
    let mut s = server(false);
    let mut out = Vec::new();
    s.serve(
        std::io::Cursor::new(b"{broken\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\n"),
        &mut out,
    )
    .unwrap();
    let text = String::from_utf8(out).unwrap();
    let lines: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["error"]["code"], -32700);
    assert_eq!(lines[1]["id"], 2);
}
#[test]
fn denied_tool_call_has_a_tool_error_result() {
    let mut s = server(false);
    s.handle(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}));
    let r=s.handle(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"memory_forget","arguments":{"id":"x"}}})).unwrap();
    assert_eq!(r["result"]["isError"], true);
}
#[test]
fn request_id_must_not_be_null_or_fractional() {
    let mut s = server(false);
    for id in [Value::Null, json!(1.5)] {
        let r = s
            .handle(json!({"jsonrpc":"2.0","id":id,"method":"ping"}))
            .unwrap();
        assert_eq!(r["error"]["code"], -32600);
    }
}
#[test]
fn protocol_errors_do_not_echo_secret_payloads() {
    let mut s = server(false);
    s.handle(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
    let mut p = proposal();
    p["body"] = json!("API_KEY=abcdefghijklmnop123456");
    let r=s.handle(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"memory_propose","arguments":p}})).unwrap();
    assert!(!r.to_string().contains("abcdefghijklmnop123456"));
    assert_eq!(r["result"]["isError"], true);
}

#[test]
fn disabled_runtime_does_not_open_files_or_publish_tools() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory
        .path()
        .join("not-created")
        .join("memory-v2.sqlite3");
    let scope = Scope::user("local", "u");
    let result = codewhale_memory::runtime::open_tools(
        codewhale_memory::runtime::RuntimeOptions {
            enabled: false,
            database: database.clone(),
            workspace_root: None,
        },
        Access::agent(vec![scope.clone()]).unwrap(),
        scope,
        None,
    )
    .unwrap();
    assert!(result.is_none());
    assert!(!database.exists());
    assert!(!database.parent().unwrap().exists());
}
