//! WinAPI related logic to cursor manipulation.

use std::convert::TryFrom;
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};

use crossterm_winapi::{result, Coord, Handle, HandleType, ScreenBuffer};
use winapi::{
    shared::minwindef::{FALSE, TRUE},
    um::wincon::{SetConsoleCursorInfo, SetConsoleCursorPosition, CONSOLE_CURSOR_INFO, COORD},
};

/// The position of the cursor, written when you save the cursor's position.
///
/// This is `u64::MAX` initially. Otherwise, it stores the cursor's x position bit-shifted left 16
/// times or-ed with the cursor's y position, where both are `i16`s.
static SAVED_CURSOR_POS: AtomicU64 = AtomicU64::new(u64::MAX);

// The 'y' position of the cursor is not relative to the window but absolute to screen buffer.
// We can calculate the relative cursor position by subtracting the top position of the terminal window from the y position.
// This results in an 1-based coord zo subtract 1 to make cursor position 0-based.
pub fn parse_relative_y(y: i16) -> std::io::Result<i16> {
    let window = ScreenBuffer::current()?.info()?;

    let window_size = window.terminal_window();
    let screen_size = window.terminal_size();

    if y <= screen_size.height {
        Ok(y)
    } else {
        Ok(y - window_size.top)
    }
}

/// Returns the cursor position (column, row).
///
/// The top left cell is represented `0,0`.
pub fn position() -> io::Result<(u16, u16)> {
    let cursor = ScreenBufferCursor::output()?;
    let mut position = cursor.position()?;
    //    if position.y != 0 {
    position.y = parse_relative_y(position.y)?;
    //    }
    Ok(position.into())
}

pub(crate) fn show_cursor(show_cursor: bool) -> std::io::Result<()> {
    ScreenBufferCursor::from(Handle::current_out_handle()?).set_visibility(show_cursor)
}

pub(crate) fn move_to(column: u16, row: u16) -> std::io::Result<()> {
    let cursor = ScreenBufferCursor::output()?;
    cursor.move_to(column as i16, row as i16)?;
    Ok(())
}

pub(crate) fn move_up(count: u16) -> std::io::Result<()> {
    let (column, row) = position()?;
    move_to(column, row - count)?;
    Ok(())
}

pub(crate) fn move_right(count: u16) -> std::io::Result<()> {
    let (column, row) = position()?;
    move_to(column + count, row)?;
    Ok(())
}

pub(crate) fn move_down(count: u16) -> std::io::Result<()> {
    let (column, row) = position()?;
    move_to(column, row + count)?;
    Ok(())
}

pub(crate) fn move_left(count: u16) -> std::io::Result<()> {
    let (column, row) = position()?;
    move_to(column - count, row)?;
    Ok(())
}

pub(crate) fn move_to_column(new_column: u16) -> std::io::Result<()> {
    let (_, row) = position()?;
    move_to(new_column, row)?;
    Ok(())
}

pub(crate) fn move_to_row(new_row: u16) -> std::io::Result<()> {
    let (col, _) = position()?;
    move_to(col, new_row)?;
    Ok(())
}

pub(crate) fn move_to_next_line(count: u16) -> std::io::Result<()> {
    let (_, row) = position()?;
    move_to(0, row + count)?;
    Ok(())
}

pub(crate) fn move_to_previous_line(count: u16) -> std::io::Result<()> {
    let (_, row) = position()?;
    move_to(0, row - count)?;
    Ok(())
}

pub(crate) fn save_position() -> std::io::Result<()> {
    ScreenBufferCursor::output()?.save_position()?;
    Ok(())
}

pub(crate) fn restore_position() -> std::io::Result<()> {
    ScreenBufferCursor::output()?.restore_position()?;
    Ok(())
}

/// WinAPI wrapper over terminal cursor behaviour.
struct ScreenBufferCursor {
    screen_buffer: ScreenBuffer,
}

impl ScreenBufferCursor {
    fn output() -> std::io::Result<ScreenBufferCursor> {
        Ok(ScreenBufferCursor {
            screen_buffer: ScreenBuffer::from(Handle::new(HandleType::CurrentOutputHandle)?),
        })
    }

    fn position(&self) -> std::io::Result<Coord> {
        Ok(self.screen_buffer.info()?.cursor_pos())
    }

