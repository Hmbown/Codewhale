//! OSC 11 terminal-background query.
//!
//! `COLORFGBG` is the only background signal the palette had before this
//! module, and most modern terminals never set it — Windows Terminal, conhost,
//! VS Code, GNOME Terminal, Alacritty and Ghostty all omit it. Without it a
//! white terminal was indistinguishable from a black one, so detection fell
//! back to `Dark` and painted dark-tuned text onto a light surface (#4833).
//!
//! OSC 11 (`ESC ] 11 ; ? BEL`) asks the terminal for its actual background
//! color and is answered by every terminal listed above. The reply is an
//! `xterm`-style color spec, e.g.
//!
//! ```text
//! ESC ] 11 ; rgb:ffff/ffff/ffff ESC \
//! ```
//!
//! The parse is a pure function so it can be tested without a terminal; the
//! query itself is Unix-only, bounded by a short deadline, and never runs when
//! stdin/stdout are not both TTYs.

/// Upper bound on how long startup will wait for a terminal that never
/// answers. A terminal that supports OSC 11 replies in well under a
/// millisecond; anything past this is a terminal that will never reply, and
/// startup latency matters more than the answer.
pub const OSC11_QUERY_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(120);

/// The query sequence. `ESC \` (ST) is the terminator we prefer in the reply,
/// but terminals may answer with BEL instead, so the reader accepts both.
///
/// Only the Unix query path writes it — see the note on [`parse_osc11_reply`].
#[cfg_attr(not(unix), allow(dead_code))]
const OSC11_QUERY: &[u8] = b"\x1b]11;?\x1b\\";

/// Extract an RGB triple from an OSC 11 reply body.
///
/// Accepts the shapes terminals actually emit:
/// - `rgb:RRRR/GGGG/BBBB` (xterm, 1–4 hex digits per channel, any width)
/// - `#RRGGBB` / `#RGB` / `#RRRRGGGGBBBB`
///
/// Leading `ESC ] 11 ;` and the trailing BEL/ST are optional — anything
/// outside the color spec is ignored, so a reply that arrived interleaved with
/// other terminal chatter still parses.
///
/// Returns `None` when no color spec is present or a channel is malformed.
/// Channels wider than 8 bits are scaled down, not truncated, so `ffff` is
/// `255` rather than `0`.
// The parser is deliberately cross-platform while the query is Unix-only:
// there is no portable way to read a raw OSC reply off a Windows console
// handle yet, so on Windows nothing calls these. They are kept (rather than
// cfg'd out) because they are pure, fully tested on every platform, and are
// exactly what a future Windows read path would need — but that leaves them
// dead in a non-test Windows build, which `-D warnings` rejects.
#[cfg_attr(not(unix), allow(dead_code))]
#[must_use]
pub fn parse_osc11_reply(reply: &str) -> Option<(u8, u8, u8)> {
    if let Some(idx) = reply.find("rgb:") {
        return parse_slash_separated(&reply[idx + 4..]);
    }
    if let Some(idx) = reply.find('#') {
        return parse_hash_hex(&reply[idx + 1..]);
    }
    None
}

#[cfg_attr(not(unix), allow(dead_code))]
fn parse_slash_separated(spec: &str) -> Option<(u8, u8, u8)> {
    let spec: String = spec
        .chars()
        .take_while(|c| c.is_ascii_hexdigit() || *c == '/')
        .collect();
    let mut parts = spec.split('/');
    let r = scale_hex_channel(parts.next()?)?;
    let g = scale_hex_channel(parts.next()?)?;
    let b = scale_hex_channel(parts.next()?)?;
    if parts.next().is_some() {
        return None;
    }
    Some((r, g, b))
}

