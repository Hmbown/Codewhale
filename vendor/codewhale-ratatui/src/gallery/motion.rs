//! Gallery: one state change caught at fixed moments. The gallery is static,
//! so each entry starts the motions at a base instant and paints a fixed
//! offset after it; nothing reads a clock while painting.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use ratatui::{buffer::Buffer, layout::Rect};

use super::Entry;
use crate::{MotionDemo, MotionMode, MotionPolicy, MotionSet, Paint, Spinner, Theme};

/// The instant every entry measures from. Read once, when the entries are
/// built, because an `Instant` cannot be made without the clock; only
/// differences from it are ever used.
fn base() -> Instant {
    static BASE: OnceLock<Instant> = OnceLock::new();
    *BASE.get_or_init(Instant::now)
}

/// Paint the demo `after` its state change to done, under `mode`.
fn frame(area: Rect, buf: &mut Buffer, theme: &Theme, mode: MotionMode, after: Duration) {
    let (t0, policy) = (base(), MotionPolicy::new(mode, theme));
    let mut motions = MotionSet::new();
    MotionDemo::start(&mut motions, true, t0, policy);
    MotionDemo::new(&motions, t0 + after, policy, true).paint(area, buf, theme);
}

fn working(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let policy = MotionPolicy::new(MotionMode::Full, theme);
    MotionDemo::new(&MotionSet::new(), base(), policy, false).paint(area, buf, theme);
}

fn just_started(area: Rect, buf: &mut Buffer, theme: &Theme) {
    frame(area, buf, theme, MotionMode::Full, Duration::ZERO);
}

fn mid_flight(area: Rect, buf: &mut Buffer, theme: &Theme) {
    frame(
        area,
        buf,
        theme,
        MotionMode::Full,
        Duration::from_millis(100),
    );
}

fn settled(area: Rect, buf: &mut Buffer, theme: &Theme) {
    frame(
        area,
        buf,
        theme,
        MotionMode::Full,
        Duration::from_millis(500),
    );
}

/// Reduced motion, 100 ms in: already the settled frame.
fn reduced(area: Rect, buf: &mut Buffer, theme: &Theme) {
    frame(
        area,
        buf,
        theme,
        MotionMode::Reduced,
        Duration::from_millis(100),
    );
}

/// The spinner under each mode, 3 s into the same work: what `Reduced` and
/// `Still` hold still (kept from the first draft of this package).
fn modes(area: Rect, buf: &mut Buffer, theme: &Theme) {
    for (i, (mode, label)) in [
        (MotionMode::Full, "Working"),
        (MotionMode::Reduced, "Reduced motion"),
        (MotionMode::Still, "Still"),
    ]
    .into_iter()
    .enumerate()
    {
        let row = Rect::new(area.x, area.y.saturating_add(i as u16), area.width, 1);
        Spinner::new(label, Duration::from_secs(3), mode).paint(row.intersection(area), buf, theme);
    }
}

pub(crate) fn entries() -> Vec<Entry> {
    base();
    [
        ("motion-modes", modes as fn(Rect, &mut Buffer, &Theme)),
        ("motion-working", working),
        ("motion-started", just_started),
        ("motion-mid-flight", mid_flight),
        ("motion-settled", settled),
        ("motion-reduced", reduced),
    ]
    .into_iter()
    .map(|(name, draw)| Entry {
        name,
        width: 40,
        height: 3,
        draw,
    })
    .collect()
}
