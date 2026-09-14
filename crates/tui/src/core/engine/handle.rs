//! Public `EngineHandle` methods.
//!
//! The struct itself lives next door in `engine.rs` because two
//! construction sites (`Engine::new` and the test-only
//! `mock_engine_handle`) need access to its private mpsc channels.
//! The method surface — `send`, `cancel*`, `is_cancelled`,
//! `approve_tool_call` / `deny_tool_call` / `retry_tool_with_policy`,
//! `submit_user_input` / `cancel_user_input`, and `steer` — moves here
//! so the agent loop's mailbox API is reviewable on its own.

use anyhow::Result;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use codewhale_config::AppMode;
use codewhale_execpolicy::ApprovalMode;

use super::approval::{ApprovalDecision, UserInputDecision};
use super::{
    CancelReason, EngineHandle, LiveRuntimeAuthority, Op, RuntimePermissionAuthority,
    UserInputResponse,
};

#[derive(Clone)]
pub(super) struct TurnControl {
    pub id: u64,
    pub cancel: CancellationToken,
    pub reason: Arc<StdMutex<Option<CancelReason>>>,
}

#[derive(Default)]
pub(super) struct TurnControls {
    next_id: u64,
    pub active: Option<TurnControl>,
    pub pending: VecDeque<TurnControl>,
}

impl TurnControls {
    pub fn fresh(&mut self) -> TurnControl {
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("turn control id exhausted");
        TurnControl {
            id: self.next_id,
            cancel: CancellationToken::new(),
            reason: Arc::new(StdMutex::new(None)),
        }
    }

    fn target(&self) -> Option<&TurnControl> {
        self.active.as_ref().or_else(|| self.pending.front())
    }
}

pub(super) struct TurnControlGuard {
    pub controls: Arc<StdMutex<TurnControls>>,
    pub id: u64,
}

impl Drop for TurnControlGuard {
    fn drop(&mut self) {
        let mut controls = self
            .controls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if controls
            .active
            .as_ref()
            .is_some_and(|active| active.id == self.id)
        {
            controls.active = None;
        }
    }
}

#[derive(Debug)]
pub(crate) struct SteerInput {
    pub(super) turn_id: Option<u64>,
    pub(crate) content: String,
}

impl std::ops::Deref for SteerInput {
    type Target = str;
    fn deref(&self) -> &str {
        &self.content
    }
}

impl std::fmt::Display for SteerInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.content.fmt(f)
    }
}

pub(crate) struct SteerPermit {
    permit: mpsc::OwnedPermit<SteerInput>,
    turn_id: Option<u64>,
}

impl SteerPermit {
    pub(crate) fn send(self, content: String) {
        self.permit.send(SteerInput {
            turn_id: self.turn_id,
            content,
        });
    }
}

impl EngineHandle {
    /// Called only while Runtime holds the idle turn admission claim. The
    /// following SendMessage refreshes the existing prompt/config projection.
    pub(crate) fn restore_runtime_goal(
        &self,
        goal: Option<&codewhale_protocol::ThreadGoal>,
    ) -> Result<()> {
        let mut state = self
            .goal_state
            .lock()
            .map_err(|_| anyhow::anyhow!("goal state lock poisoned"))?;
        let current = state.snapshot();
        if current.goal_id.as_deref() != goal.map(|goal| goal.goal_id.as_str()) {
            *state = goal.map_or_else(crate::tools::goal::GoalState::default, |goal| {
                crate::tools::goal::GoalState::from_snapshot(
                    &crate::tools::goal::GoalSnapshot::from_thread_goal(goal),
                )
            });
        }
        Ok(())
    }

    /// True when the caller must preflight a concrete provider client before
    /// committing UI/runtime turn state. Test and embedding handles with an
    /// injected model client return false because that client owns model I/O.
    #[must_use]
    pub(crate) fn client_preflight_required(&self) -> bool {
        self.client_preflight_required
    }

    /// Send an operation to the engine
    pub async fn send(&self, op: Op) -> Result<()> {
        let authority = Self::change_mode_authority(&op);
        let permit = self.tx_op.clone().reserve_owned().await?;
        if let Some(authority) = authority {
            self.publish_runtime_authority(authority);
        }
        self.send_reserved_op(permit, op);
        Ok(())
    }

    /// Try to send an operation without blocking.
    ///
    /// Returns `Err` if the channel is full or closed.  Use this for
    /// non-critical, refresh-type ops (e.g. `Op::ListSubAgents`) that can
    /// safely be dropped and re-requested on the next drain cycle.
    pub fn try_send(&self, op: Op) -> Result<()> {
        let authority = Self::change_mode_authority(&op);
        let result = self.tx_op.clone().try_reserve_owned();
        // A full channel already guarantees that the engine will wake and
        // drain an operation. Publish the typed authority anyway: the drain
        // applies pending authority before handling that queued operation, so
        // a posture edit never blocks behind refresh traffic. A closed
        // channel has no engine left to observe the update.
        if !matches!(&result, Err(mpsc::error::TrySendError::Closed(_)))
            && let Some(authority) = authority
        {
            self.publish_runtime_authority(authority);
        }
        // Keep the public error bound to the rejected operation. Callers use
        // TrySendError<Op> to distinguish a retryable full mailbox from a
        // stopped engine; reservation errors otherwise carry a Sender<Op>.
        match result {
            Ok(permit) => {
                self.send_reserved_op(permit, op);
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                Err(mpsc::error::TrySendError::Full(op).into())
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Err(mpsc::error::TrySendError::Closed(op).into())
            }
        }
    }