#[cfg_attr(not(unix), allow(dead_code))]
fn parse_hash_hex(spec: &str) -> Option<(u8, u8, u8)> {
    let digits: String = spec.chars().take_while(char::is_ascii_hexdigit).collect();
    if !digits.len().is_multiple_of(3) || digits.is_empty() || digits.len() > 12 {
        return None;
    }
    let width = digits.len() / 3;
    let r = scale_hex_channel(&digits[..width])?;
    let g = scale_hex_channel(&digits[width..width * 2])?;
    let b = scale_hex_channel(&digits[width * 2..])?;
    Some((r, g, b))
}

/// Normalize a hex channel of arbitrary width (1–4 digits) to 8 bits by
/// rescaling across the channel's full range: `f` → `255`, `ffff` → `255`,
/// `8000` → `128`.
#[cfg_attr(not(unix), allow(dead_code))]
fn scale_hex_channel(digits: &str) -> Option<u8> {
    if digits.is_empty() || digits.len() > 4 || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(digits, 16).ok()?;
    let max = (1u32 << (4 * digits.len() as u32)) - 1;
    Some(((value * 255 + max / 2) / max) as u8)
}

/// Ask the terminal for its background color, giving up after `timeout`.
///
/// Returns `None` — never blocks past `timeout`, never panics — when:
/// - stdin and stdout are not both TTYs (piped output, CI, `codewhale < file`),
/// - the platform has no supported query path (non-Unix; see the module docs),
/// - the terminal does not answer, or answers with something unparsable.
///
/// # Caveat
///
/// This reads from stdin, so it must only be called while the terminal is in
/// raw mode and before the event loop starts. Bytes that arrive during the
/// window and are not part of the reply are *not* discarded: they are the
/// user's type-ahead, and are handed to
/// [`carry_typed_ahead`] for the event loop to
/// replay in order (#5925).
#[must_use]
pub fn query_terminal_background(timeout: std::time::Duration) -> Option<(u8, u8, u8)> {
    let reply = query_terminal(OSC11_QUERY, timeout)?;
    parse_osc11_reply(&String::from_utf8_lossy(&reply))
}

/// Write `query` to the terminal and read back one reply, giving up after
/// `timeout`. The reply is the bytes up to (not including) its BEL or `ESC \`
/// terminator; an `ESC` that opens the reply is kept. Shared by the OSC 11
/// background query and the kitty graphics probe (`tui::mark`), under the
/// same caveat as [`query_terminal_background`]: raw mode on, event loop not
/// yet reading stdin.
#[cfg(unix)]
pub fn query_terminal(query: &[u8], timeout: std::time::Duration) -> Option<Vec<u8>> {
    query_terminal_inner(query, timeout, false)
}

/// CSI-terminated variant of [`query_terminal`] for the sixel probe
/// (`tui::mark`): a primary-DA reply ends at its alphabetic final byte
/// (`c`), which is neither BEL nor `ESC \`, so the plain reader would keep
/// swallowing input — including the user's own typed-ahead keystrokes —
/// until its byte cap. Stops after the final byte of a reply that opened
/// with `ESC [` and keeps the same raw-mode caveat.
#[cfg(unix)]
pub fn query_terminal_csi(query: &[u8], timeout: std::time::Duration) -> Option<Vec<u8>> {
    query_terminal_inner(query, timeout, true)
}

/// Largest reply any of the three probes can produce. Past this the answer
/// is not one of ours.
const MAX_REPLY_BYTES: usize = 128;

// ---------------------------------------------------------------------------
// Type-ahead carried across the probe window (#5925).
//
// The probe readers below are the only readers of the tty between raw-mode
// entry and the input pump, so anything the user typed at launch arrives in
// the same stream as the replies. The reader keeps its reply and parks every
// other byte here; `tui::startup_input` drains this and replays it into the
// composer. The buffers live in this module rather than with the replay
// logic because `src/palette/` is `#[path]`-included by test harnesses that
// do not compile the `tui` module tree.
// ---------------------------------------------------------------------------

/// Upper bound on carried type-ahead. A terminal that answers a probe does
/// so in well under a millisecond, so this only ever holds a line or two;
/// the cap stops a wedged tty from growing the buffer without limit.
pub const MAX_CARRIED_BYTES: usize = 4096;

