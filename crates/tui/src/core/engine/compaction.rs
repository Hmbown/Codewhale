//! Engine-owned compaction lifecycle, recovery and checkpoint installation.
//! Automatic, manual and emergency paths share the existing session and event authority.

use super::*;

impl Engine {
    pub(super) async fn emit_compaction_started(
        &mut self,
        id: String,
        auto: bool,
        message: String,
    ) {
        let _ = self
            .tx_event
            .send(Event::CompactionStarted { id, auto, message })
            .await;
    }

    pub(super) async fn emit_compaction_completed(
        &mut self,
        id: String,
        auto: bool,
        message: String,
        messages_before: Option<usize>,
        messages_after: Option<usize>,
    ) {
        let summary_prompt = self.rendered_compaction_summary();
        // Every call site runs after message replacement and checkpoint
        // commit. Reuse the same complete estimate as context pressure.
        let post_input_tokens = Some(self.estimated_input_tokens() as u64);
        let _ = self
            .tx_event
            .send(Event::CompactionCompleted {
                id,
                auto,
                message,
                messages_before,
                messages_after,
                summary_prompt,
                post_input_tokens,
            })
            .await;
    }

    pub(super) async fn emit_compaction_cancelled(
        &mut self,
        id: String,
        auto: bool,
        message: String,
    ) {
        let _ = self
            .tx_event
            .send(Event::CompactionCancelled { id, auto, message })
            .await;
    }

