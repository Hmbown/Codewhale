//! Real Engine/process acceptance; only an in-process fake provider is used.
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use wait_timeout::ChildExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const MODEL: &str = "issue-draft-fixture-model";
const ORIGINAL: &str = "ORIGINAL_TASK_CONTEXT_ISSUE_REPORT";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn active_agent_drafts_converge_and_resume_in_the_same_session() {
    let workspace = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let count = Arc::new(AtomicUsize::new(0));
    let handle = Arc::new(Mutex::new(None));
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(Scenario {
            count: count.clone(),
            handle: handle.clone(),
        })
        .mount(&server)
        .await;
    let output = run_exec(workspace.path(), home.path(), &server, None);
    assert_success(&output);
    assert_eq!(count.load(Ordering::SeqCst), 4);
    let (session, directory) = report_directory(home.path()).expect("saved report directory");
    let files = std::fs::read_dir(&directory)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(files.len(), 1, "identical model reports must converge");
    let before = std::fs::read(files[0].path()).unwrap();
    let saved: Value = serde_json::from_slice(&before).unwrap();
    assert_eq!(saved["model"], MODEL);
    assert_eq!(saved["session"], session);
    assert_eq!(
        saved["id"],
        handle.lock().unwrap().as_ref().unwrap().as_str()
    );
    let text = String::from_utf8(before.clone()).unwrap();
    for private in [
        "fixture-private-token",
        "private-machine-owner",
        "fixture-url-password",
    ] {
        assert!(!text.contains(private));
    }
    // A new process resumes the real saved session and asks report_read for
    // the opaque handle from the earlier model-visible result.
    let resumed = run_exec(workspace.path(), home.path(), &server, Some(&session));
    assert_success(&resumed);
    assert_eq!(count.load(Ordering::SeqCst), 6);
    assert_eq!(std::fs::read(files[0].path()).unwrap(), before);
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
    assert!(!home.path().join("unexpected-gh-call").exists());
    let requests = server.received_requests().await.unwrap();
    for request in requests {
        assert_eq!(request.url.path(), "/v1/chat/completions");
        assert_eq!(request.body_json::<Value>().unwrap()["model"], MODEL);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_provider_leaves_no_generated_draft_or_fallback_success() {
    let workspace = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"error":{"message":"fixture provider unavailable", "type":"authentication_error"}})))
        .mount(&server).await;
    let output = run_exec(workspace.path(), home.path(), &server, None);
    assert!(!output.status.success());
    assert!(report_directory(home.path()).is_none());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("DRAFT_SAVED_AND_ORIGINAL_TASK_CONTINUED"));
    assert!(!stdout.contains("ready_for_review"));
    assert!(!home.path().join("unexpected-gh-call").exists());
    let requests = server.received_requests().await.unwrap();
    assert!(!requests.is_empty());
    for request in requests {
        assert_eq!(request.url.path(), "/v1/chat/completions");
        assert_eq!(request.body_json::<Value>().unwrap()["model"], MODEL);
    }
}

#[derive(Clone)]
struct Scenario {
    count: Arc<AtomicUsize>,
    handle: Arc<Mutex<Option<String>>>,
}
impl Respond for Scenario {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let sequence = self.count.fetch_add(1, Ordering::SeqCst);
        let body: Value = request.body_json().unwrap();
        assert_eq!(body["model"], MODEL);
        assert!(
            body["messages"].to_string().contains(ORIGINAL),
            "same task context must reach every request"
        );
        let response = match sequence {
            0 => draft_call("draft-first"),
            1 => {
                let first = receipt(&body, "draft-first");
                assert_eq!(first["publication"], "unavailable");
                assert_eq!(first["duplicate_search"], "not_performed");
                assert!(first["review"].as_str().unwrap().contains(MODEL));
                for private in [
                    "fixture-private-token",
                    "private-machine-owner",
                    "fixture-url-password",
                ] {
                    assert!(!first.to_string().contains(private));
                }
                *self.handle.lock().unwrap() = Some(first["report_id"].as_str().unwrap().into());
                draft_call("draft-repeat")
            }
            2 => {
                let repeated = receipt(&body, "draft-repeat");
                assert_eq!(
                    repeated["report_id"],
                    self.handle.lock().unwrap().as_ref().unwrap().as_str()
                );
                tool_sse(
                    "draft-read",
                    json!({"action":"report_read", "report_id":repeated["report_id"]}),
                )
            }
            3 => {
                assert_eq!(receipt(&body, "draft-read")["state"], "ready_for_review");
                final_sse()
            }
            4 => {
                assert!(
                    body["messages"]
                        .to_string()
                        .contains("RESUME_EXISTING_DRAFT")
                );
                tool_sse(
                    "draft-resume-read",
                    json!({"action":"report_read", "report_id":self.handle.lock().unwrap().as_ref().unwrap()}),
                )
            }
            5 => {
                assert_eq!(
                    receipt(&body, "draft-resume-read")["report_id"],
                    self.handle.lock().unwrap().as_ref().unwrap().as_str()
                );
                final_sse()
            }
            _ => panic!("unexpected extra provider request"),
        };
        ResponseTemplate::new(200)
            .insert_header("content-type", "text/event-stream")
            .set_body_string(response)
    }
}

fn draft_call(id: &str) -> String {
    tool_sse(
        id,
        json!({"action":"report_draft", "report":{
            "title":"Runtime result delivery failed", "expected":"The agent receives the completed tool result",
            "actual":"Result missing. Authorization:\nBearer\tfixture-private-token /Users/private-machine-owner/workspace https://user:fixture-url-password@example.invalid/private",
            "impact":"Original task needs a retry", "steps":["Run a tool", "Wait for its result"],
            "observed":["The Runtime result was absent"], "inferred":["The Runtime may have dropped an event"],
            "reported_tool":"fixture tool", "reported_provider":"fixture provider"
        }}),
    )
}

