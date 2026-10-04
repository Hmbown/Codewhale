use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::time::Duration;

use super::{
    read_terminal_reply, settle_terminal_reply, take_carried_type_ahead, take_consumed_unreplayable,
};

#[test]
fn terminal_reply_reader_leaves_one_burst_typed_suffix_on_the_descriptor() {
    struct Case {
        query: &'static [u8],
        response: &'static [u8],
        reply: &'static [u8],
        csi: bool,
    }
    let cases = [
        Case {
            query: b"\x1b]11;?\x07",
            response: b"\x1b]11;rgb:1e1e/1e1e/1e1e\x07",
            reply: b"\x1b]11;rgb:1e1e/1e1e/1e1e",
            csi: false,
        },
        Case {
            query: b"\x1b]11;?\x1b\\",
            response: b"\x1b]11;rgb:1e1e/1e1e/1e1e\x1b\\",
            reply: b"\x1b]11;rgb:1e1e/1e1e/1e1e",
            csi: false,
        },
        Case {
            query: b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\",
            response: b"\x1b_Gi=31;OK\x1b\\",
            reply: b"\x1b_Gi=31;OK",
            csi: false,
        },
        Case {
            query: b"\x1b[c",
            response: b"\x1b[?62;4c",
            reply: b"\x1b[?62;4c",
            csi: true,
        },
    ];

    for case in cases {
        let (mut reader, mut writer) = std::io::pipe().expect("create isolated input pipe");
        let prefix = b"/plu";
        let suffix = b"gin list\r";
        let burst = [prefix.as_slice(), case.response, suffix.as_slice()].concat();
        assert_eq!(
            writer.write(&burst).expect("write one input burst"),
            burst.len()
        );

        // Keep the writer open: a buffered read-ahead must not be rescued by
        // EOF/readable-HUP while the real tty would have no new bytes ready.
        let (answered, reply, carried) = read_terminal_reply(
            reader.as_raw_fd(),
            case.query,
            Duration::from_secs(1),
            case.csi,
        );
        drop(writer);
        let mut remaining = Vec::new();
        reader
            .read_to_end(&mut remaining)
            .expect("read remaining typed input");

        assert!(
            answered,
            "complete reply was already on the pipe: {:?}",
            case.response
        );
        assert_eq!(reply, case.reply, "query {:?}", case.query);
        assert_eq!(
            carried, prefix,
            "type-ahead before the reply must be replayable"
        );
        assert_eq!(
            remaining, suffix,
            "the input pump must still receive the exact suffix"
        );
    }
}

#[test]
fn terminal_reply_reader_retains_incomplete_control_reply_separately_from_typeahead() {
    let (reader, mut writer) = std::io::pipe().expect("create isolated input pipe");
    let prefix = b"/plu";
    let incomplete = b"\x1b]11;rgb:1e/1e\r\r";
    let burst = [prefix.as_slice(), incomplete.as_slice()].concat();
    assert_eq!(
        writer.write(&burst).expect("write incomplete reply"),
        burst.len()
    );
    drop(writer);

    let (answered, reply, carried) = read_terminal_reply(
        reader.as_raw_fd(),
        b"\x1b]11;?\x07",
        Duration::from_secs(1),
        false,
    );

    assert!(
        !answered,
        "an unterminated control reply is not a valid answer"
    );
    assert_eq!(
        reply, incomplete,
        "the caller must account for every unreplayable byte"
    );
    assert_eq!(
        carried, prefix,
        "control payload and its Enters must not become user input"
    );
}

/// The probe hands type-ahead to the event loop, and never hands it an
/// unterminated control reply. One test owns the process-wide buffers so
/// parallel tests cannot race on them.
#[test]
fn the_probe_parks_type_ahead_for_the_event_loop() {
    let _ = take_carried_type_ahead();
    let _ = take_consumed_unreplayable();

    let (reader, mut writer) = std::io::pipe().expect("create isolated input pipe");
    writer
        .write_all(b"ls -\x1b]11;rgb:fafa/f8f8/f5f5\x07")
        .expect("write burst");
    let reply = settle_terminal_reply(
        reader.as_raw_fd(),
        b"\x1b]11;?\x07",
        Duration::from_secs(1),
        false,
    );
    drop(writer);
    assert_eq!(
        reply.as_deref(),
        Some(b"\x1b]11;rgb:fafa/f8f8/f5f5".as_slice())
    );
    assert_eq!(take_carried_type_ahead(), b"ls -");
    assert!(take_consumed_unreplayable().is_empty());

    let (reader, mut writer) = std::io::pipe().expect("create isolated input pipe");
    writer.write_all(b"q\x1b]11;rgb:1e").expect("write burst");
    drop(writer);
    let reply = settle_terminal_reply(
        reader.as_raw_fd(),
        b"\x1b]11;?\x07",
        Duration::from_secs(1),
        false,
    );
    assert_eq!(reply, None);
    assert_eq!(take_carried_type_ahead(), b"q");
    assert_eq!(take_consumed_unreplayable(), b"\x1b]11;rgb:1e");
}
