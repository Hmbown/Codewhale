//! Conformance against the vendored JavaScript oracle.
//!
//! The receipts in `vendor/whale-character-v2/handoff` were produced by the
//! reference with seed 11, ambient motion off and `step(1/30)` exactly. The
//! Rust Director replays the same inputs and must reproduce every pose
//! within floating-point tolerance, the packed 32×16 terminal bytes, and the
//! SHA-256 geometry receipt over the visible named paths.

use super::acting::{Activity, Context, Director, Event, Options, Presence, Span, acting_for};
use super::data::{Act, N, PARAMS, Pose, p, tables};
use super::rig::{
    CRUISE_DIRECTION, Direction, MARK_DIRECTION, OPEN_DIRECTION, build, path_string, rig_pose,
    signed_area,
};
use super::scene::{Grid, View, braille, colored_braille, rasterize, scene};
use super::stage::{self, Inputs, Stage, Tier};
use super::{Role, Shape, props};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};

const MOTION: &str = include_str!("../../tests/fixtures/whale-motion/motion-conformance.json");
const SCENARIOS: &str = include_str!("../../tests/fixtures/whale-motion/scenarios.json");
const SCENARIO_RECEIPTS: &str =
    include_str!("../../tests/fixtures/whale-motion/scenario-conformance.json");

/// `catalogue.js` CASES: action, label, presence, owner kind.
const CASES: [(&str, &str, &str, Option<&str>); 17] = [
    ("rest", "Resting", "Idle", None),
    ("listen", "Listening", "Listening", None),
    ("think", "Thinking", "Thinking", None),
    ("busy", "Plain work", "Working", Some("unknown")),
    ("read", "Read", "Working", Some("reading")),
    ("search", "Search", "Working", Some("searching")),
    ("write", "Edit", "Working", Some("editing")),
    ("run", "Run", "Working", Some("executing")),
    ("browse", "Browse", "Working", Some("browsing")),
    ("talk", "Reply", "Working", Some("responding")),
    ("pod", "Delegate · 3 agents", "Working", Some("delegating")),
    ("needs", "Needs you", "NeedsYou", None),
    ("done", "Done", "Done", None),
    ("hmm", "Stuck · proposed", "Stuck", None),
    ("sleep", "Asleep", "Offline", None),
    ("computer", "Computer use", "Working", Some("computer")),
    ("connect", "Connected app", "Working", Some("network")),
];

fn presence_named(name: &str) -> Presence {
    match name {
        "Offline" => Presence::Offline,
        "Idle" => Presence::Idle,
        "Listening" => Presence::Listening,
        "Thinking" => Presence::Thinking,
        "Working" => Presence::Working,
        "NeedsYou" => Presence::NeedsYou,
        "Done" => Presence::Done,
        "Stuck" => Presence::Stuck,
        other => panic!("presence {other}"),
    }
}

/// `scenarios.js` context: Live, turn `fixture-turn`, completed.
fn fixture_context() -> Context {
    Context {
        live: true,
        turn_id: Some("fixture-turn".into()),
        status: Some("completed".into()),
        now_ms: Some(1000.),
        failed_at_ms: Some(0.),
    }
}

/// Apply a JSON context patch the way `{...context, ...step.context}` does.
fn patched(patch: &Value) -> Context {
    let mut context = fixture_context();
    if let Some(object) = patch.as_object() {
        for (key, value) in object {
            match key.as_str() {
                "freshness" => context.live = value.as_str() == Some("Live"),
                "turnId" => context.turn_id = value.as_str().map(str::to_string),
                "status" => context.status = value.as_str().map(str::to_string),
                "nowMs" => context.now_ms = value.as_f64(),
                "failedAtMs" => context.failed_at_ms = value.as_f64(),
                _ => {}
            }
        }
    }
    context
}