    fn move_to(&self, x: i16, y: i16) -> std::io::Result<()> {
        if x < 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Argument Out of Range Exception when setting cursor position to X: {x}"),
            ));
        }

        if y < 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Argument Out of Range Exception when setting cursor position to Y: {y}"),
            ));
        }

        let position = COORD { X: x, Y: y };

        unsafe {
            if result(SetConsoleCursorPosition(
                **self.screen_buffer.handle(),
                position,
            ))
            .is_err()
            {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    fn set_visibility(&self, visible: bool) -> std::io::Result<()> {
        let cursor_info = CONSOLE_CURSOR_INFO {
            dwSize: 100,
            bVisible: if visible { TRUE } else { FALSE },
        };

        unsafe {
            if result(SetConsoleCursorInfo(
                **self.screen_buffer.handle(),
                &cursor_info,
            ))
            .is_err()
            {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    fn restore_position(&self) -> std::io::Result<()> {
        if let Ok(val) = u32::try_from(SAVED_CURSOR_POS.load(Ordering::Relaxed)) {
            let x = (val >> 16) as i16;
            let y = val as i16;
            self.move_to(x, y)?;
        }

        Ok(())
    }

    fn save_position(&self) -> std::io::Result<()> {
        let position = self.position()?;

        let upper = u32::from(position.x as u16) << 16;
        let lower = u32::from(position.y as u16);
        let bits = u64::from(upper | lower);
        SAVED_CURSOR_POS.store(bits, Ordering::Relaxed);

        Ok(())
    }
}

impl From<Handle> for ScreenBufferCursor {
    fn from(handle: Handle) -> Self {
        ScreenBufferCursor {
            screen_buffer: ScreenBuffer::from(handle),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        move_down, move_left, move_right, move_to, move_to_column, move_to_next_line,
        move_to_previous_line, move_to_row, move_up, position, restore_position, save_position,
    };
    use crate::terminal::sys::temp_screen_buffer;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_move_to_winapi() {
        let _test_screen = temp_screen_buffer().unwrap();

        let (saved_x, saved_y) = position().unwrap();

        move_to(saved_x + 1, saved_y + 1).unwrap();
        assert_eq!(position().unwrap(), (saved_x + 1, saved_y + 1));

        move_to(saved_x, saved_y).unwrap();
        assert_eq!(position().unwrap(), (saved_x, saved_y));
    }

    #[test]
    #[serial]
    fn test_move_right_winapi() {
        let _test_screen = temp_screen_buffer().unwrap();

        let (saved_x, saved_y) = position().unwrap();
        move_right(1).unwrap();
        assert_eq!(position().unwrap(), (saved_x + 1, saved_y));
    }

    #[test]
    #[serial]
    fn test_move_left_winapi() {
        let _test_screen = temp_screen_buffer().unwrap();

        move_to(2, 0).unwrap();

        move_left(2).unwrap();

        assert_eq!(position().unwrap(), (0, 0));
    }

    #[test]
    #[serial]
    fn test_move_up_winapi() {
        let _test_screen = temp_screen_buffer().unwrap();

        move_to(0, 2).unwrap();

        move_up(2).unwrap();

        assert_eq!(position().unwrap(), (0, 0));
    }

    #[test]
    #[serial]
    fn test_move_to_next_line_winapi() {
        let _test_screen = temp_screen_buffer().unwrap();

        move_to(0, 2).unwrap();

        move_to_next_line(2).unwrap();

        assert_eq!(position().unwrap(), (0, 4));
    }

    #[test]
    #[serial]
    fn test_move_to_previous_line_winapi() {
        let _test_screen = temp_screen_buffer().unwrap();

        move_to(0, 2).unwrap();

        move_to_previous_line(2).unwrap();

        assert_eq!(position().unwrap(), (0, 0));
    }

    #[test]
    #[serial]
    fn test_move_to_column_winapi() {
        let _test_screen = temp_screen_buffer().unwrap();

        move_to(0, 2).unwrap();

        move_to_column(12).unwrap();

        assert_eq!(position().unwrap(), (12, 2));
    }

    #[test]
    #[serial]
    fn test_move_to_row_winapi() {
        let _test_screen = temp_screen_buffer().unwrap();

        move_to(0, 2).unwrap();

        move_to_row(5).unwrap();

        assert_eq!(position().unwrap(), (0, 5));
    }

    #[test]
    #[serial]
    fn test_move_down_winapi() {
        let _test_screen = temp_screen_buffer().unwrap();

        move_to(0, 0).unwrap();

        move_down(2).unwrap();

        assert_eq!(position().unwrap(), (0, 2));
    }

    #[test]
    #[serial]
    fn test_save_restore_position_winapi() {
        let _test_screen = temp_screen_buffer().unwrap();

        let (saved_x, saved_y) = position().unwrap();

        save_position().unwrap();
        move_to(saved_x + 1, saved_y + 1).unwrap();
        restore_position().unwrap();

        let (x, y) = position().unwrap();

        assert_eq!(x, saved_x);
        assert_eq!(y, saved_y);
    }
}
