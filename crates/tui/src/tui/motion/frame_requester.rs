//! Coalescing frame-request scheduler.
//!
//! Widgets ask for a future frame; this scheduler merges requests and emits
//! at most one wake deadline compatible with the existing frame-cap
//! philosophy in [`crate::tui::frame_rate_limiter`]. It does **not** run a
//! competing animation loop — the main `ui` poll loop remains the sole
//! emitter of `terminal.draw`.

use std::time::Duration;
use std::time::Instant;

use super::mode::MotionPolicy;

/// Coalesced request for a future redraw.
#[derive(Debug, Default)]
pub struct FrameRequester {
    /// Earliest instant a requester wants a frame.
    next_due: Option<Instant>,
    #[cfg(test)]
    request_count: u64,
    #[cfg(test)]
    emit_count: u64,
}

impl FrameRequester {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Request a frame as soon as the motion policy and frame cap allow.
    pub fn request_frame(&mut self, now: Instant, policy: MotionPolicy) {
        self.request_at(now, now, policy);
    }

    /// Request a frame no earlier than `earliest`.
    pub fn request_at(&mut self, now: Instant, earliest: Instant, policy: MotionPolicy) {
        if !policy.should_request_animation_frames() {
            return;
        }
        let capped = earliest.max(now);
        #[cfg(test)]
        {
            self.request_count = self.request_count.saturating_add(1);
        }
        self.next_due = Some(match self.next_due {
            Some(existing) => existing.min(capped),
            None => capped,
        });
    }

    /// Time until a coalesced frame should emit, if one is pending.
    #[must_use]
    pub fn due_in(&self, now: Instant) -> Option<Duration> {
        let due = self.next_due?;
        Some(due.saturating_duration_since(now))
    }

    /// Consume a due frame request. Returns true when the main loop should
    /// set `needs_redraw` for animation (not for state changes).
    pub fn take_due(&mut self, now: Instant, policy: MotionPolicy) -> bool {
        if !policy.should_request_animation_frames() {
            self.next_due = None;
            return false;
        }
        let Some(due) = self.next_due else {
            return false;
        };
        if now < due {
            return false;
        }
        self.next_due = None;
        #[cfg(test)]
        {
            self.emit_count = self.emit_count.saturating_add(1);
        }
        true
    }

    #[must_use]
    #[cfg(test)]
    pub fn request_count(&self) -> u64 {
        self.request_count
    }

    #[must_use]
    #[cfg(test)]
    pub fn emit_count(&self) -> u64 {
        self.emit_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::motion::MotionPolicy;

    #[test]
    fn coalesces_multiple_requests_into_one_emit() {
        let policy = MotionPolicy::from_settings(false, true, false);
        let mut req = FrameRequester::new();
        let t0 = Instant::now();

        for _ in 0..20 {
            req.request_frame(t0, policy);
        }
        assert!(req.due_in(t0).is_some());
        assert_eq!(req.request_count(), 20);
        assert!(req.take_due(t0, policy));
        assert_eq!(req.emit_count(), 1);
        assert!(!req.take_due(t0, policy));
    }

    #[test]
    fn reduced_motion_drops_animation_frame_requests() {
        let policy = MotionPolicy::from_settings(true, true, false);
        let mut req = FrameRequester::new();
        let t0 = Instant::now();
        req.request_frame(t0, policy);
        assert!(req.due_in(t0).is_none());
        assert!(!req.take_due(t0, policy));
    }

    #[test]
    fn turning_motion_off_discards_a_pending_frame() {
        let full = MotionPolicy::from_settings(false, true, false);
        let still = MotionPolicy::from_settings(false, false, false);
        let mut req = FrameRequester::new();
        let now = Instant::now();
        req.request_frame(now, full);
        assert!(!req.take_due(now, still));
        assert!(req.due_in(now).is_none());
        assert!(!req.take_due(now, full));
    }
}
