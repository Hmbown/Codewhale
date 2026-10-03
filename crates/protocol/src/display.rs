//! Pure path and size display shared by host and portable commands.
use std::path::Path;

/// Quote an OS path for terminals, logs, JSON display fields, and errors.
///
/// The result is always one line. Terminal controls, line separators, bidi
/// formatting controls, quotes, and backslashes are escaped. Unix paths keep
/// non-UTF-8 bytes exact as `\xNN`; Windows preserves unpaired UTF-16 units as
/// `\u{NNNN}`.
#[must_use]
pub fn quote_os_path(path: &Path) -> String {
    quote_os_path_inner(path)
}

#[cfg(unix)]
fn quote_os_path_inner(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt as _;
    let bytes = path.as_os_str().as_bytes();
    if let Ok(text) = std::str::from_utf8(bytes) {
        return quote_path_text(text);
    }
    let mut out = String::from("\"");
    for byte in bytes {
        match byte {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            0x20..=0x7e => out.push(char::from(*byte)),
            _ => out.push_str(&format!("\\x{byte:02x}")),
        }
    }
    out.push('"');
    out
}

#[cfg(windows)]
fn quote_os_path_inner(path: &Path) -> String {
    use std::os::windows::ffi::OsStrExt as _;
    let mut out = String::from("\"");
    for decoded in char::decode_utf16(path.as_os_str().encode_wide()) {
        match decoded {
            Ok(character) => push_escaped_path_character(&mut out, character),
            Err(error) => out.push_str(&format!("\\u{{{:04x}}}", error.unpaired_surrogate())),
        }
    }
    out.push('"');
    out
}

#[cfg(not(any(unix, windows)))]
fn quote_os_path_inner(path: &Path) -> String {
    quote_path_text(&path.to_string_lossy())
}

#[cfg(not(windows))]
fn quote_path_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        push_escaped_path_character(&mut out, character);
    }
    out.push('"');
    out
}

fn push_escaped_path_character(out: &mut String, character: char) {
    match character {
        '"' => out.push_str("\\\""),
        '\\' => out.push_str("\\\\"),
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        '\u{1b}' => out.push_str("\\x1b"),
        character if character.is_control() || is_bidi_format_control(character) => {
            out.extend(character.escape_unicode());
        }
        character => out.push(character),
    }
}

fn is_bidi_format_control(character: char) -> bool {
    matches!(
        character,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

pub fn display_path_with_home(path: &Path, home: Option<&Path>) -> String {
    let Some(home) = home else {
        return path.display().to_string();
    };
    if let Ok(rest) = path.strip_prefix(home) {
        if rest.as_os_str().is_empty() {
            return "~".to_string();
        }
        let sep = std::path::MAIN_SEPARATOR_STR;
        let mut out = String::from("~");
        for component in rest.components() {
            out.push_str(sep);
            out.push_str(&component.as_os_str().to_string_lossy());
        }
        return out;
    }
    path.display().to_string()
}

pub fn format_byte_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    if bytes >= MIB {
        format!("{} MB", bytes.div_ceil(MIB))
    } else if bytes >= KIB {
        format!("{} KB", bytes.div_ceil(KIB))
    } else {
        format!("{bytes} B")
    }
}

/// Substitute placeholders in the template once; runtime values are opaque.
pub fn interpolate(template: &str, replacements: &[(&str, &str)]) -> String {
    let mut message = String::with_capacity(template.len());
    let mut cursor = 0;
    while let Some(relative_start) = template[cursor..].find('{') {
        let start = cursor + relative_start;
        message.push_str(&template[cursor..start]);
        let Some(relative_end) = template[start..].find('}') else {
            message.push_str(&template[start..]);
            return message;
        };
        let end = start + relative_end + 1;
        let placeholder = &template[start..end];
        if let Some(value) = replacements
            .iter()
            .find_map(|(candidate, value)| (*candidate == placeholder).then_some(*value))
        {
            message.push_str(value);
        } else {
            message.push_str(placeholder);
        }
        cursor = end;
    }
    message.push_str(&template[cursor..]);
    message
}
