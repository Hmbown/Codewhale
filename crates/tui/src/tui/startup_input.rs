//! Type-ahead integrity across TUI startup (#5925).
//!
//! Startup asks the terminal three questions whose answers arrive on stdin —
//! the OSC 11 background query, the kitty graphics probe, and the sixel
//! primary-DA probe (see [`codewhale_palette::osc11`]). All three run after raw
//! mode is on and before the [`crate::tui::ui::TerminalInputPump`] exists, so
//! for that window Codewhale is the only reader of the tty. Anything the user
//! has already typed sits in the same buffer as the replies.
//!
//! Before this module the probe readers consumed those bytes and threw them
//! away: a `/plugin install …` typed at launch reached the composer as
//! `gin install …`, no longer began with `/`, and was submitted to the model
//! as a prose prompt (#5925).
//!
//! The contract now is: a probe reader keeps only its own reply and hands
//! every other byte it consumed to [`osc11::carry_typed_ahead`]. The event
//! loop replays that buffer, in order, into the same `pending` queue the input
//! pump feeds, before the pump is spawned — so replayed keys are delivered
//! ahead of anything still sitting in the tty. Bytes that cannot be turned
//! back into a key event (a control byte, an escape sequence, invalid UTF-8)
//! are never guessed at: they are named in an INFO receipt and recorded as
//! evidence that the shell did not see the whole line.

use std::collections::VecDeque;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use codewhale_palette::osc11;

/// What [`decode`] could and could not turn back into key events.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct DecodedTypeAhead {
    pub(crate) events: Vec<Event>,
    /// Bytes deliberately not replayed. Never guessed at — an escape
    /// sequence replayed as a bare `Esc` plus letters would type garbage
    /// into the composer, and a synthesized Ctrl+C would cancel work the
    /// user never asked to cancel.
    pub(crate) undecodable: Vec<u8>,
}

fn plain_key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

/// Byte length of the UTF-8 sequence a lead byte opens, or `None` when it is
/// not a valid lead byte.
fn utf8_sequence_len(lead: u8) -> Option<usize> {
    match lead {
        0x00..=0x7f => Some(1),
        0xc2..=0xdf => Some(2),
        0xe0..=0xef => Some(3),
        0xf0..=0xf4 => Some(4),
        _ => None,
    }
}

/// Exclusive end of the escape sequence that starts at `start` (an `ESC`).
///
/// `ESC [` / `ESC O` run to their final byte (`@`..=`~`); control strings
/// run to ST (OSC also accepts BEL). A bare `ESC x` is two bytes; a trailing
/// `ESC` is one. Used only to keep a sequence together
/// so it is dropped as a unit.
fn escape_sequence_end(bytes: &[u8], start: usize) -> usize {
    match bytes.get(start + 1) {
        Some(kind @ (b']' | b'_' | b'P' | b'^' | b'X')) => {
            let mut end = start + 2;
            while end < bytes.len() {
                if *kind == b']' && bytes[end] == 0x07 {
                    return end + 1;
                }
                if bytes[end..].starts_with(b"\x1b\\") {
                    return end + 2;
                }
                end += 1;
            }
            end
        }
        Some(b'[' | b'O') => {
            let mut end = start + 2;
            while end < bytes.len() {
                let byte = bytes[end];
                end += 1;
                if (0x40..=0x7e).contains(&byte) {
                    break;
                }
            }
            end
        }
        Some(_) => start + 2,
        None => start + 1,
    }
}

