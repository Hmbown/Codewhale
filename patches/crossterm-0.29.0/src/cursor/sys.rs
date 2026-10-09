//! This module provides platform related functions.

#[cfg(unix)]
#[cfg(feature = "events")]
pub use self::unix::position;
#[cfg(windows)]
#[cfg(feature = "events")]
pub use self::windows::position;
#[cfg(windows)]
pub(crate) use self::windows::{
    move_down, move_left, move_right, move_to, move_to_column, move_to_next_line,
    move_to_previous_line, move_to_row, move_up, restore_position, save_position, show_cursor,
};

#[cfg(windows)]
pub(crate) mod windows;

#[cfg(unix)]
#[cfg(feature = "events")]
pub(crate) mod unix;
