//! Authenticated local control attachment to the already held Runtime owner.
//! The private receipt routes; the connected kernel process authenticates it.
//! This forwards the existing bounded dispatcher protocol, never HTTP bearers.

use anyhow::Result;
use std::path::PathBuf;

#[cfg(any(unix, windows))]
mod platform {
    use super::*;
    use crate::daemon_socket::AuthorizedPeer;
    use crate::daemon_socket::{default_socket_path, owner_work};
    use crate::{BoundedLines, ParsedStdioLine, parse_stdio_line, write_stdio_line};
    use anyhow::{Context as _, bail};
    use codewhale_config::private_directory::PrivateDirectory;
    use codewhale_protocol::RuntimeOwnerReceipt;
    use serde_json::{Value, json};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::io::{AsyncWriteExt as _, BufReader};
    #[cfg(unix)]
    use tokio::net::UnixStream;
    #[cfg(unix)]
    type Connection = UnixStream;
    #[cfg(windows)]
    type Connection = tokio::net::windows::named_pipe::NamedPipeClient;

    pub struct OwnerClient {
        read: BoundedLines<BufReader<tokio::io::ReadHalf<Connection>>>,
        write: tokio::io::WriteHalf<Connection>,
        peer: Arc<AuthorizedPeer>,
        receipt: RuntimeOwnerReceipt,
        routing: Option<crate::RuntimeOwnerRouting>,
        frontend: crate::daemon_socket::AttachFrontend,
    }

    pub async fn connect(
        config_path: Option<PathBuf>,
        selected: Option<PathBuf>,
    ) -> Result<OwnerClient> {
        connect_if_published(config_path, selected).await?.context(
            "the selected Runtime has no authenticated live owner; start its canonical host",
        )
    }