/// Turn carried bytes back into the key events the pump would have produced.
///
/// Only unambiguous keys are reconstructed: printable text (any UTF-8
/// scalar), Enter, Tab, and Backspace. Everything else — `ESC`, other C0
/// control bytes, invalid UTF-8 — is reported as undecodable rather than
/// approximated.
pub(crate) fn decode(bytes: &[u8]) -> DecodedTypeAhead {
    let mut decoded = DecodedTypeAhead::default();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        match byte {
            b'\r' | b'\n' => {
                decoded.events.push(plain_key(KeyCode::Enter));
                index += 1;
                // A CRLF pair is one Enter, not two.
                if byte == b'\r' && bytes.get(index) == Some(&b'\n') {
                    index += 1;
                }
            }
            b'\t' => {
                decoded.events.push(plain_key(KeyCode::Tab));
                index += 1;
            }
            0x08 | 0x7f => {
                decoded.events.push(plain_key(KeyCode::Backspace));
                index += 1;
            }
            0x1b => {
                // An escape sequence is taken whole, never in pieces: an
                // arrow key replayed as its tail would type `[A` into the
                // composer, which is worse than losing it with a receipt.
                let end = escape_sequence_end(bytes, index);
                let sequence = &bytes[index..end];
                // An OSC 11 answer can arrive after its probe timed out,
                // during the next startup probe. It is a color measurement,
                // not a partially consumed user command.
                let color_reply = sequence
                    .strip_prefix(b"\x1b]11;")
                    .and_then(|body| {
                        body.strip_suffix(b"\x1b\\")
                            .or_else(|| body.strip_suffix(b"\x07"))
                    })
                    .and_then(|body| std::str::from_utf8(body).ok())
                    .filter(|body| {
                        body.strip_prefix("rgb:").is_some_and(|spec| {
                            spec.bytes().all(|b| b.is_ascii_hexdigit() || b == b'/')
                        }) || body
                            .strip_prefix('#')
                            .is_some_and(|spec| spec.bytes().all(|b| b.is_ascii_hexdigit()))
                    })
                    .and_then(osc11::parse_osc11_reply)
                    .is_some();
                // Focus notifications and the successful answer to our exact
                // kitty probe can also arrive during a later startup query.
                // They carry no composer input. Unknown or incomplete escape
                // strings still retain the lost-input guard below.
                let terminal_reply = color_reply
                    || matches!(sequence, b"\x1b[I" | b"\x1b[O" | b"\x1b_Gi=31;OK\x1b\\");
                if !terminal_reply {
                    decoded.undecodable.extend_from_slice(sequence);
                }
                index = end;
            }
            0x00..=0x1f => {
                decoded.undecodable.push(byte);
                index += 1;
            }
            _ => {
                let width = utf8_sequence_len(byte).unwrap_or(0);
                let end = index + width;
                match bytes
                    .get(index..end)
                    .filter(|_| width > 0)
                    .and_then(|slice| std::str::from_utf8(slice).ok())
                {
                    Some(text) => {
                        decoded
                            .events
                            .extend(text.chars().map(|ch| plain_key(KeyCode::Char(ch))));
                        index = end;
                    }
                    None => {
                        decoded.undecodable.push(byte);
                        index += 1;
                    }
                }
            }
        }
    }
    decoded
}

/// Printable rendering of raw bytes for a log receipt.
pub(crate) fn escape_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| match byte {
            0x20..=0x7e => (*byte as char).to_string(),
            _ => format!("\\x{byte:02x}"),
        })
        .collect()
}

/// What the replay did, for the caller's own bookkeeping.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct StartupInputReceipt {
    /// Key events pushed onto the pending queue.
    pub(crate) replayed: usize,
    /// Bytes startup consumed that never became key events.
    pub(crate) dropped: Vec<u8>,
}

impl StartupInputReceipt {
    /// Whether the shell can prove it saw everything the user typed. When
    /// this is false the composer holds the next submit instead of sending
    /// a line it cannot vouch for.
    pub(crate) fn whole_line_proven(&self) -> bool {
        self.dropped.is_empty()
    }
}

