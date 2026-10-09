use std::io;
use std::time::Duration;

use crossterm_winapi::Handle;
use winapi::{
    shared::winerror::WAIT_TIMEOUT,
    um::{
        synchapi::WaitForMultipleObjects,
        winbase::{INFINITE, WAIT_ABANDONED_0, WAIT_FAILED, WAIT_OBJECT_0},
    },
};

#[cfg(feature = "event-stream")]
pub(crate) use super::waker::Waker;

#[derive(Debug)]
pub(crate) struct WinApiPoll {
    #[cfg(feature = "event-stream")]
    waker: Waker,
}

impl WinApiPoll {
    #[cfg(not(feature = "event-stream"))]
    pub(crate) fn new() -> WinApiPoll {
        WinApiPoll {}
    }

    #[cfg(feature = "event-stream")]
    pub(crate) fn new() -> std::io::Result<WinApiPoll> {
        Ok(WinApiPoll {
            waker: Waker::new()?,
        })
    }
}

impl WinApiPoll {
    pub fn poll(&mut self, timeout: Option<Duration>) -> std::io::Result<Option<bool>> {
        let dw_millis = if let Some(duration) = timeout {
            duration.as_millis() as u32
        } else {
            INFINITE
        };

        let console_handle = Handle::current_in_handle()?;

        #[cfg(feature = "event-stream")]
        let semaphore = self.waker.semaphore();
        #[cfg(feature = "event-stream")]
        let handles = &[*console_handle, **semaphore.handle()];
        #[cfg(not(feature = "event-stream"))]
        let handles = &[*console_handle];

        let output =
            unsafe { WaitForMultipleObjects(handles.len() as u32, handles.as_ptr(), 0, dw_millis) };

        match output {
            output if output == WAIT_OBJECT_0 => {
                // input handle triggered
                Ok(Some(true))
            }
            #[cfg(feature = "event-stream")]
            output if output == WAIT_OBJECT_0 + 1 => {
                // semaphore handle triggered
                let _ = self.waker.reset();
                Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Poll operation was woken up by `Waker::wake`",
                ))
            }
            WAIT_TIMEOUT | WAIT_ABANDONED_0 => {
                // timeout elapsed
                Ok(None)
            }
            WAIT_FAILED => Err(io::Error::last_os_error()),
            _ => Err(io::Error::new(
                io::ErrorKind::Other,
                "WaitForMultipleObjects returned unexpected result.",
            )),
        }
    }

    #[cfg(feature = "event-stream")]
    pub fn waker(&self) -> Waker {
        self.waker.clone()
    }
}
