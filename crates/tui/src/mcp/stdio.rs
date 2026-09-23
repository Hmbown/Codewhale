use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::Mutex as TokioMutex;

use super::wire::MAX_MCP_RESPONSE_BYTES;
use super::{McpServerConfig, McpTransport};
use crate::child_env;

pub(super) struct StdioTransport {
    pub(super) child: Arc<TokioMutex<Child>>,
    pub(super) stdin: ChildStdin,
    pub(super) reader: tokio::io::BufReader<ChildStdout>,
    /// Partial frame bytes survive cancellation of the receive future.
    pub(super) pending_line: Vec<u8>,
    /// Tail of stderr lines from the spawned MCP server. A background task
    /// drains the child's stderr into this buffer so a mid-run crash leaves
    /// some context behind instead of `Stdio::null` swallowing it.
    pub(super) stderr_tail: Arc<StderrTail>,
    /// Plugin authority can change in another process while this child is
    /// idle. The connection-level cancellation token therefore also owns a
    /// process watcher instead of waiting for a later tool call to drop the
    /// transport.
    pub(super) authority_cancel_watch: Option<tokio::task::JoinHandle<()>>,
    /// Holds reviewed executable/script handles for the process lifetime.
    pub(super) _reviewed_launch: Option<super::ReviewedStdioLaunch>,
}

/// How long `StdioTransport::shutdown` waits for the child to exit on SIGTERM
/// before `kill_on_drop` fires SIGKILL. Tuned short so a hung MCP server
/// can't stall TUI exit; well-behaved servers almost always exit within
/// a few hundred ms.
pub(super) const STDIO_SHUTDOWN_GRACE: Duration = Duration::from_millis(2_000);

/// How many lines of MCP-server stderr to keep around for crash diagnostics.
/// Bounded so a chatty server can't grow this without limit; large enough to
/// catch typical Node/Python startup or panic output.
const STDERR_TAIL_CAPACITY: usize = 64;

/// Bounded ring buffer for the most recent stderr lines from a spawned MCP
/// server. Used by `StdioTransport` to surface server-side context when the
/// transport read side fails (server crashed, exited early, etc).
#[derive(Default)]
pub(super) struct StderrTail {
    lines: TokioMutex<VecDeque<String>>,
}

impl StderrTail {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self {
            lines: TokioMutex::new(VecDeque::with_capacity(STDERR_TAIL_CAPACITY)),
        })
    }

    pub(super) async fn push(&self, line: String) {
        let mut buf = self.lines.lock().await;
        if buf.len() >= STDERR_TAIL_CAPACITY {
            buf.pop_front();
        }
        buf.push_back(line);
    }

    async fn snapshot(&self) -> Vec<String> {
        self.lines.lock().await.iter().cloned().collect()
    }
}