/// Replay everything startup consumed into `pending`, oldest byte first.
///
/// Call once, before the input pump is spawned, so replayed keys are ahead
/// of anything the pump reads next. Emits an INFO receipt whenever startup
/// touched the user's input at all — a future report of a mangled command
/// then has a line naming the exact bytes.
pub(crate) fn replay_into(pending: &mut VecDeque<Event>) -> StartupInputReceipt {
    let carried = osc11::take_carried_type_ahead();
    let mut dropped = osc11::take_consumed_unreplayable();
    let mut decoded = decode(&carried);
    dropped.extend_from_slice(&decoded.undecodable);

    // A queued Enter is not a fresh acknowledgement of missing startup
    // bytes. Preserve line boundaries as literal text, so even multiple
    // queued Enters cannot clear the hold and then submit a damaged line.
    // The next real submit still goes through the composer's existing hold.
    if !dropped.is_empty() {
        for event in &mut decoded.events {
            if *event == plain_key(KeyCode::Enter) {
                *event = plain_key(KeyCode::Char('\n'));
            }
        }
    }

    let receipt = StartupInputReceipt {
        replayed: decoded.events.len(),
        dropped,
    };
    if carried.is_empty() && receipt.dropped.is_empty() {
        return receipt;
    }
    // The replay is prepended, not appended: these bytes were consumed
    // before anything still sitting in the tty, so they must be delivered
    // first or the line is reordered.
    for event in decoded.events.into_iter().rev() {
        pending.push_front(event);
    }
    tracing::info!(
        target: "startup_input",
        consumed_bytes = carried.len(),
        replayed_keys = receipt.replayed,
        replayed = %escape_bytes(&carried),
        dropped_bytes = receipt.dropped.len(),
        dropped = %escape_bytes(&receipt.dropped),
        "startup terminal probes consumed typed-ahead input; replaying it into the composer"
    );
    receipt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_a_typed_slash_command_line_in_order() {
        let decoded = decode(b"/plugin list\r");
        let typed: String = decoded
            .events
            .iter()
            .filter_map(|event| match event {
                Event::Key(KeyEvent {
                    code: KeyCode::Char(ch),
                    ..
                }) => Some(*ch),
                _ => None,
            })
            .collect();
        assert_eq!(typed, "/plugin list");
        assert_eq!(
            decoded.events.last(),
            Some(&plain_key(KeyCode::Enter)),
            "the trailing carriage return must replay as Enter"
        );
        assert!(decoded.undecodable.is_empty());
    }

    #[test]
    fn crlf_replays_as_one_enter() {
        let decoded = decode(b"hi\r\n");
        assert_eq!(
            decoded
                .events
                .iter()
                .filter(|e| **e == plain_key(KeyCode::Enter))
                .count(),
            1
        );
    }

    #[test]
    fn delayed_color_replies_are_consumed_without_damaging_typeahead() {
        for reply in [
            "\x1b]11;rgb:1e1e/1e1e/1e1e\x1b\\",
            "\x1b]11;rgb:1e/1e/1e\x07",
            "\x1b]11;#1e1e1e\x07",
        ] {
            assert_eq!(
                decode(format!("/plugin{reply} list\r").as_bytes()),
                decode(b"/plugin list\r"),
                "late reply: {reply:?}"
            );
        }
        let literal = "11;rgb:1e1e/1e1e/1e1e";
        assert_eq!(
            decode(literal.as_bytes()).events,
            literal
                .chars()
                .map(|c| plain_key(KeyCode::Char(c)))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn unknown_or_incomplete_control_strings_never_replay_their_payload() {
        for sequence in [
            "\x1b]11;rgb:1e/1e/1egarbage\x07",
            "\x1b]11;rgb:1e/1e/1e\r\r\x07",
            "\x1b]52;payload\r\r\x1b\\",
            "\x1b_payload\r\r\x1b\\",
            "\x1bPpayload\r\r\x1b\\",
            "\x1b^payload\r\r\x1b\\",
            "\x1bXpayload\r\r\x1b\\",
            "\x1b]11;rgb:1e/1e/1e",
            "\x1b_Gi=31;OK",
            "\x1b_Gi=32;OK\x1b\\",
            "\x1b_Gi=31;OK\r\r\x1b\\",
        ] {
            let decoded = decode(sequence.as_bytes());
            assert!(decoded.events.is_empty(), "{sequence:?}: {decoded:?}");
            assert_eq!(decoded.undecodable, sequence.as_bytes());
        }
    }

    #[test]
    fn complete_focus_and_kitty_probe_replies_do_not_damage_input() {
        for reply in ["\x1b[I", "\x1b[O", "\x1b_Gi=31;OK\x1b\\"] {
            assert_eq!(
                decode(format!("/plugin{reply} list\r").as_bytes()),
                decode(b"/plugin list\r"),
                "terminal metadata: {reply:?}"
            );
        }
    }

    #[test]
    fn escape_sequences_and_invalid_utf8_are_reported_never_guessed() {
        let decoded = decode(b"a\x1b[Ab\xff");
        // The arrow key is dropped whole — replaying its tail would type
        // `[A` into the composer. Only `a` and `b` come back as keys.
        assert_eq!(decoded.undecodable, b"\x1b[A\xff".to_vec());
        assert_eq!(decoded.events.len(), 2);
    }

    #[test]
    fn a_lone_escape_at_the_end_is_one_dropped_byte() {
        let decoded = decode(b"hi\x1b");
        assert_eq!(decoded.undecodable, vec![0x1b]);
        assert_eq!(decoded.events.len(), 2);
    }

    #[test]
    fn multibyte_text_survives_the_round_trip() {
        let decoded = decode("héllo→".as_bytes());
        let typed: String = decoded
            .events
            .iter()
            .filter_map(|event| match event {
                Event::Key(KeyEvent {
                    code: KeyCode::Char(ch),
                    ..
                }) => Some(*ch),
                _ => None,
            })
            .collect();
        assert_eq!(typed, "héllo→");
        assert!(decoded.undecodable.is_empty());
    }

    #[test]
    fn a_receipt_with_dropped_bytes_does_not_prove_the_whole_line() {
        let proven = StartupInputReceipt {
            replayed: 3,
            dropped: Vec::new(),
        };
        assert!(proven.whole_line_proven());
        let lossy = StartupInputReceipt {
            replayed: 3,
            dropped: vec![0x1b],
        };
        assert!(!lossy.whole_line_proven());
    }

    #[test]
    fn escape_bytes_names_unprintable_bytes() {
        assert_eq!(escape_bytes(b"/pl\x1b"), "/pl\\x1b");
    }
}