    /// Bind controls and enqueue under one lock, preserving the same FIFO as
    /// the operation mailbox even when several senders hold reserved slots.
    pub(crate) fn send_reserved_op(&self, permit: mpsc::OwnedPermit<Op>, op: Op) {
        let mut controls = self
            .turn_controls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if matches!(&op, Op::SendMessage { .. }) {
            let control = controls.fresh();
            controls.pending.push_back(control);
        }
        permit.send(op);
    }

    fn change_mode_authority(op: &Op) -> Option<LiveRuntimeAuthority> {
        let Op::ChangeMode {
            mode,
            allow_shell,
            trust_mode,
            auto_approve,
            approval_mode,
            configured_sandbox_mode,
        } = op
        else {
            return None;
        };
        Some(LiveRuntimeAuthority::from_fields(
            *mode,
            *allow_shell,
            *trust_mode,
            *auto_approve,
            *approval_mode,
            configured_sandbox_mode.clone(),
        ))
    }

    fn publish_runtime_authority(&self, authority: LiveRuntimeAuthority) {
        let mut state = self
            .live_runtime_authority
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.revision = state.revision.wrapping_add(1).max(1);
        state.authority = authority;
    }

    pub(crate) fn publish_turn_authority(
        &self,
        mode: AppMode,
        allow_shell: bool,
        trust_mode: bool,
        auto_approve: bool,
        approval_mode: ApprovalMode,
        configured_sandbox_mode: Option<String>,
    ) {
        self.publish_runtime_authority(LiveRuntimeAuthority::from_fields(
            mode,
            allow_shell,
            trust_mode,
            auto_approve,
            approval_mode,
            configured_sandbox_mode,
        ));
    }

    /// Exact live permission authority for runtime approval and elevation
    /// gates. This is the same typed state the active engine turn drains.
    #[must_use]
    pub(crate) fn runtime_permission_authority(&self) -> RuntimePermissionAuthority {
        self.live_runtime_authority
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .authority
            .permission_snapshot()
    }

    /// Reserve capacity for a runtime steer before it mutates durable state.
    /// The owned permit lets the caller persist and dispatch synchronously,
    /// without a cancellation point between those two operations.
    pub(crate) async fn reserve_steer(&self) -> Result<SteerPermit> {
        let permit = self.tx_steer.clone().reserve_owned().await?;
        let turn_id = self
            .turn_controls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .target()
            .map(|control| control.id);
        Ok(SteerPermit { permit, turn_id })
    }

    /// Cancel the current request (user-initiated path — keeps the
    /// public `cancel()` signature stable). Equivalent to
    /// `cancel_with_reason(CancelReason::User)`.
    pub fn cancel(&self) {
        self.cancel_with_reason(CancelReason::User);
    }

    /// Cancel the current request and latch the reason so downstream
    /// "request cancelled" error messages can name a cause.
    pub fn cancel_with_reason(&self, reason: CancelReason) {
        // Keep turn activation excluded until both the admitted control and
        // the legacy shared token have been canceled.
        let controls = self
            .turn_controls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(control) = controls.target() {
            *control
                .reason
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(reason);
            control.cancel.cancel();
        }
        match self.cancel_reason.lock() {
            Ok(mut slot) => *slot = Some(reason),
            Err(poisoned) => *poisoned.into_inner() = Some(reason),
        }
        match self.cancel_token.lock() {
            Ok(token) => token.cancel(),
            Err(poisoned) => poisoned.into_inner().cancel(),
        }
        crate::retry_status::clear();
    }

    /// Check if a request is currently cancelled
    #[must_use]
    #[allow(dead_code)]
    pub fn is_cancelled(&self) -> bool {
        if let Some(control) = self
            .turn_controls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .target()
        {
            return control.cancel.is_cancelled();
        }
        match self.cancel_token.lock() {
            Ok(token) => token.is_cancelled(),
            Err(poisoned) => poisoned.into_inner().is_cancelled(),
        }
    }

    /// Pause or resume the current pausable command.
    pub fn set_paused(&self, paused: bool) {
        match self.shared_paused.lock() {
            Ok(mut slot) => *slot = paused,
            Err(poisoned) => *poisoned.into_inner() = paused,
        }
    }

    /// Check whether the engine pause gate is set.
    #[cfg(test)]
    #[must_use]
    pub fn is_paused(&self) -> bool {
        match self.shared_paused.lock() {
            Ok(slot) => *slot,
            Err(poisoned) => *poisoned.into_inner(),
        }
    }

