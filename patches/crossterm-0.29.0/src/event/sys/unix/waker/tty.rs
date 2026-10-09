use std::{
    io::{self, Write},
    os::unix::net::UnixStream,
    sync::{Arc, Mutex},
};

/// Allows to wake up the EventSource::try_read() method.
#[derive(Clone, Debug)]
pub(crate) struct Waker {
    inner: Arc<Mutex<UnixStream>>,
}

impl Waker {
    /// Create a new `Waker`.
    pub(crate) fn new(writer: UnixStream) -> Self {
        Self {
            inner: Arc::new(Mutex::new(writer)),
        }
    }

    /// Wake up the [`Poll`] associated with this `Waker`.
    ///
    /// Readiness is set to `Ready::readable()`.
    pub(crate) fn wake(&self) -> io::Result<()> {
        self.inner.lock().unwrap().write_all(&[0])?;
        Ok(())
    }
}
