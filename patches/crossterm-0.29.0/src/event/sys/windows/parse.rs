use crossterm_winapi::{ControlKeyState, EventFlags, KeyEventRecord, ScreenBuffer};
use winapi::um::{
    wincon::{
        CAPSLOCK_ON, LEFT_ALT_PRESSED, LEFT_CTRL_PRESSED, RIGHT_ALT_PRESSED, RIGHT_CTRL_PRESSED,
        SHIFT_PRESSED,
    },
    winuser::{
        GetForegroundWindow, GetKeyboardLayout, GetWindowThreadProcessId, ToUnicodeEx, VK_BACK,
        VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_F1, VK_F24, VK_HOME, VK_INSERT,
        VK_LEFT, VK_MENU, VK_NEXT, VK_NUMPAD0, VK_NUMPAD9, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_SHIFT,
        VK_TAB, VK_UP,
    },
};

use crate::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

#[derive(Default)]
pub struct MouseButtonsPressed {
    pub(crate) left: bool,
    pub(crate) right: bool,
    pub(crate) middle: bool,
}

pub(crate) fn handle_mouse_event(
    mouse_event: crossterm_winapi::MouseEvent,
    buttons_pressed: &MouseButtonsPressed,
) -> Option<Event> {
    if let Ok(Some(event)) = parse_mouse_event_record(&mouse_event, buttons_pressed) {
        return Some(Event::Mouse(event));
    }

    None
}

enum WindowsKeyEvent {
    KeyEvent(KeyEvent),
    Surrogate(u16),
}

pub(crate) fn handle_key_event(
    key_event: KeyEventRecord,
    surrogate_buffer: &mut Option<u16>,
) -> Option<Event> {
    let windows_key_event = parse_key_event_record(&key_event)?;
    match windows_key_event {
        WindowsKeyEvent::KeyEvent(key_event) => {
            // Discard any buffered surrogate value if another valid key event comes before the
            // next surrogate value.
            *surrogate_buffer = None;
            Some(Event::Key(key_event))
        }
        WindowsKeyEvent::Surrogate(new_surrogate) => {
            let ch = handle_surrogate(surrogate_buffer, new_surrogate)?;
            let modifiers = KeyModifiers::from(&key_event.control_key_state);
            let key_event = KeyEvent::new(KeyCode::Char(ch), modifiers);
            Some(Event::Key(key_event))
        }
    }
}

fn handle_surrogate(surrogate_buffer: &mut Option<u16>, new_surrogate: u16) -> Option<char> {
    match *surrogate_buffer {
        Some(buffered_surrogate) => {
            *surrogate_buffer = None;
            std::char::decode_utf16([buffered_surrogate, new_surrogate])
                .next()
                .unwrap()
                .ok()
        }
        None => {
            *surrogate_buffer = Some(new_surrogate);
            None
        }
    }
}

impl From<&ControlKeyState> for KeyModifiers {
    fn from(state: &ControlKeyState) -> Self {
        let shift = state.has_state(SHIFT_PRESSED);
        let alt = state.has_state(LEFT_ALT_PRESSED | RIGHT_ALT_PRESSED);
        let control = state.has_state(LEFT_CTRL_PRESSED | RIGHT_CTRL_PRESSED);

        let mut modifier = KeyModifiers::empty();

        if shift {
            modifier |= KeyModifiers::SHIFT;
        }
        if control {
            modifier |= KeyModifiers::CONTROL;
        }
        if alt {
            modifier |= KeyModifiers::ALT;
        }

        modifier
    }
}

enum CharCase {
    LowerCase,
    UpperCase,
}

fn try_ensure_char_case(ch: char, desired_case: CharCase) -> char {
    match desired_case {
        CharCase::LowerCase if ch.is_uppercase() => {
            let mut iter = ch.to_lowercase();
            // Unwrap is safe; iterator yields one or more chars.
            let ch_lower = iter.next().unwrap();
            if iter.next().is_none() {
                ch_lower
            } else {
                ch
            }
        }
        CharCase::UpperCase if ch.is_lowercase() => {
            let mut iter = ch.to_uppercase();
            // Unwrap is safe; iterator yields one or more chars.
            let ch_upper = iter.next().unwrap();
            if iter.next().is_none() {
                ch_upper
            } else {
                ch
            }
        }
        _ => ch,
    }
}

