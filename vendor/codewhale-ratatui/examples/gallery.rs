//! Browse every component in every terminal profile.
//!
//!   cargo run --example gallery                         # browse interactively
//!   cargo run --example gallery -- --print dark-256     # print every component once
//!   cargo run --example gallery -- --dump out/          # write every component x profile
//!   cargo run --example gallery -- --svg target/readme-buffers # real buffers as SVG
//!
//! Browsing: ↑↓ choose a component, p / Shift+P change the profile, w /
//! Shift+W change the width (the component's own, then 40, 80 and 120
//! columns), PgUp/PgDn scroll tall previews, Home/End jump through them,
//! q quit. Every area's entries (`src/gallery/*.rs`) are listed.
//! `--print` writes ANSI to stdout, so you can see a profile in any terminal
//! without raw mode. `--dump` writes `<component>.<profile>.ans` (view with
//! `cat`) and `.txt` (glyphs plus the roles each run was painted with).
//!
//! Profiles: dark-truecolor, dark-graphite, light-truecolor, dark-256,
//! light-256, ansi-16, unknown-ground, no-color, ascii.

use std::io::{self, Write as _};
use std::path::Path;

use codewhale_ratatui::{
    Depth, KeyHint, KeyHints, Paint, Panel, Picker, PickerItem, PickerState, Role, Theme, gallery,
    testing::{self, Profile},
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::Rect,
    text::{Line, Span},
    widgets::Widget,
};

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--dump") => dump(
            Path::new(args.get(1).map_or("gallery-out", String::as_str)),
            false,
        ),
        Some("--svg") => dump(
            Path::new(args.get(1).map_or("target/readme-buffers", String::as_str)),
            true,
        ),
        Some("--print") => print(args.get(1).map(String::as_str)),
        Some("--help" | "-h") => {
            println!("usage: gallery [--print [profile] | --dump [dir] | --svg [dir]]");
            Ok(())
        }
        None => browse(),
        Some(arg) => Err(io::Error::other(format!(
            "unknown argument {arg}; use --help"
        ))),
    }
}

fn profile_arg(name: Option<&str>) -> io::Result<Vec<Profile>> {
    match name {
        None => Ok(Profile::ALL.to_vec()),
        Some(name) => Profile::from_name(name).map(|p| vec![p]).ok_or_else(|| {
            let names: Vec<_> = Profile::ALL.iter().map(|p| p.name()).collect();
            io::Error::other(format!(
                "unknown profile {name}; one of {}",
                names.join(", ")
            ))
        }),
    }
}

fn print(profile: Option<&str>) -> io::Result<()> {
    let mut out = io::stdout().lock();
    for profile in profile_arg(profile)? {
        let theme = profile.theme();
        for entry in gallery::entries() {
            writeln!(out, "\n{} · {}", entry.name, profile.name())?;
            out.write_all(testing::ansi(&gallery::render(&entry, &theme)).as_bytes())?;
        }
    }
    Ok(())
}