    /// Render the accumulated compaction summary prompt to plain text so it
    /// can travel in events and be persisted by host layers. All emit sites
    /// run after `commit_compaction_checkpoint`, so this reflects the checkpoint
    /// state the engine will use for subsequent requests.
    pub(super) fn rendered_compaction_summary(&self) -> Option<String> {
        self.session
            .compaction_summary_prompt
            .as_ref()
            .map(|prompt| match prompt {
                SystemPrompt::Text(text) => text.clone(),
                SystemPrompt::Blocks(blocks) => blocks
                    .iter()
                    .map(|block| block.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            })
            .filter(|text| !text.trim().is_empty())
    }

    pub(super) async fn emit_compaction_failed(&mut self, id: String, auto: bool, message: String) {
        let _ = self
            .tx_event
            .send(Event::CompactionFailed { id, auto, message })
            .await;
    }

    pub(super) fn claim_compaction(&self, id: &str) -> Option<CancellationToken> {
        self.compaction_cancellation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .claim(id)
    }

    pub(super) fn finish_compaction(&self, id: &str) {
        self.compaction_cancellation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .finish(id);
    }

    /// Pressure and effective trigger in append-only turn metadata. Numeric
    /// estimates never modify the session-pinned system/tool prefix.
    pub(super) fn context_pressure_line(
        &self,
        current_text: &str,
        prompt_context: &NextTurnPromptContext,
        system_prompt: Option<&SystemPrompt>,
    ) -> Option<String> {
        // The engine owns automatic compaction. Asking the model to warn the
        // user here created a competing save/compact ceremony before the
        // automatic request-boundary guard could do its work (#5620).
        if self.config.compaction.enabled {
            return None;
        }
        let input_tokens = self.active_input_tokens_with_current_text(current_text, system_prompt);
        let budget = route_context_budget_for_route(
            prompt_context.provider,
            &prompt_context.model,
            prompt_context.route_limits,
            input_tokens,
        )?;
        context_pressure_message(budget.usage_percent()).map(|warning| format!(
            "{warning}. Estimated input: {input_tokens} tokens ({:.1}% of route budget). Automatic compaction is explicitly disabled for this session. A manual /compact saves the original conversation and its model-written handoff before replacing context.",
            budget.usage_percent(),
        ))
    }

    pub(super) fn prepare_compaction_envelope(
        &self,
        mut config: CompactionConfig,
    ) -> PreparedCompactionEnvelope {
        // Host-supplied configs may not carry the workspace; compaction needs
        // it only to re-state the user's `/anchor` file after the summary.
        config
            .workspace
            .get_or_insert_with(|| self.config.workspace.clone());
        let mut prepared = PreparedCompactionEnvelope::new(config);
        prepared.session_id = Some(self.session.id.clone());
        prepared
    }

    pub(super) async fn handle_manual_compaction_op(
        &mut self,
        id: String,
        route: ResolvedRuntimeRoute,
        compaction: CompactionConfig,
    ) {
        self.emit_compaction_started(
            id.clone(),
            false,
            "Manual context compaction started".to_string(),
        )
        .await;
        let Some(cancel_token) = self.claim_compaction(&id) else {
            let message = "Context compaction canceled before it started".to_string();
            self.emit_compaction_cancelled(id, false, message).await;
            let _ = self
                .tx_event
                .send(Event::TurnComplete {
                    usage: Usage::default(),
                    parent_route_usage: Usage::default(),
                    routed_usage_dropped_records: 0,
                    status: TurnOutcomeStatus::Interrupted,
                    error: None,
                    tool_catalog: None,
                    base_url: None,
                })
                .await;
            return;
        };
        if let Err(err) = self.install_resolved_runtime_route(route) {
            let message =
                format!("Cannot compact context because its provider route is not ready: {err}");
            self.finish_compaction(&id);
            self.emit_compaction_failed(id, false, message.clone())
                .await;
            let _ = self
                .tx_event
                .send(Event::error(ErrorEnvelope::fatal_auth(message)))
                .await;
            return;
        }
        self.config.compaction = compaction;
        self.handle_manual_compaction(id, cancel_token).await;
    }

    pub(super) async fn emit_compaction_usage(&self, usage: &Usage, elapsed: Duration) {
        if *usage == Usage::default() {
            return;
        }
        let _ = self
            .tx_event
            .send(Event::RoutedTurnUsage {
                usage: usage.clone(),
                duration_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
                first_token_ms: None,
                request_ms: None,
            })
            .await;
    }

    pub(super) async fn handle_manual_compaction(
        &mut self,
        id: String,
        cancel_token: CancellationToken,
    ) {
        let zero_usage = Usage {
            input_tokens: 0,
            output_tokens: 0,
            ..Usage::default()
        };
        let Some(client) = self.deepseek_client.clone() else {
            let message = "Manual compaction unavailable: API client not configured".to_string();
            self.finish_compaction(&id);
            self.emit_compaction_failed(id, false, message.clone())
                .await;
            let _ = self
                .tx_event
                .send(Event::error(ErrorEnvelope::fatal_auth(message.clone())))
                .await;
            let _ = self
                .tx_event
                .send(Event::TurnComplete {
                    usage: zero_usage,
                    parent_route_usage: Usage::default(),
                    routed_usage_dropped_records: 0,
                    status: TurnOutcomeStatus::Failed,
                    error: Some(message),
                    tool_catalog: None,
                    base_url: None,
                })
                .await;
            return;
        };

        let messages_before = self.session.messages.len();
        // Message counts alone do not show the win the user cares about: a
        // compaction that drops few but enormous messages reads as a no-op.
        // The emergency path already reports tokens; manual and auto now match.
        let tokens_before = self.estimated_input_tokens();
        let mut turn_status = TurnOutcomeStatus::Completed;
        let mut turn_error = None;

        let prepared = self.prepare_compaction_envelope(self.config.compaction.clone());

        let started = Instant::now();
        let mut compaction_usage = Usage::default();
        let compaction_result = tokio::select! {
            biased;
            _ = cancel_token.cancelled() => None,
            result = compact_messages_safe(
                &client,
                &self.session.messages,
                self.session.system_prompt.as_ref(),
                &prepared,
                &mut compaction_usage,
            ) => Some(result),
        };
        self.session.total_usage.add(&compaction_usage);
        self.record_goal_usage_for_turn(&compaction_usage, started.elapsed());
        self.emit_compaction_usage(&compaction_usage, started.elapsed())
            .await;

        let Some(compaction_result) = compaction_result else {
            self.finish_compaction(&id);
            self.emit_compaction_cancelled(
                id,
                false,
                "Context compaction canceled; conversation context was not changed".to_string(),
            )
            .await;
            let _ = self
                .tx_event
                .send(Event::TurnComplete {
                    usage: compaction_usage,
                    parent_route_usage: Usage::default(),
                    routed_usage_dropped_records: 0,
                    status: TurnOutcomeStatus::Interrupted,
                    error: None,
                    tool_catalog: None,
                    base_url: None,
                })
                .await;
            return;
        };

        match compaction_result {
            Ok(mut result) => {
                if !result.messages.is_empty() || self.session.messages.is_empty() {
                    self.append_compaction_agent_topology(&mut result.messages)
                        .await;
                    if cancel_token.is_cancelled() {
                        self.finish_compaction(&id);
                        self.emit_compaction_cancelled(
                            id,
                            false,
                            "Context compaction canceled; conversation context was not changed"
                                .to_string(),
                        )
                        .await;
                        let _ = self
                            .tx_event
                            .send(Event::TurnComplete {
                                usage: compaction_usage,
                                parent_route_usage: Usage::default(),
                                routed_usage_dropped_records: 0,
                                status: TurnOutcomeStatus::Interrupted,
                                error: None,
                                tool_catalog: None,
                                base_url: None,
                            })
                            .await;
                        return;
                    }
                    let messages_after = result.messages.len();
                    let retries_used = result.retries_used;
                    let coverage_clause = result.coverage.receipt_clause();
                    self.session.replace_messages(result.messages);
                    if let Some(pm) = self.session.prefix_stability.as_mut() {
                        pm.note_history_reset("compaction");
                    }
                    self.commit_compaction_checkpoint(result.summary_prompt);
                    self.emit_session_updated().await;
                    let removed = messages_before.saturating_sub(messages_after);
                    let tokens_after = self.estimated_input_tokens();
                    let message = if retries_used > 0 {
                        format!(
                            "Compaction complete: {messages_before} → {messages_after} messages ({removed} removed, {retries_used} retries), ~{tokens_before} → ~{tokens_after} tokens ({coverage_clause})"
                        )
                    } else {
                        format!(
                            "Compaction complete: {messages_before} → {messages_after} messages ({removed} removed), ~{tokens_before} → ~{tokens_after} tokens ({coverage_clause})"
                        )
                    };
                    self.emit_compaction_completed(
                        id.clone(),
                        false,
                        message,
                        Some(messages_before),
                        Some(messages_after),
                    )
                    .await;
                } else {
                    let message = "Compaction skipped: produced empty result".to_string();
                    self.emit_compaction_failed(id.clone(), false, message.clone())
                        .await;
                    turn_status = TurnOutcomeStatus::Failed;
                    turn_error = Some(message);
                }
            }
            Err(err) => {
                let message = crate::compaction::report_compaction_failure(
                    "Manual context compaction failed",
                    &id,
                    false,
                    &err,
                );
                self.emit_compaction_failed(id.clone(), false, message.clone())
                    .await;
                let _ = self.tx_event.send(Event::status(message.clone())).await;
                turn_status = TurnOutcomeStatus::Failed;
                turn_error = Some(message);
            }
        }

        self.finish_compaction(&id);

        let _ = self
            .tx_event
            .send(Event::TurnComplete {
                usage: compaction_usage,
                parent_route_usage: Usage::default(),
                routed_usage_dropped_records: 0,
                status: turn_status,
                error: turn_error,
                tool_catalog: None,
                base_url: None,
            })
            .await;
    }

    pub(super) async fn recover_context_overflow(
        &mut self,
        client: &dyn crate::core::model_client::ModelClient,
        tools: Option<&[Tool]>,
        reason: &str,
        turn: &mut TurnContext,
    ) -> bool {
        let Some(target_budget) = context_input_budget_for_route(
            self.api_provider,
            &self.session.model,
            self.active_route_limits,
            0,
        ) else {
            return false;
        };

        let id = format!("compact_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        turn.stop_diagnostics.emergency_compaction_attempts = turn
            .stop_diagnostics
            .emergency_compaction_attempts
            .saturating_add(1);
        let start_message = format!("Emergency context compaction started ({reason})");
        self.emit_compaction_started(id.clone(), true, start_message)
            .await;
        let Some(compaction_cancel) = self.claim_compaction(&id) else {
            self.emit_compaction_cancelled(
                id,
                true,
                "Emergency context compaction canceled before it started; conversation context was not changed"
                    .to_string(),
            )
            .await;
            return false;
        };
        let turn_cancel = self.cancel_token.clone();

        let before_tokens = self.estimated_input_tokens();
        let before_count = self.session.messages.len();

        let mut forced_config = self.config.compaction.clone();
        forced_config.enabled = true;
        forced_config.token_threshold = forced_config
            .token_threshold
            .min(target_budget.saturating_sub(1))
            .max(1);
        let mut prepared = self.prepare_compaction_envelope(forced_config);
        prepared.tools = tools.map(<[Tool]>::to_vec);

        let started = Instant::now();
        let mut compaction_usage = Usage::default();
        let (compaction_result, turn_was_canceled) = tokio::select! {
            biased;
            _ = turn_cancel.cancelled() => (None, true),
            _ = compaction_cancel.cancelled() => (None, false),
            result = compact_messages_safe(
                client,
                &self.session.messages,
                self.session.system_prompt.as_ref(),
                &prepared,
                &mut compaction_usage,
            ) => (Some(result), false),
        };
        turn.add_usage(&compaction_usage);
        self.emit_compaction_usage(&compaction_usage, started.elapsed())
            .await;
        let Some(compaction_result) = compaction_result else {
            self.finish_compaction(&id);
            let message = if turn_was_canceled {
                "Emergency context compaction canceled with the active turn; conversation context was not changed"
            } else {
                "Emergency context compaction canceled; conversation context was not changed"
            }
            .to_string();
            self.emit_compaction_cancelled(id, true, message).await;
            return false;
        };

        let result = match compaction_result {
            Ok(result) => result,
            Err(err) => {
                let message =
                    format!("Context recovery failed: {err}. Original conversation was preserved.");
                self.emit_compaction_failed(id.clone(), true, message).await;
                self.finish_compaction(&id);
                return false;
            }
        };
        let retries_used = result.retries_used;
        let summary_prompt = result.summary_prompt;
        let mut compacted_messages = result.messages;

        let turn_was_canceled = turn_cancel.is_cancelled();
        if turn_was_canceled || compaction_cancel.is_cancelled() {
            self.finish_compaction(&id);
            let message = if turn_was_canceled {
                "Emergency context compaction canceled with the active turn; conversation context was not changed"
            } else {
                "Emergency context compaction canceled; conversation context was not changed"
            }
            .to_string();
            self.emit_compaction_cancelled(id, true, message).await;
            return false;
        }

        if !compacted_messages.is_empty() || self.session.messages.is_empty() {
            self.append_compaction_agent_topology(&mut compacted_messages)
                .await;
            let turn_was_canceled = turn_cancel.is_cancelled();
            if turn_was_canceled || compaction_cancel.is_cancelled() {
                self.finish_compaction(&id);
                let message = if turn_was_canceled {
                    "Emergency context compaction canceled with the active turn; conversation context was not changed"
                } else {
                    "Emergency context compaction canceled; conversation context was not changed"
                }
                .to_string();
                self.emit_compaction_cancelled(id, true, message).await;
                return false;
            }
        }
        // Validate the complete candidate before the only history swap. Bare
        // front-trimming after a failed summary silently lost user state and
        // could leave orphan tool results in an apparently recovered session.
        let after_tokens = crate::compaction::estimate_input_tokens_for_pressure(
            &compacted_messages,
            self.session.system_prompt.as_ref(),
        );
        let after_count = compacted_messages.len();
        let recovered = after_tokens <= target_budget && after_tokens < before_tokens;

        if recovered {
            self.session.replace_messages(compacted_messages);
            turn.clear_parent_input_tokens();
            if let Some(pm) = self.session.prefix_stability.as_mut() {
                pm.note_history_reset("compaction");
            }
            self.commit_compaction_checkpoint(summary_prompt);
            self.emit_session_updated().await;
            let removed = before_count.saturating_sub(after_count);
            let mut details = format!(
                "Emergency compaction complete: {before_count} → {after_count} messages ({removed} removed), ~{before_tokens} → ~{after_tokens} tokens"
            );
            if retries_used > 0 {
                details.push_str(&format!(" ({retries_used} retries)"));
            }
            self.emit_compaction_completed(
                id.clone(),
                true,
                details.clone(),
                Some(before_count),
                Some(after_count),
            )
            .await;
            let _ = self.tx_event.send(Event::status(details)).await;
            self.finish_compaction(&id);
            return true;
        }

        // Two distinct failures were previously conflated into one banner.
        // When the provider rejected the request (its bill counts framing we
        // cannot see), our estimate may already sit within the budget while
        // the pass removed nothing — reporting that as "failed to reduce
        // below model limit" with an estimate printed *under* the budget
        // reads as self-contradictory. Name the actual outcome instead.
        let message = if after_tokens > target_budget {
            format!(
                "Emergency context compaction failed to reduce request below model limit \
                 (estimate ~{after_tokens} tokens, budget ~{target_budget}). Original conversation was preserved."
            )
        } else {
            format!(
                "Emergency context compaction made no progress (estimate ~{after_tokens} tokens \
                 is already within the ~{target_budget} budget; the provider may count the \
                 request differently). Original conversation was preserved."
            )
        };
        self.emit_compaction_failed(id.clone(), true, message.clone())
            .await;
        let _ = self.tx_event.send(Event::status(message)).await;
        self.finish_compaction(&id);
        false
    }

    /// Keep the rendered checkpoint for host persistence and repeat-compaction
    /// metadata. The model sees the checkpoint exactly once through ordinary
    /// conversation history; the stable system prefix never carries it.
    pub(super) fn commit_compaction_checkpoint(&mut self, summary_prompt: Option<SystemPrompt>) {
        let Some(summary_prompt) = summary_prompt else {
            return;
        };
        self.session.compaction_summary_prompt = Some(summary_prompt);
    }

    /// Capture the current session-owned Agent topology at the replacement
    /// history boundary. This is the Codewhale equivalent of Codex clearing
    /// its world-state reference after standalone compaction so the next turn
    /// receives fresh environment/subagent context instead of trusting the
    /// narrative summary as live process state.
    pub(super) async fn append_compaction_agent_topology(&self, messages: &mut Vec<Message>) {
        let snapshots = {
            let manager = self.subagent_manager.read().await;
            manager.list_for_session(&self.session.id)
        };
        crate::runtime_handoff::replace_agent_topology_checkpoint(messages, &snapshots);
    }
}