    pub async fn connect_if_published(
        config_path: Option<PathBuf>,
        selected: Option<PathBuf>,
    ) -> Result<Option<OwnerClient>> {
        connect_frontend(
            config_path,
            selected,
            crate::daemon_socket::AttachFrontend::Control,
            None,
            None,
            None,
            None,
        )
        .await
    }
    pub async fn connect_acp_if_published(
        config_path: Option<PathBuf>,
        selected: Option<PathBuf>,
    ) -> Result<Option<OwnerClient>> {
        connect_frontend(
            config_path,
            selected,
            crate::daemon_socket::AttachFrontend::Acp,
            None,
            None,
            None,
            None,
        )
        .await
    }
    pub async fn connect_listener_if_published(
        config_path: Option<PathBuf>,
        selected: Option<PathBuf>,
        listener: crate::RuntimeListenerSelection,
        expected_owner: RuntimeOwnerReceipt,
    ) -> Result<Option<OwnerClient>> {
        listener.validate_bounds()?;
        connect_frontend(
            config_path,
            selected,
            crate::daemon_socket::AttachFrontend::Listener,
            Some(listener),
            Some(expected_owner),
            None,
            None,
        )
        .await
    }
    pub async fn connect_scoped_control_if_published(
        config_path: Option<PathBuf>,
        selected: Option<PathBuf>,
        scope: crate::RuntimeFrontendScope,
        owner: RuntimeOwnerReceipt,
    ) -> Result<Option<OwnerClient>> {
        scope.validate_bounds()?;
        connect_frontend(
            config_path,
            selected,
            crate::daemon_socket::AttachFrontend::Control,
            None,
            Some(owner),
            Some(scope),
            None,
        )
        .await
    }
    pub async fn connect_selected_acp_if_published(
        config_path: Option<PathBuf>,
        selected: Option<PathBuf>,
        scope: crate::RuntimeFrontendScope,
        model: String,
        owner: RuntimeOwnerReceipt,
    ) -> Result<Option<OwnerClient>> {
        scope.validate_bounds()?;
        anyhow::ensure!(
            !model.trim().is_empty() && model.len() <= 1024,
            "invalid selected ACP model"
        );
        connect_frontend(
            config_path,
            selected,
            crate::daemon_socket::AttachFrontend::Acp,
            None,
            Some(owner),
            Some(scope),
            Some(model),
        )
        .await
    }
    async fn connect_frontend(
        config_path: Option<PathBuf>,
        selected: Option<PathBuf>,
        frontend: crate::daemon_socket::AttachFrontend,
        listener: Option<crate::RuntimeListenerSelection>,
        expected_owner: Option<RuntimeOwnerReceipt>,
        scope: Option<crate::RuntimeFrontendScope>,
        acp_model: Option<String>,
    ) -> Result<Option<OwnerClient>> {
        let socket_path = owner_work(move || match selected {
            Some(path) => Ok(path),
            None => default_socket_path().map_err(Into::into),
        })
        .await?;
        #[cfg(unix)]
        let directory = socket_path
            .parent()
            .context("owner endpoint has no private parent")?
            .to_path_buf();
        #[cfg(windows)]
        let directory = owner_work(crate::daemon_windows::owner_directory).await?;
        #[cfg(unix)]
        let name = format!(
            "{}.owner.json",
            socket_path
                .file_name()
                .and_then(|name| name.to_str())
                .context("invalid owner endpoint basename")?
        );
        #[cfg(windows)]
        let name = "daemon.owner.json".to_string();
        let receipt_name = name.clone();
        let found = owner_work(move || {
            let parent = match PrivateDirectory::inspect(&directory) {
                Ok(parent) => Arc::new(parent),
                Err(error)
                    if error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
                {
                    return Ok(None);
                }
                Err(error) => return Err(error),
            };
            let Some((bytes, file)) = parent.read_private_receipt(&name, 16384)? else {
                return Ok(None);
            };
            let receipt: RuntimeOwnerReceipt = serde_json::from_slice(&bytes)?;
            Ok(Some((parent, receipt, file)))
        })
        .await?;
        let Some((parent, receipt, file)) = found else {
            return Ok(None);
        };
        anyhow::ensure!(
            receipt.version == 1
                && receipt.pid > 0
                && receipt.socket_path == socket_path
                && !receipt.lease_generation.is_empty(),
            "selected owner receipt is invalid"
        );
        anyhow::ensure!(
            receipt.config_path == config_path,
            "selected owner config does not match; refusing another store"
        );
        #[cfg(unix)]
        let (stream, peer) = {
            let stream =
                tokio::time::timeout(Duration::from_secs(5), UnixStream::connect(&socket_path))
                    .await
                    .context("owner connect deadline expired")??;
            let credential = stream
                .peer_cred()
                .context("owner peer credentials unavailable")?;
            let pid = credential
                .pid()
                .and_then(|pid| u32::try_from(pid).ok())
                .context("owner peer PID unavailable")?;
            anyhow::ensure!(
                credential.uid() == PrivateDirectory::current_user_id()
                    && receipt.principal == credential.uid().to_string()
                    && pid == receipt.pid,
                "connected process is not the selected Runtime owner"
            );
            (
                stream,
                Arc::new(AuthorizedPeer {
                    pid,
                    start: receipt.process_start.clone(),
                }),
            )
        };
        #[cfg(windows)]
        let (stream, peer) = {
            let (stream, process) = crate::daemon_windows::connect_owner(&receipt).await?;
            let peer = AuthorizedPeer {
                pid: process.pid(),
                start: process.start().to_string(),
                process,
            };
            (stream, Arc::new(peer))
        };
        let held_parent = parent.clone();
        let held_receipt = receipt.clone();
        let check = peer.clone();
        owner_work(move || {
            check.check()?;
            anyhow::ensure!(
                held_parent.is_at_selected_path()?,
                "selected owner parent changed"
            );
            let (bytes, current) = held_parent
                .read_private_receipt(&receipt_name, 16384)?
                .context("selected owner receipt withdrawn")?;
            anyhow::ensure!(
                serde_json::from_slice::<RuntimeOwnerReceipt>(&bytes)? == held_receipt
                    && PrivateDirectory::same_file_identity(&file, &current)?,
                "selected owner receipt changed before attach"
            );
            Ok(())
        })
        .await?;
        anyhow::ensure!(
            expected_owner
                .as_ref()
                .is_none_or(|expected| expected == &receipt),
            "selected owner changed before frontend admission"
        );
        let (read, mut write) = tokio::io::split(stream);
        let mut replies = BoundedLines::new(BufReader::new(read));
        let attach_id = uuid::Uuid::new_v4().to_string();
        let attach = json!({"jsonrpc":"2.0","id":attach_id,"method":"daemon/attach","params":{
            "client":{"name":"codewhale-stdio","version":env!("CARGO_PKG_VERSION"),"pid":std::process::id()},
            "mode":"attach","frontend":frontend,"listener":listener,"scope":scope,"acp_model":acp_model,"expect_daemon_version":env!("CARGO_PKG_VERSION"),"expect_owner":receipt
        }});
        tokio::time::timeout(
            Duration::from_secs(5),
            write_stdio_line(&mut write, &attach),
        )
        .await
        .context("owner attach write deadline expired; outcome uncertain")??;
        let line = tokio::time::timeout(Duration::from_secs(5), replies.next_line())
            .await
            .context("owner attach response deadline expired")??
            .context("owner closed before attach")?;
        anyhow::ensure!(line.len() <= 16384, "oversized owner attach response");
        let response: Value = serde_json::from_str(&line)?;
        anyhow::ensure!(
            response["id"] == attach_id
                && response["error"].is_null()
                && response["result"]["role"] == "attached",
            "selected owner refused guest attachment"
        );
        let returned: RuntimeOwnerReceipt =
            serde_json::from_value(response["result"]["owner_receipt"].clone())
                .context("owner attach receipt unavailable")?;
        anyhow::ensure!(
            returned == receipt,
            "connected owner did not confirm the exact captured generation/store"
        );
        let routing = response["result"]
            .get("runtime_routing")
            .filter(|value| !value.is_null())
            .map(|value| serde_json::from_value(value.clone()))
            .transpose()?;
        Ok(Some(OwnerClient {
            read: replies,
            write,
            peer,
            receipt,
            routing,
            frontend,
        }))
    }