fn dump(dir: &Path, svg: bool) -> io::Result<()> {
    let entries = gallery::entries();
    let names: std::collections::BTreeSet<_> = entries.iter().map(|e| e.name).collect();
    let safe_name = |name: &str| {
        !name.is_empty()
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    };
    if names.len() != entries.len() || names.iter().any(|name| !safe_name(name)) {
        return Err(io::Error::other(
            "gallery entry names must be unique safe basenames",
        ));
    }
    let manifest_path = dir.join("manifest.json");
    let previous: Vec<serde_json::Value> = if svg && manifest_path.exists() {
        serde_json::from_slice(&std::fs::read(&manifest_path)?)?
    } else {
        Vec::new()
    };
    let previous_names = previous
        .iter()
        .map(|entry| entry["name"].as_str().filter(|name| safe_name(name)))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| io::Error::other("invalid name in previous SVG manifest"))?;
    std::fs::create_dir_all(dir)?;
    let mut written = 0;
    for profile in Profile::ALL {
        let theme = profile.theme();
        for entry in &entries {
            let buf = gallery::render(entry, &theme);
            let stem = format!("{}.{}", entry.name, profile.name());
            if svg {
                std::fs::write(
                    dir.join(format!("{stem}.svg")),
                    testing::svg(&buf, &gallery::theme_for(entry, &theme)),
                )?;
                written += 1;
            } else {
                std::fs::write(dir.join(format!("{stem}.ans")), testing::ansi(&buf))?;
                std::fs::write(
                    dir.join(format!("{stem}.txt")),
                    testing::styled(&buf, &gallery::theme_for(entry, &theme)),
                )?;
                written += 2;
            }
        }
    }
    if svg {
        // Only the prior manifest owns obsolete exports; preserve unrelated files.
        for name in previous_names.iter().filter(|name| !names.contains(**name)) {
            for profile in Profile::ALL {
                let path = dir.join(format!("{name}.{}.svg", profile.name()));
                match std::fs::remove_file(path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
        }
        let manifest: Vec<_> = entries.iter().map(|entry| {
            serde_json::json!({"name": entry.name, "width": entry.width, "height": entry.height})
        }).collect();
        std::fs::write(manifest_path, serde_json::to_string_pretty(&manifest)?)?;
    }
    println!("Wrote {written} files to {}", dir.display());
    Ok(())
}

// Restore the terminal even when construction or drawing fails.
struct RestoreTerminal;
impl Drop for RestoreTerminal {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

fn browse() -> io::Result<()> {
    enable_raw_mode()?;
    let _restore = RestoreTerminal;
    codewhale_ratatui::detect::probe_terminal_background();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    run(&mut terminal)
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let entries = gallery::entries();
    let items: Vec<PickerItem> = entries.iter().map(|e| PickerItem::new(e.name)).collect();
    let mut state = PickerState::new(0);
    // `None` is this terminal as detected; otherwise a forced profile.
    let mut profile: Option<usize> = None;
    let mut scroll = 0u16;
    let mut viewport_height = 1u16;
    let mut content_height = 1u16;
    // `None` is the component's own width; otherwise one of `testing::WIDTHS`.
    let mut width: Option<usize> = None;
    let mut expanded = false;
    loop {
        let theme = profile.map_or_else(Theme::detect, |i| Profile::ALL[i].theme());
        let profile_name = profile.map_or("this terminal", |i| Profile::ALL[i].name());
        let forced_width = width.map(|i| testing::WIDTHS[i]);
        terminal.draw(|frame| {
            let area = frame.area();
            let buf = frame.buffer_mut();
            let hints = KeyHints::new(vec![
                KeyHint::new(if theme.ascii() { "Up/Down" } else { "↑↓" }, "choose"),
                KeyHint::new("p", "next profile"),
                KeyHint::new("PgUp/PgDn", "scroll"),
                KeyHint::new("w", "next width"),
                KeyHint::new("f", "expand"),
                KeyHint::new("q", "quit"),
            ]);
            let inner = Panel::new(Depth::Stage)
                .title("Codewhale components")
                .aside(profile_name)
                .hints(&hints)
                .draw(area, buf, &theme);
            let rail = Rect {
                width: if expanded || inner.width < 40 { 0 } else { 20 },
                ..inner
            };
            let rail_inner = Panel::new(Depth::Deep).draw(rail, buf, &theme);
            state.scroll_into_view(items.len(), rail_inner.height);
            Picker::new(&items, state).paint(rail_inner, buf, &theme);

            let gap = if rail.width == 0 { 0 } else { 2 };
            let stage = Rect {
                x: rail.right().saturating_add(gap),
                width: inner.width.saturating_sub(rail.width + gap),
                ..inner
            };
            let entry = &entries[state.selected];
            let render_width = forced_width.unwrap_or(entry.width).min(stage.width);
            let capacity = if entry.name.contains("scene") {
                entry.height
            } else {
                entry
                    .height
                    .saturating_mul(entry.width.div_ceil(render_width.max(1)))
                    .max(1)
            };
            let full = gallery::render_at(entry, &theme, render_width, capacity);
            content_height = content_rows(&full, entry.height);
            let canvas = Rect {
                y: stage.y.saturating_add(2),
                width: render_width,
                height: content_height.min(stage.height.saturating_sub(2)),
                ..stage
            }
            .intersection(inner);
            viewport_height = canvas.height.max(1);
            scroll = scroll.min(content_height.saturating_sub(canvas.height));
            let position = if content_height > canvas.height {
                format!(
                    " · rows {}-{} of {}",
                    scroll + 1,
                    scroll + canvas.height,
                    content_height
                )
            } else {
                String::new()
            };
            Line::from(Span::styled(
                format!(
                    "{} · {}x{}{position}",
                    entry.name, render_width, content_height
                ),
                theme.fg(Role::Muted),
            ))
            .render(
                Rect {
                    height: stage.height.min(1),
                    ..stage
                },
                buf,
            );
            copy_viewport(&full, canvas, scroll, buf);
        })?;
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Up | KeyCode::Char('k') => {
                    state.prev(items.len());
                    scroll = 0;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    state.next(items.len());
                    scroll = 0;
                }
                KeyCode::PageDown => scroll = scroll.saturating_add(viewport_height),
                KeyCode::PageUp => scroll = scroll.saturating_sub(viewport_height),
                KeyCode::Home => scroll = 0,
                KeyCode::End => scroll = content_height,
                KeyCode::Char('f') => {
                    expanded = !expanded;
                    scroll = 0;
                }
                KeyCode::Char('p') if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                    profile = match profile {
                        None => Some(0),
                        Some(i) if i + 1 < Profile::ALL.len() => Some(i + 1),
                        Some(_) => None,
                    };
                }
                KeyCode::Char('w') if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                    scroll = 0;
                    width = match width {
                        None => Some(0),
                        Some(i) if i + 1 < testing::WIDTHS.len() => Some(i + 1),
                        Some(_) => None,
                    };
                }
                KeyCode::Char('W') | KeyCode::Char('w') => {
                    scroll = 0;
                    width = match width {
                        None => Some(testing::WIDTHS.len() - 1),
                        Some(0) => None,
                        Some(i) => Some(i - 1),
                    };
                }
                KeyCode::Char('P') | KeyCode::Char('p') => {
                    profile = match profile {
                        None => Some(Profile::ALL.len() - 1),
                        Some(0) => None,
                        Some(i) => Some(i - 1),
                    };
                }
                _ => {}
            }
        }
    }
}