impl StdioTransport {
    pub(super) fn spawn(
        server_name: &str,
        command: &str,
        config: &McpServerConfig,
        cancel_token: tokio_util::sync::CancellationToken,
    ) -> Result<Self> {
        let reviewed_launch = if let Some(reviewed_plugin) = config.reviewed_plugin.as_ref() {
            // This is deliberately the last trust check before constructing
            // and spawning the lazy stdio child. It re-reads only the
            // Codewhale-owned plugin bundle, never user MCP/provider config or
            // credential files, and fails closed on any content/capability
            // drift after pool construction.
            Some(reviewed_plugin.prepare_stdio_launch(
                server_name,
                command,
                &config.args,
                config.cwd.as_deref(),
            )?)
        } else {
            None
        };
        let mut cmd = reviewed_launch.as_ref().map_or_else(
            || {
                let mut command_process = tokio::process::Command::new(command);
                command_process.args(&config.args);
                command_process
            },
            |launch| {
                let mut command_process = tokio::process::Command::new(&launch.command);
                command_process.args(&launch.args);
                command_process
            },
        );
        crate::utils::suppress_tokio_console_window(&mut cmd);
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        let launch_cwd = reviewed_launch
            .as_ref()
            .and_then(|launch| launch.cwd.as_ref())
            .or(config.cwd.as_ref().filter(|_| reviewed_launch.is_none()));
        if let Some(cwd) = launch_cwd {
            cmd.current_dir(cwd);
        }
        #[cfg(unix)]
        if let Some(cwd_fd) = reviewed_launch
            .as_ref()
            .and_then(|launch| launch.cwd_fd.as_ref())
        {
            use std::os::fd::AsRawFd as _;
            let fd = cwd_fd.as_raw_fd();
            // SAFETY: the closure calls only async-signal-safe `fchdir` on an
            // inherited directory descriptor before exec.
            unsafe {
                cmd.pre_exec(move || {
                    if libc::fchdir(fd) == 0 {
                        Ok(())
                    } else {
                        Err(std::io::Error::last_os_error())
                    }
                });
            }
        }

        // Expand `${NAME}` placeholders so secret env values can be sourced
        // from the process environment instead of being stored in cleartext
        // in the MCP config. The child env is allowlist-sanitized below, so
        // these vars would not otherwise be inherited by the child.
        let expanded_env = super::expanded_mcp_stdio_env(config)
            .with_context(|| format!("MCP server '{server_name}' env expansion failed"))?;

        // User-configured MCP keeps the compatibility-oriented Node/Python
        // bootstrap allowlist (#1244). Reviewed plugins receive only the base
        // secret-scrubbed child environment plus their explicitly reviewed
        // mappings, so namespaces such as NPM_CONFIG_* are never inherited
        // ambiently across the consent boundary.
        if let Some(reviewed_plugin) = config.reviewed_plugin.as_ref() {
            cmd.env_clear();
            for (key, value) in child_env::sanitized_plugin_mcp_env_from(
                reviewed_plugin.host_environment.entries().iter().cloned(),
                child_env::string_map_env(&expanded_env),
            ) {
                cmd.env(key, value);
            }
        } else {
            child_env::apply_to_tokio_command_mcp(
                &mut cmd,
                child_env::string_map_env(&expanded_env),
            );
        }

        let mut child = cmd.spawn().map_err(|error| {
            let message = if error.kind() == std::io::ErrorKind::NotFound
                && super::is_node_command(command)
                && launch_cwd.is_none_or(|directory| directory.is_dir())
            {
                format!("MCP server {server_name} could not start because Node.js was not found. Install Node.js from https://nodejs.org/ and restart Codewhale with node on PATH. Built-in Computer Use requires Node.js 20 or newer.")
            } else if config.reviewed_plugin.is_some() {
                format!(
                    "MCP stdio spawn failed (transport=stdio server={server_name} reviewed-plugin argv_count={} env_count={})",
                    config.args.len(),
                    expanded_env.len(),
                )
            } else {
                let env_keys: Vec<&str> = expanded_env.keys().map(String::as_str).collect();
                format!(
                    "MCP stdio spawn failed (transport=stdio server={server_name} cmd={command:?} args={:?} env_keys={env_keys:?})",
                    config.args,
                )
            };
            anyhow::Error::new(error).context(message)
        })?;

        let stdin = child.stdin.take().context("Failed to get MCP stdin")?;
        let stdout = child.stdout.take().context("Failed to get MCP stdout")?;
        let stderr = child.stderr.take().context("Failed to get MCP stderr")?;

        // Drain stderr into a bounded ring buffer so a crash mid-run leaves
        // diagnostic breadcrumbs instead of disappearing into `Stdio::null`.
        // The task exits naturally when the child closes its stderr
        // (kill_on_drop / exit / explicit shutdown).
        let stderr_tail = StderrTail::new();
        {
            let tail = Arc::clone(&stderr_tail);
            // A reviewed plugin child receives environment-backed values that
            // are intentionally absent from its manifest. Still drain its
            // stderr to avoid blocking, but do not retain or surface arbitrary
            // child output that could echo those credentials into a chat or
            // persisted transcript.
            let capture_lines = config.reviewed_plugin.is_none();
            tokio::spawn(async move {
                let mut lines = tokio::io::BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if capture_lines {
                        tail.push(line).await;
                    }
                }
            });
        }

        let child = Arc::new(TokioMutex::new(child));
        let authority_cancel_watch = config.reviewed_plugin.as_ref().map(|_| {
            let watched_child = Arc::clone(&child);
            tokio::spawn(async move {
                cancel_token.cancelled().await;
                terminate_child_for_authority_change(&watched_child).await;
            })
        });

        Ok(Self {
            child,
            stdin,
            reader: tokio::io::BufReader::new(stdout),
            pending_line: Vec::new(),
            stderr_tail,
            authority_cancel_watch,
            _reviewed_launch: reviewed_launch,
        })
    }
}

/// Format the captured stderr tail for inclusion in an error message. Empty
/// tails return `None` so the caller can fall back to its original message.
async fn format_stderr_context(tail: &StderrTail) -> Option<String> {
    let lines = tail.snapshot().await;
    if lines.is_empty() {
        return None;
    }
    Some(format!(
        "MCP server stderr (last {} line{}):\n{}",
        lines.len(),
        if lines.len() == 1 { "" } else { "s" },
        lines.join("\n"),
    ))
}

