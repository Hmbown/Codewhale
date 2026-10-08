//! MCP server implementation for exposing Codewhale tools over stdio.

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

use crate::session_manager::SessionManager;
use crate::tools::spec::{ToolError, ToolResult};
use crate::tools::{ToolContext, ToolRegistryBuilder};

#[derive(Debug, Default, Deserialize)]
struct McpServerConfigFile {
    #[serde(default)]
    server: McpServerSection,
}

#[derive(Debug, Default, Deserialize)]
struct McpServerSection {
    expose_tools: Option<Vec<String>>,
    /// Withhold tools that write files or run commands. This stdio server has
    /// no channel for an out-of-band approval prompt, so "approval required"
    /// means those tools are refused. Defaults to `true`; only the operator's
    /// own config file can turn it off.
    require_approval: Option<bool>,
}

#[derive(Debug, Clone)]
struct McpServerSettings {
    expose_tools: Vec<String>,
    require_approval: bool,
}

impl McpServerSettings {
    fn load() -> Result<Self> {
        let path = default_config_path();
        if let Some(path) = path.filter(|p| p.exists()) {
            let contents = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read MCP server config: {}", path.display()))?;
            Self::from_toml(&contents)
                .with_context(|| format!("Failed to parse MCP server config: {}", path.display()))
        } else {
            Ok(Self {
                expose_tools: default_expose_tools(),
                require_approval: true,
            })
        }
    }

    fn from_toml(contents: &str) -> Result<Self> {
        let config: McpServerConfigFile = toml::from_str(contents)?;
        Ok(Self {
            expose_tools: config
                .server
                .expose_tools
                .unwrap_or_else(default_expose_tools),
            require_approval: config.server.require_approval.unwrap_or(true),
        })
    }
}

#[derive(Debug, Clone)]
struct ExposedTool {
    public: String,
    internal: String,
}

pub async fn run_mcp_server(workspace: PathBuf) -> Result<()> {
    // Settings load is a synchronous config read; keep it off the async
    // worker per the blocking-call convention.
    let settings = tokio::task::spawn_blocking(McpServerSettings::load)
        .await
        .context("MCP server settings task failed")??;
    let mut server = McpServer::new(workspace, settings)?;
    // stdout carries the protocol; the notice goes to stderr so an operator
    // whose config lists write tools sees why they are missing.
    for name in server.withheld_tools() {
        eprintln!(
            "codewhale mcp server: not exposing '{name}': it writes files or runs \
             commands and require_approval is on. Set require_approval = false under \
             [server] in the MCP server config to allow it."
        );
    }
    server.run().await
}

struct McpServer {
    workspace: PathBuf,
    registry: crate::tools::ToolRegistry,
    exposed_tools: Vec<ExposedTool>,
    require_approval: bool,
    phase: SessionPhase,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SessionPhase {
    Uninitialized,
    InitializeResponded,
    Ready,
}

impl McpServer {
    fn new(workspace: PathBuf, settings: McpServerSettings) -> Result<Self> {
        let exposed_tools = build_exposed_tools(&settings.expose_tools);
        let mut internal_names: HashSet<String> = HashSet::new();
        for tool in &exposed_tools {
            internal_names.insert(tool.internal.clone());
        }

        let mut builder = ToolRegistryBuilder::new()
            .with_file_tools()
            .with_search_tools();

        if internal_names.contains("apply_patch") {
            builder = builder.with_patch_tools();
        }
        if internal_names.contains("exec_shell") {
            builder = builder.with_shell_tools();
        }

        let context = ToolContext::new(workspace.clone());
        let registry = builder.build(context);

        Ok(Self {
            workspace,
            registry,
            exposed_tools,
            require_approval: settings.require_approval,
            phase: SessionPhase::Uninitialized,
        })
    }

    /// The serialized stdio loop runs on the caller's runtime: a JSON-RPC
    /// stdio server answers one request at a time by definition, so it
    /// needs no private `Runtime` and no `block_on` (#6140).
    async fn run(&mut self) -> Result<()> {
        self.run_io(tokio::io::stdin(), tokio::io::stdout()).await
    }

