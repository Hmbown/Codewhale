//! The dot-whale pet as a ratatui widget.
//!
//! `pet_core` is the same `rs/pet_sim.rs` the standalone runner compiles —
//! one file, one authority. This crate adds only the render path: particles →
//! braille cells → ratatui `Buffer`, coloured with the frame's computed RGB.
//!
//! Demo mode (no real terminal needed — renders through TestBackend and emits
//! truecolor ANSI so you can see the actual frame):
//!   cargo run --offline -- ../tape.tsv [--frame N]
//! With `--live` it runs the tape looped on a real crossterm-less ANSI stream.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

#[path = "../../rs/pet_sim.rs"]
mod pet_sim;
use pet_sim::*;

#[path = "../../../crates/tui/src/tui/ambient_life/pet_widget.rs"]
mod pet_widget;
use pet_widget::PetWidget;

fn parse_row(c: Vec<&str>) -> Option<(f64, PetState)> {
    if c.len() < 10 || c[0] == "dt" { return None; }
    let channel = ChannelId::from_key(c[4]).expect("known tape channel");
    Some((c[0].parse().unwrap(), PetState {
        activity: c[1].parse().unwrap(), coherence: c[2].parse().unwrap(),
        attention: c[3].parse().unwrap(), channel,
        observed: c[5].parse().unwrap(), roam_x: c[6].parse().unwrap(),
        roam_y: c[7].parse().unwrap(), flip: c[8].parse().unwrap(), lit: c[9].parse().unwrap(),
    }))
}

/// Render one frame through ratatui's TestBackend and emit it as truecolor
/// ANSI — what a real terminal would show, without needing a PTY.
fn emit_ansi(buf: &Buffer, area: Rect) {
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = &buf[(x, y)];
            let sym = cell.symbol();
            if sym == " " { print!(" "); continue; }
            match cell.fg {
                Color::Rgb(r, g, b) => print!("\x1b[38;2;{r};{g};{b}m{sym}\x1b[0m"),
                _ => print!("{sym}"),
            }
        }
        println!();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let motion = !args.iter().any(|s| s == "--reduced-motion");
    let tape_path = args.get(1).map(|s| s.as_str()).unwrap_or("../tape.tsv");
    let frame_target: Option<usize> = args.iter().position(|a| a == "--frame")
        .and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok());
    let tape = std::fs::read_to_string(tape_path).expect("tape.tsv");
    let rows: Vec<(f64, PetState)> =
        tape.trim().lines().filter_map(|l| parse_row(l.split('\t').collect())).collect();

    let mut sim = PetSim::whale();
    let (w, h) = (90u16, 30u16);
    let backend = ratatui::backend::TestBackend::new(w, h);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();

    let frames: Vec<usize> = match frame_target {
        Some(f) => vec![f],
        None => vec![60, 240, 480, 780, 1080, 1320, 1560, 1800],
    };

    let mut next = frames.iter().peekable();
    for (i, (dt, s)) in rows.iter().enumerate() {
        sim.step(*dt, s, motion, 1.0);
        if next.peek() == Some(&&i) {
            terminal.draw(|f| {
                let wgt = PetWidget { sim: &sim, state: s };
                f.render_widget(wgt, f.area());
            }).unwrap();
            println!("── tape frame {i} · {} · {} ──", sim.frame.channel, sim.frame.arch);
            emit_ansi(terminal.backend().buffer(), Rect::new(0, 0, w, h));
            next.next();
            if next.peek().is_none() { break; }
        }
    }
}
