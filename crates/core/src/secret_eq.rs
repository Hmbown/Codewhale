//! The one constant-time comparison for bearer tokens, nonces and proofs.
//!
//! Every credential check in the engine (Runtime API token, web and mobile
//! session proofs, computer-display tokens, the app-server bearer) goes
//! through [`constant_time_eq`], so a timing fix lands once.

/// Compares the full length of both inputs regardless of where they first
/// differ, so an auth failure leaks neither the matching prefix length nor,
/// beyond the loop bound, which input was shorter.
#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        diff |= usize::from(x ^ y);
    }
    std::hint::black_box(diff) == 0
}

#[cfg(test)]
mod tests {
    use super::constant_time_eq;

    #[test]
    fn equal_inputs_match() {
        assert!(constant_time_eq(b"", b""));
        assert!(constant_time_eq(b"secret-token", b"secret-token"));
    }

    #[test]
    fn different_inputs_do_not_match() {
        assert!(!constant_time_eq(b"secret-token", b"secret-tokem"));
        assert!(!constant_time_eq(b"secret", b"secret-token"));
        assert!(!constant_time_eq(b"secret-token", b"secret"));
        assert!(!constant_time_eq(b"", b"x"));
        // A zero-padded prefix must not collide with the shorter input.
        assert!(!constant_time_eq(b"abc", b"abc\0"));
    }
}
