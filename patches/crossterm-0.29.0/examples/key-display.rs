//! Demonstrates the display format of key events.
//!
//! This example demonstrates the display format of key events, which is useful for displaying in
//! the help section of a terminal application.
//!
//! cargo run --example key-display

use std::io;

use crossterm::event::KeyModifiers;
use crossterm::{
    event::{read, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};

const HELP: &str = r#"Key display
 - Press any key to see its display format
 - Use Esc to quit
"#;

fn main() -> io::Result<()> {
    println!("{}", HELP);
    enable_raw_mode()?;
    if let Err(e) = print_events() {
        println!("Error: {:?}\r", e);
    }
    disable_raw_mode()?;
    Ok(())
}

fn print_events() -> io::Result<()> {
    while let Ok(event) = read() {
        let Some(event) = event.as_key_press_event() else {
            continue;
        };
        let modifier = match event.modifiers {
            KeyModifiers::NONE => "".to_string(),
            _ => format!("{:}+", event.modifiers),
        };
        println!("Key pressed: {modifier}{code}\r", code = event.code);
        if event.code == KeyCode::Esc {
            break;
        }
    }
    Ok(())
}