    impl OwnerClient {
        pub fn routing(&self) -> Option<&crate::RuntimeOwnerRouting> {
            self.routing.as_ref()
        }

        pub fn receipt(&self) -> &RuntimeOwnerReceipt {
            &self.receipt
        }
        pub async fn send(&mut self, id: Value, method: &str, params: Value) -> Result<()> {
            let frame = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});
            let bytes = serde_json::to_vec(&frame)?;
            anyhow::ensure!(
                bytes.len() <= crate::MAX_RUNTIME_IMAGE_BODY_BYTES,
                "owner request exceeds transport bound"
            );
            let check = self.peer.clone();
            owner_work(move || check.check()).await?;
            tokio::time::timeout(
                Duration::from_secs(5),
                write_stdio_line(&mut self.write, &frame),
            )
            .await
            .context("owner write deadline expired; outcome uncertain, not replayed")??;
            Ok(())
        }
        pub async fn recv(&mut self) -> Result<Option<Value>> {
            let Some(line) = self.read.next_line().await? else {
                return Ok(None);
            };
            let check = self.peer.clone();
            owner_work(move || check.check()).await?;
            serde_json::from_str(&line).map(Some).map_err(Into::into)
        }
        pub async fn forward<I, O>(mut self, input: I, output: O) -> Result<()>
        where
            I: tokio::io::AsyncRead + Unpin,
            O: tokio::io::AsyncWrite + Unpin,
        {
            let mut input = BoundedLines::new(BufReader::new(input));
            let mut output = tokio::io::BufWriter::new(output);
            let mut input_open = true;
            loop {
                tokio::select! {
                    line = input.next_line(), if input_open => match line? {
                        None => {
                            input_open=false;
                            tokio::time::timeout(Duration::from_secs(5),write_stdio_line(&mut self.write,&json!({"jsonrpc":"2.0","method":"daemon/input_closed","params":{}}))).await.context("owner input-close write outcome uncertain, not replayed")??;
                            self.write.shutdown().await?;
                        }
                        Some(line) if self.frontend==crate::daemon_socket::AttachFrontend::Acp => {
                            let check=self.peer.clone();
                            owner_work(move || check.check()).await?;
                            let value:Value=serde_json::from_str(&line)?;
                            anyhow::ensure!(value.is_object() && value["jsonrpc"]=="2.0","ACP frame must be a JSON-RPC object");
                            // Preserve response IDs/permission replies verbatim.
                            // Actual ACP authority/parser stays in AcpServer.
                            tokio::time::timeout(Duration::from_secs(5),async {self.write.write_all(line.as_bytes()).await?;self.write.write_all(b"\n").await?;self.write.flush().await}).await.context("ACP write outcome uncertain; not replayed")??;
                        }
                        Some(line) => match parse_stdio_line(&line) {
                            ParsedStdioLine::Blank => {},
                            ParsedStdioLine::Rejected(response) => write_stdio_line(&mut output, &response).await?,
                            ParsedStdioLine::Request(request) => {
                                self.send(request.id.unwrap_or(Value::Null),&request.method,request.params).await?;
                            }
                        }
                    },
                    line = self.read.next_line() => match line? {
                        None if !input_open => return Ok(()),
                        None => bail!("selected owner connection closed; pending operation outcomes may be uncertain"),
                        Some(line) => {
                            let _: Value = serde_json::from_str(&line).context("invalid owner response")?;
                            output.write_all(line.as_bytes()).await?;
                            output.write_all(b"\n").await?;
                            output.flush().await?;
                        }
                    }
                }
            }
        }
    }

    pub async fn forward_stdio(config_path: Option<PathBuf>) -> Result<()> {
        connect(config_path, None)
            .await?
            .forward(tokio::io::stdin(), tokio::io::stdout())
            .await
    }
}

#[cfg(any(unix, windows))]
pub use platform::{
    OwnerClient, connect, connect_acp_if_published, connect_if_published,
    connect_listener_if_published, connect_scoped_control_if_published,
    connect_selected_acp_if_published, forward_stdio,
};

#[cfg(not(any(unix, windows)))]
pub async fn forward_stdio(_config_path: Option<PathBuf>) -> Result<()> {
    anyhow::bail!("authenticated live-owner control is not implemented on this platform")
}
