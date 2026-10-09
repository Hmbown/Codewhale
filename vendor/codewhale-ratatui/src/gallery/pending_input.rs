//! Pending input and attached context as fixture data. The host owns the
//! queue and every mutation; these entries only show what a caller reported.

use ratatui::{buffer::Buffer, layout::Rect};

use super::Entry;
use crate::{
    ContextPreviewItem, ContextPreviewState, Paint, PendingCard, PendingCardContext,
    PendingCardWords, PendingInputItem, PendingInputPreview, PendingInputStatus, Theme,
};

fn queued(area: Rect, buf: &mut Buffer, theme: &Theme) {
    PendingInputPreview::new(
        vec![
            PendingInputItem::new(
                "q-1",
                "Tighten the parser tests before landing",
                PendingInputStatus::Queued,
            )
            .label("tests"),
            PendingInputItem::new(
                "q-2",
                "Run the release checklist after the tests pass",
                PendingInputStatus::Queued,
            ),
        ],
        Vec::new(),
    )
    .selected("q-1")
    .paint(area, buf, theme);
}

fn steering(area: Rect, buf: &mut Buffer, theme: &Theme) {
    PendingInputPreview::new(
        vec![
            PendingInputItem::new(
                "s-1",
                "Keep the migration reversible",
                PendingInputStatus::Steering,
            ),
            PendingInputItem::new(
                "s-2",
                "Also check the 256-color profile",
                PendingInputStatus::Steering,
            )
            .label("review"),
            PendingInputItem::new(
                "e-1",
                "Rewording the second paragraph",
                PendingInputStatus::Editing,
            )
            .label("draft"),
        ],
        Vec::new(),
    )
    .selected("e-1")
    .paint(area, buf, theme);
}

fn paused(area: Rect, buf: &mut Buffer, theme: &Theme) {
    PendingInputPreview::new(
        vec![
            PendingInputItem::new(
                "p-1",
                "Restart the shell after the upgrade",
                PendingInputStatus::Paused,
            ),
            PendingInputItem::new(
                "f-1",
                "Comparing the two snapshots",
                PendingInputStatus::InFlight,
            ),
        ],
        Vec::new(),
    )
    .selected("p-1")
    .paint(area, buf, theme);
}

fn attached_context(area: Rect, buf: &mut Buffer, theme: &Theme) {
    PendingInputPreview::new(
        vec![PendingInputItem::new(
            "q-1",
            "Continue from these notes",
            PendingInputStatus::Queued,
        )],
        vec![
            ContextPreviewItem::new("c-1", "notes.md", ContextPreviewState::Included)
                .detail("4.2 kB"),
            ContextPreviewItem::new("c-2", "draft.md", ContextPreviewState::Unconfirmed),
            ContextPreviewItem::new("c-3", "old.log", ContextPreviewState::Removable)
                .detail("stale"),
        ],
    )
    .paint(area, buf, theme);
}

fn native_mixed(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let mut card = PendingCard::new(PendingCardWords::default());
    card.priority_rows
        .push("1 child request needs attention".into());
    card.context = vec![
        PendingCardContext {
            kind: "file".into(),
            label: "notes.md".into(),
            detail: Some("4.2 kB".into()),
            included: true,
            removable: true,
            selected: false,
        },
        PendingCardContext {
            kind: "file".into(),
            label: "draft.md".into(),
            detail: None,
            included: false,
            removable: true,
            selected: true,
        },
    ];
    card.sending.push("Check the parser boundary".into());
    card.editing = Some("Keep the migration reversible".into());
    card.queued.push("Compare the two snapshots".into());
    card.paint(area, buf, theme);
}

fn native_queued(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let mut card = PendingCard::new(PendingCardWords::default());
    card.queued = vec!["Check the narrow layout".into(), "Read the receipt".into()];
    card.paint(area, buf, theme);
}

pub(crate) fn entries() -> Vec<Entry> {
    vec![
        Entry {
            name: "pending-queued",
            width: 40,
            height: 4,
            draw: queued,
        },
        Entry {
            name: "pending-steering",
            width: 40,
            height: 5,
            draw: steering,
        },
        Entry {
            name: "pending-paused",
            width: 40,
            height: 4,
            draw: paused,
        },
        Entry {
            name: "pending-context",
            width: 40,
            height: 4,
            draw: attached_context,
        },
        Entry {
            name: "pending-native-mixed",
            width: 40,
            height: 15,
            draw: native_mixed,
        },
        Entry {
            name: "pending-native-queued",
            width: 40,
            height: 2,
            draw: native_queued,
        },
    ]
}
