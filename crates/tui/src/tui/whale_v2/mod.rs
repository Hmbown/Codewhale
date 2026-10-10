//! The Whale character v2 in the terminal: the Rust port of the executable
//! reference (`acting`, `rig`, `props`, `scene`) that the Codewhale desktop
//! app paints, rasterized to Braille cells for the Watch/habitat tank.
//!
//! Replaces the particle splat that `pet_watch::graphics::Renderer::render`
//! used to turn into cells: the owner's frames still carry the old 980-point
//! world, but the cells now come from one [`Director`] fed by the same owner
//! truth the desktop whale reads (`EngineOwnerProjection`). No tool, caption
//! or argument is classified here, and there is no second clock: [`Whale`]
//! steps the Director in at most 1/30 s slices from wall time.
//!
//! Known limitations:
//! - The Kitty pixel image path (`CODEWHALE_PET_GRAPHICS`) still draws the
//!   particle world; only the Braille cells are v2.
//! - The owner has no Listening or Stuck state, so the whale never performs
//!   them. `Unknown` presence and a disconnected producer read as Offline
//!   (asleep), never as Idle.
//! - No cove or calf-free habitat: this is the bare character, as on the
//!   desktop's compact surfaces.
//! - Geometry, tables and the conformance vectors are vendored from
//!   `codewhale-app/vendor/whale-character-v2`; the app copy stays the
//!   reference until both consume one crate.

mod acting;
mod data;
mod math;
mod props;
mod rig;
mod scene;

use acting::{Activity, Context, Director, Options, Presence, Span};
use codewhale_protocol::engine_owner::{
    EngineOwnerProjection, OwnerActivityKind, OwnerFreshness, OwnerPresence,
};
use std::time::{Duration, Instant};

/// Longest catch-up after a stalled interval: resume, never replay.
const MAX_CATCH_UP: Duration = Duration::from_secs(1);
const SAME_FRAME: Duration = Duration::from_millis(4);
const STEP: f64 = 1. / 30.;

#[derive(Clone, Debug, PartialEq)]
struct Inputs {
    presence: Presence,
    activity: Option<Activity>,
    context: Context,
}

/// One Director for the foreground session, on one wall clock.
pub struct Whale {
    session: Option<String>,
    director: Director,
    inputs: Option<Inputs>,
    clock: Option<Instant>,
}

impl Default for Whale {
    fn default() -> Self {
        Self {
            session: None,
            director: Director::new(Options::default()),
            inputs: None,
            clock: None,
        }
    }
}

impl Whale {
    /// Apply owner truth for `session`. A different session replaces the
    /// Director outright: no pending clip, calf count or completed-turn
    /// identity carries across.
    pub fn observe(
        &mut self,
        session: &str,
        owner: Option<&EngineOwnerProjection>,
        producer_connected: bool,
        reduced: bool,
    ) {
        if self.session.as_deref() != Some(session) {
            self.session = Some(session.to_string());
            self.director = Director::new(Options {
                reduced,
                ..Options::default()
            });
            self.inputs = None;
            self.clock = None;
        }
        self.director.set_reduced(reduced);
        let inputs = inputs(owner, producer_connected);
        if self.inputs.as_ref() != Some(&inputs) {
            self.director.set(
                inputs.presence,
                inputs.activity.clone(),
                inputs.context.clone(),
            );
            self.inputs = Some(inputs);
        }
    }

    /// Advance the clock to `now` in steps of at most 1/30 s.
    pub fn advance(&mut self, now: Instant) {
        let Some(last) = self.clock else {
            self.clock = Some(now);
            return;
        };
        let elapsed = now.saturating_duration_since(last);
        if elapsed < SAME_FRAME {
            return;
        }
        self.clock = Some(now);
        let mut remaining = elapsed.min(MAX_CATCH_UP).as_secs_f64();
        while remaining > 1e-9 {
            let dt = remaining.min(STEP);
            self.director.step(dt);
            remaining -= dt;
        }
    }

    /// The packed Braille cells (`cols × rows`, row-major) the widget paints.
    pub fn cells(&self, cols: usize, rows: usize) -> Vec<u8> {
        scene::braille(&self.director, cols, rows).cells
    }
}

fn inputs(owner: Option<&EngineOwnerProjection>, producer_connected: bool) -> Inputs {
    let Some(owner) = owner.filter(|_| producer_connected) else {
        return Inputs {
            presence: Presence::Offline,
            activity: None,
            context: Context::default(),
        };
    };
    let presence = match owner.authoritative_presence {
        OwnerPresence::Unknown => Presence::Offline,
        OwnerPresence::Idle => Presence::Idle,
        OwnerPresence::NeedsYou => Presence::NeedsYou,
        OwnerPresence::Done => Presence::Done,
        // Reasoning with no foreground tool is Thinking; anything else is work.
        OwnerPresence::Working if owner.activity_kind == Some(OwnerActivityKind::Thinking) => {
            Presence::Thinking
        }
        OwnerPresence::Working => Presence::Working,
    };
    Inputs {
        presence,
        activity: Some(Activity {
            kind: owner.activity_kind.map(|kind| kind.as_str().to_string()),
            observed: owner.observed,
            parallel: Some(f64::from(owner.parallel_agent_count)),
            active: owner
                .active_spans
                .iter()
                .map(|span| Span {
                    kind: span.activity_kind.as_str().to_string(),
                    since_ms: span.started_at_ms,
                })
                .collect(),
        }),
        context: Context {
            live: owner.freshness == OwnerFreshness::Fresh,
            turn_id: owner.turn_id.clone(),
            status: owner.turn_outcome.map(|status| status.as_str().to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn owner(presence: &str, kind: Option<&str>) -> EngineOwnerProjection {
        serde_json::from_value(json!({
            "schemaVersion": 1, "sessionId": "s", "cursor": 1, "observed": true,
            "freshness": "fresh", "authoritativePresence": presence,
            "activityKind": kind, "observedAtMs": 1000.0, "parallelAgentCount": 0,
            "activeSpans": [], "turnId": null, "turnOutcome": null,
            "doneEffectId": null, "failedToolAge": null,
        }))
        .expect("owner projection")
    }

    fn frame(owner: Option<&EngineOwnerProjection>, connected: bool) -> Vec<u8> {
        let mut whale = Whale::default();
        whale.observe("s", owner, connected, false);
        let start = Instant::now();
        whale.advance(start);
        // Two seconds of owner time, in the 33 ms cadence the view thread uses.
        for i in 1..=60 {
            whale.advance(start + Duration::from_millis(33 * i));
        }
        whale.cells(32, 16)
    }

    #[test]
    fn owner_truth_changes_the_braille_whale() {
        let resting = frame(Some(&owner("idle", None)), true);
        let reading = frame(Some(&owner("working", Some("reading"))), true);
        let offline = frame(Some(&owner("working", Some("reading"))), false);
        assert!(resting.iter().any(|b| *b != 0), "the whale is drawn");
        assert_ne!(resting, reading, "work reads differently from rest");
        assert_eq!(
            offline,
            frame(None, false),
            "a disconnected producer is Offline"
        );
        assert_ne!(offline, reading);
        for line in 0..16 {
            let row: String = reading[line * 32..(line + 1) * 32]
                .iter()
                .map(|b| {
                    if *b == 0 {
                        ' '
                    } else {
                        char::from_u32(0x2800 + u32::from(*b)).unwrap()
                    }
                })
                .collect();
            eprintln!("{row}");
        }
    }
}