/// Best-effort SIGTERM. On Unix uses `libc::kill`; on Windows there's no
/// equivalent so we let `kill_on_drop` (TerminateProcess) handle it via the
/// subsequent Drop. Returns whether a signal was actually sent.
fn send_sigterm(child: &Child) -> bool {
    #[cfg(unix)]
    {
        if let Some(pid) = child.id() {
            // SAFETY: pid was just obtained from `child.id()`. `libc::kill`
            // with `SIGTERM` is async-signal-safe and never observes invalid
            // memory. Worst case (pid wrap / process already gone) returns
            // ESRCH, which we deliberately ignore.
            unsafe {
                let _ = libc::kill(pid as i32, libc::SIGTERM);
            }
            return true;
        }
        false
    }
    #[cfg(not(unix))]
    {
        let _ = child;
        false
    }
}

async fn terminate_child_for_authority_change(child: &Arc<TokioMutex<Child>>) {
    let mut child = child.lock().await;
    terminate_child(&mut child).await;
}

async fn terminate_child(child: &mut Child) {
    // Reap an already-exited child before resolving its PID. Until it is
    // reaped, the OS cannot recycle that identity; after it is reaped there is
    // nothing left to signal. This avoids a PID-only watcher ever targeting an
    // unrelated process after rapid PID reuse.
    if child.try_wait().is_ok_and(|status| status.is_some()) {
        return;
    }

    #[cfg(unix)]
    send_sigterm(child);

    #[cfg(not(unix))]
    let _ = child.start_kill();

    match tokio::time::timeout(STDIO_SHUTDOWN_GRACE, child.wait()).await {
        Ok(Ok(_)) => {}
        Ok(Err(_)) | Err(_) => {
            // SIGTERM is advisory. Revocation and explicit shutdown must not
            // leave the reviewed child alive indefinitely.
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
    }
}

#[async_trait::async_trait]
impl McpTransport for StdioTransport {
    async fn send(&mut self, mut msg: Vec<u8>) -> Result<()> {
        msg.push(b'\n');
        self.stdin.write_all(&msg).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    /// Non-blocking liveness probe: a reaped child means the transport is
    /// dead even though the `Ready` flag is still set (#6187). The sync
    /// trait contract forbids awaiting the lock, so a contended lock reads
    /// as alive — the read side observes the death on the next call.
    fn probe_dead(&self) -> bool {
        match self.child.try_lock() {
            Ok(mut child) => matches!(child.try_wait(), Ok(Some(_))),
            Err(_) => false,
        }
    }

    async fn recv(&mut self) -> Result<Vec<u8>> {
        loop {
            // Bounded read: a server emitting a newline-free multi-GB "line"
            // must not OOM us (read_line is unbounded).
            let bytes = match read_line_capped(
                &mut self.reader,
                &mut self.pending_line,
                MAX_MCP_RESPONSE_BYTES,
            )
            .await
            {
                Ok(b) => b,
                Err(err) => {
                    if let Some(stderr) = format_stderr_context(&self.stderr_tail).await {
                        anyhow::bail!("Stdio transport read error: {err}\n{stderr}");
                    }
                    return Err(err.into());
                }
            };
            if bytes == 0 {
                // Let the stderr drain task catch up before snapshotting, and
                // name the exit status: a reviewed plugin's stderr is never
                // retained, so the status is the only reason the operator
                // gets when the child dies before the handshake (#5916).
                tokio::task::yield_now().await;
                let exit = self.child.lock().await.try_wait().ok().flatten();
                let exit = exit.map_or_else(String::new, |status| format!(" ({status})"));
                if let Some(stderr) = format_stderr_context(&self.stderr_tail).await {
                    anyhow::bail!("Stdio transport closed{exit}\n{stderr}");
                }
                anyhow::bail!("Stdio transport closed{exit}");
            }

            let line_bytes = std::mem::take(&mut self.pending_line);
            let line = String::from_utf8_lossy(&line_bytes);
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            return Ok(trimmed.as_bytes().to_vec());
        }
    }

    /// Send SIGTERM and wait up to `STDIO_SHUTDOWN_GRACE` for graceful exit,
    /// then force termination and reap the child as the backstop.
    async fn shutdown(&mut self) {
        let mut child = self.child.lock().await;
        terminate_child(&mut child).await;
    }
}

/// Session changes can drop a pool without explicitly awaiting shutdown.
/// Keep the owned child alive for the same bounded cleanup as explicit
/// shutdown, so servers can release input and recording resources. Runtime
/// teardown still drops the cleanup future and invokes `kill_on_drop`.
impl Drop for StdioTransport {
    fn drop(&mut self) {
        if let Some(watch) = self.authority_cancel_watch.take() {
            watch.abort();
        }
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let child = Arc::clone(&self.child);
            let reviewed_launch = self._reviewed_launch.take();
            runtime.spawn(async move {
                let _reviewed_launch = reviewed_launch;
                let mut child = child.lock().await;
                terminate_child(&mut child).await;
            });
            return;
        }
        if let Ok(mut child) = self.child.try_lock()
            && !child.try_wait().is_ok_and(|status| status.is_some())
        {
            send_sigterm(&child);
        }
    }
}

/// Continue one newline-terminated line in caller-owned `out`, aborting if it
/// exceeds `max` bytes. Cancellation retains consumed bytes; the caller clears
/// the buffer only after receiving a complete frame. Returns the total bytes
/// accumulated; 0 means EOF.
async fn read_line_capped<R>(
    reader: &mut R,
    out: &mut Vec<u8>,
    max: usize,
) -> std::io::Result<usize>
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    use tokio::io::AsyncBufReadExt;
    loop {
        let (chunk, consumed, done) = {
            let available = reader.fill_buf().await?;
            if available.is_empty() {
                (Vec::new(), 0usize, true)
            } else if let Some(pos) = available.iter().position(|&b| b == b'\n') {
                (available[..=pos].to_vec(), pos + 1, true)
            } else {
                (available.to_vec(), available.len(), false)
            }
        };
        if consumed > 0 {
            reader.consume(consumed);
        }
        out.extend_from_slice(&chunk);
        if out.len() > max {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("MCP stdio line exceeded {max} bytes"),
            ));
        }
        if done {
            break;
        }
    }
    Ok(out.len())
}

