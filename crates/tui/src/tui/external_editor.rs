//! External editor support for the composer.
//!
//! Spawns `$VISUAL`/`$EDITOR` (fallback `vi`) on a temp file pre-populated with
//! the composer's current contents. The TUI is suspended for the duration of
//! the edit and re-entered on return. The temp file is cleaned up in all paths
//! (success, editor failure, IO error) via [`tempfile::NamedTempFile`].
//!
//! Reference: codex-rs's `tui/src/external_editor.rs` — the design here mirrors
//! that approach but is synchronous (called inline from the TUI event loop) and
//! handles its own raw-mode toggling rather than relying on the caller.

use std::env;
use std::fs;
use std::io::{self, Stdout, Write};
use std::process::Command;

use crossterm::{
    event::DisableFocusChange,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;
use tempfile::Builder;

use super::color_compat::ColorCompatBackend;

/// Outcome of a single external-editor invocation.
#[derive(Debug, PartialEq, Eq)]
pub enum EditorOutcome {
    /// Editor exited cleanly and the file contents differ from the seed.
    Edited(String),
    /// Editor exited cleanly but the contents are unchanged (or empty after
    /// trimming). The composer should be left as-is.
    Unchanged,
    /// Editor exited non-zero or could not be spawned. The composer should be
    /// left as-is and a status toast shown.
    Cancelled,
}

/// Resolve the editor command, preferring `$VISUAL` over `$EDITOR`, falling
/// back to `vi`. Returns the raw string for the test path; `spawn_editor`
/// splits it via `shlex` (Unix) so users can set `EDITOR="code --wait"`.
fn resolve_editor() -> String {
    env::var("VISUAL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| env::var("EDITOR").ok().filter(|s| !s.trim().is_empty()))
        .unwrap_or_else(|| "vi".to_string())
}

#[cfg(unix)]
fn split_command(raw: &str) -> Option<Vec<String>> {
    shlex::split(raw)
}

#[cfg(not(unix))]
fn split_command(raw: &str) -> Option<Vec<String>> {
    // On Windows we do not support shell-quoted editor commands; treat the
    // full string as the program name.
    if raw.trim().is_empty() {
        None
    } else {
        Some(vec![raw.to_string()])
    }
}

/// Run the external editor without touching terminal state. Exposed for tests.
///
/// Returns:
/// - `Ok(EditorOutcome::Edited(new))` if the editor exited cleanly and the
///   contents differ from `seed`.
/// - `Ok(EditorOutcome::Unchanged)` if the editor exited cleanly but the
///   contents match `seed`.
/// - `Ok(EditorOutcome::Cancelled)` if the editor exited non-zero or could not
///   be spawned.
///
/// The temp file is removed on every path because [`tempfile::NamedTempFile`]
/// is dropped at the end of the function.
pub fn run_editor_raw(seed: &str) -> io::Result<EditorOutcome> {
    let mut tmp = Builder::new()
        .prefix("deepseek-edit-")
        .suffix(".md")
        .tempfile()?;
    tmp.write_all(seed.as_bytes())?;
    tmp.flush()?;
    let path = tmp.path().to_path_buf();

    let raw = resolve_editor();
    let parts = match split_command(&raw) {
        Some(p) if !p.is_empty() => p,
        _ => return Ok(EditorOutcome::Cancelled),
    };

    let mut cmd = Command::new(&parts[0]);
    if parts.len() > 1 {
        cmd.args(&parts[1..]);
    }
    cmd.arg(&path);

    let status = match cmd.status() {
        Ok(s) => s,
        Err(_) => return Ok(EditorOutcome::Cancelled),
    };
    if !status.success() {
        return Ok(EditorOutcome::Cancelled);
    }

    let new = fs::read_to_string(&path)?;
    // tmp goes out of scope here — file is unlinked.
    if new == seed {
        Ok(EditorOutcome::Unchanged)
    } else {
        Ok(EditorOutcome::Edited(new))
    }
}

/// Append the file (and optional line) arguments in the spelling `program`
/// understands.
///
/// Every caller that opens a real file goes through here so the three paths
/// cannot drift on how a line number is spelled. Without a line this is exactly
/// `cmd.arg(path)`, which is what it has always been.
///
/// `file_stem` rather than the whole program name, so an absolute path and a
/// Windows `.exe` suffix both still match. An editor we do not recognize gets
/// the bare path: opening the right file at the wrong line beats a spurious
/// argument the editor treats as a second file to open.
fn push_target_args(cmd: &mut Command, program: &str, path: &std::path::Path, line: Option<u32>) {
    let Some(line) = line else {
        cmd.arg(path);
        return;
    };
    let stem = std::path::Path::new(program)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(program)
        .to_ascii_lowercase();

    match stem.as_str() {
        // The vi family and the editors that copied its `+N` convention.
        "vi" | "vim" | "nvim" | "view" | "gvim" | "nano" | "pico" | "emacs" | "emacsclient"
        | "kak" | "micro" | "joe" => {
            cmd.arg(format!("+{line}"));
            cmd.arg(path);
        }
        // VS Code and its forks need an explicit flag before `file:line`.
        "code" | "code-insiders" | "codium" | "vscodium" | "cursor" | "windsurf" => {
            cmd.arg("--goto");
            cmd.arg(format!("{}:{line}", path.display()));
        }
        // Sublime, Zed and JetBrains launchers all take `file:line` directly.
        "subl" | "sublime_text" | "zed" | "idea" | "pycharm" | "goland" | "clion" | "rustrover"
        | "webstorm" => {
            cmd.arg(format!("{}:{line}", path.display()));
        }
        _ => {
            cmd.arg(path);
        }
    }
}

/// Run the external editor on a real file, in place.
///
/// Unlike [`run_editor_raw`] there is no temp file and no seed: the file on
/// disk *is* the document, so a `hooks.toml` the user edits stays edited even
/// if the editor exits non-zero. The outcome only reports whether the bytes
/// moved, which is what the caller needs in order to decide whether to reload.
pub fn run_editor_on_path(path: &std::path::Path, line: Option<u32>) -> io::Result<EditorOutcome> {
    let before = fs::read_to_string(path).unwrap_or_default();

    let raw = resolve_editor();
    let parts = match split_command(&raw) {
        Some(p) if !p.is_empty() => p,
        _ => return Ok(EditorOutcome::Cancelled),
    };
    let mut cmd = Command::new(&parts[0]);
    if parts.len() > 1 {
        cmd.args(&parts[1..]);
    }
    push_target_args(&mut cmd, &parts[0], path, line);
    let status = match cmd.status() {
        Ok(status) => status,
        Err(_) => return Ok(EditorOutcome::Cancelled),
    };

    let after = fs::read_to_string(path).unwrap_or_default();
    if after == before {
        // A non-zero exit with no change is the editor being quit; a
        // non-zero exit that *did* change the file still changed the file.
        return Ok(if status.success() {
            EditorOutcome::Unchanged
        } else {
            EditorOutcome::Cancelled
        });
    }
    Ok(EditorOutcome::Edited(after))
}

/// Suspend the TUI, run the external editor on `path`, then re-enter it.
///
/// The suspend/resume dance is [`spawn_editor_for_input`]'s, factored so the
/// composer and a file editor cannot drift on terminal-mode restoration.
pub(crate) fn spawn_editor_for_path(
    terminal: &mut Terminal<ColorCompatBackend<Stdout>>,
    use_alt_screen: bool,
    use_mouse_capture: bool,
    use_bracketed_paste: bool,
    path: &std::path::Path,
    line: Option<u32>,
) -> io::Result<EditorOutcome> {
    with_suspended_tui(
        terminal,
        use_alt_screen,
        use_mouse_capture,
        use_bracketed_paste,
        || run_editor_on_path(path, line),
    )
}

/// Suspend the TUI, run the external editor on `current`, then re-enter the
/// TUI. Returns the new composer text iff the user saved changes.
///
/// On any error (raw-mode toggle, IO, editor spawn failure), the function
/// still attempts to fully restore the terminal before returning.
pub(crate) fn spawn_editor_for_input(
    terminal: &mut Terminal<ColorCompatBackend<Stdout>>,
    use_alt_screen: bool,
    use_mouse_capture: bool,
    use_bracketed_paste: bool,
    current: &str,
) -> io::Result<EditorOutcome> {
    with_suspended_tui(
        terminal,
        use_alt_screen,
        use_mouse_capture,
        use_bracketed_paste,
        || run_editor_raw(current),
    )
}

/// Hand the terminal to a child, run `body`, and restore the TUI.
///
/// Restoration is best-effort and runs on every path, including a `body` that
/// failed: leaving raw mode or the alt screen wrong is worse than the error
/// being reported.
fn with_suspended_tui(
    terminal: &mut Terminal<ColorCompatBackend<Stdout>>,
    use_alt_screen: bool,
    use_mouse_capture: bool,
    use_bracketed_paste: bool,
    body: impl FnOnce() -> io::Result<EditorOutcome>,
) -> io::Result<EditorOutcome> {
    // 0. Stop reading the tty.
    // #6165: suspending crossterm state is not enough. The input pump runs on
    // its own thread and keeps calling `event::read()` whatever mode the
    // terminal is in, so a child launched without this pause competes with
    // Codewhale for every keystroke — the editor cannot be quit and the
    // fragments land in the composer. Pausing here, rather than at each call
    // site, is what makes `/hooks edit` and the composer editor correct by
    // the same construction. Fail closed: a pump that will not stop means the
    // handoff would reproduce the defect, so the editor does not run.
    let input_pause = crate::tui::ui::pause_terminal_input_for_child()?;

    // 1. Suspend.
    // Focus reporting is about to be disabled. Fail closed to the quiet state
    // so a stale FocusLost cannot authorize a surprise notification while an
    // external editor owns the terminal.
    crate::tui::notifications::set_terminal_focused(true);
    // #443: pop keyboard enhancement flags first so the editor
    // process doesn't inherit a half-configured input mode. Best-
    // effort — matches the shutdown / panic paths in main.rs.
    // Use the Windows-aware helper: the raw crossterm execute!() is a
    // no-op on Windows and would leave the editor process in Kitty mode.
    suspend_tui_child_modes(
        terminal.backend_mut(),
        use_mouse_capture,
        use_bracketed_paste,
    );
    let _ = disable_raw_mode();
    if use_alt_screen {
        let _ = super::ui::leave_alt_screen(terminal.backend_mut());
    }

    // 2. Run the child (synchronous; inherits stdio).
    let result = body();

    // 3. Resume — best-effort restoration regardless of `result`.
    let _ = enable_raw_mode();
    if use_alt_screen {
        let _ = super::ui::enter_alt_screen(terminal.backend_mut());
    }
    super::ui::recover_terminal_modes(
        terminal.backend_mut(),
        use_mouse_capture,
        use_bracketed_paste,
    );
    // Reporting was unavailable during the handoff. Resume focused and wait
    // for a fresh FocusLost before background-only delivery is eligible.
    crate::tui::notifications::set_terminal_focused(true);
    // Force a full repaint so a SIGWINCH during the edit doesn't leave the
    // viewport stale.
    let _ = terminal.clear();

    // 4. Take the tty back, after the modes it reads under are restored.
    drop(input_pause);

    result
}

fn suspend_tui_child_modes<W: Write>(
    writer: &mut W,
    use_mouse_capture: bool,
    use_bracketed_paste: bool,
) {
    super::ui::pop_keyboard_enhancement_flags(writer);
    super::ui::disable_alternate_scroll_mode(writer);
    let _ = execute!(writer, DisableFocusChange);
    if use_mouse_capture {
        disable_mouse_capture_for_child(writer);
    }
    if use_bracketed_paste {
        super::ui::disable_bracketed_paste_mode(writer);
    }
    let _ = writer.flush();
}

fn disable_mouse_capture_for_child<W: Write>(writer: &mut W) {
    // Crossterm's mouse-capture command takes a WinAPI path on Windows and
    // does not emit bytes into PTY-style terminals such as mintty. External
    // editors inherit the PTY state, so send the xterm reset sequences
    // directly here.
    const DISABLE_MOUSE_CAPTURE: &[u8] = b"\x1b[?1006l\x1b[?1015l\x1b[?1003l\x1b[?1002l\x1b[?1000l";
    if let Err(err) = writer.write_all(DISABLE_MOUSE_CAPTURE) {
        tracing::debug!(?err, "DisableMouseCapture direct reset ignored");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::sync::Mutex;

    /// Serialize tests that mutate process-global env vars.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        keys: Vec<(&'static str, Option<OsString>)>,
    }
    impl EnvGuard {
        fn new(keys: &[&'static str]) -> Self {
            let saved: Vec<_> = keys.iter().map(|k| (*k, env::var_os(k))).collect();
            Self { keys: saved }
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (k, v) in &self.keys {
                match v {
                    Some(val) => unsafe { env::set_var(k, val) },
                    None => unsafe { env::remove_var(k) },
                }
            }
        }
    }

    /// The file on disk is the document: a `hooks.toml` the user edits stays
    /// edited, and the outcome only reports whether the bytes moved.
    #[test]
    #[cfg(unix)]
    fn editing_a_path_in_place_reports_only_whether_the_bytes_moved() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _guard = EnvGuard::new(&["VISUAL", "EDITOR"]);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hooks.toml");
        fs::write(&path, "# seed\n").unwrap();

        // An editor that saves nothing.
        unsafe { env::set_var("VISUAL", "true") };
        unsafe { env::remove_var("EDITOR") };
        assert_eq!(
            run_editor_on_path(&path, None).unwrap(),
            EditorOutcome::Unchanged,
            "an editor that changes nothing must not trigger a reload"
        );

        // An editor that appends a line.
        let script = dir.path().join("append.sh");
        fs::write(&script, "#!/bin/sh\nprintf 'x\\n' >> \"$1\"\n").unwrap();
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        unsafe { env::set_var("VISUAL", script.to_str().unwrap()) };
        match run_editor_on_path(&path, None).unwrap() {
            EditorOutcome::Edited(text) => assert!(text.contains("# seed") && text.contains('x')),
            other => panic!("expected Edited, got {other:?}"),
        }
        assert!(
            fs::read_to_string(&path).unwrap().contains('x'),
            "the edit belongs to the real file, not a temp copy"
        );
    }

    /// The line argument is spelled per editor family, and an unknown editor
    /// gets the bare path rather than an argument it would treat as a file.
    #[test]
    fn push_target_args_spells_the_line_per_editor_family() {
        use std::ffi::OsStr;

        let path = std::path::Path::new("/w/src/main.rs");
        let args_for = |program: &str, line: Option<u32>| -> Vec<String> {
            let mut cmd = Command::new(program);
            push_target_args(&mut cmd, program, path, line);
            cmd.get_args()
                .map(OsStr::to_string_lossy)
                .map(|s| s.into_owned())
                .collect()
        };

        // No line: byte-identical to the historical `cmd.arg(path)`.
        assert_eq!(args_for("vim", None), vec!["/w/src/main.rs"]);
        assert_eq!(args_for("code", None), vec!["/w/src/main.rs"]);

        // vi family, including an absolute path and a .exe suffix.
        assert_eq!(args_for("vim", Some(12)), vec!["+12", "/w/src/main.rs"]);
        assert_eq!(args_for("nano", Some(3)), vec!["+3", "/w/src/main.rs"]);
        assert_eq!(
            args_for("/usr/bin/nvim", Some(9)),
            vec!["+9", "/w/src/main.rs"]
        );
        // `.exe` is stripped by `file_stem` on every platform. A backslashed
        // Windows *path* only splits on Windows, so it is not asserted here.
        assert_eq!(args_for("vim.exe", Some(5)), vec!["+5", "/w/src/main.rs"]);

        // VS Code and forks need the flag; Zed/Sublime/JetBrains take file:line.
        assert_eq!(
            args_for("code", Some(42)),
            vec!["--goto", "/w/src/main.rs:42"]
        );
        assert_eq!(args_for("zed", Some(42)), vec!["/w/src/main.rs:42"]);

        // Unknown editor: open the right file, never an invented argument.
        assert_eq!(args_for("my-editor", Some(42)), vec!["/w/src/main.rs"]);
    }

    #[test]
    fn resolve_editor_prefers_visual_over_editor() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _g = EnvGuard::new(&["VISUAL", "EDITOR"]);
        unsafe {
            env::set_var("VISUAL", "vis-cmd");
            env::set_var("EDITOR", "ed-cmd");
        }
        assert_eq!(resolve_editor(), "vis-cmd");
    }

    #[test]
    fn resolve_editor_falls_back_to_vi() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _g = EnvGuard::new(&["VISUAL", "EDITOR"]);
        unsafe {
            env::remove_var("VISUAL");
            env::remove_var("EDITOR");
        }
        assert_eq!(resolve_editor(), "vi");
    }

    /// Editor that immediately exits 0 without touching the file ⇒ Unchanged.
    #[test]
    #[cfg(unix)]
    fn run_editor_unchanged_when_editor_is_noop() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _g = EnvGuard::new(&["VISUAL", "EDITOR"]);
        unsafe {
            env::remove_var("VISUAL");
            env::set_var("EDITOR", "true");
        }
        let out = run_editor_raw("seed text").expect("editor ok");
        assert_eq!(out, EditorOutcome::Unchanged);
    }

    /// Editor that exits non-zero ⇒ Cancelled.
    #[test]
    #[cfg(unix)]
    fn run_editor_cancelled_on_nonzero_exit() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _g = EnvGuard::new(&["VISUAL", "EDITOR"]);
        unsafe {
            env::remove_var("VISUAL");
            env::set_var("EDITOR", "false");
        }
        let out = run_editor_raw("seed").expect("call ok");
        assert_eq!(out, EditorOutcome::Cancelled);
    }

    /// Spawning an editor binary that doesn't exist ⇒ Cancelled (graceful).
    #[test]
    #[cfg(unix)]
    fn run_editor_cancelled_when_editor_missing() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _g = EnvGuard::new(&["VISUAL", "EDITOR"]);
        unsafe {
            env::remove_var("VISUAL");
            env::set_var("EDITOR", "/nonexistent/codewhale-test-editor");
        }
        let out = run_editor_raw("seed").expect("call ok");
        assert_eq!(out, EditorOutcome::Cancelled);
    }

    /// Editor that rewrites the file ⇒ Edited(new).
    #[test]
    #[cfg(unix)]
    fn run_editor_returns_edited_contents() {
        use std::os::unix::fs::PermissionsExt;

        let _lock = ENV_LOCK.lock().unwrap();
        let _g = EnvGuard::new(&["VISUAL", "EDITOR"]);
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("ed.sh");
        fs::write(&script, "#!/bin/sh\nprintf 'edited body' > \"$1\"\n").unwrap();
        let mut perms = fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).unwrap();

        unsafe {
            env::remove_var("VISUAL");
            env::set_var("EDITOR", script.to_string_lossy().to_string());
        }
        let out = run_editor_raw("seed body").expect("editor ok");
        assert_eq!(out, EditorOutcome::Edited("edited body".to_string()));
    }

    /// Verify that the temp file is unlinked after `run_editor_raw` returns,
    /// regardless of outcome. We test the success path with a script that
    /// echoes the file path to a side channel before exiting.
    #[test]
    #[cfg(unix)]
    fn run_editor_cleans_up_temp_file() {
        use std::os::unix::fs::PermissionsExt;

        let _lock = ENV_LOCK.lock().unwrap();
        let _g = EnvGuard::new(&["VISUAL", "EDITOR"]);
        let dir = tempfile::tempdir().unwrap();
        let path_capture = dir.path().join("capture.txt");
        let script = dir.path().join("ed.sh");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf '%s' \"$1\" > \"{}\"\nprintf 'x' > \"$1\"\n",
                path_capture.display()
            ),
        )
        .unwrap();
        let mut perms = fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).unwrap();

        unsafe {
            env::remove_var("VISUAL");
            env::set_var("EDITOR", script.to_string_lossy().to_string());
        }
        let _ = run_editor_raw("seed").expect("editor ok");

        let captured = fs::read_to_string(&path_capture).expect("captured path");
        assert!(!captured.is_empty(), "editor should have received a path");
        assert!(
            !std::path::Path::new(&captured).exists(),
            "temp file {captured:?} should be cleaned up after run_editor_raw returns"
        );
    }

    #[test]
    fn suspend_tui_child_modes_disables_every_inherited_mode() {
        let mut out = Vec::new();

        suspend_tui_child_modes(&mut out, true, true);

        let seq = String::from_utf8_lossy(&out);
        assert!(
            seq.contains("\x1b[?1007l"),
            "external editor suspend must disable alternate-scroll mode: {seq:?}"
        );
        assert!(
            seq.contains("\x1b[?1004l"),
            "external editor suspend must disable focus events: {seq:?}"
        );
        assert!(
            seq.contains("\x1b[?2004l"),
            "external editor suspend must disable bracketed paste: {seq:?}"
        );
        assert!(
            seq.contains("\x1b[?1000l"),
            "external editor suspend must disable mouse capture when active: {seq:?}"
        );
    }

    #[test]
    fn suspend_tui_child_modes_leaves_mouse_capture_alone_when_inactive() {
        let mut out = Vec::new();

        suspend_tui_child_modes(&mut out, false, true);

        let seq = String::from_utf8_lossy(&out);
        assert!(
            !seq.contains("\x1b[?1000l"),
            "external editor suspend must not emit mouse-capture reset when inactive: {seq:?}"
        );
    }

    #[test]
    fn resume_tui_child_modes_reenables_shared_terminal_modes() {
        let mut out = Vec::new();

        crate::tui::ui::recover_terminal_modes(&mut out, true, true);

        let seq = String::from_utf8_lossy(&out);
        assert!(
            !seq.contains("\x1b[?1007h"),
            "must not enable alternate-scroll"
        );
        assert!(seq.contains("\x1b[?1007l"), "must reset alternate-scroll");
        assert!(
            seq.contains("\x1b[?1004h"),
            "external editor resume must restore focus events: {seq:?}"
        );
        assert!(
            seq.contains("\x1b[?2004h"),
            "external editor resume must restore bracketed paste: {seq:?}"
        );
    }

    #[test]
    fn resume_tui_child_modes_leaves_alternate_scroll_off_when_mouse_capture_inactive() {
        let mut out = Vec::new();

        crate::tui::ui::recover_terminal_modes(&mut out, false, true);

        let seq = String::from_utf8_lossy(&out);
        assert!(
            !seq.contains("\x1b[?1007h"),
            "external editor resume must not enable alternate-scroll without mouse capture: {seq:?}"
        );
        assert!(
            seq.contains("\x1b[?1007l"),
            "external editor resume must reset alternate-scroll without mouse capture: {seq:?}"
        );
        assert!(
            seq.contains("\x1b[?1004h"),
            "external editor resume must still restore focus events: {seq:?}"
        );
        assert!(
            seq.contains("\x1b[?2004h"),
            "external editor resume must still restore bracketed paste: {seq:?}"
        );
    }
}