/// Bytes a probe consumed that were not part of its reply.
static CARRIED_TYPE_AHEAD: std::sync::Mutex<Vec<u8>> = std::sync::Mutex::new(Vec::new());
/// Bytes a probe consumed that cannot be replayed as keystrokes.
static CONSUMED_UNREPLAYABLE: std::sync::Mutex<Vec<u8>> = std::sync::Mutex::new(Vec::new());

/// Park non-reply bytes for the event loop to replay.
#[cfg_attr(not(unix), allow(dead_code))]
pub fn carry_typed_ahead(bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    let Ok(mut carried) = CARRIED_TYPE_AHEAD.lock() else {
        note_consumed_unreplayable(bytes);
        return;
    };
    let room = MAX_CARRIED_BYTES.saturating_sub(carried.len());
    let (keep, overflow) = bytes.split_at(room.min(bytes.len()));
    carried.extend_from_slice(keep);
    drop(carried);
    note_consumed_unreplayable(overflow);
}

/// Record bytes startup consumed and cannot hand back.
pub fn note_consumed_unreplayable(bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    if let Ok(mut dropped) = CONSUMED_UNREPLAYABLE.lock() {
        let room = MAX_CARRIED_BYTES.saturating_sub(dropped.len());
        dropped.extend_from_slice(&bytes[..room.min(bytes.len())]);
    }
}

/// Take everything parked by [`carry_typed_ahead`].
pub fn take_carried_type_ahead() -> Vec<u8> {
    CARRIED_TYPE_AHEAD
        .lock()
        .map(|mut carried| std::mem::take(&mut *carried))
        .unwrap_or_default()
}

/// Take everything recorded by [`note_consumed_unreplayable`].
pub fn take_consumed_unreplayable() -> Vec<u8> {
    CONSUMED_UNREPLAYABLE
        .lock()
        .map(|mut dropped| std::mem::take(&mut *dropped))
        .unwrap_or_default()
}

/// What the caller should do after feeding one byte to [`ProbeSplit`].
#[cfg_attr(not(unix), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbeStep {
    /// Keep reading.
    Continue,
    /// The reply is complete.
    Done,
    /// The reply ended with the `ESC` of an `ESC \` string terminator: read
    /// one more byte and give it to [`ProbeSplit::finish_string_terminator`].
    AwaitStringTerminator,
    /// Carried type-ahead hit its cap; stop reading.
    Overflow,
}

/// Splits one probe stream into the terminal's reply and the user's
/// type-ahead (#5925).
///
/// Pure and byte-driven so the split — the actual defect in #5925, where
/// everything that was not the reply was thrown away — is testable without a
/// terminal. The reply always opens with the same two bytes the query did
/// (`ESC ]` for OSC 11, `ESC _` for the kitty graphics query, `ESC [` for the
/// sixel primary-DA query); anything before that introducer is input the user
/// typed, and an `ESC` that does not go on to match it is theirs too.
#[cfg_attr(not(unix), allow(dead_code))]
#[derive(Debug)]
pub(crate) struct ProbeSplit<'a> {
    introducer: &'a [u8],
    stop_at_csi_final: bool,
    carried: Vec<u8>,
    /// A partial introducer match, still undecided between reply and input.
    undecided: Vec<u8>,
    reply: Vec<u8>,
    in_reply: bool,
}

#[cfg_attr(not(unix), allow(dead_code))]
impl<'a> ProbeSplit<'a> {
    /// `query` is the sequence just written; its first two bytes are the
    /// introducer the reply will open with.
    pub(crate) fn for_query(query: &'a [u8], stop_at_csi_final: bool) -> Self {
        Self {
            introducer: if query.len() >= 2 && query[0] == 0x1b {
                &query[..2]
            } else {
                &[]
            },
            stop_at_csi_final,
            carried: Vec::new(),
            undecided: Vec::new(),
            reply: Vec::new(),
            in_reply: false,
        }
    }