fn activity_json(value: &Value) -> Option<Activity> {
    value.as_object().map(|_| Activity {
        kind: value["kind"].as_str().map(str::to_string),
        observed: value["observed"].as_bool() == Some(true),
        parallel: value["parallel"].as_f64(),
        active: value["active"]
            .as_array()
            .map(|list| {
                list.iter()
                    .map(|s| Span {
                        kind: s["kind"].as_str().unwrap().into(),
                        since_ms: s["sinceMs"].as_f64().unwrap(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn fixture(seed: u32) -> Director {
    Director::new(Options {
        seed,
        reduced: false,
        ambient: false,
    })
}

fn set_case(d: &mut Director, case: &(&str, &str, &str, Option<&str>)) {
    let (_, _, presence, kind) = *case;
    let activity = kind.map(|kind| Activity {
        kind: Some(kind.into()),
        observed: kind != "unknown",
        parallel: Some(3.),
        active: Vec::new(),
    });
    let context = Context {
        status: Some(
            if presence == "Stuck" {
                "failed"
            } else {
                "completed"
            }
            .into(),
        ),
        ..fixture_context()
    };
    d.set(presence_named(presence), activity, context);
}

fn assert_pose(actual: &Pose, expected: &Value, what: &str) {
    for (i, name) in PARAMS.iter().enumerate() {
        let want = expected[*name]
            .as_f64()
            .unwrap_or_else(|| panic!("{what}: {name}"));
        let got = actual[i];
        assert!(
            (got - want).abs() <= 1e-9 * (1. + want.abs()),
            "{what}: {name} = {got}, oracle {want}"
        );
    }
}

fn geometry_hash(d: &Director) -> String {
    let parts = scene(d, View::hero(512., 1.));
    let text = parts
        .shapes
        .iter()
        .filter(|s| s.opacity >= 0.5)
        .map(|s| format!("{}:{}", s.id, path_string(&s.path)))
        .collect::<Vec<_>>()
        .join("\n");
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn advance(d: &mut Director, frames: usize) {
    for _ in 0..frames {
        d.step(1. / 30.);
    }
}

#[test]
fn the_153_motion_samples_match_pose_terminal_bytes_and_geometry() {
    let frames: Vec<Value> = serde_json::from_str(MOTION).unwrap();
    assert_eq!(frames.len(), CASES.len() * 9);
    let (mut poses, mut bytes, mut hashes) = (0, 0, 0);
    let mut misses = Vec::new();
    for case in &CASES {
        let mut d = fixture(11);
        set_case(&mut d, case);
        let mut at = 0;
        for sample in frames.iter().filter(|f| f["state"] == case.0) {
            let frame = sample["frame"].as_u64().unwrap() as usize;
            while at < frame {
                d.step(1. / 30.);
                at += 1;
            }
            assert_eq!(d.acting.id(), case.0);
            assert_pose(&d.pose(), &sample["pose"], &format!("{}@{frame}", case.0));
            poses += 1;
            let calves: Vec<f64> = d.calves.iter().map(|c| c.value).collect();
            for (i, want) in sample["visibleCalves"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                assert!((calves[i] - want.as_f64().unwrap()).abs() < 1e-9);
            }
            if braille(&d, 32, 16).hex() == sample["brailleHex"].as_str().unwrap() {
                bytes += 1;
            } else {
                misses.push(format!("braille {}@{frame}", case.0));
            }
            assert_eq!(
                colored_braille(&d, 32, 16, true).grid.hex(),
                sample["brailleHex"].as_str().unwrap(),
                "colored geometry {}@{frame}",
                case.0
            );
            if geometry_hash(&d) == sample["geometryHash"].as_str().unwrap() {
                hashes += 1;
            } else {
                misses.push(format!("geometry {}@{frame}", case.0));
            }
        }
    }
    assert_eq!(poses, 153);
    assert_eq!((bytes, hashes), (153, 153), "mismatches: {misses:?}");
}

#[test]
fn the_32_owner_scenarios_reproduce_actions_calves_poses_and_spouts() {
    let fixtures: Value = serde_json::from_str(SCENARIOS).unwrap();
    let receipts: Vec<Value> = serde_json::from_str(SCENARIO_RECEIPTS).unwrap();
    let scenarios = fixtures["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 32);
    assert_eq!(receipts.len(), 32);
    let mut samples = 0;
    for scenario in scenarios {
        let id = scenario["id"].as_str().unwrap();
        let receipt = receipts.iter().find(|r| r["id"] == id).expect(id);
        let mut d = fixture(11);
        let mut frame = 0;
        for (index, step) in scenario["steps"].as_array().unwrap().iter().enumerate() {
            let after = step["after"].as_u64().unwrap_or(0) as usize;
            advance(&mut d, after);
            frame += after;
            if let Some(event) = step.get("event") {
                let kind = event["kind"].as_str().unwrap().to_string();
                d.event(match event["type"].as_str().unwrap() {
                    "tool" => Event::Tool { kind },
                    _ => Event::Failed {
                        kind,
                        status: event["status"].as_str().map(str::to_string),
                    },
                });
            } else {
                d.set(
                    presence_named(step["presence"].as_str().unwrap()),
                    activity_json(&step["activity"]),
                    patched(&step["context"]),
                );
            }
            assert_eq!(d.acting.id(), step["expected"].as_str().unwrap(), "{id}");
            if let Some(calves) = step["calves"].as_u64() {
                let n = d.calves.iter().filter(|c| c.target > 0.).count();
                assert_eq!(n as u64, calves, "{id}");
            }
            let expected = &receipt["frames"][index];
            assert_eq!(expected["frame"].as_u64().unwrap() as usize, frame, "{id}");
            assert_eq!(d.acting.id(), expected["acting"].as_str().unwrap(), "{id}");
            assert_pose(&d.pose(), &expected["pose"], &format!("{id}#{index}"));
            let targets = d.calves.iter().filter(|c| c.target > 0.).count();
            assert_eq!(
                targets as u64,
                expected["targetCalves"].as_u64().unwrap(),
                "{id}"
            );
            let spouts = d
                .emissions
                .iter()
                .filter(|e| matches!(e.kind, super::data::Emit::Spout { .. }))
                .count();
            assert_eq!(spouts as u64, expected["spouts"].as_u64().unwrap(), "{id}");
            samples += 1;
        }
        if let Some(spouts) = scenario["spouts"].as_u64() {
            advance(&mut d, 100);
            let count = d
                .emissions
                .iter()
                .filter(|e| matches!(e.kind, super::data::Emit::Spout { .. }))
                .count();
            assert_eq!(count as u64, spouts, "{id}");
        }
    }
    assert!(samples >= 32);
}

#[test]
fn all_51_terminal_stills_match_the_reference_text() {
    for (cols, rows) in [(20, 10), (32, 16), (48, 24)] {
        for case in &CASES {
            let mut d = Director::new(Options {
                seed: 7,
                reduced: true,
                ambient: false,
            });
            set_case(&mut d, case);
            let grid = braille(&d, cols, rows);
            assert!(grid.cells.iter().any(|b| *b != 0));
            let path = format!(
                "{}/tests/fixtures/whale-motion/braille/{}-{cols}x{rows}.txt",
                env!("CARGO_MANIFEST_DIR"),
                case.0
            );
            let expected = std::fs::read_to_string(path).unwrap();
            assert_eq!(
                expected,
                format!("{}\n{}\n", grid.text(), case.1),
                "{}",
                case.0
            );
        }
    }
}

#[test]
fn presence_mapping_is_strict_about_observation_and_freshness() {
    let live = fixture_context();
    for case in &CASES {
        let context = Context {
            status: Some(
                if case.2 == "Stuck" {
                    "failed"
                } else {
                    "completed"
                }
                .into(),
            ),
            ..live.clone()
        };
        let activity = case.3.map(|kind| Activity {
            kind: Some(kind.into()),
            observed: kind != "unknown",
            ..Activity::default()
        });
        assert_eq!(
            acting_for(presence_named(case.2), activity.as_ref(), &context).id(),
            case.0
        );
    }
    let reading = Activity {
        kind: Some("reading".into()),
        observed: true,
        ..Activity::default()
    };
    // Missing or stale freshness is plain work.
    let stale = Context {
        live: false,
        ..live.clone()
    };
    assert_eq!(
        acting_for(Presence::Working, Some(&reading), &stale),
        Act::Busy
    );
    // Unobserved activity is plain work.
    let unobserved = Activity {
        observed: false,
        ..reading.clone()
    };
    assert_eq!(
        acting_for(Presence::Working, Some(&unobserved), &live),
        Act::Busy
    );
    for kind in [
        "unknown",
        "waiting",
        "error",
        "unsupported",
        "new-plugin-kind",
    ] {
        let activity = Activity {
            kind: Some(kind.into()),
            ..reading.clone()
        };
        assert_eq!(
            acting_for(Presence::Working, Some(&activity), &live),
            Act::Busy,
            "{kind}"
        );
    }
    // Error or waiting activity alone is never Stuck or Needs you.
    assert_eq!(acting_for(Presence::Working, None, &live), Act::Busy);
}

#[test]
fn no_departed_state_emits_particles_or_continues_its_wave() {
    let read = |d: &mut Director| {
        d.set(
            Presence::Working,
            Some(Activity {
                kind: Some("reading".into()),
                observed: true,
                ..Activity::default()
            }),
            fixture_context(),
        )
    };
    for state in ["think", "needs", "hmm", "done"] {
        let case = CASES.iter().find(|c| c.0 == state).unwrap();
        for interrupt in [1, 6, 9, 18, 24, 40] {
            let mut d = fixture(7);
            set_case(&mut d, case);
            advance(&mut d, interrupt);
            let before = d.emissions.len();
            read(&mut d);
            assert!(d.particles.is_empty());
            advance(&mut d, 90);
            let owner = Act::from_id(state).unwrap();
            assert!(
                d.emissions.iter().skip(before).all(|e| e.owner != owner),
                "{state}@{interrupt}"
            );
            assert_eq!(d.pose()[p::spout], 0.);
            assert_eq!(d.pose()[p::splash], 0.);
            assert!(
                d.shots
                    .iter()
                    .all(|s| s.owner == Act::Read || s.exit || s.wake)
            );
        }
    }
}

#[test]
fn calves_show_exact_reported_counts_and_fade_independently() {
    for parallel in [
        None,
        Some(0.),
        Some(-1.),
        Some(f64::NAN),
        Some(1.5),
        Some(1.),
        Some(2.),
        Some(3.),
        Some(8.),
    ] {
        let mut d = Director::new(Options {
            reduced: true,
            ambient: false,
            ..Options::default()
        });
        d.set(
            Presence::Working,
            Some(Activity {
                kind: Some("delegating".into()),
                observed: true,
                parallel,
                active: Vec::new(),
            }),
            fixture_context(),
        );
        let n = match parallel {
            Some(v) if v.fract() == 0. && v > 0. => v.min(3.) as usize,
            _ => 0,
        };
        assert_eq!(d.calves.iter().filter(|c| c.value > 0.).count(), n);
        let bodies = scene(&d, View::hero(512., 1.))
            .shapes
            .iter()
            .filter(|s| s.id.starts_with("calf-") && s.id.ends_with("-body"))
            .count();
        assert_eq!(bodies, n);
    }
    let mut d = fixture(7);
    set_case(&mut d, &CASES[10]);
    advance(&mut d, 60);
    let values: Vec<f64> = d.calves.iter().map(|c| c.value).collect();
    set_case(&mut d, &CASES[4]);
    d.step(1. / 30.);
    for (i, calf) in d.calves.iter().enumerate() {
        assert!(calf.value > 0. && calf.value < values[i]);
    }
    advance(&mut d, 90);
    assert!(d.calves.iter().all(|c| c.value < 0.002));
}

#[test]
fn authored_offsets_begin_at_zero_and_beats_end_within_twelve_frames() {
    for act in Act::ALL {
        let data = tables().act(act);
        for clip in [Some(&data.enter), data.exit.as_ref(), data.beat.as_ref()]
            .into_iter()
            .flatten()
        {
            for (param, keys) in &clip.tracks {
                assert_eq!(keys[0].value, 0., "{}.{}", act.id(), PARAMS[*param]);
            }
        }
        if let Some(beat) = &data.beat {
            assert!(beat.dur <= 12., "{}", act.id());
        }
    }
}

#[test]
fn wake_opens_the_eye_at_once_and_reduced_motion_settles_on_the_poster() {
    let mut d = fixture(7);
    set_case(&mut d, &CASES[14]);
    advance(&mut d, 100);
    assert!(d.pose()[p::lid] > 0.98);
    set_case(&mut d, &CASES[0]);
    assert_eq!(d.acting, Act::Rest);
    advance(&mut d, 4);
    assert!(d.pose()[p::lid] < 0.8);
    d.set_reduced(true);
    assert_eq!(d.pose()[p::lid], d.target_for(Act::Rest)[p::lid]);
    assert!(d.shots.is_empty());
    set_case(&mut d, &CASES[14]);
    set_case(&mut d, &CASES[0]);
    d.set_reduced(false);
    assert_eq!(d.acting, Act::Rest);
    assert!(d.pose()[p::lid] < 0.1);
}

#[test]
fn beats_need_matching_fresh_onsets_and_never_replay() {
    let mut d = fixture(7);
    set_case(&mut d, &CASES[4]);
    advance(&mut d, 60);
    set_case(&mut d, &CASES[7]);
    assert!(!d.event(Event::Tool {
        kind: "reading".into()
    }));
    assert!(d.event(Event::Tool {
        kind: "executing".into()
    }));
    assert!(d.shots.iter().any(|s| s.beat && s.owner == Act::Run));
    let activity = Activity {
        kind: Some("executing".into()),
        observed: true,
        parallel: None,
        active: vec![
            Span {
                kind: "executing".into(),
                since_ms: 10.,
            },
            Span {
                kind: "reading".into(),
                since_ms: 8.,
            },
        ],
    };
    let mut next = fixture(7);
    next.set(Presence::Working, Some(activity.clone()), fixture_context());
    let count = next.events.len();
    next.set(Presence::Working, Some(activity.clone()), fixture_context());
    assert_eq!(
        next.events.len(),
        count,
        "a repeated onset is not a new beat"
    );
    advance(&mut next, 15);
    assert!(!next.shots.iter().any(|s| s.beat));
    // A stale report observes nothing.
    let mut stale = fixture(7);
    stale.set(
        Presence::Working,
        Some(activity),
        Context {
            live: false,
            ..fixture_context()
        },
    );
    assert!(stale.events.is_empty());
}

#[test]
fn done_flourishes_once_per_completed_turn() {
    let spouts = |d: &Director| {
        d.emissions
            .iter()
            .filter(|e| matches!(e.kind, super::data::Emit::Spout { .. }))
            .count()
    };
    let mut d = fixture(7);
    set_case(&mut d, &CASES[12]);
    advance(&mut d, 30);
    assert_eq!(spouts(&d), 1);
    assert!(d.pose()[p::spout] > 0.9 && d.pose()[p::splash] > 0.7);
    set_case(&mut d, &CASES[1]);
    advance(&mut d, 20);
    set_case(&mut d, &CASES[12]);
    advance(&mut d, 100);
    assert_eq!(spouts(&d), 1, "Listening then Done again cannot repeat it");
    assert_eq!(d.pose()[p::spout], 0.);
    assert_eq!(d.pose()[p::splash], 0.);
    d.set(
        Presence::Done,
        None,
        Context {
            turn_id: Some("t2".into()),
            ..fixture_context()
        },
    );
    advance(&mut d, 30);
    assert_eq!(spouts(&d), 2);
    let mut anonymous = fixture(7);
    anonymous.set(
        Presence::Done,
        None,
        Context {
            turn_id: None,
            ..fixture_context()
        },
    );
    advance(&mut anonymous, 100);
    assert!(anonymous.emissions.is_empty());
    // A canceled or failed turn is not a completion.
    let mut canceled = fixture(7);
    canceled.set(
        Presence::Done,
        None,
        Context {
            status: Some("canceled".into()),
            ..fixture_context()
        },
    );
    advance(&mut canceled, 100);
    assert!(canceled.emissions.is_empty());
}

#[test]
fn failure_is_bounded_and_never_inferred_from_cancellation() {
    let live = fixture_context();
    for status in [
        Some("canceled"),
        Some("cancelled"),
        Some("interrupted"),
        Some("completed"),
        None,
    ] {
        let context = Context {
            status: status.map(str::to_string),
            ..live.clone()
        };
        assert_eq!(acting_for(Presence::Stuck, None, &context), Act::Rest);
    }
    let expired = Context {
        status: Some("failed".into()),
        now_ms: Some(3000.),
        ..live.clone()
    };
    assert_eq!(acting_for(Presence::Stuck, None, &expired), Act::Rest);
    for index in [0, 14, 11, 12, 1] {
        let mut d = fixture(7);
        set_case(&mut d, &CASES[index]);
        assert!(!d.event(Event::Failed {
            kind: "executing".into(),
            status: Some("failed".into())
        }));
    }
    let mut d = fixture(7);
    set_case(&mut d, &CASES[7]);
    for status in [Some("canceled"), Some("interrupted"), None] {
        assert!(!d.event(Event::Failed {
            kind: "executing".into(),
            status: status.map(str::to_string)
        }));
    }
    assert!(d.event(Event::Failed {
        kind: "executing".into(),
        status: Some("failed".into())
    }));
    advance(&mut d, 13);
    assert!(!d.shots.iter().any(|s| s.beat));
}

#[test]
fn the_pad_saturates_at_four_lines() {
    let mut d = fixture(7);
    set_case(&mut d, &CASES[6]);
    for _ in 0..8 {
        d.event(Event::Tool {
            kind: "editing".into(),
        });
        advance(&mut d, 15);
    }
    assert_eq!(d.lines, 4.);
    assert!(d.pose()[p::padLines] > 3.8);
}

#[test]
fn every_parameter_stays_within_its_rate_limit_across_state_pairs() {
    // The oracle runs all 17×17 pairs at 30 and 60 fps; one frame rate and
    // every pair keeps the debug test fast while exercising each envelope.
    for from in &CASES {
        for to in &CASES {
            let mut d = fixture(7);
            set_case(&mut d, from);
            advance(&mut d, 110);
            d.set(
                presence_named(to.2),
                to.3.map(|kind| Activity {
                    kind: Some(kind.into()),
                    observed: kind != "unknown",
                    parallel: Some(3.),
                    active: Vec::new(),
                }),
                Context {
                    turn_id: Some(format!("transition-{}{}", from.0, to.0)),
                    status: Some(
                        if to.2 == "Stuck" {
                            "failed"
                        } else {
                            "completed"
                        }
                        .into(),
                    ),
                    ..fixture_context()
                },
            );
            let mut prev = d.pose();
            for frame in 0..60 {
                d.step(1. / 30.);
                let pose = d.pose();
                for i in 0..N {
                    assert!(pose[i].is_finite());
                    let delta = if i == p::wrenchSpin {
                        super::acting::angle_delta(pose[i], prev[i])
                    } else {
                        pose[i] - prev[i]
                    };
                    assert!(
                        delta.abs() <= tables().rates[i] + 1e-7,
                        "{}->{} {} @{frame}",
                        from.0,
                        to.0,
                        PARAMS[i]
                    );
                }
                prev = pose;
            }
        }
    }
}

#[test]
fn named_parts_keep_topology_and_finite_cubic_coordinates() {
    let mut topology = std::collections::HashMap::new();
    for dir in [MARK_DIRECTION, CRUISE_DIRECTION, OPEN_DIRECTION] {
        for case in &CASES {
            for size in [16., 24., 32., 512.] {
                let mut d = Director::new(Options {
                    reduced: true,
                    ..Options::default()
                });
                set_case(&mut d, case);
                let parts = scene(
                    &d,
                    View {
                        dir,
                        ..View::fitted(size, 1.)
                    },
                );
                for part in &parts.shapes {
                    if ["calf-", "bubble-", "spout-"]
                        .iter()
                        .any(|x| part.id.starts_with(x))
                    {
                        continue;
                    }
                    let signature: Vec<usize> =
                        part.path.iter().map(|c| c.coords().len()).collect();
                    let known = topology.entry(part.id.clone()).or_insert(signature.clone());
                    assert_eq!(*known, signature, "{}", part.id);
                    assert!(
                        part.path
                            .iter()
                            .flat_map(|c| c.coords())
                            .all(|v| v.is_finite())
                    );
                }
                for id in [
                    "body", "pouch", "pleat-1", "pleat-2", "jaw-band", "eye", "eyelid", "flipper",
                    "flukes",
                ] {
                    assert!(parts.shapes.iter().any(|s| s.id == id), "{id}");
                }
            }
        }
    }
}

#[test]
fn attentive_turns_keep_winding_and_area() {
    for id in ["pleat-1", "pleat-2", "flipper"] {
        for frame in 0..=30 {
            let mut pose = rig_pose();
            pose[p::yaw] = frame as f64 / 30.;
            let parts = build(MARK_DIRECTION, &pose, 0, 24., 1.);
            let shape = parts.shapes.iter().find(|s| s.id == id).unwrap();
            assert!(
                signed_area(&shape.path) < -0.8,
                "{id} at turn frame {frame}"
            );
        }
    }
    let states = [
        "rest", "think", "read", "search", "write", "browse", "needs", "hmm",
    ];
    for from in states {
        for to in states {
            let mut d = fixture(7);
            set_case(&mut d, CASES.iter().find(|c| c.0 == from).unwrap());
            advance(&mut d, 110);
            set_case(&mut d, CASES.iter().find(|c| c.0 == to).unwrap());
            for frame in 0..90 {
                d.step(1. / 30.);
                let parts = scene(&d, View::hero(512., 1.));
                let fin = parts.shapes.iter().find(|s| s.id == "flipper").unwrap();
                if fin.opacity > 0.5 {
                    assert!(signed_area(&fin.path) < -20., "{from}->{to}@{frame}");
                }
            }
        }
    }
}

#[test]
fn held_props_follow_the_whale_under_translation_rotation_and_scale() {
    let theta = 17f64.to_radians();
    let scale = 0.83;
    for name in [
        "read", "search", "write", "run", "browse", "computer", "connect",
    ] {
        let d = Director::new(Options::default());
        let mut base = d.target_for(Act::from_id(name).unwrap());
        base[p::x] = 0.;
        base[p::y] = 0.;
        base[p::rot] = 0.;
        base[p::scale] = 1.;
        let mut moved = base;
        moved[p::x] = 9.;
        moved[p::y] = -6.;
        moved[p::rot] = 17.;
        moved[p::scale] = scale;
        let shapes =
            |pose: &Pose| props::shapes(pose, &build(MARK_DIRECTION, pose, 0, 24., 1.), false);
        let (before, after) = (shapes(&base), shapes(&moved));
        for (a, b) in before.iter().zip(&after) {
            if a.opacity <= 0. {
                continue;
            }
            for (ca, cb) in a.path.iter().zip(&b.path) {
                let (ca, cb) = (ca.coords(), cb.coords());
                for j in (0..ca.len()).step_by(2) {
                    let x = 9. + scale * (ca[j] * theta.cos() - ca[j + 1] * theta.sin());
                    let y = -6. + scale * (ca[j] * theta.sin() + ca[j + 1] * theta.cos());
                    assert!(
                        (cb[j] - x).hypot(cb[j + 1] - y) < 1e-6,
                        "{name}/{} detached",
                        a.id
                    );
                }
            }
        }
    }
}

#[test]
fn attention_stays_in_frame_and_keeps_its_profile() {
    let mut d = fixture(7);
    set_case(&mut d, &CASES[0]);
    advance(&mut d, 100);
    set_case(&mut d, &CASES[11]);
    for frame in 0..400 {
        d.step(1. / 30.);
        let dirs: [Direction; 3] = [MARK_DIRECTION, CRUISE_DIRECTION, OPEN_DIRECTION];
        for dir in dirs {
            let parts = scene(
                &d,
                View {
                    dir,
                    ..View::hero(512., 1.)
                },
            );
            let find = |id: &str| parts.shapes.iter().find(|s| s.id == id).unwrap().opacity;
            assert_eq!(find("flipper"), 0., "attention grows a second limb");
            assert_eq!(find("eye-far"), 0., "attention becomes a frontal face");
            for shape in parts.shapes.iter().filter(|s| s.opacity > 0.) {
                for v in shape.path.iter().flat_map(|c| c.coords()) {
                    assert!(v.abs() < 62., "{} clips at frame {frame}", shape.id);
                }
            }
        }
    }
}

#[test]
fn braille_bits_holes_threshold_and_cell_aspect() {
    use super::rig::{Cmd, Parts};
    let rect = |x: f64, y: f64, w: f64, h: f64| {
        vec![
            Cmd::M([x, y]),
            Cmd::C([x, y, x + w, y, x + w, y]),
            Cmd::C([x + w, y, x + w, y + h, x + w, y + h]),
            Cmd::C([x + w, y + h, x, y + h, x, y + h]),
            Cmd::C([x, y + h, x, y, x, y]),
            Cmd::Z,
        ]
    };
    let shape = |id: &str, role, opacity, path| Shape {
        id: id.into(),
        path,
        role,
        opacity,
    };
    let parts = |shapes| Parts {
        shapes,
        anchors: Default::default(),
        small: false,
        unit_scale: 1.,
    };
    let body = shape("body", Role::Body, 1., rect(-62., -62., 124., 124.));
    let hole = shape("eye", Role::Hole, 1., rect(-31., -31., 62., 62.));
    let grid = rasterize(&parts(vec![body.clone(), hole]), 8, 4, 0.5);
    assert_eq!(grid.cells[8 + 3], 0, "the eye is a real hole");
    assert_eq!(grid.cells[0], 255);
    let faint = Shape {
        opacity: 0.49,
        ..body.clone()
    };
    assert!(
        rasterize(&parts(vec![faint]), 32, 16, 0.5)
            .cells
            .iter()
            .all(|b| *b == 0)
    );
    let half = Shape {
        opacity: 0.5,
        ..body.clone()
    };
    assert!(
        rasterize(&parts(vec![half]), 32, 16, 0.5)
            .cells
            .iter()
            .any(|b| *b != 0)
    );
    let square = parts(vec![shape(
        "body",
        Role::Body,
        1.,
        rect(-15., -15., 30., 30.),
    )]);
    let dots = |g: Grid| g.cells.iter().map(|b| b.count_ones()).sum::<u32>();
    assert!(dots(rasterize(&square, 32, 16, 1.)) < dots(rasterize(&square, 32, 16, 0.5)));
    let one = Grid {
        cols: 1,
        rows: 1,
        cells: vec![0x80],
    };
    assert_eq!(one.text(), "\u{2880}");
}

#[test]
fn reduced_motion_and_a_new_session_carry_no_residual_activity() {
    let mut d = Director::new(Options {
        seed: 11,
        ..Options::default()
    });
    set_case(&mut d, &CASES[15]);
    advance(&mut d, 80);
    d.set_reduced(true);
    let first = braille(&d, 32, 16);
    advance(&mut d, 60);
    assert_eq!(braille(&d, 32, 16), first, "reduced motion does not move");
    let fresh = Director::new(Options {
        reduced: true,
        ..Options::default()
    });
    assert_eq!(fresh.acting, Act::Rest);
    assert!(fresh.calves.iter().all(|c| c.target == 0.));
    assert!(fresh.particles.is_empty());
}

// ---------------------------------------------------------------------------
// The app-facing Stage: one Director per foreground session, one clock.
// ---------------------------------------------------------------------------

fn inputs(presence: Presence, activity: Option<Activity>, context: Context) -> Inputs {
    Inputs {
        presence,
        activity,
        context,
    }
}

#[test]
fn a_new_foreground_session_replaces_the_director() {
    let mut stage = Stage::new();
    let pod = Activity {
        kind: Some("delegating".into()),
        observed: true,
        parallel: Some(3.),
        active: Vec::new(),
    };
    let now = Instant::now();
    stage.observe(
        Some("session-a"),
        inputs(Presence::Working, Some(pod.clone()), fixture_context()),
        false,
    );
    stage.advance(now);
    stage.advance(now + Duration::from_millis(500));
    assert_eq!(stage.acting(), Act::Pod);
    assert!(stage.director().calves.iter().all(|c| c.value > 0.1));
    let done = || inputs(Presence::Done, None, fixture_context());
    stage.observe(Some("session-a"), done(), false);
    let spouts = |stage: &Stage| stage.director().emissions.len();
    stage.advance(now + Duration::from_millis(1500));
    let celebrated = spouts(&stage);
    // Another session starts clean: no calves, no remembered turn.
    stage.observe(
        Some("session-b"),
        inputs(Presence::Idle, None, Context::default()),
        false,
    );
    assert_eq!(stage.acting(), Act::Rest);
    assert!(
        stage
            .director()
            .calves
            .iter()
            .all(|c| c.value == 0. && c.target == 0.)
    );
    assert!(stage.director().shots.is_empty());
    assert_eq!(spouts(&stage), 0);
    assert!(celebrated >= 1);
}

#[test]
fn the_shared_clock_is_idempotent_per_frame_and_caps_catch_up() {
    let mut stage = Stage::new();
    stage.observe(
        None,
        inputs(Presence::Idle, None, Context::default()),
        false,
    );
    let start = Instant::now();
    stage.advance(start);
    stage.advance(start + Duration::from_millis(100));
    let f = stage.director().f;
    assert!((f - 3.).abs() < 1e-9, "100 ms is three frames, got {f}");
    // A second surface painting in the same frame does not step again.
    stage.advance(start + Duration::from_millis(101));
    assert_eq!(stage.director().f, f);
    // A long hidden interval resumes without replaying it.
    stage.advance(start + Duration::from_secs(600));
    assert!((stage.director().f - f - 30.).abs() < 1e-6);
}

#[test]
fn surfaces_schedule_within_their_caps_and_not_at_all_when_reduced() {
    let mut stage = Stage::new();
    stage.observe(
        None,
        inputs(Presence::Idle, None, Context::default()),
        false,
    );
    let now = Instant::now();
    stage.advance(now);
    stage.advance(now + Duration::from_secs(1));
    assert_eq!(
        stage.cadence(Tier::Hero),
        Some(Duration::from_secs_f64(1. / 8.))
    );
    assert_eq!(
        stage.cadence(Tier::Small),
        Some(Duration::from_secs_f64(1. / 2.))
    );
    stage.observe(
        None,
        inputs(Presence::Thinking, None, Context::default()),
        false,
    );
    assert_eq!(
        stage.cadence(Tier::Hero),
        Some(Duration::from_secs_f64(1. / 30.))
    );
    assert_eq!(
        stage.cadence(Tier::Small),
        Some(Duration::from_secs_f64(1. / 15.))
    );
    stage.observe(
        None,
        inputs(Presence::Thinking, None, Context::default()),
        true,
    );
    assert_eq!(stage.cadence(Tier::Hero), None);
    assert_eq!(stage.cadence(Tier::Small), None);
    // Reduced motion paints the poster; stepping changes nothing.
    let before = stage.director().pose();
    stage.advance(now + Duration::from_secs(3));
    assert_eq!(stage.director().pose(), before);
}

#[test]
fn owner_activity_is_read_as_served_and_never_inferred() {
    // The owner's canonical kind drives the action; a tool name or caption
    // beside it is never consulted.
    let served = json!({
        "observed": true, "kind": "editing", "tool": "exec_command",
        "label": "Running a command", "parallel": 2,
        "active": [{"kind": "editing", "sinceMs": 1200}, {"kind": "reading"}, "junk"]
    });
    let activity = stage::activity(&served, true);
    assert_eq!(activity.kind.as_deref(), Some("editing"));
    assert_eq!(activity.parallel, Some(2.));
    assert_eq!(
        activity.active,
        vec![Span {
            kind: "editing".into(),
            since_ms: 1200.
        }]
    );
    let live = fixture_context();
    assert_eq!(
        acting_for(Presence::Working, Some(&activity), &live),
        Act::Write
    );
    // No kind: plain work, even with a descriptive tool name.
    let nameless = stage::activity(
        &json!({"observed": true, "tool": "read_file", "label": "Reading files"}),
        true,
    );
    assert_eq!(
        acting_for(Presence::Working, Some(&nameless), &live),
        Act::Busy
    );
    // The owner said unobserved: plain work.
    let unobserved = stage::activity(&served, false);
    assert_eq!(
        acting_for(Presence::Working, Some(&unobserved), &live),
        Act::Busy
    );
    // Stale freshness: plain work.
    let stale = Context {
        live: false,
        ..live
    };
    assert_eq!(
        acting_for(Presence::Working, Some(&activity), &stale),
        Act::Busy
    );
}

fn every_coordinate(path: &super::rig::Path) -> Vec<f64> {
    path.iter().flat_map(|c| c.coords().to_vec()).collect()
}

#[test]
fn optical_sizes_land_on_the_half_pixel_grid() {
    // 16/24/32 px icons snap every visible contour coordinate to half a
    // device pixel, at 1× and 2×; the hero rig is never snapped.
    for (size, dpr) in [
        (16., 1.),
        (24., 1.),
        (32., 1.),
        (16., 2.),
        (24., 2.),
        (32., 2.),
    ] {
        let parts = build(MARK_DIRECTION, &rig_pose(), 2, size, dpr);
        assert!(parts.small);
        let half = 124. / (size * dpr) / 2.;
        for shape in parts.shapes.iter().filter(|s| s.opacity > 0.) {
            for v in every_coordinate(&shape.path) {
                let cells = v / half;
                assert!(
                    (cells - cells.round()).abs() < 1e-6,
                    "{} at {size}px@{dpr}x: {v} is off the grid",
                    shape.id
                );
            }
        }
    }
    assert!(!build(MARK_DIRECTION, &rig_pose(), 0, 512., 2.).small);
    assert_eq!(View::fitted(28., 2.).lod, 2);
    assert_eq!(View::fitted(40., 2.).lod, 0);
}

#[test]
fn one_ink_subtracts_apertures_while_color_paints_them() {
    let parts = build(MARK_DIRECTION, &rig_pose(), 0, 512., 1.);
    let mono = super::holes_for("body", &parts.shapes, true);
    let color = super::holes_for("body", &parts.shapes, false);
    // One ink reveals the real background through the eye and throat.
    assert!(!mono.is_empty(), "one ink must cut the apertures");
    assert!(
        color.is_empty(),
        "the color body keeps its painted apertures"
    );
    assert!(
        parts
            .shapes
            .iter()
            .any(|s| s.role == Role::Hole && s.id == "eye"),
        "color paints the light eye aperture as its own shape"
    );
    // Prop details are compound holes in every theme.
    let mut pose = rig_pose();
    pose[p::page] = 1.;
    let with_page = build(MARK_DIRECTION, &pose, 0, 512., 1.);
    let mut shapes = with_page.shapes.clone();
    shapes.extend(props::shapes(&pose, &with_page, false));
    for mono in [true, false] {
        assert!(!super::holes_for("prop-page", &shapes, mono).is_empty());
    }
}

fn cove_stage(presence: Presence) -> Stage {
    let mut stage = Stage::new();
    stage.observe(
        Some("session"),
        inputs(presence, None, Context::default()),
        false,
    );
    stage
}

fn step_stage(stage: &mut Stage, start: Instant, frames: u64) -> Instant {
    let mut now = start;
    for _ in 0..frames {
        now += Duration::from_micros(33_334);
        stage.advance(now);
    }
    now
}

fn max_drift(a: &super::Parts, b: &super::Parts) -> f64 {
    a.shapes
        .iter()
        .zip(&b.shapes)
        .flat_map(|(a, b)| {
            every_coordinate(&a.path)
                .into_iter()
                .zip(every_coordinate(&b.path))
                .map(|(x, y)| (x - y).abs())
        })
        .fold(0., f64::max)
}

#[test]
fn the_cove_rides_the_director_clock_and_only_turns_the_resting_gaze() {
    let mut stage = cove_stage(Presence::Idle);
    let view = View::hero(320., 2.);
    let start = Instant::now();
    stage.advance(start);
    let a = stage.cove_scene(view, false);
    // Scenery is only move/cubic/close with finite coordinates, and the
    // cove sits behind and in front of the character.
    assert!(a.behind.len() >= 10 && a.front.len() >= 2);
    for layer in a.behind.iter().chain(&a.front) {
        for path in &layer.paths {
            assert!(matches!(path.first(), Some(super::Cmd::M(_))));
            assert!(every_coordinate(path).iter().all(|v| v.is_finite()));
        }
    }
    // The water moves on the Director's own clock.
    let now = step_stage(&mut stage, start, 20);
    let b = stage.cove_scene(view, false);
    assert_ne!(a.behind, b.behind, "the water should move with the clock");
    // Without a pointer the cove never alters the performance.
    assert!(max_drift(&b.parts, &scene(stage.director(), view)) < 1e-9);
    // At rest the gaze follows the pointer, easing in over the frames.
    stage.cove_observe(62., 62.);
    let mut now = now;
    for _ in 0..30 {
        now = step_stage(&mut stage, now, 1);
        let _ = stage.cove_scene(view, false);
    }
    let gazing = stage.cove_scene(view, false);
    assert!(
        max_drift(&gazing.parts, &scene(stage.director(), view)) > 0.1,
        "the resting whale should turn toward the pointer"
    );
    // A tap ripples the water but never reaches the Director's inputs.
    let acting = stage.acting();
    let presence = stage.director().presence;
    for x in [0., 10., 20., 30.] {
        stage.cove_tap(x, 40.);
    }
    assert_eq!(stage.cove().ripples(), 3, "at most three ripples");
    assert_eq!(stage.acting(), acting);
    assert_eq!(stage.director().presence, presence);
    let now = step_stage(&mut stage, now, 1);
    let rippling = stage.cove_scene(view, false);
    assert!(rippling.front.len() > gazing.front.len());
    // A ripple keeps the hero at its active cap until it has spread.
    assert_eq!(
        stage.cadence(Tier::Hero),
        Some(Duration::from_secs_f64(1. / 30.))
    );
    let now = step_stage(&mut stage, now, 90);
    assert_eq!(stage.cove_scene(view, false).front.len(), 2);
    // Working: the gaze eases back to the Director's own pose.
    stage.cove_leave();
    stage.observe(
        Some("session"),
        inputs(Presence::Thinking, None, Context::default()),
        false,
    );
    let mut now = now;
    for _ in 0..90 {
        now = step_stage(&mut stage, now, 1);
        let _ = stage.cove_scene(view, false);
    }
    let working = stage.cove_scene(view, false);
    let drift = max_drift(&working.parts, &scene(stage.director(), view));
    assert!(drift < 0.05, "working gaze stays the Director's: {drift}");
}

#[test]
fn the_cove_is_still_under_reduced_motion_and_resets_with_the_session() {
    let mut stage = cove_stage(Presence::Idle);
    let view = View::hero(320., 1.);
    stage.observe(
        Some("session"),
        inputs(Presence::Idle, None, Context::default()),
        true,
    );
    stage.cove_observe(-62., 20.);
    stage.cove_tap(0., 40.);
    let a = stage.cove_scene(view, false);
    let start = Instant::now();
    stage.advance(start);
    step_stage(&mut stage, start, 45);
    let b = stage.cove_scene(view, false);
    assert_eq!(a.behind, b.behind, "reduced motion paints a still cove");
    assert_eq!(a.front, b.front);
    // No gaze under reduced motion: the poster is exactly the Director's.
    assert!(max_drift(&b.parts, &scene(stage.director(), view)) < 1e-9);
    assert_eq!(stage.cadence(Tier::Hero), None);
    // Dark and light palettes differ; geometry does not.
    let dark = stage.cove_scene(view, true);
    let light = stage.cove_scene(view, false);
    assert_ne!(dark.behind[0].color, light.behind[0].color);
    assert_eq!(dark.behind[0].paths, light.behind[0].paths);
    // A new foreground session starts in a still cove.
    stage.observe(
        Some("other"),
        inputs(Presence::Idle, None, Context::default()),
        true,
    );
    assert_eq!(stage.cove().ripples(), 0);
}
