//! Gallery: the mode and status-line pickers, rebuilt from kit parts. They
//! double as usage examples for [`Picker`] inside a [`Panel`].

use std::borrow::Cow;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::Widget,
};

use super::{Entry, arrows};
use crate::{
    Depth, KeyHint, KeyHints, Paint, Panel, Picker, PickerItem, PickerMatches, PickerState,
    PickerTabs, Role, State, StatusMark, Theme, centered, text,
};

/// The engine's `/mode` rows (`AppMode::Agent`, `Plan`, `Operate`; English
/// names from `locales/en.json`), with the hints rewritten verbs first.
fn mode_items() -> Vec<PickerItem> {
    vec![
        PickerItem::new("Work")
            .key('1')
            .detail("Works in this session; asks before edits and commands"),
        PickerItem::new("Plan")
            .key('2')
            .detail("Researches without changing files, then proposes a plan"),
        PickerItem::new("Operate")
            .key('3')
            .detail("Runs parallel agents on your goal and checks their work"),
    ]
}

fn mode_picker(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let items = mode_items();
    let hints = KeyHints::new(vec![
        KeyHint::new(arrows(theme), "move"),
        KeyHint::new("Enter", "select"),
        KeyHint::new("Esc", "cancel"),
    ]);
    let popup = centered(area, 72, items.len() as u16 + 8, 44, 8);
    let inner = Panel::new(Depth::Overlay)
        .title("Mode")
        .focused(true)
        .hints(&hints)
        .draw(popup, buf, theme);
    Picker::new(&items, PickerState::new(0)).paint(inner, buf, theme);
}

/// The engine's `/statusline` rows (`StatusItem::all()`, labels and hints
/// from `crates/tui/src/config.rs`), with the default footer checked.
fn status_items() -> Vec<PickerItem> {
    let row = |label: &'static str, on: bool, detail: &'static str| {
        PickerItem::new(label).checked(on).detail(detail)
    };
    vec![
        row("Mode", true, "Work, Plan or Operate"),
        row("Model", true, "The model the next message goes to"),
        row(
            "Context window %",
            true,
            "Tokens used of the model's context window",
        ),
        row("Session cost", true, "Running total for this session"),
        row(
            "Account balance",
            false,
            "Prepaid credit left with the active provider",
        ),
        row(
            "Prompt cache hit rate",
            true,
            "Share of the prompt served from cache",
        ),
        row(
            "Output tokens",
            true,
            "Output tokens of the live or last turn",
        ),
        row(
            "Time to first token",
            true,
            "Average wait for the first token",
        ),
        row("Output rate", true, "Average tokens per second"),
        row("Workspace", false, "The directory this session writes to"),
        PickerItem::new("Git branch")
            .checked(false)
            .disabled("Not a git repository"),
    ]
}

fn status_picker(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let items = status_items();
    let shown = items.iter().filter(|i| i.checked == Some(true)).count();
    let hints = KeyHints::new(vec![
        KeyHint::new("Space", "toggle"),
        KeyHint::new("a", "all"),
        KeyHint::new("n", "none"),
        KeyHint::new("Enter", "save"),
        KeyHint::new("Esc", "cancel"),
    ]);
    let popup = centered(area, 72, 18, 40, 8);
    let aside = format!("{shown} of {} shown", items.len());
    let inner = Panel::new(Depth::Overlay)
        .title("Status line")
        .aside(aside)
        .hints(&hints)
        .draw(popup, buf, theme);
    Line::from(Span::styled(
        "Choose what the footer shows.",
        theme.fg(Role::Muted),
    ))
    .render(
        Rect {
            height: inner.height.min(1),
            ..inner
        },
        buf,
    );
    let list = Rect {
        y: inner.y.saturating_add(2),
        height: inner.height.saturating_sub(2),
        ..inner
    };
    let mut state = PickerState::new(9);
    state.scroll_into_view(items.len(), list.height);
    Picker::new(&items, state).paint(list, buf, theme);
}

