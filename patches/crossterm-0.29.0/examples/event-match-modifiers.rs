//! Demonstrates how to match on modifiers like: Control, alt, shift.
//!
//! cargo run --example event-match-modifiers

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

fn match_event(event: Event) {
    if let Some(key) = event.as_key_press_event() {
        match key {
            KeyEvent {
                modifiers: KeyModifiers::CONTROL,
                code,
                ..
            } => {
                println!("Control + {:?}", code);
            }
            KeyEvent {
                modifiers: KeyModifiers::SHIFT,
                code,
                ..
            } => {
                println!("Shift + {:?}", code);
            }
            KeyEvent {
                modifiers: KeyModifiers::ALT,
                code,
                ..
            } => {
                println!("Alt + {:?}", code);
            }

            // Match on multiple modifiers:
            KeyEvent {
                code, modifiers, ..
            } => {
                if modifiers == (KeyModifiers::ALT | KeyModifiers::SHIFT) {
                    println!("Alt + Shift {:?}", code);
                } else {
                    println!("({:?}) with key: {:?}", modifiers, code)
                }
            }
        }
    }
}

fn main() {
    match_event(Event::Key(KeyEvent::new(
        KeyCode::Char('z'),
        KeyModifiers::CONTROL,
    )));
    match_event(Event::Key(KeyEvent::new(
        KeyCode::Left,
        KeyModifiers::SHIFT,
    )));
    match_event(Event::Key(KeyEvent::new(
        KeyCode::Delete,
        KeyModifiers::ALT,
    )));
    match_event(Event::Key(KeyEvent::new(
        KeyCode::Right,
        KeyModifiers::ALT | KeyModifiers::SHIFT,
    )));
    match_event(Event::Key(KeyEvent::new(
        KeyCode::Home,
        KeyModifiers::ALT | KeyModifiers::CONTROL,
    )));
}