#[cfg(test)]
mod read_cap_tests {
    use super::read_line_capped;

    #[tokio::test]
    async fn cancelled_partial_read_preserves_next_frame() {
        use futures_util::FutureExt;
        use tokio::io::AsyncWriteExt;
        let (mut writer, reader) = tokio::io::duplex(4096);
        let mut reader = tokio::io::BufReader::new(reader);
        let prefix = br#"{"jsonrpc":"2.0","id":"1","result":"#;
        writer.write_all(prefix).await.unwrap();
        let mut pending = Vec::new();
        // Poll through the consumed prefix to Pending, then drop the future.
        assert!(
            read_line_capped(&mut reader, &mut pending, 1024)
                .now_or_never()
                .is_none()
        );
        assert_eq!(pending, prefix);
        writer.write_all(b"null}\n").await.unwrap();
        read_line_capped(&mut reader, &mut pending, 1024)
            .await
            .unwrap();
        let first: serde_json::Value =
            serde_json::from_slice(&std::mem::take(&mut pending)).unwrap();
        assert_eq!(first["id"], "1");
        writer
            .write_all(b"{\"id\":\"2\",\"result\":true}\n")
            .await
            .unwrap();
        read_line_capped(&mut reader, &mut pending, 1024)
            .await
            .unwrap();
        let second: serde_json::Value = serde_json::from_slice(&pending).unwrap();
        assert_eq!(second["id"], "2");
        assert_eq!(second["result"], true);
    }

    #[tokio::test]
    async fn resumed_frame_still_enforces_cap_at_newline() {
        use futures_util::FutureExt;
        use tokio::io::AsyncWriteExt;
        let (mut writer, reader) = tokio::io::duplex(4096);
        let mut reader = tokio::io::BufReader::new(reader);
        let mut pending = Vec::new();
        writer.write_all(b"1234").await.unwrap();
        assert!(
            read_line_capped(&mut reader, &mut pending, 6)
                .now_or_never()
                .is_none()
        );
        writer.write_all(b"567\n").await.unwrap();
        assert_eq!(
            read_line_capped(&mut reader, &mut pending, 6)
                .await
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::InvalidData
        );
    }

    #[tokio::test]
    async fn reads_a_line_and_reports_eof() {
        let data = b"hello\nworld\n".to_vec();
        let mut reader = tokio::io::BufReader::new(std::io::Cursor::new(data));
        let mut out = Vec::new();
        assert_eq!(
            read_line_capped(&mut reader, &mut out, 1024).await.unwrap(),
            6
        );
        assert_eq!(out, b"hello\n");
        out.clear();
        assert_eq!(
            read_line_capped(&mut reader, &mut out, 1024).await.unwrap(),
            6
        );
        assert_eq!(out, b"world\n");
        out.clear();
        // EOF.
        assert_eq!(
            read_line_capped(&mut reader, &mut out, 1024).await.unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn aborts_on_newline_free_line_over_cap() {
        let data = vec![b'x'; 4096]; // no newline
        let mut reader = tokio::io::BufReader::new(std::io::Cursor::new(data));
        let mut out = Vec::new();
        let err = read_line_capped(&mut reader, &mut out, 1024)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }
}