    async fn run_io<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
        &mut self,
        input: R,
        mut output: W,
    ) -> Result<()> {
        let mut reader = tokio::io::BufReader::new(input);
        let mut frame = Vec::new();
        loop {
            let response = match crate::mcp::read_line_capped(
                &mut reader,
                &mut frame,
                crate::mcp::MAX_MCP_RESPONSE_BYTES,
            )
            .await
            {
                Ok(0) => break,
                Ok(_) if frame.iter().all(u8::is_ascii_whitespace) => {
                    frame.clear();
                    continue;
                }
                Ok(_) => match serde_json::from_slice(&frame) {
                    Ok(message) => self.handle_message(message).await,
                    Err(_) => respond_error(Some(&Value::Null), -32700, "Invalid JSON".into()),
                },
                Err(err) => {
                    let response = respond_error(
                        Some(&Value::Null),
                        -32700,
                        "Invalid or oversized JSON-RPC frame".into(),
                    );
                    if let Some(response) = response {
                        output.write_all(response.to_string().as_bytes()).await?;
                        output.write_all(b"\n").await?;
                        output.flush().await?;
                    }
                    return Err(err).context("Failed to read bounded MCP input");
                }
            };
            frame.clear();
            if let Some(response) = response {
                let payload = serde_json::to_vec(&response)?;
                output.write_all(&payload).await?;
                output.write_all(b"\n").await?;
                output.flush().await?;
            }
        }
        Ok(())
    }

    async fn handle_message(&mut self, message: Value) -> Option<Value> {
        let id = message.get("id");
        let method = message.get("method").and_then(Value::as_str);
        if !message.is_object()
            || message.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
            || method.is_none_or(str::is_empty)
            || id.is_some_and(|id| !(id.is_null() || id.is_string() || id.is_i64() || id.is_u64()))
            || message
                .get("params")
                .is_some_and(|params| !params.is_object())
        {
            return respond_error(
                Some(&Value::Null),
                -32600,
                "Invalid JSON-RPC request".into(),
            );
        }
        let method = method.unwrap_or_default();
        if matches!(method, "tools/list" | "tools/call" | "resources/list")
            && self.phase != SessionPhase::Ready
        {
            return respond_error(
                id,
                -32600,
                "A completed initialize / notifications/initialized handshake is required".into(),
            );
        }

        match method {
            "initialize" => {
                // A notification has no response carrying the negotiated version
                // and cannot advance the handshake.
                id?;
                if self.phase != SessionPhase::Uninitialized {
                    return respond_error(id, -32600, "Initialize may only be sent once".into());
                }
                let requested = message
                    .pointer("/params/protocolVersion")
                    .and_then(Value::as_str);
                if requested.is_none_or(|version| version.trim().is_empty())
                    || ["name", "version"].iter().any(|field| {
                        message["params"]["clientInfo"][*field]
                            .as_str()
                            .is_none_or(|value| value.trim().is_empty())
                    })
                    || !message["params"]["capabilities"].is_object()
                {
                    return respond_error(id, -32602, "Invalid MCP initialize parameters".into());
                }
                self.phase = SessionPhase::InitializeResponded;
                respond(id, initialize_response(requested))
            }
            "notifications/initialized" => {
                if id.is_none() && self.phase == SessionPhase::InitializeResponded {
                    self.phase = SessionPhase::Ready;
                    None
                } else {
                    respond_error(
                        id,
                        -32600,
                        "Expected initialized notification after initialize".into(),
                    )
                }
            }
            "tools/list" => respond(id, self.list_tools_response()),
            "tools/call" => {
                // Calls without an identity are notifications and must not run
                // a tool whose result the client cannot acknowledge.
                id?;
                let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
                match self.call_tool(params).await {
                    Ok(result) => respond(id, result),
                    Err(err) => respond_error(id, err.code, err.message),
                }
            }
            "resources/list" => respond(id, self.list_resources_response().await),
            "ping" => respond(id, json!({})),
            _ => respond_error(id, -32601, format!("Method not found: {method}")),
        }
    }

