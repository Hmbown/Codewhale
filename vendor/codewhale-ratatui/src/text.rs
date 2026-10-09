//! Text that is safe to measure and paint.
//!
//! Every string a caller passes can come from a model, a tool or a file name.
//! ratatui 0.30 already drops control characters when it writes cells, so an
//! escape sequence cannot drive the terminal. Bidirectional overrides pass,
//! though, and can make `rm -rf ~/x` display as something else, so every
//! component runs caller text through [`display_safe`] first.

use std::borrow::Cow;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::glyphs;

/// Characters that reorder or hide text without drawing anything.
fn is_hidden_control(c: char) -> bool {
    matches!(c,
        '\u{202A}'..='\u{202E}'   // LRE RLE PDF LRO RLO
        | '\u{2066}'..='\u{2069}' // LRI RLI FSI PDI
        | '\u{200E}' | '\u{200F}' | '\u{061C}' // LRM RLM ALM
    ) || c.is_control()
}

/// `text` without bidi controls or other control characters.
#[must_use]
pub fn display_safe(text: &str) -> Cow<'_, str> {
    if text.chars().any(is_hidden_control) {
        Cow::Owned(text.chars().filter(|c| !is_hidden_control(*c)).collect())
    } else {
        Cow::Borrowed(text)
    }
}

/// Display width in terminal cells.
#[must_use]
pub fn width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

/// Fit `text` into `max` cells, ending with `…` (`...` when ASCII-safe)
/// when anything was cut. Cuts between graphemes, so a wide character or a
/// combining sequence is never split. Use it for names, paths and IDs.
#[must_use]
pub fn truncate(text: &str, max: usize, ascii: bool) -> Cow<'_, str> {
    cut(text, max, ascii, false)
}

/// Like [`truncate`], but for prose: the cut lands between words where it
/// can, because a clipped clause reads as a sentence and a clipped word
/// reads as a bug. From the engine's `ui_text::semantic_truncate`.
#[must_use]
pub fn truncate_words(text: &str, max: usize, ascii: bool) -> Cow<'_, str> {
    cut(text, max, ascii, true)
}

fn cut(text: &str, max: usize, ascii: bool, at_word: bool) -> Cow<'_, str> {
    if width(text) <= max {
        return Cow::Borrowed(text);
    }
    if max == 0 {
        return Cow::Borrowed("");
    }
    // ASCII has no one-cell ellipsis, and a lone `.` makes a clipped
    // sentence read as finished. Spend three cells where there are four.
    let ellipsis = match (ascii, max) {
        (false, _) => glyphs::ELLIPSIS,
        (true, 4..) => "...",
        (true, _) => ".",
    };
    let budget = max.saturating_sub(width(ellipsis));
    let mut used = 0;
    let mut end = 0;
    let mut word_end = None;
    let mut in_word = false;
    for (at, g) in text.grapheme_indices(true) {
        let w = width(g);
        if used + w > budget {
            break;
        }
        used += w;
        end = at + g.len();
        if g.chars().all(char::is_whitespace) {
            if in_word {
                word_end = Some(at);
            }
            in_word = false;
        } else {
            in_word = true;
        }
    }
    let body = match word_end {
        Some(word_end) if at_word => text[..word_end].trim_end(),
        _ => text[..end].trim_end(),
    };
    let body = if body.is_empty() {
        text[..end].trim_end()
    } else {
        body
    };
    Cow::Owned(format!("{body}{ellipsis}"))
}

/// Pad `text` with spaces to exactly `cells` wide (truncating first).
#[must_use]
pub fn pad(text: &str, cells: usize, ascii: bool) -> String {
    let fitted = truncate(text, cells, ascii);
    let gap = cells.saturating_sub(width(&fitted));
    format!("{fitted}{}", " ".repeat(gap))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bidi_overrides_are_stripped() {
        let spoof = "rm -rf ~/\u{202E}txt.exe";
        assert_eq!(display_safe(spoof), "rm -rf ~/txt.exe");
        assert!(matches!(display_safe("plain"), Cow::Borrowed(_)));
        assert_eq!(display_safe("a\u{1b}[31mb"), "a[31mb");
    }

    #[test]
    fn truncate_respects_cell_width() {
        assert_eq!(truncate("Shoreline", 20, false), "Shoreline");
        assert_eq!(truncate("Shoreline light", 10, false), "Shoreline…");
        // In ASCII a cut says so: `Shoreline.` would read as a full stop.
        assert_eq!(truncate("Shoreline light", 10, true), "Shoreli...");
        assert_eq!(truncate("Shoreline light", 3, true), "Sh.");
        // A wide character is never split.
        assert_eq!(truncate("鲸鱼鲸鱼", 5, false), "鲸鱼…");
        assert_eq!(width(&pad("ab", 4, false)), 4);
        // A combining sequence stays whole.
        assert_eq!(truncate("cafe\u{301} au lait", 6, false), "cafe\u{301}…");
    }

    #[test]
    fn prose_is_cut_between_words() {
        let hint = "Works in this session; asks before edits and shell commands";
        assert_eq!(
            truncate_words(hint, 40, false),
            "Works in this session; asks before…"
        );
        assert_eq!(
            truncate(hint, 40, false),
            "Works in this session; asks before edit…"
        );
        // One long word still fits by cutting inside it.
        assert_eq!(truncate_words("supercalifragilistic", 8, false), "superca…");
    }
}