// Attempts to return the character for a key event accounting for the user's keyboard layout.
// The returned character (if any) is capitalized (if applicable) based on shift and capslock state.
// Returns None if the key doesn't map to a character or if it is a dead key.
// We use the *currently* active keyboard layout (if it can be determined). This layout may not
// correspond to the keyboard layout that was active when the user typed their input, since console
// applications get their input asynchronously from the terminal. By the time a console application
// can process a key input, the user may have changed the active layout. In this case, the character
// returned might not correspond to what the user expects, but there is no way for a console
// application to know what the keyboard layout actually was for a key event, so this is our best
// effort. If a console application processes input in a timely fashion, then it is unlikely that a
// user has time to change their keyboard layout before a key event is processed.
fn get_char_for_key(key_event: &KeyEventRecord) -> Option<char> {
    let virtual_key_code = key_event.virtual_key_code as u32;
    let virtual_scan_code = key_event.virtual_scan_code as u32;
    let key_state = [0u8; 256];
    let mut utf16_buf = [0u16, 16];
    let dont_change_kernel_keyboard_state = 0x4;

    // Best-effort attempt at determining the currently active keyboard layout.
    // At the time of writing, this works for a console application running in Windows Terminal, but
    // doesn't work under a Conhost terminal. For Conhost, the window handle returned by
    // GetForegroundWindow() does not appear to actually be the foreground window which has the
    // keyboard layout associated with it (or perhaps it is, but also has special protection that
    // doesn't allow us to query it).
    // When this determination fails, the returned keyboard layout handle will be null, which is an
    // acceptable input for ToUnicodeEx, as that argument is optional. In this case ToUnicodeEx
    // appears to use the keyboard layout associated with the current thread, which will be the
    // layout that was inherited when the console application started (or possibly when the current
    // thread was spawned). This is then unfortunately not updated when the user changes their
    // keyboard layout in the terminal, but it's what we get.
    let active_keyboard_layout = unsafe {
        let foreground_window = GetForegroundWindow();
        let foreground_thread = GetWindowThreadProcessId(foreground_window, std::ptr::null_mut());
        GetKeyboardLayout(foreground_thread)
    };

    let ret = unsafe {
        ToUnicodeEx(
            virtual_key_code,
            virtual_scan_code,
            key_state.as_ptr(),
            utf16_buf.as_mut_ptr(),
            utf16_buf.len() as i32,
            dont_change_kernel_keyboard_state,
            active_keyboard_layout,
        )
    };

    // -1 indicates a dead key.
    // 0 indicates no character for this key.
    if ret < 1 {
        return None;
    }

    let mut ch_iter = std::char::decode_utf16(utf16_buf.into_iter().take(ret as usize));
    let mut ch = ch_iter.next()?.ok()?;
    if ch_iter.next().is_some() {
        // Key doesn't map to a single char.
        return None;
    }

    let is_shift_pressed = key_event.control_key_state.has_state(SHIFT_PRESSED);
    let is_capslock_on = key_event.control_key_state.has_state(CAPSLOCK_ON);
    let desired_case = if is_shift_pressed ^ is_capslock_on {
        CharCase::UpperCase
    } else {
        CharCase::LowerCase
    };
    ch = try_ensure_char_case(ch, desired_case);
    Some(ch)
}

fn parse_key_event_record(key_event: &KeyEventRecord) -> Option<WindowsKeyEvent> {
    let modifiers = KeyModifiers::from(&key_event.control_key_state);
    let virtual_key_code = key_event.virtual_key_code as i32;

    // We normally ignore all key release events, but we will make an exception for an Alt key
    // release if it carries a u_char value, as this indicates an Alt code.
    let is_alt_code = virtual_key_code == VK_MENU && !key_event.key_down && key_event.u_char != 0;
    if is_alt_code {
        let utf16 = key_event.u_char;
        match utf16 {
            surrogate @ 0xD800..=0xDFFF => {
                return Some(WindowsKeyEvent::Surrogate(surrogate));
            }
            unicode_scalar_value => {
                // Unwrap is safe: We tested for surrogate values above and those are the only
                // u16 values that are invalid when directly interpreted as unicode scalar
                // values.
                let ch = std::char::from_u32(unicode_scalar_value as u32).unwrap();
                let key_code = KeyCode::Char(ch);
                let kind = if key_event.key_down {
                    KeyEventKind::Press
                } else {
                    KeyEventKind::Release
                };
                let key_event = KeyEvent::new_with_kind(key_code, modifiers, kind);
                return Some(WindowsKeyEvent::KeyEvent(key_event));
            }
        }
    }

    // Don't generate events for numpad key presses when they're producing Alt codes.
    let is_numpad_numeric_key = (VK_NUMPAD0..=VK_NUMPAD9).contains(&virtual_key_code);
    let is_only_alt_modifier = modifiers.contains(KeyModifiers::ALT)
        && !modifiers.contains(KeyModifiers::SHIFT | KeyModifiers::CONTROL);
    if is_only_alt_modifier && is_numpad_numeric_key {
        return None;
    }

    let parse_result = match virtual_key_code {
        VK_SHIFT | VK_CONTROL | VK_MENU => None,
        VK_BACK => Some(KeyCode::Backspace),
        VK_ESCAPE => Some(KeyCode::Esc),
        VK_RETURN => Some(KeyCode::Enter),
        VK_F1..=VK_F24 => Some(KeyCode::F((key_event.virtual_key_code - 111) as u8)),
        VK_LEFT => Some(KeyCode::Left),
        VK_UP => Some(KeyCode::Up),
        VK_RIGHT => Some(KeyCode::Right),
        VK_DOWN => Some(KeyCode::Down),
        VK_PRIOR => Some(KeyCode::PageUp),
        VK_NEXT => Some(KeyCode::PageDown),
        VK_HOME => Some(KeyCode::Home),
        VK_END => Some(KeyCode::End),
        VK_DELETE => Some(KeyCode::Delete),
        VK_INSERT => Some(KeyCode::Insert),
        VK_TAB if modifiers.contains(KeyModifiers::SHIFT) => Some(KeyCode::BackTab),
        VK_TAB => Some(KeyCode::Tab),
        _ => {
            let utf16 = key_event.u_char;
            match utf16 {
                0x00..=0x1f => {
                    // Some key combinations generate either no u_char value or generate control
                    // codes. To deliver back a KeyCode::Char(...) event we want to know which
                    // character the key normally maps to on the user's keyboard layout.
                    // The keys that intentionally generate control codes (ESC, ENTER, TAB, etc.)
                    // are handled by their virtual key codes above.
                    get_char_for_key(key_event).map(KeyCode::Char)
                }
                surrogate @ 0xD800..=0xDFFF => {
                    // Codewhale patch: Windows Terminal sends a key-down and a key-up record
                    // for each UTF-16 half of a pasted astral character. Pairing a half with
                    // its own key-up decodes to nothing and drops the character, so only the
                    // key-down halves take part in surrogate pairing.
                    if !key_event.key_down {
                        return None;
                    }
                    return Some(WindowsKeyEvent::Surrogate(surrogate));
                }
                unicode_scalar_value => {
                    // Unwrap is safe: We tested for surrogate values above and those are the only
                    // u16 values that are invalid when directly interpreted as unicode scalar
                    // values.
                    let ch = std::char::from_u32(unicode_scalar_value as u32).unwrap();
                    Some(KeyCode::Char(ch))
                }
            }
        }
    };

    if let Some(key_code) = parse_result {
        let kind = if key_event.key_down {
            KeyEventKind::Press
        } else {
            KeyEventKind::Release
        };
        let key_event = KeyEvent::new_with_kind(key_code, modifiers, kind);
        return Some(WindowsKeyEvent::KeyEvent(key_event));
    }

    None
}