    pub(crate) fn feed(&mut self, byte: u8) -> ProbeStep {
        if !self.in_reply {
            self.feed_before_reply(byte);
            if self.carried.len() >= MAX_CARRIED_BYTES {
                return ProbeStep::Overflow;
            }
            return ProbeStep::Continue;
        }
        // BEL, or the ESC of an `ESC \` string terminator, ends the reply.
        if byte == 0x07 {
            return ProbeStep::Done;
        }
        if byte == 0x1b {
            return ProbeStep::AwaitStringTerminator;
        }
        self.reply.push(byte);
        // A CSI reply (`ESC [` …) ends at its first final byte (`@..=~`):
        // keep the final and stop, so a DA answer never eats past itself.
        if self.stop_at_csi_final
            && self.reply.len() >= 3
            && self.reply[0] == 0x1b
            && self.reply[1] == b'['
            && (0x40..=0x7e).contains(&byte)
        {
            return ProbeStep::Done;
        }
        if self.reply.len() >= MAX_REPLY_BYTES {
            return ProbeStep::Overflow;
        }
        ProbeStep::Continue
    }

    fn feed_before_reply(&mut self, byte: u8) {
        if self.undecided.is_empty() {
            if byte == 0x1b && !self.introducer.is_empty() {
                self.undecided.push(byte);
            } else {
                self.carried.push(byte);
            }
            return;
        }
        self.undecided.push(byte);
        if self.introducer.starts_with(&self.undecided) {
            if self.undecided.len() == self.introducer.len() {
                self.in_reply = true;
                self.reply = std::mem::take(&mut self.undecided);
            }
            return;
        }
        // Not our reply after all — an `Esc` keypress, or an escape sequence
        // from some other source. Everything held is the user's, except a
        // fresh `ESC` which may still open the reply we are waiting for.
        let restarts = byte == 0x1b;
        if restarts {
            self.undecided.pop();
        }
        self.carried.append(&mut self.undecided);
        if restarts {
            self.undecided.push(byte);
        }
    }

    /// The byte read after an [`ProbeStep::AwaitStringTerminator`]. The `\`
    /// of `ESC \` belongs to the reply; anything else is the user's next
    /// keystroke and must not vanish with the terminator.
    pub(crate) fn finish_string_terminator(&mut self, byte: u8) {
        if byte != b'\\' {
            self.carried.push(byte);
        }
    }

    /// Consume the split: `(reply, carried type-ahead)`. An undecided
    /// introducer is input the terminal never claimed.
    pub(crate) fn finish(mut self) -> (Vec<u8>, Vec<u8>) {
        self.carried.append(&mut self.undecided);
        (self.reply, self.carried)
    }
}

/// Read a probe reply off stdin without eating the user's type-ahead.
///
/// The reply always opens with the same two bytes the query did (`ESC ]` for
/// OSC 11, `ESC _` for the kitty graphics query, `ESC [` for the sixel
/// primary-DA query), so every byte before that introducer — and the one
/// lookahead byte after an `ESC` that turned out not to open `ESC \` — is
/// input the user typed, not terminal chatter. Those bytes are carried to
/// [`carry_typed_ahead`] for replay instead of being dropped on the
/// floor (#5925). Bytes this reader consumed but cannot hand back (a reply
/// the terminal never terminated) are recorded as dropped so the startup
/// receipt names them.
#[cfg(unix)]
fn query_terminal_inner(
    query: &[u8],
    timeout: std::time::Duration,
    stop_at_csi_final: bool,
) -> Option<Vec<u8>> {
    use std::io::Write;
    use std::os::fd::AsRawFd;

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let in_fd = stdin.as_raw_fd();
    let out_fd = stdout.as_raw_fd();

    // SAFETY: `isatty` only inspects the descriptor; both fds are owned by the
    // std handles held above for the duration of the call.
    let both_tty = unsafe { libc::isatty(in_fd) == 1 && libc::isatty(out_fd) == 1 };
    if !both_tty {
        return None;
    }

    {
        let mut out = stdout.lock();
        out.write_all(query).ok()?;
        out.flush().ok()?;
    }

    let (answered, reply, carried) = read_terminal_reply(in_fd, query, timeout, stop_at_csi_final);
    carry_typed_ahead(&carried);
    if !answered {
        // An incomplete control reply is not safe to replay as typing.
        note_consumed_unreplayable(&reply);
        return None;
    }
    Some(reply)
}

