//! Built-in art adapter over the shared avatar-pack contract.
//! No separate action vocabulary or animation clock lives here.
use crate::avatar::{Frame, Pack};
use std::sync::OnceLock;
pub const TERMINAL: &[u8] = include_bytes!("terminal.rgba");
pub const TILE: usize = 96;
pub const FRAMES: usize = 36;
pub fn pack() -> &'static Pack {
    static PACK: OnceLock<Pack> = OnceLock::new();
    PACK.get_or_init(|| {
        Pack::parse(include_bytes!("avatar.json")).expect("validated built-in avatar")
    })
}
pub fn sample(
    act: &str,
    frame: f64,
    reduced: bool,
    view: Option<&str>,
    action: Option<&str>,
) -> Frame {
    pack().sample(act, frame, reduced, view, action)
}
pub fn pixel(index: usize, x: usize, y: usize) -> [u8; 4] {
    if index >= FRAMES || x >= TILE || y >= TILE {
        return [0; 4];
    }
    let at = ((index * TILE + y) * TILE + x) * 4;
    TERMINAL[at..at + 4].try_into().expect("four channels")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn girl_has_distinct_frames_for_all_native_actions_and_authored_views() {
        assert_eq!(TERMINAL.len(), FRAMES * TILE * TILE * 4);
        for act in crate::avatar::ACTS {
            let clip = &pack().actions[&pack().states[act]];
            assert_eq!(clip.frames.len(), 2, "{act} has two authored poses");
            let tile = |index: u16| {
                &TERMINAL[index as usize * TILE * TILE * 4..(index as usize + 1) * TILE * TILE * 4]
            };
            assert_ne!(
                tile(clip.frames[0]),
                tile(clip.frames[1]),
                "{act} poses differ"
            );
            for index in &clip.frames {
                let pixels = tile(*index);
                assert!(
                    pixels.as_chunks::<4>().0.iter().any(|p| p[3] > 200),
                    "{act} paints"
                );
                assert!(
                    pixels.as_chunks::<4>().0.iter().any(|p| p[3] == 0),
                    "{act} is transparent"
                );
            }
            assert_eq!(
                sample(act, 10000., true, None, None).index,
                clip.poster as usize
            );
        }
        assert_ne!(
            sample("rest", 0., false, Some("side"), None),
            sample("rest", 0., false, Some("back"), None)
        );
    }
}