// The 'y' position of a mouse event or resize event is not relative to the window but absolute to screen buffer.
// This means that when the mouse cursor is at the top left it will be x: 0, y: 2295 (e.g. y = number of cells conting from the absolute buffer height) instead of relative x: 0, y: 0 to the window.
pub fn parse_relative_y(y: i16) -> std::io::Result<i16> {
    let window_size = ScreenBuffer::current()?.info()?.terminal_window();
    Ok(y - window_size.top)
}

fn parse_mouse_event_record(
    event: &crossterm_winapi::MouseEvent,
    buttons_pressed: &MouseButtonsPressed,
) -> std::io::Result<Option<MouseEvent>> {
    let modifiers = KeyModifiers::from(&event.control_key_state);

    let xpos = event.mouse_position.x as u16;
    let ypos = parse_relative_y(event.mouse_position.y)? as u16;

    let button_state = event.button_state;

    let kind = match event.event_flags {
        EventFlags::PressOrRelease | EventFlags::DoubleClick => {
            if button_state.left_button() && !buttons_pressed.left {
                Some(MouseEventKind::Down(MouseButton::Left))
            } else if !button_state.left_button() && buttons_pressed.left {
                Some(MouseEventKind::Up(MouseButton::Left))
            } else if button_state.right_button() && !buttons_pressed.right {
                Some(MouseEventKind::Down(MouseButton::Right))
            } else if !button_state.right_button() && buttons_pressed.right {
                Some(MouseEventKind::Up(MouseButton::Right))
            } else if button_state.middle_button() && !buttons_pressed.middle {
                Some(MouseEventKind::Down(MouseButton::Middle))
            } else if !button_state.middle_button() && buttons_pressed.middle {
                Some(MouseEventKind::Up(MouseButton::Middle))
            } else {
                None
            }
        }
        EventFlags::MouseMoved => {
            let button = if button_state.right_button() {
                MouseButton::Right
            } else if button_state.middle_button() {
                MouseButton::Middle
            } else {
                MouseButton::Left
            };
            if button_state.release_button() {
                Some(MouseEventKind::Moved)
            } else {
                Some(MouseEventKind::Drag(button))
            }
        }
        EventFlags::MouseWheeled => {
            // Vertical scroll
            // from https://docs.microsoft.com/en-us/windows/console/mouse-event-record-str
            // if `button_state` is negative then the wheel was rotated backward, toward the user.
            if button_state.scroll_down() {
                Some(MouseEventKind::ScrollDown)
            } else if button_state.scroll_up() {
                Some(MouseEventKind::ScrollUp)
            } else {
                None
            }
        }
        EventFlags::MouseHwheeled => {
            if button_state.scroll_left() {
                Some(MouseEventKind::ScrollLeft)
            } else if button_state.scroll_right() {
                Some(MouseEventKind::ScrollRight)
            } else {
                None
            }
        }
        _ => None,
    };

    Ok(kind.map(|kind| MouseEvent {
        kind,
        column: xpos,
        row: ypos,
        modifiers,
    }))
}