#[cfg(unix)]
fn read_terminal_reply(
    in_fd: std::os::fd::RawFd,
    query: &[u8],
    timeout: std::time::Duration,
    stop_at_csi_final: bool,
) -> (bool, Vec<u8>, Vec<u8>) {
    use std::time::Instant;

    let deadline = Instant::now() + timeout;
    let mut split = ProbeSplit::for_query(query, stop_at_csi_final);
    let answered = loop {
        let Some(byte) = read_terminal_byte(in_fd, deadline) else {
            break false;
        };
        match split.feed(byte) {
            ProbeStep::Continue => {}
            ProbeStep::Done => break true,
            ProbeStep::Overflow => break false,
            ProbeStep::AwaitStringTerminator => {
                // Consume the `\` of an `ESC \` terminator so it cannot
                // surface later as a keypress once the event loop owns
                // stdin. Anything else is the user's next keystroke.
                let terminator_deadline = Instant::now() + std::time::Duration::from_millis(5);
                if let Some(byte) = read_terminal_byte(in_fd, terminator_deadline) {
                    split.finish_string_terminator(byte);
                }
                break true;
            }
        }
    };

    let (reply, carried) = split.finish();
    (answered, reply, carried)
}

/// Poll and read the same unbuffered descriptor. `StdinLock` reads ahead into
/// Rust's shared buffer: polling the tty afterward misses those bytes, and
/// crossterm's later fd reader cannot recover them either.
#[cfg(unix)]
fn read_terminal_byte(fd: std::os::fd::RawFd, deadline: std::time::Instant) -> Option<u8> {
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() || !wait_readable(fd, remaining) {
            return None;
        }
        let mut byte = 0u8;
        // SAFETY: the caller owns the open fd throughout the startup probe;
        // `byte` is writable for exactly the single byte requested. The input
        // pump has not started, so there is no competing reader.
        let count = unsafe { libc::read(fd, std::ptr::addr_of_mut!(byte).cast(), 1) };
        if count == 1 {
            return Some(byte);
        }
        if count != -1 || std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted
        {
            return None;
        }
    }
}

#[cfg(all(test, unix))]
#[path = "osc11_tests.rs"]
mod tests;

/// Block until `fd` has data or `timeout` elapses. `true` means readable.
#[cfg(unix)]
fn wait_readable(fd: std::os::fd::RawFd, timeout: std::time::Duration) -> bool {
    let mut pollfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    let millis = i32::try_from(timeout.as_millis())
        .unwrap_or(i32::MAX)
        .max(1);
    // SAFETY: `pollfd` is a live, correctly-initialized single-element array
    // and the count matches.
    let rc = unsafe { libc::poll(std::ptr::addr_of_mut!(pollfd), 1, millis) };
    rc > 0 && (pollfd.revents & libc::POLLIN) != 0
}

/// Non-Unix platforms have no portable way to read a raw OSC reply back off
/// the console handle, so detection falls through to the environment-based
/// sources. Callers treat `None` as "no evidence", never as "dark".
#[cfg(not(unix))]
pub fn query_terminal(_query: &[u8], _timeout: std::time::Duration) -> Option<Vec<u8>> {
    None
}

/// Non-Unix twin of [`query_terminal_csi`]: no console to ask, no evidence.
#[cfg(not(unix))]
pub fn query_terminal_csi(_query: &[u8], _timeout: std::time::Duration) -> Option<Vec<u8>> {
    None
}