    fn list_tools_response(&self) -> Value {
        let mut tools = Vec::new();
        let mut seen = HashSet::new();
        for entry in &self.exposed_tools {
            if !seen.insert(entry.public.clone()) {
                continue;
            }
            if let Some(tool) = self.registry.get(&entry.internal) {
                // A tool this server would refuse is not advertised.
                if self.require_approval && !tool.is_read_only() {
                    continue;
                }
                tools.push(json!({
                    "name": entry.public,
                    "description": tool.description(),
                    "inputSchema": tool.input_schema(),
                }));
            }
        }
        // MCP spec: `nextCursor` must be omitted (or be a string) when there
        // are no more results. Emitting `null` violates the spec and breaks
        // strict clients (e.g. Claude Code) that validate the response shape.
        json!({ "tools": tools })
    }

    /// Configured tools that `require_approval` keeps out of `tools/list`.
    fn withheld_tools(&self) -> Vec<String> {
        if !self.require_approval {
            return Vec::new();
        }
        let mut seen = HashSet::new();
        self.exposed_tools
            .iter()
            .filter(|entry| seen.insert(entry.public.clone()))
            .filter(|entry| {
                self.registry
                    .get(&entry.internal)
                    .is_some_and(|tool| !tool.is_read_only())
            })
            .map(|entry| entry.public.clone())
            .collect()
    }

    async fn list_resources_response(&self) -> Value {
        let mut resources = Vec::new();
        resources.push(json!({
            "uri": format!("file://{}", self.workspace.display()),
            "name": "workspace",
            "description": "Workspace root",
            "mimeType": "inode/directory",
        }));

        // `SessionManager` does synchronous filesystem work; the listing is
        // a borrow-free unit so it can run on the blocking pool.
        let sessions = tokio::task::spawn_blocking(|| {
            SessionManager::default_location().and_then(|manager| manager.list_sessions())
        })
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default();
        for session in sessions {
            resources.push(json!({
                "uri": format!("codewhale://session/{}", session.id),
                "name": session.title,
                "description": format!("{} messages", session.message_count),
                "mimeType": "application/json",
            }));
        }

        // Same spec point as `list_tools_response`: omit `nextCursor` when
        // there are no further pages rather than emitting `null`.
        json!({ "resources": resources })
    }

    async fn call_tool(&mut self, params: Value) -> Result<Value, RpcError> {
        let params = params.as_object().ok_or_else(|| RpcError {
            code: -32602,
            message: "Invalid params for tools/call".to_string(),
        })?;
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| RpcError {
                code: -32602,
                message: "Missing tool name".to_string(),
            })?;

        let internal = self
            .exposed_tools
            .iter()
            .find(|tool| tool.public == name)
            .map(|tool| tool.internal.clone())
            .ok_or_else(|| RpcError {
                code: -32602,
                message: format!("Tool not exposed: {name}"),
            })?;

        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if !arguments.is_object() {
            return Err(RpcError {
                code: -32602,
                message: "Tool arguments must be an object".into(),
            });
        }
        // Approval comes from the operator's config, never from the caller:
        // a request cannot vouch for itself.
        if self.require_approval
            && self
                .registry
                .get(&internal)
                .is_some_and(|tool| !(tool.is_read_only() && tool.is_read_only_for(&arguments)))
        {
            return Err(RpcError {
                code: -32001,
                message: format!(
                    "Tool '{name}' writes files or runs commands and needs approval, which \
                     this server cannot request. Set require_approval = false in the MCP \
                     server config to allow it."
                ),
            });
        }
        let result = self.registry.execute_full(&internal, arguments).await;
        Ok(tool_result_to_mcp(result))
    }
}

fn default_config_path() -> Option<PathBuf> {
    crate::config::effective_home_dir().map(|home| home.join(".deepseek").join("mcp_server.toml"))
}