/// Themes, each in a tab, for the fuzzy picker.
fn theme_items() -> Vec<PickerItem> {
    vec![
        PickerItem::new("Shoreline").detail("Graphite ground, follows the terminal"),
        PickerItem::new("Shoreline light").detail("Paper ground"),
        PickerItem::new("Deep sea").detail("Navy ground, blue ombre"),
        PickerItem::new("Graphite").detail("The exact token grounds"),
        PickerItem::new("Reef").detail("Warm dark ground"),
        PickerItem::new("Tidepool").detail("Teal accents on dark"),
        PickerItem::new("Sandbar").detail("Warm light ground"),
        PickerItem::new("Lighthouse")
            .detail("High contrast light")
            .disabled("Needs a light terminal"),
    ]
}

fn theme_tabs() -> (Vec<Cow<'static, str>>, Vec<Option<usize>>) {
    let labels = vec!["All".into(), "Dark".into(), "Light".into()];
    let of = vec![
        Some(1),
        Some(2),
        Some(1),
        Some(1),
        Some(1),
        Some(1),
        Some(2),
        Some(2),
    ];
    (labels, of)
}

/// The query "shore" over the themes: matched characters underlined, the
/// best match first, the count at the right.
fn picker_query(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let items = theme_items();
    let query = "shore";
    let matches = PickerMatches::rank(&items, query, None);
    Picker::new(&items, PickerState::new(0))
        .query(query, query.len())
        .matches(&matches)
        .paint(area, buf, theme);
}

/// Tabs across the top, and a preview beside the list where there is room
/// (it drops below 56 columns).
fn picker_tabs_preview(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let items = theme_items();
    let (labels, of) = theme_tabs();
    let tabs = PickerTabs::new(&labels, 1).assign(&of).all(0);
    let matches = PickerMatches::rank(&items, "", Some(&tabs));
    let preview = |area: Rect, buf: &mut Buffer, theme: &Theme, item: &PickerItem, _: usize| {
        let (width, ascii) = (usize::from(area.width), theme.ascii());
        let label = text::display_safe(&item.label);
        let detail = text::display_safe(item.detail.as_deref().unwrap_or_default());
        let mut lines = vec![
            Line::from(Span::styled(
                text::truncate(&label, width, ascii).into_owned(),
                theme
                    .fg(Role::Foreground)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            )),
            Line::from(Span::styled(
                text::truncate_words(&detail, width, ascii).into_owned(),
                theme.fg(Role::Muted),
            )),
        ];
        for state in [State::Working, State::NeedsYou, State::Done] {
            lines.push(Line::from(StatusMark::new(state).spans(theme)));
        }
        for (i, line) in lines.into_iter().enumerate() {
            let rect = Rect {
                y: area.y + i as u16,
                height: 1,
                ..area
            };
            if rect.y < area.bottom() {
                line.render(rect, buf);
            }
        }
    };
    Picker::new(&items, PickerState::new(0))
        .query("", 0)
        .tabs(tabs)
        .matches(&matches)
        .preview(&preview)
        .paint(area, buf, theme);
}

/// Nothing matches: the empty state names the query and the next step.
fn picker_no_match(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let items = theme_items();
    let query = "zebra";
    let matches = PickerMatches::rank(&items, query, None);
    Picker::new(&items, PickerState::new(0))
        .query(query, query.len())
        .matches(&matches)
        .paint(area, buf, theme);
}

pub(crate) fn entries() -> Vec<Entry> {
    vec![
        Entry {
            name: "mode-picker",
            width: 80,
            height: 13,
            draw: mode_picker,
        },
        Entry {
            name: "status-picker",
            width: 80,
            height: 20,
            draw: status_picker,
        },
        Entry {
            name: "picker-query",
            width: 64,
            height: 6,
            draw: picker_query,
        },
        Entry {
            name: "picker-tabs-preview",
            width: 80,
            height: 8,
            draw: picker_tabs_preview,
        },
        Entry {
            name: "picker-no-match",
            width: 64,
            height: 8,
            draw: picker_no_match,
        },
    ]
}