    /// Approve a pending tool call
    pub async fn approve_tool_call(&self, id: impl Into<String>) -> Result<()> {
        self.tx_approval
            .send(ApprovalDecision::Approved { id: id.into() })
            .await?;
        Ok(())
    }

    /// Deny a pending tool call
    pub async fn deny_tool_call(&self, id: impl Into<String>) -> Result<()> {
        self.tx_approval
            .send(ApprovalDecision::Denied { id: id.into() })
            .await?;
        Ok(())
    }

    /// Retry a tool call with an elevated sandbox policy.
    pub async fn retry_tool_with_policy(
        &self,
        id: impl Into<String>,
        policy: crate::sandbox::SandboxPolicy,
    ) -> Result<()> {
        self.tx_approval
            .send(ApprovalDecision::RetryWithPolicy {
                id: id.into(),
                policy,
            })
            .await?;
        Ok(())
    }

    /// Submit a response for request_user_input.
    pub async fn submit_user_input(
        &self,
        id: impl Into<String>,
        response: UserInputResponse,
    ) -> Result<()> {
        self.tx_user_input
            .send(UserInputDecision::Submitted {
                id: id.into(),
                response,
            })
            .await?;
        Ok(())
    }

    /// Cancel a request_user_input prompt.
    pub async fn cancel_user_input(&self, id: impl Into<String>) -> Result<()> {
        self.tx_user_input
            .send(UserInputDecision::Cancelled { id: id.into() })
            .await?;
        Ok(())
    }

    /// Steer an in-flight turn with additional user input.
    pub async fn steer(&self, content: impl Into<String>) -> Result<()> {
        self.reserve_steer().await?.send(content.into());
        Ok(())
    }

    /// Request a snapshot of the current session state.
    /// Returns the snapshot directly via a oneshot channel, avoiding
    /// competition with the SSE event stream on the mpsc receiver.
    pub async fn get_session_snapshot(&self) -> Result<crate::core::ops::SessionSnapshot> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));
        self.send(Op::GetSessionSnapshot { tx }).await?;
        rx.await
            .map_err(|_| anyhow::anyhow!("Engine dropped session snapshot oneshot"))
    }

    /// Query after the active turn settles, without competing with events.
    /// The caller must keep draining events and bound this future: an active
    /// turn can be awaiting provider/tool work or a full event channel.
    pub(crate) async fn get_subagent_settlement(
        &self,
    ) -> Result<crate::core::ops::SubAgentSettlement> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = Arc::new(StdMutex::new(Some(tx)));
        self.send(Op::GetSubAgentSettlement { tx }).await?;
        rx.await
            .map_err(|_| anyhow::anyhow!("Engine dropped child settlement receipt"))
    }

    /// Request active provider request concurrency state.
    pub async fn get_provider_runtime_status(
        &self,
    ) -> Result<crate::core::ops::ProviderRuntimeStatus> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));
        self.send(Op::GetProviderRuntimeStatus { tx }).await?;
        rx.await
            .map_err(|_| anyhow::anyhow!("Engine dropped provider runtime status oneshot"))
    }

    /// Run the bounded initial connection pass on the engine-owned MCP pool.
    ///
    /// The returned manager snapshot and every later tool call therefore see
    /// the same connections and catalog generation. Unlike `reload_mcp`, this
    /// does not force a config re-read or drop ready transports. Optional
    /// servers are connected in the background at engine spawn; this waits
    /// only if the caller explicitly asked for the settled receipt.
    pub async fn bootstrap_mcp(&self) -> Result<crate::core::ops::McpManagerUpdate> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));
        self.send(Op::BootstrapMcp { tx }).await?;
        rx.await
            .map_err(|_| anyhow::anyhow!("Engine dropped MCP bootstrap oneshot"))?
            .map_err(anyhow::Error::msg)
    }

    /// Retry one failed server through the existing engine-owned pool.
    pub async fn retry_mcp_server(
        &self,
        name: impl Into<String>,
    ) -> Result<crate::core::ops::McpManagerUpdate> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));
        self.send(Op::RetryMcpServer {
            name: name.into(),
            tx,
        })
        .await?;
        rx.await
            .map_err(|_| anyhow::anyhow!("Engine dropped MCP retry oneshot"))?
            .map_err(anyhow::Error::msg)
    }

    /// Force the engine-owned MCP pool to reload and reconnect, returning a
    /// snapshot from the exact live pool that supplies the next model turn.
    pub async fn reload_mcp(
        &self,
        config_path: std::path::PathBuf,
    ) -> Result<crate::core::ops::McpManagerUpdate> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));
        self.send(Op::ReloadMcp { config_path, tx }).await?;
        rx.await
            .map_err(|_| anyhow::anyhow!("Engine dropped MCP reload oneshot"))?
            .map_err(anyhow::Error::msg)
    }
}