/// Read-only by default: writing or executing tools must be named in the
/// operator's config and allowed with `require_approval = false`.
fn default_expose_tools() -> Vec<String> {
    vec!["file_read".to_string(), "search".to_string()]
}

fn build_exposed_tools(names: &[String]) -> Vec<ExposedTool> {
    let mut tools = Vec::new();
    for name in names {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            continue;
        }
        let public = trimmed.to_string();
        let internal = match trimmed {
            "file_read" => "read_file",
            "file_write" => "write_file",
            "file_edit" => "edit_file",
            "shell" => "exec_shell",
            "search" => "grep_files",
            "file_search" => "file_search",
            other => other,
        }
        .to_string();
        tools.push(ExposedTool { public, internal });
    }
    tools
}

fn tool_result_to_mcp(result: Result<ToolResult, ToolError>) -> Value {
    match result {
        Ok(tool_result) => {
            let mut response = json!({
                "content": [{ "type": "text", "text": tool_result.content }],
                "isError": !tool_result.success,
            });
            if let Some(metadata) = tool_result.metadata {
                response["structuredContent"] = metadata;
            }
            response
        }
        Err(err) => json!({
            "content": [{ "type": "text", "text": err.to_string() }],
            "isError": true,
        }),
    }
}

fn initialize_response(requested: Option<&str>) -> Value {
    // Per spec, echo the requested revision when we support it; otherwise
    // answer with the newest revision we do support and let the client decide.
    let negotiated = match requested {
        Some(version) if crate::mcp::MCP_SUPPORTED_PROTOCOL_VERSIONS.contains(&version) => version,
        _ => crate::mcp::MCP_PROTOCOL_VERSION,
    };
    json!({
        "protocolVersion": negotiated,
        "serverInfo": {
            "name": "codewhale-mcp-server",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "capabilities": {
            "tools": {},
            "resources": {},
        }
    })
}

fn respond(id: Option<&Value>, result: Value) -> Option<Value> {
    id.map(|id| json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn respond_error(id: Option<&Value>, code: i64, message: String) -> Option<Value> {
    id.map(|id| {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message }
        })
    })
}

