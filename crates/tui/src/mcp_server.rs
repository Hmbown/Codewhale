//! MCP server implementation for exposing Codewhale tools over stdio.

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

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
            let config: McpServerConfigFile = toml::from_str(&contents).with_context(|| {
                format!("Failed to parse MCP server config: {}", path.display())
            })?;
            let expose_tools = config
                .server
                .expose_tools
                .unwrap_or_else(default_expose_tools);
            let require_approval = config.server.require_approval.unwrap_or(false);
            Ok(Self {
                expose_tools,
                require_approval,
            })
        } else {
            Ok(Self {
                expose_tools: default_expose_tools(),
                require_approval: false,
            })
        }
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
    server.run().await
}

struct McpServer {
    workspace: PathBuf,
    registry: crate::tools::ToolRegistry,
    exposed_tools: Vec<ExposedTool>,
    require_approval: bool,
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
        })
    }

    /// The serialized stdio loop runs on the caller's runtime: a JSON-RPC
    /// stdio server answers one request at a time by definition, so it
    /// needs no private `Runtime` and no `block_on` (#6140).
    async fn run(&mut self) -> Result<()> {
        let stdin = tokio::io::BufReader::new(tokio::io::stdin());
        let mut stdout = tokio::io::stdout();
        let mut lines = stdin.lines();

        while let Some(line) = lines.next_line().await? {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Ok(message) = serde_json::from_str::<Value>(trimmed) else {
                continue;
            };

            if let Some(response) = self.handle_message(message).await {
                let payload = serde_json::to_string(&response)?;
                stdout.write_all(payload.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
        }

        Ok(())
    }

    async fn handle_message(&mut self, message: Value) -> Option<Value> {
        let method = message.get("method").and_then(Value::as_str)?;
        let id = message.get("id").cloned();

        match method {
            "initialize" => respond(
                id.as_ref(),
                initialize_response(
                    message
                        .pointer("/params/protocolVersion")
                        .and_then(Value::as_str),
                ),
            ),
            "tools/list" => respond(id.as_ref(), self.list_tools_response()),
            "tools/call" => {
                let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
                match self.call_tool(params).await {
                    Ok(result) => respond(id.as_ref(), result),
                    Err(err) => respond_error(id.as_ref(), err.code, err.message),
                }
            }
            "resources/list" => respond(id.as_ref(), self.list_resources_response().await),
            "ping" => respond(id.as_ref(), json!({})),
            "notifications/initialized" => None,
            _ => respond_error(id.as_ref(), -32601, format!("Method not found: {method}")),
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

        if self.require_approval
            && !params
                .get("approved")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            return Err(RpcError {
                code: -32001,
                message: "Approval required. Resend with approved=true.".to_string(),
            });
        }

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
        let result = self.registry.execute_full(&internal, arguments).await;
        Ok(tool_result_to_mcp(result))
    }
}

fn default_config_path() -> Option<PathBuf> {
    crate::config::effective_home_dir().map(|home| home.join(".deepseek").join("mcp_server.toml"))
}

fn default_expose_tools() -> Vec<String> {
    vec![
        "file_read".to_string(),
        "file_write".to_string(),
        "search".to_string(),
        "apply_patch".to_string(),
        "shell".to_string(),
    ]
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
