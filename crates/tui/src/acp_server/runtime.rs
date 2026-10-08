//! Projection of the existing Runtime owner; no direct provider/tool authority.
use super::*;
use crate::runtime_threads::{
    ExternalApprovalDecision, RuntimeEventRecord, RuntimeTurnStatus, RuntimeTurnStopReason,
    StartTurnRequest, TurnRecord,
};
use std::time::Duration;
struct Permission {
    thread_id: String,
    turn_id: String,
    approval_id: String,
    execution_id: String,
}
struct Projection<'a> {
    session: &'a str,
    thread: &'a str,
    turn: &'a str,
    cursor: u64,
    emit: bool,
    calls: HashMap<String, PendingToolCall>,
    permissions: HashMap<String, Permission>,
}
/// Drop interrupts only this transport's admitted turn on the current scheduler.
struct TurnClaim {
    runtime: Arc<RuntimeThreadManager>,
    thread_id: String,
    turn_id: String,
    armed: bool,
}
impl Drop for TurnClaim {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let runtime = Arc::clone(&self.runtime);
        let thread = self.thread_id.clone();
        let turn = self.turn_id.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _ = runtime.interrupt_turn(&thread, &turn).await;
            });
        }
    }
}
impl AcpServer {
    pub(super) async fn drive_prompt<R: AsyncBufRead + Unpin, W: AsyncWrite + Unpin>(
        &mut self,
        params: Value,
        reader: &mut codewhale_app_server::BoundedLines<R>,
        writer: &mut W,
    ) -> Result<&'static str> {
        let (session_id, prompt) = self
            .validate_prompt(&params)
            .map_err(|error| anyhow!(error.message))?;
        let binding = self
            .sessions
            .get(&session_id)
            .ok_or_else(|| anyhow!("unknown sessionId"))?;
        let thread_id = binding.thread_id.clone();
        // Subscribe before snapshot/admission; replay deduplicates by seq.
        let mut receiver = self.runtime.subscribe_events();
        let cursor = self.runtime.get_thread_detail(&thread_id).await?.latest_seq;
        let turn = self
            .runtime
            .start_acp_turn(
                &thread_id,
                StartTurnRequest {
                    prompt,
                    ..Default::default()
                },
            )
            .await?;
        let mut claim = TurnClaim {
            runtime: Arc::clone(&self.runtime),
            thread_id: thread_id.clone(),
            turn_id: turn.id.clone(),
            armed: true,
        };
        let mut projection = Projection {
            session: &session_id,
            thread: &thread_id,
            turn: &turn.id,
            cursor,
            emit: true,
            calls: HashMap::new(),
            permissions: HashMap::new(),
        };
        let terminal = match self
            .project_turn(&mut projection, &mut receiver, reader, writer)
            .await
        {
            Ok(terminal) => terminal,
            Err(error) => {
                self.interrupt_claimed_turn(&thread_id, &turn.id).await?;
                let settled = tokio::time::timeout(Duration::from_secs(30), async {
                    loop {
                        let detail = self.runtime.get_thread_detail(&thread_id).await?;
                        if let Some(record) = detail.turns.into_iter().find(|record| record.id == turn.id && !matches!(record.status, RuntimeTurnStatus::Queued | RuntimeTurnStatus::InProgress)) { return Ok::<_, anyhow::Error>(record); }
                        tokio::time::sleep(Duration::from_millis(25)).await;
                    }
                }).await.map_err(|_| anyhow!("ACP projection failed ({error}); Core cancellation is still unconfirmed, inspect its canonical receipt"))??;
                self.save_checkpoint(&thread_id, &session_id).await?;
                claim.armed = false;
                return Err(anyhow!(
                    "ACP projection failed ({error}); actual Core terminal is {:?}; partial receipts retained",
                    settled.status
                ));
            }
        };
        claim.armed = false;
        if let Some(binding) = self.sessions.get_mut(&session_id) {
            binding.cursor = projection.cursor;
        }
        self.save_checkpoint(&thread_id, &session_id).await?;
        if terminal
            .model_request_diagnostics
            .as_ref()
            .and_then(|facts| facts.stop_reason)
            == Some(RuntimeTurnStopReason::StepBudgetExhausted)
        {
            return Ok("max_turn_requests");
        }
        match terminal.status {
            RuntimeTurnStatus::Completed => Ok("end_turn"),
            RuntimeTurnStatus::Interrupted | RuntimeTurnStatus::Canceled => Ok("cancelled"),
            _ => Err(anyhow!(
                "{}",
                terminal.error.unwrap_or_else(
                    || "Core turn failed; inspect its canonical receipt".to_string()
                )
            )),
        }
    }
    async fn interrupt_claimed_turn(&self, thread: &str, turn: &str) -> Result<()> {
        match self.runtime.interrupt_turn(thread, turn).await {
            Ok(_) => Ok(()),
            Err(error) => {
                // Completion may win the race with EOF, cancel, or a failed
                // writer. Accept only the actual terminal of this claimed turn.
                let detail = self.runtime.get_thread_detail(thread).await.map_err(|read| anyhow!("ACP interruption failed ({error}); Core receipt cannot be read ({read}); inspect without replay"))?;
                if detail.turns.iter().any(|record| {
                    record.id == turn
                        && !matches!(
                            record.status,
                            RuntimeTurnStatus::Queued | RuntimeTurnStatus::InProgress
                        )
                }) {
                    Ok(())
                } else {
                    Err(anyhow!(
                        "ACP cannot confirm interruption of its claimed Core turn: {error}; inspect without replay"
                    ))
                }
            }
        }
    }
    async fn save_checkpoint(&self, thread: &str, session: &str) -> Result<()> {
        // The same full Engine snapshot writer as HTTP, never projected text.
        let axum::Json(_saved_checkpoint) = crate::runtime_api::sessions::save_session_in_runtime(
            &self.runtime,
            &self.sessions_dir,
            crate::runtime_api::sessions::SaveSessionRequest {
                thread_id: Some(thread.to_string()),
                session_id: Some(session.to_string()),
            },
        )
        .await
        .map_err(|error| {
            anyhow!(
                "Core turn finished but its checkpoint failed; do not replay: {}",
                error.message
            )
        })?;
        Ok(())
    }
    async fn project_turn<R: AsyncBufRead + Unpin, W: AsyncWrite + Unpin>(
        &self,
        projection: &mut Projection<'_>,
        receiver: &mut tokio::sync::broadcast::Receiver<RuntimeEventRecord>,
        reader: &mut codewhale_app_server::BoundedLines<R>,
        writer: &mut W,
    ) -> Result<TurnRecord> {
        let session = projection.session;
        let thread = projection.thread;
        let turn = projection.turn;
        let mut eof = false;
        let mut cancelled = false;
        let mut cancel_deadline: Option<tokio::time::Instant> = None;
        let mut replay: Option<crate::runtime_threads::RuntimeEventReplay> = None;
        let mut replay_batch = VecDeque::new();
        loop {
            if cancel_deadline.is_some_and(|deadline| tokio::time::Instant::now() >= deadline) {
                return Err(anyhow!(
                    "Core cancellation remains nonterminal; ACP cannot report a fabricated cancelled receipt"
                ));
            }
            tokio::select! {
                () = async { match cancel_deadline { Some(deadline) => tokio::time::sleep_until(deadline).await, None => std::future::pending().await } } => return Err(anyhow!("Core cancellation remains nonterminal; ACP cannot report a fabricated cancelled receipt")),
                line = reader.next_line(), if !eof => {
                    let Some(line) = line? else {
                        eof = true; cancelled = true; projection.permissions.clear(); self.interrupt_claimed_turn(thread, turn).await?;
                        cancel_deadline = Some(tokio::time::Instant::now() + Duration::from_secs(30)); continue;
                    };
                    if line.trim().is_empty() { continue; }
                    let message: Value = match serde_json::from_str(&line) {
                        Ok(message) => message,
                        Err(error) => { write_jsonrpc_error(writer, None, -32700, format!("invalid json: {error}")).await?; continue; }
                    };
                    if codewhale_app_server::is_control_input_closed(&message) {
                        eof=true; cancelled=true; projection.permissions.clear();self.interrupt_claimed_turn(thread,turn).await?;
                        cancel_deadline.get_or_insert(tokio::time::Instant::now()+Duration::from_secs(30));continue;
                    }
                    let response_id = message.get("id").cloned().map(|id| self.response_id_policy.response_id(id));
                    if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0") { write_jsonrpc_error(writer, response_id, -32600, "jsonrpc version must be 2.0").await?; continue; }
                    if is_jsonrpc_response(&message) {
                        if !cancelled && let Some(id) = message.get("id").and_then(Value::as_str) && let Some(permission) = projection.permissions.remove(id) { self.answer_permission(&permission, &message).await?; }
                        continue;
                    }
                    if message.get("method").and_then(Value::as_str) == Some("session/cancel") {
                        if message.pointer("/params/sessionId").and_then(Value::as_str) == Some(session) {
                            cancelled = true; projection.permissions.clear(); self.interrupt_claimed_turn(thread, turn).await?;
                            cancel_deadline.get_or_insert(tokio::time::Instant::now() + Duration::from_secs(30));
                        }
                        if let Some(id) = response_id { write_jsonrpc_result(writer, id, json!(null)).await?; }
                    } else if let Some(id) = response_id { write_jsonrpc_error(writer, Some(id), -32603, "a session/prompt turn is already in progress").await?; }
                }
                record = async { replay_batch.pop_front().expect("nonempty bounded replay batch") }, if !replay_batch.is_empty() => {
                    projection.emit = !cancelled && !eof;
                    if let Some(done) = self.project_event(projection, &record, writer).await? { return Ok(done); }
                }
                batch = async { replay.as_mut().expect("active replay").batches.recv().await }, if replay.is_some() && replay_batch.is_empty() => {
                    match batch {
                        Some(batch) => replay_batch.extend(batch.map_err(|error| anyhow!(error))?),
                        None => replay = None,
                    }
                }
                event = receiver.recv(), if replay.is_none() && replay_batch.is_empty() => {
                    match event {
                        Ok(record) => { projection.emit = !cancelled && !eof; if let Some(done) = self.project_event(projection, &record, writer).await? { return Ok(done); } },
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            let captured = self.runtime.replay_events(thread, Some(projection.cursor), None).await?;
                            if captured.base_seq > projection.cursor { return Err(anyhow!("ACP replay no longer covers the cursor; projection is incomplete")); }
                            replay = Some(captured);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => return Err(anyhow!("Runtime owner closed before its terminal receipt")),
                    }
                }
            }
        }
    }
    async fn answer_permission(&self, permission: &Permission, response: &Value) -> Result<()> {
        let detail = self
            .runtime
            .get_thread_detail(&permission.thread_id)
            .await?;
        if !detail.pending_approvals.iter().any(|pending| {
            pending.id == permission.approval_id
                && pending.turn_id == permission.turn_id
                && pending.tool_call_id.as_deref() == Some(permission.execution_id.as_str())
        }) {
            return Ok(());
        }
        let allow = response.get("error").is_none()
            && response
                .pointer("/result/outcome/outcome")
                .and_then(Value::as_str)
                == Some("selected")
            && response
                .pointer("/result/outcome/optionId")
                .and_then(Value::as_str)
                == Some("allow-once");
        self.runtime.deliver_external_approval(
            &permission.approval_id,
            if allow {
                ExternalApprovalDecision::Allow { remember: false }
            } else {
                ExternalApprovalDecision::Deny { remember: false }
            },
        );
        Ok(())
    }
    async fn project_event<W: AsyncWrite + Unpin>(
        &self,
        projection: &mut Projection<'_>,
        record: &RuntimeEventRecord,
        writer: &mut W,
    ) -> Result<Option<TurnRecord>> {
        let session = projection.session;
        let thread = projection.thread;
        let turn = projection.turn;
        let cursor = &mut projection.cursor;
        let calls = &mut projection.calls;
        let permissions = &mut projection.permissions;
        let emit = projection.emit;
        if record.thread_id != thread || record.seq <= *cursor {
            return Ok(None);
        }
        *cursor = record.seq;
        if record.turn_id.as_deref() != Some(turn) {
            return Ok(None);
        }
        let p = &record.payload;
        match record.event.as_str() {
            "item.delta"
                if emit && p.get("kind").and_then(Value::as_str) == Some("agent_message") =>
            {
                if let Some(text) = p.get("delta").and_then(Value::as_str) {
                    write_session_update(writer, session, text.to_string()).await?;
                }
            }
            "item.started" => {
                if let Some(tool) = p.get("tool") {
                    let id = tool
                        .get("id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| anyhow!("Tool start lacks Core identity"))?
                        .to_string();
                    let call = PendingToolCall {
                        execution_id: id.clone(),
                        name: tool
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        input: tool.get("input").cloned().unwrap_or(Value::Null),
                    };
                    if emit {
                        write_tool_call_start(writer, session, &call).await?;
                    }
                    calls.insert(id, call);
                }
            }
            "tool.execution_started" if emit => {
                let id = p
                    .get("execution_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("Dispatch lacks Core identity"))?;
                let call = calls
                    .get(id)
                    .ok_or_else(|| anyhow!("Dispatch projection lacks its admitted start"))?;
                write_tool_call_update(writer, session, call, "in_progress", None).await?;
            }
            "item.completed" | "item.failed" | "item.interrupted" if emit => {
                if let Some(item) = p.get("item")
                    && let Some(id) = item
                        .pointer("/metadata/execution_id")
                        .and_then(Value::as_str)
                    && let Some(call) = calls.remove(id)
                {
                    let blocks: Vec<codewhale_tools::ToolResultContentBlock> = item
                        .pointer("/metadata/acp_result_content")
                        .cloned()
                        .map(serde_json::from_value)
                        .transpose()?
                        .unwrap_or_default();
                    let status = if item.get("status").and_then(Value::as_str) == Some("completed")
                    {
                        "completed"
                    } else {
                        "failed"
                    };
                    write_tool_call_update_with_blocks(
                        writer,
                        session,
                        &call,
                        status,
                        item.get("detail").and_then(Value::as_str),
                        &blocks,
                    )
                    .await?;
                }
            }
            "approval.required" if emit => {
                let approval = p
                    .get("approval_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("Approval lacks Runtime identity"))?;
                let execution = p
                    .get("tool_call_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("Approval lacks Core correlator"))?;
                let detail = self.runtime.get_thread_detail(thread).await?;
                if detail.pending_approvals.iter().any(|pending| {
                    pending.id == approval
                        && pending.turn_id == turn
                        && pending.tool_call_id.as_deref() == Some(execution)
                }) {
                    let call = calls
                        .get(execution)
                        .ok_or_else(|| anyhow!("Approval projection lacks admitted tool start"))?;
                    let id = format!(
                        "codewhale-permission-{}",
                        NEXT_ACP_PERMISSION_REQUEST_ID.fetch_add(1, Ordering::Relaxed)
                    );
                    write_json_line(writer, json!({"jsonrpc":"2.0","id":id,"method":"session/request_permission","params":{"sessionId":session,"toolCall":{"toolCallId":execution,"title":tool_call_title(call),"kind":tool_call_kind(call),"status":"pending","rawInput":call.input,"content":[{"type":"content","content":{"type":"text","text":p.get("description")}}]},"options":[{"optionId":"allow-once","name":"Allow once","kind":"allow_once"},{"optionId":"reject-once","name":"Reject","kind":"reject_once"}]}})).await?;
                    permissions.insert(
                        id,
                        Permission {
                            thread_id: thread.to_string(),
                            turn_id: turn.to_string(),
                            approval_id: approval.to_string(),
                            execution_id: execution.to_string(),
                        },
                    );
                }
            }
            "approval.withdrawn" | "approval.decided" => {
                if let Some(id) = p.get("approval_id").and_then(Value::as_str) {
                    permissions.retain(|_, permission| permission.approval_id != id);
                }
            }
            crate::runtime_threads::RUNTIME_STORE_FAILURE_EVENT
                if p.get("terminal").and_then(Value::as_bool) == Some(true) =>
            {
                return Err(anyhow!(
                    "Core store failure has no terminal receipt: {}; inspect without replay",
                    p.get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("canonical store unavailable")
                ));
            }
            "turn.completed" => {
                permissions.clear();
                let terminal: TurnRecord = serde_json::from_value(
                    p.get("turn")
                        .cloned()
                        .ok_or_else(|| anyhow!("Terminal event lacks its actual record"))?,
                )?;
                if terminal.id != turn {
                    return Err(anyhow!("Terminal does not match admitted turn"));
                }
                return Ok(Some(terminal));
            }
            _ => {}
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{Rig, fixture_config};
    use super::*;
    fn event(seq: u64, thread: &str, turn: &str, name: &str, payload: Value) -> RuntimeEventRecord {
        serde_json::from_value(json!({"seq":seq,"timestamp":chrono::Utc::now(),"thread_id":thread,"turn_id":turn,"event":name,"payload":payload})).unwrap()
    }
    #[tokio::test(flavor = "current_thread")]
    async fn projection_deduplicates_replay_and_refuses_dispatch_without_core_start() -> Result<()>
    {
        let mut rig = Rig::new(fixture_config(), vec![])?;
        let id = rig.new_session().await;
        let thread = rig.server.sessions[&id].thread_id.clone();
        let mut p = Projection {
            session: &id,
            thread: &thread,
            turn: "turn-current",
            cursor: 0,
            emit: true,
            calls: HashMap::new(),
            permissions: HashMap::new(),
        };
        let mut output = Vec::new();
        let start = event(
            1,
            &thread,
            p.turn,
            "item.started",
            json!({"tool":{"id":"core-call","name":"read","input":{"path":"x"}}}),
        );
        rig.server
            .project_event(&mut p, &start, &mut output)
            .await?;
        let before = output.len();
        rig.server
            .project_event(&mut p, &start, &mut output)
            .await?;
        assert_eq!(output.len(), before);
        let dispatch = event(
            2,
            &thread,
            p.turn,
            "tool.execution_started",
            json!({"execution_id":"unknown"}),
        );
        assert!(
            rig.server
                .project_event(&mut p, &dispatch, &mut output)
                .await
                .is_err()
        );
        rig.close().await;
        Ok(())
    }
    #[tokio::test(flavor = "current_thread")]
    async fn projection_filters_foreign_turns_reasoning_and_unminted_approvals() -> Result<()> {
        let mut rig = Rig::new(fixture_config(), vec![])?;
        let id = rig.new_session().await;
        let thread = rig.server.sessions[&id].thread_id.clone();
        let mut p = Projection {
            session: &id,
            thread: &thread,
            turn: "turn-current",
            cursor: 0,
            emit: true,
            calls: HashMap::new(),
            permissions: HashMap::new(),
        };
        let mut output = Vec::new();
        for record in [
            event(
                1,
                "foreign",
                p.turn,
                "item.delta",
                json!({"kind":"agent_message","delta":"foreign"}),
            ),
            event(
                2,
                &thread,
                "foreign-turn",
                "item.delta",
                json!({"kind":"agent_message","delta":"other turn"}),
            ),
            event(
                3,
                &thread,
                p.turn,
                "item.delta",
                json!({"kind":"reasoning","delta":"private reasoning"}),
            ),
            event(
                4,
                &thread,
                p.turn,
                "approval.required",
                json!({"approval_id":"unminted","tool_call_id":"provider-id"}),
            ),
        ] {
            rig.server
                .project_event(&mut p, &record, &mut output)
                .await?;
        }
        assert!(output.is_empty());
        assert!(p.permissions.is_empty());
        rig.close().await;
        Ok(())
    }
    #[tokio::test(flavor = "current_thread")]
    async fn terminal_store_fault_refuses_a_fabricated_completion() -> Result<()> {
        let mut rig = Rig::new(fixture_config(), vec![])?;
        let id = rig.new_session().await;
        let thread = rig.server.sessions[&id].thread_id.clone();
        let mut p = Projection {
            session: &id,
            thread: &thread,
            turn: "turn-current",
            cursor: 0,
            emit: true,
            calls: HashMap::new(),
            permissions: HashMap::new(),
        };
        let mut output = Vec::new();
        let failure = event(
            1,
            &thread,
            p.turn,
            crate::runtime_threads::RUNTIME_STORE_FAILURE_EVENT,
            json!({"terminal":true,"message":"fixture store fault"}),
        );
        let error = rig
            .server
            .project_event(&mut p, &failure, &mut output)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("no terminal receipt"));
        assert!(output.is_empty());
        rig.close().await;
        Ok(())
    }
    struct FailedAnswerWriter(Vec<u8>);
    impl AsyncWrite for FailedAnswerWriter {
        fn poll_write(
            mut self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            bytes: &[u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            if String::from_utf8_lossy(bytes).contains("agent_message_chunk") {
                return std::task::Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "fixture client disappeared after tool effect",
                )));
            }
            self.0.extend_from_slice(bytes);
            std::task::Poll::Ready(Ok(bytes.len()))
        }
        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }
        fn poll_shutdown(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }
    }
    #[tokio::test(flavor = "current_thread")]
    async fn failed_transport_after_completed_write_retains_actual_core_terminal_and_checkpoint()
    -> Result<()> {
        use crate::llm_client::mock::canned;
        let mut config = fixture_config();
        config.yolo = Some(true);
        let mut rig = Rig::new(
            config,
            vec![
                canned::tool_call_turn(
                    "write",
                    "write",
                    r#"{"path":"writer-failure.txt","content":"completed"}"#,
                ),
                canned::simple_text_turn("final answer"),
            ],
        )?;
        let id = rig.new_session().await;
        let (client, input) = tokio::io::duplex(1024);
        let mut reader = codewhale_app_server::BoundedLines::new(BufReader::new(input));
        let mut writer = FailedAnswerWriter(Vec::new());
        let error = tokio::time::timeout(
            Duration::from_secs(20),
            rig.server.drive_prompt(
                json!({"sessionId":id,"prompt":"write then answer"}),
                &mut reader,
                &mut writer,
            ),
        )
        .await?
        .unwrap_err();
        drop(client);
        assert!(error.to_string().contains("partial receipts retained"));
        assert_eq!(
            std::fs::read_to_string(rig.workspace.join("writer-failure.txt"))?,
            "completed"
        );
        let store = crate::session_manager::SessionManager::new(rig.server.sessions_dir.clone())?;
        let saved = store.load_session(&id)?;
        assert!(
            saved
                .messages
                .iter()
                .flat_map(|m| &m.content)
                .any(|b| matches!(b, codewhale_models::ContentBlock::ToolResult { .. }))
        );
        let thread = &rig.server.sessions[&id].thread_id;
        let detail = rig.server.runtime.get_thread_detail(thread).await?;
        assert!(!matches!(
            detail.turns.last().unwrap().status,
            RuntimeTurnStatus::Queued | RuntimeTurnStatus::InProgress
        ));
        rig.close().await;
        Ok(())
    }
}