#[derive(Debug)]
struct RpcError {
    code: i64,
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn initialize_request() -> Value {
        json!({
            "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "clientInfo": {"name": "native-server-test", "version": "1"},
                "capabilities": {}
            }
        })
    }

    async fn complete_handshake(server: &mut McpServer) {
        let response = server.handle_message(initialize_request()).await.unwrap();
        assert_eq!(response["result"]["protocolVersion"], "2024-11-05");
        assert!(
            server
                .handle_message(json!({
                    "jsonrpc": "2.0", "method": "notifications/initialized"
                }))
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn native_server_validates_identity_without_echoing_request_data() {
        let mut server = McpServer::new(
            PathBuf::from("."),
            McpServerSettings {
                expose_tools: default_expose_tools(),
                require_approval: true,
            },
        )
        .unwrap();
        for request in [
            json!([]),
            Value::Null,
            json!({"method": "ping"}),
            json!({"jsonrpc": "2", "id": 1, "method": "ping"}),
            json!({"jsonrpc": "2.0", "id": true, "method": "ping"}),
            json!({"jsonrpc": "2.0", "id": 1.5, "method": "ping"}),
            json!({"jsonrpc": "2.0", "id": 1, "method": 7}),
            json!({"jsonrpc": "2.0", "id": 1, "method": "ping", "params": "PRIVATE_TOKEN=sentinel"}),
        ] {
            let response = server.handle_message(request).await.unwrap();
            assert!(response["id"].is_null(), "{response}");
            assert_eq!(response["error"]["code"], -32600, "{response}");
            assert!(!response.to_string().contains("PRIVATE_TOKEN"));
            assert!(!response.to_string().contains("sentinel"));
        }
        let response = server
            .handle_message(json!({"jsonrpc": "2.0", "id": null, "method": "ping"}))
            .await
            .unwrap();
        assert!(response["id"].is_null());
        assert_eq!(response["result"], json!({}));
        assert!(
            server
                .handle_message(json!({"jsonrpc": "2.0", "method": "ping"}))
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn native_server_requires_completed_handshake_before_any_tool_effect() {
        let workspace = tempfile::tempdir().unwrap();
        let mut server = McpServer::new(
            workspace.path().to_path_buf(),
            McpServerSettings {
                expose_tools: vec!["file_write".into()],
                require_approval: false,
            },
        )
        .unwrap();
        let write = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
            "name": "file_write", "arguments": {"path": "canary.txt", "content": "written"}
        }});
        let response = server.handle_message(write.clone()).await.unwrap();
        assert_eq!(response["error"]["code"], -32600);
        assert!(!workspace.path().join("canary.txt").exists());
        assert!(
            server
                .handle_message(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
                .await
                .is_none()
        );
        let mut notification = initialize_request();
        notification.as_object_mut().unwrap().remove("id");
        assert!(server.handle_message(notification).await.is_none());
        assert_eq!(
            server.handle_message(write.clone()).await.unwrap()["error"]["code"],
            -32600
        );
        let mut malformed = initialize_request();
        malformed["params"]["clientInfo"]["name"] = json!("");
        assert_eq!(
            server.handle_message(malformed).await.unwrap()["error"]["code"],
            -32602
        );
        let mut initialize = initialize_request();
        initialize["id"] = Value::Null;
        let response = server.handle_message(initialize).await.unwrap();
        assert!(response["id"].is_null());
        assert_eq!(response["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(
            server.handle_message(initialize_request()).await.unwrap()["error"]["code"],
            -32600
        );
        assert_eq!(
            server.handle_message(write.clone()).await.unwrap()["error"]["code"],
            -32600
        );
        assert_eq!(
            server
                .handle_message(
                    json!({"jsonrpc": "2.0", "id": 3, "method": "notifications/initialized"})
                )
                .await
                .unwrap()["error"]["code"],
            -32600
        );
        assert_eq!(
            server.handle_message(write.clone()).await.unwrap()["error"]["code"],
            -32600
        );
        assert!(
            server
                .handle_message(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
                .await
                .is_none()
        );
        let mut unacknowledged = write.clone();
        unacknowledged.as_object_mut().unwrap().remove("id");
        assert!(server.handle_message(unacknowledged).await.is_none());
        assert!(!workspace.path().join("canary.txt").exists());
        let response = server.handle_message(write).await.unwrap();
        assert!(response.get("error").is_none(), "{response}");
        assert_eq!(
            std::fs::read_to_string(workspace.path().join("canary.txt")).unwrap(),
            "written"
        );
    }

    async fn run_native_pipe(input: &[u8]) -> (Result<()>, Vec<u8>) {
        use tokio::io::AsyncReadExt;
        let mut server = McpServer::new(
            PathBuf::from("."),
            McpServerSettings {
                expose_tools: default_expose_tools(),
                require_approval: true,
            },
        )
        .unwrap();
        let (client, server_io) = tokio::io::duplex(8192);
        let (mut reader, mut writer) = tokio::io::split(client);
        let (server_reader, server_writer) = tokio::io::split(server_io);
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            tokio::join!(server.run_io(server_reader, server_writer), async {
                // Oversized input deliberately closes the server before the
                // writer finishes. Both halves still settle without a task.
                let _ = writer.write_all(input).await;
                let _ = writer.shutdown().await;
                drop(writer);
                let mut output = Vec::new();
                reader.read_to_end(&mut output).await.unwrap();
                output
            })
        })
        .await
        .expect("the bounded server pipe must settle")
    }

    #[tokio::test]
    async fn native_pipe_reports_parse_error_and_preserves_the_following_frame() {
        let (result, output) = run_native_pipe(b"{\"PRIVATE_TOKEN\":\"sentinel\",\n{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"ping\"}\n").await;
        result.unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(!text.contains("PRIVATE_TOKEN"));
        assert!(!text.contains("sentinel"));
        let responses: Vec<Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0]["error"]["code"], -32700);
        assert!(responses[0]["id"].is_null());
        assert_eq!(responses[1]["id"], 7);
        assert_eq!(responses[1]["result"], json!({}));
    }

    #[tokio::test]
    async fn native_pipe_refuses_oversized_unterminated_input() {
        let input = vec![b'x'; crate::mcp::MAX_MCP_RESPONSE_BYTES + 8192];
        let (result, output) = run_native_pipe(&input).await;
        assert!(result.is_err());
        let response: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(response["error"]["code"], -32700);
        assert!(response["id"].is_null());
    }

    #[test]
    fn exposed_tools_map_aliases() {
        let names = vec![
            "file_read".to_string(),
            "file_write".to_string(),
            "search".to_string(),
            "apply_patch".to_string(),
            "shell".to_string(),
        ];
        let tools = build_exposed_tools(&names);
        let mut map = HashMap::new();
        for tool in tools {
            map.insert(tool.public, tool.internal);
        }
        assert_eq!(map.get("file_read").map(String::as_str), Some("read_file"));
        assert_eq!(
            map.get("file_write").map(String::as_str),
            Some("write_file")
        );
        assert_eq!(map.get("search").map(String::as_str), Some("grep_files"));
        assert_eq!(
            map.get("apply_patch").map(String::as_str),
            Some("apply_patch")
        );
        assert_eq!(map.get("shell").map(String::as_str), Some("exec_shell"));
    }

    #[tokio::test]
    async fn list_responses_omit_null_next_cursor() {
        // MCP spec: `nextCursor` must be omitted (or be a string) when there
        // are no further pages. Emitting `null` breaks strict clients such as
        // Claude Code, which validate the response shape.
        let settings = McpServerSettings {
            expose_tools: vec!["file_read".to_string(), "apply_patch".to_string()],
            require_approval: false,
        };
        let server = McpServer::new(PathBuf::from("."), settings).expect("build server");

        let tools_value = server.list_tools_response();
        let tools = tools_value
            .as_object()
            .expect("tools/list response is an object");
        assert!(tools.contains_key("tools"));
        assert!(
            tools.get("nextCursor").is_none(),
            "tools/list must omit nextCursor when there are no more pages"
        );

        let resources_value = server.list_resources_response().await;
        let resources = resources_value
            .as_object()
            .expect("resources/list response is an object");
        assert!(resources.contains_key("resources"));
        assert!(
            resources.get("nextCursor").is_none(),
            "resources/list must omit nextCursor when there are no more pages"
        );
    }

    #[tokio::test]
    async fn retired_deepseek_tools_are_not_exposed() {
        // #6140: the `deepseek`/`deepseek-reply` tools called a provider
        // client directly — a second model authority beside the engine.
        // Configs still naming them degrade to "tool not exposed" rather
        // than silently running.
        let settings = McpServerSettings {
            expose_tools: vec!["deepseek".to_string(), "deepseek-reply".to_string()],
            require_approval: false,
        };
        let mut server = McpServer::new(PathBuf::from("."), settings).expect("build server");
        complete_handshake(&mut server).await;

        let tools = server.list_tools_response();
        assert_eq!(
            tools["tools"].as_array().map(Vec::len),
            Some(0),
            "retired tools must not be advertised: {tools}"
        );

        let response = server
            .handle_message(json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {"name": "deepseek", "arguments": {"prompt": "hi"}}
            }))
            .await;
        // The name resolves through `exposed_tools` but no registry tool
        // backs it, so the call answers with an isError result.
        let response = response.expect("tools/call responds");
        assert_eq!(response["result"]["isError"], json!(true), "{response}");
    }

    #[test]
    fn default_settings_expose_only_read_only_tools() {
        assert_eq!(default_expose_tools(), vec!["file_read", "search"]);
        let settings = McpServerSettings {
            expose_tools: default_expose_tools(),
            require_approval: true,
        };
        let server = McpServer::new(PathBuf::from("."), settings).expect("build server");
        let tools = server.list_tools_response();
        let names: Vec<&str> = tools["tools"]
            .as_array()
            .expect("tools array")
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect();
        assert_eq!(names, vec!["file_read", "search"], "{tools}");
    }

    #[test]
    fn existing_config_without_require_approval_withholds_and_names_write_tools() {
        let settings = McpServerSettings::from_toml(
            "[server]\nexpose_tools = [\"file_read\", \"file_write\", \"apply_patch\"]\n",
        )
        .expect("parse config");
        assert!(settings.require_approval);
        let server = McpServer::new(PathBuf::from("."), settings).expect("build server");
        assert_eq!(server.withheld_tools(), vec!["file_write", "apply_patch"]);

        let allowed = McpServerSettings::from_toml(
            "[server]\nexpose_tools = [\"file_write\"]\nrequire_approval = false\n",
        )
        .expect("parse config");
        let server = McpServer::new(PathBuf::from("."), allowed).expect("build server");
        assert!(server.withheld_tools().is_empty());
    }

    #[tokio::test]
    async fn caller_cannot_self_approve_write_tools() {
        let workspace = tempfile::tempdir().expect("tempdir");
        let canary = workspace.path().join("canary.txt");
        let settings = McpServerSettings {
            expose_tools: vec![
                "file_read".to_string(),
                "file_write".to_string(),
                "apply_patch".to_string(),
            ],
            require_approval: true,
        };
        let mut server =
            McpServer::new(workspace.path().to_path_buf(), settings).expect("build server");
        complete_handshake(&mut server).await;

        let tools = server.list_tools_response();
        let names: Vec<&str> = tools["tools"]
            .as_array()
            .expect("tools array")
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect();
        assert_eq!(names, vec!["file_read"], "{tools}");

        for (name, arguments) in [
            ("file_write", json!({"path": "canary.txt", "content": "x"})),
            (
                "apply_patch",
                json!({"patch": "--- /dev/null\n+++ b/canary.txt\n@@ -0,0 +1 @@\n+x\n"}),
            ),
        ] {
            let response = server
                .handle_message(json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "tools/call",
                    "params": {"name": name, "approved": true, "arguments": arguments}
                }))
                .await
                .expect("tools/call responds");
            assert_eq!(
                response["error"]["code"],
                json!(-32001),
                "{name}: {response}"
            );
            assert!(!canary.exists(), "{name} must not run");
        }
    }

    #[tokio::test]
    async fn operator_config_can_allow_write_tools() {
        let workspace = tempfile::tempdir().expect("tempdir");
        let settings = McpServerSettings {
            expose_tools: vec!["file_write".to_string()],
            require_approval: false,
        };
        let mut server =
            McpServer::new(workspace.path().to_path_buf(), settings).expect("build server");
        complete_handshake(&mut server).await;
        let response = server
            .handle_message(json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {
                    "name": "file_write",
                    "arguments": {"path": "allowed.txt", "content": "x"}
                }
            }))
            .await
            .expect("tools/call responds");
        assert!(response.get("error").is_none(), "{response}");
        assert!(workspace.path().join("allowed.txt").exists(), "{response}");
    }

    #[test]
    fn initialize_uses_standard_mcp_shape_and_codewhale_identity() {
        let response = initialize_response(Some(crate::mcp::MCP_PROTOCOL_VERSION));
        assert_eq!(
            response["protocolVersion"],
            crate::mcp::MCP_PROTOCOL_VERSION
        );
        assert_eq!(response["serverInfo"]["name"], "codewhale-mcp-server");
        assert_eq!(response["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
        assert!(response["capabilities"]["tools"].is_object());
    }

    #[test]
    fn initialize_negotiates_supported_revisions() {
        // A client asking for an older dated revision gets it echoed back;
        // an unknown or missing revision answers with the newest supported.
        for requested in ["2025-03-26", "2024-11-05"] {
            let response = initialize_response(Some(requested));
            assert_eq!(response["protocolVersion"], requested);
        }
        for requested in [Some("2099-01-01"), None] {
            let response = initialize_response(requested);
            assert_eq!(
                response["protocolVersion"],
                crate::mcp::MCP_PROTOCOL_VERSION
            );
        }
    }
}
