use std::sync::{Arc, Mutex};

use ::mio::{Registry, Token};

/// Allows to wake up the `mio::Poll::poll()` method.
/// This type wraps `mio::Waker`, for more information see its documentation.
#[derive(Clone, Debug)]
pub(crate) struct Waker {
    inner: Arc<Mutex<::mio::Waker>>,
}

impl Waker {
    /// Create a new `Waker`.
    pub(crate) fn new(registry: &Registry, waker_token: Token) -> std::io::Result<Self> {
        Ok(Self {
            inner: Arc::new(Mutex::new(mio::Waker::new(registry, waker_token)?)),
        })
    }

    /// Wake up the [`Poll`] associated with this `Waker`.
    ///
    /// Readiness is set to `Ready::readable()`.
    pub(crate) fn wake(&self) -> std::io::Result<()> {
        self.inner.lock().unwrap().wake()
    }

    /// Resets the state so the same waker can be reused.
    ///
    /// This function is not impl
    #[allow(dead_code, clippy::clippy::unnecessary_wraps)]
    pub(crate) fn reset(&self) -> std::io::Result<()> {
        Ok(())
    }
}
