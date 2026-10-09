//! Bounded, dependency-free measurements of actual terminal-buffer rendering.
use codewhale_ratatui::{
    Caps, Message, MotionMode, NativeComposer, OceanColumn, Paint, PostureBar, SessionList,
    SessionRow, TerminalShell, Theme, Workbar, WorkbarPanel, WorkbarRow, color::ColorDepth,
    detect::Appearance, gallery,
};
use ratatui::{buffer::Buffer, layout::Rect};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

fn measure(name: &str, samples: usize, batch: usize, mut render: impl FnMut()) {
    for _ in 0..batch {
        render();
    }
    let mut timings = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        for _ in 0..batch {
            render();
        }
        timings.push(start.elapsed().as_secs_f64() * 1_000_000.0 / batch as f64);
    }
    timings.sort_by(f64::total_cmp);
    println!(
        "{name:32} {:12.3} {:12.3}",
        timings[samples / 2],
        timings[(samples * 95).div_ceil(100).saturating_sub(1)]
    );
}

fn prepared(
    name: &str,
    component: &dyn Paint,
    area: Rect,
    theme: &Theme,
    samples: usize,
    batch: usize,
) {
    let mut buf = Buffer::empty(area);
    measure(name, samples, batch, || {
        buf.reset();
        component.paint(area, &mut buf, theme);
        black_box(&buf);
    });
}

fn main() {
    let quick = std::env::args().any(|arg| arg == "--quick");
    let (samples, batch) = if quick { (5, 2) } else { (31, 20) };
    let theme = Theme::new(Caps {
        depth: ColorDepth::TrueColor,
        ascii: false,
        appearance: Appearance::Dark,
    })
    .tui();
    println!("Codewhale rendering · native Underwater · {samples} samples × {batch} paints");
    println!("{:32} {:>12} {:>12}", "scenario", "median µs", "p95 µs");
    let composer = NativeComposer::new("Review the Unicode text: 海洋 e\u{301} 👩‍💻").focused(true);
    prepared(
        "composer / 112×5",
        &composer,
        Rect::new(0, 0, 112, 5),
        &theme,
        samples,
        batch,
    );
    let dock = Workbar::new(
        WorkbarPanel::Tasks,
        (0..10)
            .map(|i| WorkbarRow::new(i.to_string(), "Review the native component"))
            .collect(),
    );
    prepared(
        "workbar / 112×5",
        &dock,
        Rect::new(0, 0, 112, 5),
        &theme,
        samples,
        batch,
    );
    for count in [10, 100, 10_000] {
        let sessions = SessionList::new(
            (0..count)
                .map(|i| SessionRow::new(format!("{i:08x}"), "A saved Codewhale session"))
                .collect(),
        );
        prepared(
            &format!("sessions {count} / 112×38"),
            &sessions,
            Rect::new(0, 0, 112, 38),
            &theme,
            samples,
            batch,
        );
    }
    let entries = gallery::entries();
    let work = entries
        .iter()
        .find(|entry| entry.name == "showcase-work")
        .expect("native showcase-work fixture");
    let area = Rect::new(0, 0, 112, 38);
    let mut buf = Buffer::empty(area);
    let native_theme = gallery::theme_for(work, &theme);
    // Build foregrounds before the ocean pass: the full gallery fixture
    // already finishes its grounds, which would skip most work on reapply.
    let shell = TerminalShell::new(5).workbar_rows(5);
    let regions = shell.areas(area);
    shell.paint(area, &mut buf, &theme);
    let body = (0..28)
        .map(|i| format!("Review {i}: Keep the native composer, dock and 海洋 text in view."))
        .collect::<Vec<_>>()
        .join("\n");
    Message::native(&body).paint(regions.conversation, &mut buf, &theme);
    composer.paint(regions.composer, &mut buf, &theme);
    PostureBar::new("ask").paint(regions.posture, &mut buf, &theme);
    dock.paint(regions.workbar, &mut buf, &theme);
    let foreground = buf.clone();
    let ocean = OceanColumn::new(Duration::from_millis(22_500), MotionMode::Full);
    measure("ocean + fixture clone / 112×38", samples, batch, || {
        let mut buf = foreground.clone();
        ocean.apply(area, &mut buf, &theme);
        black_box(&buf);
    });
    measure("native work / construct + paint", samples, batch, || {
        buf.reset();
        (work.draw)(area, &mut buf, &native_theme);
        black_box(&buf);
    });
}
