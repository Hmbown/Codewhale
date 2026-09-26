//! Local audio dispatch. Policy decides the cue; this sink never substitutes a
//! bell for a missing/unsupported WAV. Tests inject a sink and never call an OS
//! player. A single in-flight WAV keeps repeated categories from stacking audio.
use super::sound_policy::SoundCue;
#[cfg(not(test))]
use std::io;
use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioOutcome {
    Emitted,
    Dispatched,
    Unsupported,
    Failed,
    Busy,
}

pub fn emit_terminal(cue: &SoundCue, out: &mut dyn Write) -> AudioOutcome {
    let bytes: &[u8] = match cue {
        SoundCue::Bell | SoundCue::Beep => b"\x07",
        SoundCue::DoubleBell => b"\x07\x07",
        SoundCue::Whale | SoundCue::File(_) => return AudioOutcome::Unsupported,
    };
    if out.write_all(bytes).and_then(|()| out.flush()).is_ok() {
        AudioOutcome::Emitted
    } else {
        AudioOutcome::Failed
    }
}

pub const WHALE_WAV: &[u8] = include_bytes!("../../assets/audio/codewhale-whale-call.wav");

#[cfg(not(test))]
pub fn dispatch(cue: &SoundCue, out: &mut dyn Write) -> AudioOutcome {
    #[cfg(target_os = "windows")]
    if matches!(cue, SoundCue::Bell | SoundCue::Beep | SoundCue::DoubleBell) {
        use windows::Win32::System::Diagnostics::Debug::MessageBeep;
        use windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_STYLE;
        let count = if *cue == SoundCue::DoubleBell { 2 } else { 1 };
        for _ in 0..count {
            if unsafe { MessageBeep(MESSAGEBOX_STYLE(0)) }.is_err() {
                return AudioOutcome::Failed;
            }
        }
        return AudioOutcome::Emitted;
    }
    if matches!(cue, SoundCue::Bell | SoundCue::Beep | SoundCue::DoubleBell) {
        return emit_terminal(cue, out);
    }
    dispatch_wav(cue)
}

#[cfg(test)]
pub fn dispatch(_cue: &SoundCue, _out: &mut dyn Write) -> AudioOutcome {
    // Production entry points are also fail-closed in library tests.
    AudioOutcome::Unsupported
}

#[cfg(not(test))]
static PLAYING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(not(test))]
fn dispatch_wav(cue: &SoundCue) -> AudioOutcome {
    use std::sync::atomic::Ordering;
    if !cfg!(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux"
    )) {
        return AudioOutcome::Unsupported;
    }
    if PLAYING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return AudioOutcome::Busy;
    }
    let cue = cue.clone();
    match std::thread::Builder::new()
        .name("notification-audio".into())
        .spawn(move || {
            struct Reset;
            impl Drop for Reset {
                fn drop(&mut self) {
                    PLAYING.store(false, Ordering::SeqCst);
                }
            }
            let _reset = Reset;
            if let Err(error) = play_wav(&cue) {
                // Do not log file names or external-player output (both may contain private data).
                tracing::warn!(kind = ?error.kind(), "notification audio playback failed");
            }
        }) {
        Ok(_) => AudioOutcome::Dispatched,
        Err(_) => {
            PLAYING.store(false, Ordering::SeqCst);
            AudioOutcome::Failed
        }
    }
}

#[cfg(not(test))]
fn play_wav(cue: &SoundCue) -> io::Result<()> {
    let mut bundled = None;
    let path = match cue {
        SoundCue::Whale => {
            let mut file = tempfile::Builder::new()
                .prefix("codewhale-call-")
                .suffix(".wav")
                .tempfile()?;
            file.write_all(WHALE_WAV)?;
            file.flush()?;
            let path = file.path().to_path_buf();
            bundled = Some(file);
            path
        }
        SoundCue::File(path) => std::fs::canonicalize(path)?,
        _ => return Err(io::Error::other("expected WAV cue")),
    };
    // Retain the private temporary file until the synchronous player exits.
    let result = play_file(&path);
    drop(bundled);
    result
}

#[cfg(all(not(test), target_os = "windows"))]
fn play_file(path: &std::path::Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Media::Audio::{PlaySoundW, SND_FILENAME, SND_NODEFAULT};
    use windows::core::PCWSTR;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // Synchronous in the worker: the bundled file must outlive playback.
    if unsafe { PlaySoundW(PCWSTR(wide.as_ptr()), None, SND_FILENAME | SND_NODEFAULT) }.as_bool() {
        Ok(())
    } else {
        Err(io::Error::other("audio player failed"))
    }
}

#[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
fn play_file(path: &std::path::Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    let player = "/usr/bin/afplay";
    #[cfg(target_os = "linux")]
    let player = "aplay";
    let status = std::process::Command::new(player)
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other("audio player failed"))
    }
}

#[cfg(all(
    not(test),
    not(any(target_os = "windows", target_os = "macos", target_os = "linux"))
))]
fn play_file(_path: &std::path::Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "WAV playback unsupported",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_whale_is_the_complete_pcm_wav_without_clipped_samples() {
        assert_eq!(&WHALE_WAV[..4], b"RIFF");
        assert_eq!(&WHALE_WAV[8..12], b"WAVE");
        assert_eq!(WHALE_WAV.len(), 136754);
        assert_eq!(u16::from_le_bytes(WHALE_WAV[22..24].try_into().unwrap()), 1);
        assert_eq!(
            u32::from_le_bytes(WHALE_WAV[24..28].try_into().unwrap()),
            44100
        );
        assert_eq!(
            u16::from_le_bytes(WHALE_WAV[34..36].try_into().unwrap()),
            16
        );
        let samples: Vec<i16> = WHALE_WAV[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .copied()
            .map(i16::from_le_bytes)
            .collect();
        assert_eq!(samples.first(), Some(&0));
        assert_eq!(samples.last(), Some(&0));
        assert!(
            samples
                .iter()
                .all(|sample| *sample != i16::MIN && *sample != i16::MAX)
        );
    }

    #[test]
    fn terminal_sink_has_exact_bell_bytes_and_no_wav_fallback() {
        for (cue, bytes) in [
            (SoundCue::Bell, &b"\x07"[..]),
            (SoundCue::Beep, &b"\x07"[..]),
            (SoundCue::DoubleBell, &b"\x07\x07"[..]),
        ] {
            let mut out = Vec::new();
            assert_eq!(emit_terminal(&cue, &mut out), AudioOutcome::Emitted);
            assert_eq!(out, bytes);
        }
        for cue in [SoundCue::Whale, SoundCue::File("missing.wav".into())] {
            let mut out = Vec::new();
            assert_eq!(emit_terminal(&cue, &mut out), AudioOutcome::Unsupported);
            assert!(out.is_empty());
        }
    }

    #[test]
    fn production_audio_entry_is_silent_in_library_tests() {
        let mut out = Vec::new();
        assert_eq!(
            dispatch(&SoundCue::Whale, &mut out),
            AudioOutcome::Unsupported
        );
        assert_eq!(
            dispatch(&SoundCue::Bell, &mut out),
            AudioOutcome::Unsupported
        );
        assert!(out.is_empty());
    }
}
