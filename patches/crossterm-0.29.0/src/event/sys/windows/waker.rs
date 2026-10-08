use std::sync::{Arc, Mutex};

use crossterm_winapi::Semaphore;

/// Allows to wake up the `WinApiPoll::poll()` method.
#[derive(Clone, Debug)]
pub(crate) struct Waker {
    inner: Arc<Mutex<Semaphore>>,
}

impl Waker {
    /// Creates a new waker.
    ///
    /// `Waker` is based on the `Semaphore`. You have to use the semaphore
    /// handle along with the `WaitForMultipleObjects`.
    pub(crate) fn new() -> std::io::Result<Self> {
        let inner = Semaphore::new()?;

        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }

    /// Wakes the `WaitForMultipleObjects`.
    pub(crate) fn wake(&self) -> std::io::Result<()> {
        self.inner.lock().unwrap().release()?;
        Ok(())
    }

    /// Replaces the current semaphore with a new one allowing us to reuse the same `Waker`.
    pub(crate) fn reset(&self) -> std::io::Result<()> {
        *self.inner.lock().unwrap() = Semaphore::new()?;
        Ok(())
    }

    /// Returns the semaphore associated with the waker.
    pub(crate) fn semaphore(&self) -> Semaphore {
        self.inner.lock().unwrap().clone()
    }
}
