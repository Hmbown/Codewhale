//! Shared user-visible elapsed/duration formatter.
//!
//! Every TUI, workflow, Fleet, sub-agent, task/activity, toast, status, and
//! receipt surface renders durations through these helpers so the grammar
//! stays consistent everywhere: `0s`, `42s`, `1m 05s`, `12m 34s`.
//!
//! Rules:
//! - Whole seconds under a minute render bare (`0s`, `59s`).
//! - At one minute and above the seconds zero-pad (`60s` -> `1m 00s`,
//!   `61s` -> `1m 01s`).
//! - Minutes keep growing past 60 (`3600s` -> `60m 00s`); no hours unit is
//!   introduced, so a large value can never be misread at the wrong scale.
//! - Sub-second values render as milliseconds (`350ms`) so a fast receipt
//!   never rounds down to a misleading `0s`.
//!
//! Display copy only: timeout/config input units, persisted values, and
//! machine-readable fields stay in their original units; this module exists
//! solely to render durations for humans.

/// Format a whole-seconds duration for a user-visible indicator.
#[must_use]
pub fn format_elapsed_secs(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else {
        format!("{}m {:02}s", secs / 60, secs % 60)
    }
}

/// Format a millisecond duration for a user-visible indicator.
///
/// Sub-second values render as milliseconds (`350ms`); anything at or above
/// one second delegates to [`format_elapsed_secs`].
#[must_use]
pub fn format_elapsed_ms(ms: u64) -> String {
    if ms == 0 {
        "0s".to_string()
    } else if ms < 1_000 {
        format!("{ms}ms")
    } else {
        format_elapsed_secs(ms / 1_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_render_bare_under_a_minute() {
        assert_eq!(format_elapsed_secs(0), "0s");
        assert_eq!(format_elapsed_secs(1), "1s");
        assert_eq!(format_elapsed_secs(42), "42s");
        assert_eq!(format_elapsed_secs(59), "59s");
    }

    #[test]
    fn minutes_zero_pad_seconds() {
        assert_eq!(format_elapsed_secs(60), "1m 00s");
        assert_eq!(format_elapsed_secs(61), "1m 01s");
        assert_eq!(format_elapsed_secs(754), "12m 34s");
        assert_eq!(format_elapsed_secs(3599), "59m 59s");
    }

    #[test]
    fn minutes_keep_growing_past_one_hour() {
        // Hours are intentionally not introduced; the value clamps at the
        // minute/second representation.
        assert_eq!(format_elapsed_secs(3600), "60m 00s");
        assert_eq!(format_elapsed_secs(3661), "61m 01s");
    }

    #[test]
    fn milliseconds_delegate_at_one_second() {
        assert_eq!(format_elapsed_ms(0), "0s");
        assert_eq!(format_elapsed_ms(350), "350ms");
        assert_eq!(format_elapsed_ms(999), "999ms");
        assert_eq!(format_elapsed_ms(1_000), "1s");
        assert_eq!(format_elapsed_ms(59_999), "59s");
        assert_eq!(format_elapsed_ms(60_000), "1m 00s");
        assert_eq!(format_elapsed_ms(61_000), "1m 01s");
        assert_eq!(format_elapsed_ms(3_599_000), "59m 59s");
        assert_eq!(format_elapsed_ms(3_600_000), "60m 00s");
    }
}