fn receipt(body: &Value, id: &str) -> Value {
    let messages = body["messages"].as_array().unwrap();
    let content = messages
        .iter()
        .find(|message| message["role"] == "tool" && message["tool_call_id"] == id)
        .unwrap_or_else(|| panic!("missing receipt {id}"))["content"]
        .as_str()
        .unwrap();
    // The provider adapter replaces exact duplicate results with a reference
    // to earlier full content in this same request. Follow its verified digest.
    let content = if let Some(reference) = content.strip_prefix("<TOOL_RESULT_REF sha=\"") {
        let digest = reference.split('"').next().unwrap();
        messages
            .iter()
            .filter_map(|message| message["content"].as_str())
            .find(|candidate| {
                Sha256::digest(candidate.as_bytes())
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
                    == digest
            })
            .expect("duplicate receipt must reference full content in this request")
    } else {
        content
    };
    serde_json::from_str(content).unwrap_or_else(|_| panic!("invalid draft receipt: {content}"))
}

fn chunk(value: Value) -> String {
    format!("data: {value}\n\n")
}
fn tool_sse(id: &str, args: Value) -> String {
    format!(
        "{}{}data: [DONE]\n\n",
        chunk(
            json!({"id":"fixture", "object":"chat.completion.chunk", "model":MODEL, "choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":id,"type":"function","function":{"name":"github","arguments":args.to_string()}}]},"finish_reason":null}]})
        ),
        chunk(
            json!({"id":"fixture", "model":MODEL, "choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":10,"completion_tokens":2,"total_tokens":12}})
        )
    )
}
fn final_sse() -> String {
    format!(
        "{}{}data: [DONE]\n\n",
        chunk(
            json!({"id":"fixture", "model":MODEL, "choices":[{"index":0,"delta":{"content":"DRAFT_SAVED_AND_ORIGINAL_TASK_CONTINUED"},"finish_reason":null}]})
        ),
        chunk(
            json!({"id":"fixture", "model":MODEL, "choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":2,"total_tokens":12}})
        )
    )
}

fn report_directory(home: &Path) -> Option<(String, PathBuf)> {
    std::fs::read_dir(home.join(".codewhale/sessions"))
        .ok()?
        .filter_map(Result::ok)
        .find_map(|entry| {
            let path = entry.path().join("artifacts/issue-reports");
            path.is_dir()
                .then(|| (entry.file_name().to_string_lossy().into_owned(), path))
        })
}

fn run_exec(
    workspace: &Path,
    home: &Path,
    server: &MockServer,
    resume: Option<&str>,
) -> std::process::Output {
    std::fs::create_dir_all(home.join(".codewhale")).unwrap();
    std::fs::write(
        home.join(".codewhale/config.toml"),
        "allow_shell = false\ntelemetry = false\n\n[retry]\nenabled = false\n",
    )
    .unwrap();
    let mut command = Command::new(crate::binary::codewhale());
    command.env_clear();
    for key in [
        "PATH",
        "PATHEXT",
        "SystemRoot",
        "SystemDrive",
        "WINDIR",
        "COMSPEC",
        "TEMP",
        "TMP",
        "LANG",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command
        .current_dir(workspace)
        .args(["--workspace"])
        .arg(workspace)
        .args([
            "--no-project-config",
            "exec",
            "--auto",
            "--provider",
            "deepseek",
            "--model",
            MODEL,
            "--allowed-tools",
            "github",
            "--output-format",
            "stream-json",
        ])
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("CODEWHALE_HOME", home.join(".codewhale"))
        .env("CODEWHALE_CONFIG_PATH", home.join(".codewhale/config.toml"))
        .env("DEEPSEEK_API_KEY", "fixture-key-not-real")
        .env("DEEPSEEK_BASE_URL", server.uri())
        .env("CODEWHALE_BASE_URL", server.uri())
        .env("DEEPSEEK_MODEL", MODEL)
        .env("CODEWHALE_MODEL", MODEL)
        .env("CODEWHALE_TELEMETRY", "0")
        // Any accidental use of the existing gh adapter must fail. No real gh
        // credentials or binary is reachable through this test override.
        .env("CODEWHALE_GH_BIN", home.join("missing-gh-fixture"))
        .env("RUST_LOG", "warn")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(id) = resume {
        command.args(["--resume", id]);
    }
    command.arg(if resume.is_some() {
        "RESUME_EXISTING_DRAFT: read the saved draft from our earlier work and continue that task.".to_string()
    } else {
        format!("{ORIGINAL}: observed a Runtime result-delivery failure. Draft it locally and continue the original task.")
    });
    let mut child = command.spawn().unwrap();
    let stdout = drain(child.stdout.take().unwrap());
    let stderr = drain(child.stderr.take().unwrap());
    let status = child
        .wait_timeout(Duration::from_secs(60))
        .unwrap()
        .unwrap_or_else(|| {
            child.kill().ok();
            child.wait().ok();
            panic!("issue fixture timed out");
        });
    std::process::Output {
        status,
        stdout: stdout.join().unwrap(),
        stderr: stderr.join().unwrap(),
    }
}
fn drain(mut stream: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        bytes
    })
}
fn assert_success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "fixture failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("DRAFT_SAVED_AND_ORIGINAL_TASK_CONTINUED")
    );
}