// Extra capacity accommodates wrapping; don't let End land in empty capacity.
fn content_rows(buf: &ratatui::buffer::Buffer, minimum: u16) -> u16 {
    let occupied = if buf.area.width == 0 {
        0
    } else {
        buf.content
            .chunks(usize::from(buf.area.width))
            .rposition(|row| {
                row.iter()
                    .any(|cell| cell.symbol().chars().any(|c| !c.is_whitespace()))
            })
            .map_or(0, |row| u16::try_from(row + 1).unwrap_or(u16::MAX))
    };
    occupied.max(minimum.min(buf.area.height))
}

/// Copy an actual rendered buffer into the visible scroll window.
fn copy_viewport(
    source: &ratatui::buffer::Buffer,
    area: Rect,
    offset: u16,
    dest: &mut ratatui::buffer::Buffer,
) {
    let area = area.intersection(dest.area);
    for row in 0..area.height.min(source.area.height.saturating_sub(offset)) {
        for col in 0..area.width.min(source.area.width) {
            dest[(area.x + col, area.y + row)] =
                source[(source.area.x + col, source.area.y + offset + row)].clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scroll_can_reach_last_row_and_stays_in_viewport() {
        use ratatui::buffer::Buffer;
        let mut source = Buffer::empty(Rect::new(0, 0, 2, 66));
        source[(0, 65)].set_symbol("Z");
        let mut dest = Buffer::filled(Rect::new(0, 0, 8, 8), ratatui::buffer::Cell::new("."));
        copy_viewport(&source, Rect::new(3, 2, 2, 3), 63, &mut dest);
        assert_eq!(dest[(3, 4)].symbol(), "Z");
        assert_eq!(dest[(2, 4)].symbol(), ".");
        assert_eq!(dest[(5, 4)].symbol(), ".");
    }

    #[test]
    fn end_reaches_the_final_whale_action_without_scrolling_into_empty_capacity() {
        let entry = gallery::entries()
            .into_iter()
            .find(|entry| entry.name == "whale-actions")
            .unwrap();
        let theme = Profile::DarkTrue.theme();
        for width in [80, 84] {
            let full = gallery::render_at(&entry, &theme, width, 132);
            let height = content_rows(&full, entry.height);
            assert_eq!(height, 66);
            let mut view = ratatui::buffer::Buffer::empty(Rect::new(0, 0, width, 20));
            let area = view.area;
            copy_viewport(&full, area, height - 20, &mut view);
            assert!(testing::text(&view).contains("Calling a connected app"));
        }
    }
}
