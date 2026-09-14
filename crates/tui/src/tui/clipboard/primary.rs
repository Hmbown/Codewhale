//! One bounded native PRIMARY transport. Keeping the handle alive preserves
//! X11/Wayland ownership; display I/O never runs on the TUI thread.

use std::sync::mpsc::{self, SyncSender};
use std::time::Duration;

use anyhow::{Result, anyhow};
use arboard::{Clipboard, GetExtLinux, LinuxClipboardKind, SetExtLinux};

enum Request {
    Write(String),
    Read(SyncSender<Option<String>>),
}

pub(super) struct PrimarySelection {
    sender: SyncSender<Request>,
}

impl PrimarySelection {
    pub(super) fn spawn() -> Result<Self> {
        // At most one queued operation, even when a display server stalls.
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("primary-selection".into())
            .spawn(move || {
                let mut clipboard = Clipboard::new().ok();
                while let Ok(request) = receiver.recv() {
                    match request {
                        Request::Write(text) => {
                            if let Some(clipboard) = &mut clipboard {
                                let _ = clipboard
                                    .set()
                                    .clipboard(LinuxClipboardKind::Primary)
                                    .text(text);
                            }
                        }
                        Request::Read(reply) => {
                            let text = clipboard
                                .as_mut()
                                .and_then(|clipboard| {
                                    clipboard
                                        .get()
                                        .clipboard(LinuxClipboardKind::Primary)
                                        .text()
                                        .ok()
                                })
                                .filter(|text| {
                                    !text.is_empty() && text.len() <= super::PRIMARY_MAX_BYTES
                                });
                            let _ = reply.try_send(text);
                        }
                    }
                }
            })?;
        Ok(Self { sender })
    }

    pub(super) fn write(&self, text: &str) -> Result<()> {
        self.sender
            .try_send(Request::Write(text.to_string()))
            .map_err(|_| anyhow!("PRIMARY selection busy or unavailable"))
    }

    pub(super) fn read(&self) -> Option<String> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.sender.try_send(Request::Read(sender)).ok()?;
        // A late reply is discarded, never inserted into a subsequently edited
        // composer. Clipboard failure stays quiet and cannot submit a command.
        receiver
            .recv_timeout(Duration::from_millis(250))
            .ok()
            .flatten()
    }
}
