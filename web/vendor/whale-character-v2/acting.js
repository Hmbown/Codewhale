// Acting: presence → acting state → pose over time.
//
//   pose(t) = spring(base pose of the state)          state blends, follow-through
//           + Σ one-shot clips (enter, exit, beats)    authored, exact frame timing
//           + loop clip of the state                   the ongoing performance
//           + ambient (breath, bob, blinks, puffs)     alive, never dancing
//           + drag (flukes and fins trail the body)    secondary motion
//
// Clips are data — keyframes in frames at 30 fps — so a GPUI port reads the
// same tables. Nothing here invents activity: every acting state comes from
// presence, every work action from an Engine activity kind, every beat from an
// Engine event.
(function (root) {
  'use strict';
  const R = root.WhaleRig, PR = root.WhaleProps;
  const { lerp, clamp, D2R, TAU } = R;
  const FPS = 30;

  // ---------------------------------------------------------------------------
  // Easing and clips.
  // ---------------------------------------------------------------------------
  const EASE = {
    l: t => t,
    i: t => t * t * t,
    o: t => 1 - Math.pow(1 - t, 3),
    io: t => t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2,
    b: t => { const c1 = 1.70158, c3 = c1 + 1; return 1 + c3 * Math.pow(t - 1, 3) + c1 * Math.pow(t - 1, 2); },
    s: t => (t < 1 ? 0 : 1),
  };
  function track(keys, f) {
    if (f <= keys[0][0]) return keys[0][1];
    for (let i = 1; i < keys.length; i++) {
      const [f1, v1, e = 'io'] = keys[i];
      if (f <= f1) {
        const [f0, v0] = keys[i - 1];
        const t = f1 === f0 ? 1 : (f - f0) / (f1 - f0);
        return lerp(v0, v1, EASE[e](t));
      }
    }
    return keys[keys.length - 1][1];
  }
  function evalClip(clip, f, out, w = 1) {
    for (const k in clip.keys) out[k] = (out[k] || 0) + track(clip.keys[k], f) * w;
    return out;
  }
  // Clip builder: `k(dur, {param: [[frame, value, ease], ...]}, events)`.
  const k = (dur, keys, events = [], extra = {}) => ({ dur, keys, events, ...extra });

  // ---------------------------------------------------------------------------
  // Presence and Engine activity → acting state. One-to-one with presence;
  // Working picks its action from the owner's activity kind (the canonical
  // classifier in the shared pet owner) and falls back to plain work.
  // ---------------------------------------------------------------------------
  const KIND_TO_ACT = {
    reading: 'read', files: 'read', memory: 'read', searching: 'search', editing: 'write',
    executing: 'run', testing: 'run', browsing: 'browse', network: 'connect', computer: 'computer',
    responding: 'talk', communicating: 'talk', delegating: 'pod', thinking: 'think',
    tool: 'busy', unknown: 'busy', waiting: 'busy', error: 'busy',
  };
  function actingFor(presence, activity, context = {}) {
    switch (presence) {
      case 'Offline': return 'sleep';
      case 'Idle': return 'rest';
      case 'Listening': return 'listen';
      case 'Thinking': return 'think';
      case 'NeedsYou': return 'needs';
      case 'Done': return 'done';
      case 'Stuck': {
        const age = context.nowMs - context.failedAtMs;
        return ['failed', 'error'].includes(context.status) && age >= 0 && age < 3000 ? 'hmm' : 'rest';
      }
      case 'Working': return activity?.observed === true && context.freshness === 'Live'
        ? KIND_TO_ACT[activity.kind] || 'busy' : 'busy';
      default: return 'rest';
    }
  }

  // ---------------------------------------------------------------------------
  // Props live in the pose so springs and clips drive them like anything else.
  // ---------------------------------------------------------------------------
  const PROP_POSE = {
    page: 0, pageFlip: 0, pageRot: 0, pageDX: 0, pageDY: 0,
    pad: 0, padLines: 0, pencil: 0, scribX: 0, scribY: 0,
    lens: 0, wrench: 0, wrenchSpin: 0, wrenchY: 0, glass: 0, glassExt: 0, pod: 0,
    spout: 0, splash: 0, cursor: 0, cursorX: 0, cursorY: 0, link: 0, linkTilt: 0,
  };
  const REST = { head: 0, fluke: 6, fin: 0, lid: 0.04, smile: 0.42, lookX: 0.15, lookY: 0.05 };

  // ---------------------------------------------------------------------------
  // The acting catalogue. Frames at 30 fps. Poses are absolute; clips are offsets.
  // ---------------------------------------------------------------------------
  const ACTS = {
    rest: {
      base: { ...REST },
      enter: k(18, { squash: [[0, 0], [6, 0.03], [18, 0, 'o']], y: [[0, 0], [6, 0.8], [18, 0, 'o']] }),
      loop: k(1, {}),
      ambient: { breath: 1, bob: 1, blink: 1, dart: 1, puff: 1, period: 150 },
      poster: {},
    },
    listen: {
      base: { ...REST, yaw: 0.5, head: 6, tilt: 8, fin: 10, lid: 0, eyeScale: 1.06, brow: 0.4, smile: 0.55, lookX: -0.2, lookY: -0.05, scale: 1.04, x: 2 },
      enter: k(14, { squash: [[0, 0], [4, 0.05], [14, 0, 'o']], y: [[0, 0], [4, 1.2], [14, 0, 'o']] }),
      // Small receptive nods; the fin stays cupped.
      loop: k(96, { head: [[0, 0], [64, 0], [70, -4, 'o'], [78, 1], [86, 0, 'io']], tilt: [[0, 0], [48, 0], [60, 3], [80, 0]] }),
      ambient: { breath: 0.8, bob: 0.6, blink: 1, dart: 0, puff: 0, period: 130 },
      poster: {},
    },
    think: {
      base: { ...REST, head: 10, curl: 0.3, fin: 112, lookX: 0.62, lookY: -0.88, lid: 0.1, brow: 0.55, browTilt: 0.45, smile: 0.22, fluke: 14 },
      // The thought beat: a slow blink, then the eyes go up and away.
      enter: k(20, { lid: [[0, 0], [3, 0.95, 'i'], [6, 0.95], [11, 0, 'o']], lookX: [[0, 0], [12, 0]], lookY: [[0, 0], [12, 0]] }, [[16, 'thought']]),
      // Fin taps the chin; eyes drift between two thinking points; a thought rises.
      loop: k(72, {
        fin: [[0, 0], [40, 0], [44, 7, 'o'], [48, 0], [52, 7, 'o'], [56, 0]],
        lookX: [[0, 0], [24, 0], [28, -0.5, 'o'], [52, -0.5], [56, 0, 'o']],
        fluke: [[0, 0], [36, 5], [72, 0]],
      }, [[30, 'thought']]),
      ambient: { breath: 0.7, bob: 0.5, blink: 0.6, dart: 0, puff: 0, period: 130 },
      poster: { particles: [['thought-cloud']] },
    },
    busy: {
      // Presence says Working and nothing says what: swim in place, on it.
      base: { ...REST, head: -2, rot: -12, curl: -0.38, fin: 24, lid: 0.16, lookX: 0.5, lookY: 0.15, smile: 0.3 },
      enter: k(10, { squash: [[0, 0], [3, 0.05], [10, 0, 'o']] }),
      loop: k(24, { fluke: [[0, -12], [12, 12], [24, -12]], fin: [[0, 8], [12, -8], [24, 8]], y: [[0, 0], [6, -0.6], [18, 0.6], [24, 0]] }),
      ambient: { breath: 0.5, bob: 0.4, blink: 0.8, dart: 0, puff: 0, period: 120 },
      poster: {},
    },
    read: {
      base: { ...REST, head: -5, fin: 132, lookX: 0.62, lookY: 0.45, lid: 0.2, lidLow: 0.06, smile: 0.25, page: 1 },
      enter: k(14, { fin: [[0, 0], [4, -18, 'o'], [12, 0, 'o']], squash: [[0, 0], [3, 0.03], [10, 0, 'o']] }),
      // Three lines per cycle: scan across, snap back.
      loop: k(54, {
        lookX: [[0, -0.45], [14, 0.45, 'l'], [18, -0.45, 'o'], [32, 0.45, 'l'], [36, -0.45, 'o'], [50, 0.45, 'l'], [54, -0.45, 'o']],
        lookY: [[0, -0.3], [16, -0.3], [18, 0, 's'], [34, 0], [36, 0.3, 's'], [52, 0.3], [54, -0.3, 's']],
        head: [[0, 0.8], [27, -0.8], [54, 0.8]],
      }),
      // One real read event → one page turn (the nose nudges it along).
      beat: k(12, { pageFlip: [[0, 0], [6, 1, 'io'], [12, 0, 'io']], head: [[0, 0], [3, -3, 'o'], [8, 1], [12, 0]] }),
      exit: k(10, { pageDX: [[0, 0], [10, -6, 'i']], pageDY: [[0, 0], [10, 4, 'i']], fin: [[0, 0], [5, -20, 'o'], [10, 0]] }),
      ambient: { breath: 0.6, bob: 0.5, blink: 1, dart: 0, puff: 0, period: 130 },
      poster: {},
    },
    search: {
      base: { ...REST, head: -2, fin: 168, lookX: 0.85, lookY: 0.05, lid: 0.04, brow: 0.3, smile: 0.3, lens: 1 },
      enter: k(14, { fin: [[0, 0], [4, -20, 'o'], [12, 0, 'o']], squash: [[0, 0], [3, 0.03], [10, 0, 'o']] }),
      // Sweep the lens across; the eyes follow it.
      loop: k(64, {
        fin: [[0, -16], [32, 12], [64, -16]],
        lookX: [[0, 0.1], [32, 0.2], [64, 0.1]], lookY: [[0, 0.45], [32, -0.35], [64, 0.45]],
        head: [[0, -2], [32, 2], [64, -2]],
      }),
      beat: k(10, { fin: [[0, 0], [3, 8, 'o'], [10, 0]], eyeScale: [[0, 0], [3, 0.08], [10, 0]] }),
      exit: k(10, { fin: [[0, 0], [6, -26, 'o'], [10, 0]] }),
      ambient: { breath: 0.6, bob: 0.5, blink: 0.8, dart: 0, puff: 0, period: 130 },
      poster: {},
    },
    write: {
      base: { ...REST, head: -9, fin: 118, lookX: 0.8, lookY: 0.85, lid: 0.3, lidLow: 0.08, smile: 0.15, mouthSide: 0.45, pad: 1, pencil: 1 },
      enter: k(14, { fin: [[0, 0], [4, -16, 'o'], [12, 0, 'o']], squash: [[0, 0], [3, 0.03], [10, 0, 'o']] }),
      // Scribble: quick zigzags, the head bobbing with each stroke, flukes keeping time.
      loop: k(18, {
        scribX: [[0, -3], [3, 3, 'l'], [6, -2, 'l'], [9, 3.5, 'l'], [12, -1, 'l'], [15, 2.5, 'l'], [18, -3, 'l']],
        scribY: [[0, 0], [3, -0.8, 'l'], [6, 0.6, 'l'], [9, -0.7, 'l'], [12, 0.5, 'l'], [15, -0.4, 'l'], [18, 0, 'l']],
        head: [[0, 0], [9, -1.5], [18, 0]], fluke: [[0, -5], [9, 5], [18, -5]],
      }),
      // One real edit → one more written line.
      beat: k(12, { squash: [[0, 0], [2, 0.03], [8, 0]] }),
      exit: k(10, { fin: [[0, 0], [5, -18, 'o'], [10, 0]] }),
      ambient: { breath: 0.5, bob: 0.4, blink: 0.8, dart: 0, puff: 0, period: 120 },
      poster: {},
    },
    run: {
      // The tail curls up over the back — the mark's C — to work the wrench.
      base: { ...REST, curl: 0.64, head: 7, fluke: -24, lookX: -0.35, lookY: -0.95, lid: 0.1, brow: 0.15, smile: 0.35, wrench: 1 },
      enter: k(20, { curl: [[0, 0], [6, -0.12, 'o'], [20, 0, 'o']], fluke: [[0, 0], [6, -12], [20, 0]], squash: [[0, 0], [5, 0.05], [14, 0, 'o']] }),
      // Ratchet: a firm turn, a quick reset; the body leans into each turn.
      loop: k(30, {
        wrenchSpin: [[0, 0], [10, 42, 'io'], [16, 42], [26, 0, 'o'], [30, 0]],
        rot: [[0, 0], [10, 2], [26, 0]], fluke: [[0, 0], [10, 6], [26, 0]],
      }),
      // A new command: toss the wrench, spin, catch.
      beat: k(12, {
        wrenchY: [[0, 0], [6, -8, 'o'], [11, 0, 'i'], [12, 0]],
        wrenchSpin: [[0, 0], [11, 360, 'io'], [12, 360]],
        fluke: [[0, 0], [2, -7], [5, 5], [12, 0]],
        squash: [[0, 0], [2, 0.04], [5, -0.025], [10, 0.03], [12, 0]],
      }),
      exit: k(12, { wrenchY: [[0, 0], [12, -6, 'o']] }),
      ambient: { breath: 0.5, bob: 0.4, blink: 0.9, dart: 0, puff: 0, period: 120 },
      poster: {},
    },
    browse: {
      base: { ...REST, head: 7, y: -3, fin: 122, lookX: 0.9, lookY: -0.1, lid: 0.06, smile: 0.3, glass: 1, glassExt: 1 },
      enter: k(20, { y: [[0, 0], [5, 1.5, 'o'], [14, 0, 'o']], glassExt: [[0, 0], [10, 0], [18, 0]] }),
      // A slow look across the horizon.
      loop: k(80, { rot: [[0, -3], [40, 3], [80, -3]], head: [[0, -1], [40, 1.5], [80, -1]] }),
      // A new page: refocus.
      beat: k(10, { glassExt: [[0, 0], [4, 0.18, 'o'], [10, 0]] }),
      exit: k(10, { glassExt: [[0, 0], [8, -0.9, 'i']] }),
      ambient: { breath: 0.6, bob: 0.5, blink: 0.7, dart: 0, puff: 0, period: 130 },
      poster: {},
    },
    talk: {
      // Writing the response: a three-quarter turn toward you, talking.
      base: { ...REST, yaw: 0.3, head: 3, fin: 36, lookX: -0.3, lookY: 0, lid: 0, smile: 0.5, eyeScale: 1.02 },
      enter: k(12, { squash: [[0, 0], [3, 0.04], [12, 0, 'o']] }),
      loop: k(84, {
        mouth: [[0, 0.05], [4, 0.62], [8, 0.18], [13, 0.72], [17, 0.14], [21, 0.52], [26, 0.08], [30, 0.66], [36, 0.12], [40, 0.46], [44, 0.05], [52, 0.05], [56, 0.64], [61, 0.16], [66, 0.58], [72, 0.1], [78, 0.44], [84, 0.05]],
        fin: [[0, 0], [44, 0], [52, 32, 'o'], [64, 20], [74, 0]],
        head: [[0, 0], [8, 1.5], [22, -1], [36, 1], [50, 0], [84, 0]],
      }, [[18, 'speech'], [60, 'speech']]),
      ambient: { breath: 0.6, bob: 0.5, blink: 1, dart: 0, puff: 0, period: 120 },
      poster: { particles: [['speech']] },
    },
    pod: {
      // Coordinating agents: calves swim alongside; the whale keeps an eye on them.
      base: { ...REST, head: 2, lookX: -0.55, lookY: -0.1, smile: 0.45, pod: 1 },
      enter: k(16, { squash: [[0, 0], [4, 0.04], [14, 0, 'o']] }),
      loop: k(120, { lookX: [[0, 0], [50, 0], [56, 0.9, 'o'], [90, 0.9], [96, 0, 'o']], head: [[0, 0], [60, 0], [66, -3, 'o'], [72, 0]] }),
      ambient: { breath: 0.8, bob: 0.8, blink: 1, dart: 0, puff: 0, period: 140 },
      poster: {},
    },
    needs: {
      // A clear nose-up posture asks for attention. The whole whale acts;
      // the flipper remains tucked and the familiar profile stays intact.
      base: { ...REST, yaw: 1, rot: -24, head: 4, x: 2, y: -3, scale: 1.06, fluke: 22, fin: 0, finFar: 0, lid: 0, eyeScale: 1.06, brow: 0.3, smile: 0.42, lookX: 0.1, lookY: -0.25 },
      enter: k(42, {
        rot: [[0, 0], [4, 6, 'o'], [12, -4, 'o'], [22, 1], [36, 0, 'io']],
        y: [[0, 0], [5, 1.5], [12, -3, 'o'], [24, 0.6], [40, 0, 'io']],
        squash: [[0, 0], [4, 0.045], [10, -0.045, 'o'], [22, 0.012], [36, 0]],
        fluke: [[0, 0], [8, -8], [16, 26, 'o'], [24, -8], [34, 5], [42, 0, 'io']],
        eyeScale: [[0, 0], [8, 0], [13, 0.05, 'o'], [28, 0, 'io']],
      }),
      // Hold the readable posture. One slow settling rock, never a repeated alarm.
      loop: k(360, {
        rot: [[0, 0], [200, 0], [230, 3, 'io'], [260, 3], [300, 0, 'io']],
        fluke: [[0, 0], [210, 0], [240, 8, 'io'], [300, 0, 'io']],
        lid: [[0, 0], [140, 0], [144, 0.9, 'i'], [152, 0, 'o']],
      }),
      exit: k(12, { squash: [[0, 0], [4, 0.025], [12, 0, 'o']] }),
      ambient: { breath: 0.65, bob: 0.25, blink: 0.4, dart: 0, puff: 0, period: 150 },
      poster: {},
    },
    done: {
      base: { ...REST, head: 5, fluke: 10, lidLow: 0.62, smile: 0.9, fin: 22, lid: 0 },
      // Rise from a visible water surface and exhale one broad fountain.
      enter: k(80, {
        squash: [[0, 0], [6, 0.1, 'o'], [10, -0.08, 'o'], [22, 0.02], [30, 0, 'o']],
        y: [[0, 0], [6, 3], [16, -7, 'o'], [32, -5], [70, 0, 'io']],
        rot: [[0, 0], [6, 3], [16, -10, 'o'], [32, -8], [70, 0, 'io']],
        spout: [[0, 0], [10, 0], [18, 1, 'o'], [34, 1], [62, 0, 'io']],
        splash: [[0, 0], [6, 0.5], [14, 1, 'o'], [38, 0.8], [70, 0, 'io']],
        fluke: [[0, 0], [8, -14], [14, 30, 'o'], [24, -8], [34, 0, 'o']],
        curl: [[0, 0], [30, 0], [48, 0.5, 'io'], [60, 0.5], [78, 0, 'io']],
        head: [[0, 0], [30, 0], [48, -6, 'io'], [60, -6], [78, 0, 'io']],
        lidLow: [[0, 0], [8, 0], [12, 0.25, 'o'], [30, 0.2], [48, 0.3], [78, 0]],
      }, [[18, 'spout', { n: 5, big: true }]]),
      loop: k(1, {}),
      ambient: { breath: 0.9, bob: 0.8, blink: 0, dart: 0, puff: 0, period: 150 },
      poster: { rot: -10, y: -4, head: 7, spout: 1, splash: 1 },
    },
    hmm: {
      // A visible recoil and nose-down pause; no distressed face or thought cloud.
      base: { ...REST, rot: 22, x: -3, y: 2, scale: 0.96, head: -10, yaw: 0.45, curl: 0.7, fluke: 34, fin: 0, lookX: -0.4, lookY: 0.5, lid: 0.46, mouthSide: 0.45, smile: 0.1 },
      enter: k(30, { x: [[0, 0], [7, -4, 'o'], [24, 0, 'io']], rot: [[0, 0], [8, 5, 'o'], [30, 0, 'io']], head: [[0, 0], [3, 4, 'o'], [10, 0, 'io']], lid: [[0, 0], [2, 0.7], [5, 0.7], [9, 0, 'o']] }),
      loop: k(120, { head: [[0, 0], [60, 0], [76, 3, 'io'], [96, 0, 'io']] }),
      ambient: { breath: 0.4, bob: 0.1, blink: 0.6, dart: 0, puff: 0, period: 140 },
      poster: {},
    },
    computer: {
      base: { ...REST, head: 4, lookX: 0.8, lookY: -0.8, lid: 0.12, cursor: 1 },
      enter: k(18, { head: [[0, 0], [5, -3], [18, 0]], squash: [[0, 0], [4, 0.025], [14, 0]] }),
      loop: k(96, {
        cursorX: [[0, -1], [28, 1.5], [56, 1.5], [76, -1], [96, -1]],
        cursorY: [[0, 0], [28, -2], [56, 1], [76, 0], [96, 0]],
        head: [[0, 0], [28, 2], [56, -1], [96, 0]],
      }),
      beat: k(10, { cursorY: [[0, 0], [3, 1.5, 'o'], [10, 0]], head: [[0, 0], [3, -2], [10, 0]] }),
      exit: k(10, { cursorY: [[0, 0], [10, -4]] }),
      ambient: { breath: 0.6, bob: 0.3, blink: 0.8, dart: 0, puff: 0, period: 130 },
      poster: {},
    },
    connect: {
      base: { ...REST, head: 3, lookX: 0.8, lookY: -0.55, lid: 0.1, link: 1 },
      enter: k(16, { head: [[0, 0], [5, -2], [16, 0]] }),
      loop: k(80, { linkTilt: [[0, -4], [40, 4], [80, -4]], head: [[0, 0], [40, 1.5], [80, 0]] }),
      beat: k(10, { linkTilt: [[0, 0], [4, 10], [10, 0]] }),
      exit: k(10, {}),
      ambient: { breath: 0.6, bob: 0.3, blink: 0.8, dart: 0, puff: 0, period: 130 },
      poster: {},
    },
    sleep: {
      // Asleep: curled into the mark, breathing slow and deep.
      base: { ...REST, curl: 0.92, head: -8, lid: 1, smile: 0.28, fin: -12, fluke: 6, lookX: 0, lookY: 0 },
      enter: k(40, {
        mouth: [[0, 0], [8, 0.85, 'o'], [16, 0.85], [22, 0, 'io']],
        lid: [[0, 0], [6, 0], [24, 0]],
        fin: [[0, 0], [8, 44, 'o'], [18, 44], [26, 0]],
        squash: [[0, 0], [8, -0.07, 'o'], [18, -0.07], [26, 0]],
        curl: [[0, 0], [18, 0], [40, 0]],
      }),
      loop: k(1, {}),
      ambient: { breath: 1.7, bob: 0.5, blink: 0, dart: 0, puff: 0, sleepy: 1, period: 210 },
      poster: {},
    },
  };
  // Waking up plays before whatever comes after sleep.
  const WAKE = k(26, {
    lid: [[0, 0], [5, 0], [8, 0.45], [12, 0], [16, 0]],
    squash: [[0, 0], [10, -0.09, 'o'], [18, 0.03], [26, 0, 'o']],
    lookX: [[0, 0], [14, -0.6], [20, 0.6], [26, 0]],
  });
  // Error beat inside work: a short, honest hmm — then back to the job.
  const HMM_BEAT = k(12, { head: [[0, 0], [4, -4, 'o'], [8, -4], [12, 0]], tilt: [[0, 0], [5, -6], [8, -6], [12, 0]], mouthSide: [[0, 0], [5, 1], [8, 1], [12, 0]] });
  // Resting puff from the blowhole.
  const PUFF = k(34, { y: [[0, 0], [6, 0.7], [10, -0.6, 'o'], [34, 0, 'io']], squash: [[0, 0], [6, 0.03], [10, -0.02], [20, 0]] }, [[9, 'spout', { n: 3 }]]);
  const BLINK = k(7, { lid: [[0, 0], [2, 1, 'i'], [3, 1], [7, 0, 'o']] });

  // ---------------------------------------------------------------------------
  // Springs: per-parameter frequency (rad/s) and damping ratio.
  // ---------------------------------------------------------------------------
  const SPRING = {
    _: [9, 0.8], yaw: [8, 0.72], x: [7, 0.8], y: [7, 0.72], rot: [8, 0.65], scale: [10, 0.7], squash: [14, 0.45],
    curl: [6.5, 0.8], arch: [7, 0.8], head: [9, 0.7], tilt: [8, 0.65], fluke: [6, 0.45], flukeSpread: [8, 0.7],
    fin: [9, 0.55], finFar: [9, 0.55], lookX: [22, 0.9], lookY: [22, 0.9], lid: [26, 1], lidLow: [16, 0.9],
    eyeScale: [12, 0.5], brow: [12, 0.8], browTilt: [12, 0.8], mouth: [22, 1], smile: [10, 0.85], mouthSide: [10, 0.85],
    page: [12, 0.5], pad: [12, 0.55], pencil: [12, 0.5], lens: [12, 0.5], wrench: [12, 0.5], glass: [12, 0.55], pod: [6, 0.8],
    glassExt: [10, 0.6], cursor: [12, 0.55], link: [12, 0.55],
  };
  const PARAMS = Object.keys({ ...R.POSE, ...PROP_POSE });

  function mulberry32(a) { return () => { a |= 0; a = a + 0x6D2B79F5 | 0; let t = Math.imul(a ^ a >>> 15, 1 | a); t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t; return ((t ^ t >>> 14) >>> 0) / 4294967296; }; }

  // Presentation rates, per 30-fps frame. The final envelope bounds interruption
  // discontinuities as well as springs; it never queues a superseded performance.
  const RATE = { _: 0.16, x:2, y:2, rot:5, tilt:5, head:5, fin:12, finFar:12,
    fluke:10, arch:5, wrenchSpin:100, wrenchY:4, pageRot:6, pageDX:2, pageDY:2,
    scribX:4, scribY:2, padLines:.3, lid:.6, lidLow:.35, lookX:.5, lookY:.5,
    squash:.08, scale:.06, yaw:.11, curl:.10, mouth:.4, cursorX:2, cursorY:2, linkTilt:6 };
  const angleDelta = (a,b) => ((a-b+180)%360+360)%360-180;
  class Director {
    constructor(opts = {}) {
      this.rng=mulberry32(opts.seed || 7); this.f=0;
      this.presence='Idle'; this.activity=null; this.context={}; this.acting='rest';
      this.reduced=!!opts.reduced; this.ambientOn=opts.ambient!==false;
      this.direction='mark'; this.x=this.targetFor('rest');
      this.v=Object.fromEntries(PARAMS.map(p=>[p,0]));
      this.shots=[]; this.loops=[{clip:ACTS.rest.loop,at:0,owner:'rest'}];
      this.particles=[]; this.lines=1; this.events=[]; this.emissions=[];
      this.amb={blinkAt:40,dartAt:120,puffAt:360,look:{x:0,y:0}};
      this.poseOut={...this.x}; this.departure=null; this.changedAt=0;
      this.doneTurns=new Set(); this.spans=new Set(); this.errorSpan=null;
      this.calves=Array.from({length:3},()=>({value:0,velocity:0,target:0}));
    }
    targetFor(acting){return {...R.POSE,...PROP_POSE,...ACTS[acting].base,padLines:this.lines||1};}
    set(presence,activity=null,context={}){
      this.presence=presence; this.activity=activity; this.context={...context};
      const next=actingFor(presence,activity,context);
      const doneKey=presence==='Done' && context.status==='completed' && typeof context.turnId==='string' && context.turnId;
      const celebrate=!!doneKey && !this.doneTurns.has(doneKey);
      if(celebrate)this.doneTurns.add(doneKey);
      this.go(next,{celebrate,force:celebrate&&next==='done'});
      const n=next==='pod' && Number.isInteger(activity?.parallel) && activity.parallel>0 ? Math.min(3,activity.parallel) : 0;
      this.calves.forEach((c,i)=>{c.target=+(i<n);if(this.reduced){c.value=c.target;c.velocity=0;}});
      // Observe every onset, including those hidden by owner priority: never queue.
      const spans=activity?.observed===true&&context.freshness==='Live'&&Array.isArray(activity.active)?activity.active:[];
      for(const span of spans){if(!span||typeof span.kind!=='string'||!Number.isFinite(span.sinceMs))continue;
        const key=`${context.turnId||''}|${span.kind}|${span.sinceMs}`;
        if(!this.spans.has(key)){this.spans.add(key);this.event({type:'tool',kind:span.kind,sinceMs:span.sinceMs,source:'active'});}}
      if(this.spans.size>512)this.spans=new Set([...this.spans].slice(-256));
      this.poseOut=this.reduced?this.poster():this.poseOut;
    }
    go(next,{celebrate=false,force=false}={}){
      if(next===this.acting&&!force)return;
      const prev=this.acting,old=ACTS[prev],current=this.pose();
      // Freeze the visible offset, not the old clip's future movement or events.
      this.departure={at:this.f,offset:Object.fromEntries(PARAMS.map(p=>[p,current[p]-this.x[p]]))};
      this.shots=this.shots.filter(s=>s.wake&&this.f-s.at<WAKE.dur&&next!=='sleep');
      this.loops=[];this.particles=[];this.acting=next;this.changedAt=this.f;
      this.fromTarget={...this.x};
      this.delays={fin:3,finFar:4,fluke:4,curl:prev==='sleep'?2:3,
        lookX:next==='think'?6:0,lookY:next==='think'?6:0,lid:next==='sleep'?6:0,
        glassExt:next==='browse'?8:0};
      if(next==='write')this.lines=1;
      if(this.reduced){this.x=this.targetFor(next);this.v=Object.fromEntries(PARAMS.map(p=>[p,0]));this.shots=[];this.departure=null;this.poseOut=this.poster();return;}
      if(old.exit)this.shots.push({clip:old.exit,at:this.f,owner:prev,exit:true});
      if(prev==='sleep'&&next!=='sleep')this.shots.push({clip:WAKE,at:this.f,owner:next,wake:true});
      if(next!=='done'||celebrate)this.shots.push({clip:ACTS[next].enter,at:this.f,owner:next});
      this.loops=[{clip:ACTS[next].loop,at:this.f+Math.min(10,ACTS[next].enter.dur*.6),owner:next}];
    }
    setReduced(value){
      if(this.reduced===!!value)return;
      this.reduced=!!value;this.shots=[];this.loops=[];this.particles=[];this.departure=null;
      this.acting=actingFor(this.presence,this.activity,this.context);
      this.x=this.targetFor(this.acting);this.v=Object.fromEntries(PARAMS.map(p=>[p,0]));
      this.calves.forEach(c=>{c.value=c.target;c.velocity=0;});this.poseOut=this.poster();
      if(!this.reduced)this.loops=[{clip:ACTS[this.acting].loop,at:this.f,owner:this.acting}];
    }
    event(e){
      this.events.push({...e,f:this.f});if(this.events.length>40)this.events.shift();
      if(this.presence!=='Working'||this.context.freshness!=='Live'||this.activity?.observed!==true)return false;
      if(KIND_TO_ACT[e.kind]!==this.acting)return false;
      const act=ACTS[this.acting];let clip;
      if(e.type==='tool')clip=act.beat;
      if(e.type==='failed'&&['failed','error'].includes(e.status)&&!['rest','sleep','needs','done','hmm','listen','think'].includes(this.acting))clip=HMM_BEAT;
      if(!clip)return false;
      if(e.type==='tool'&&this.acting==='write')this.lines=Math.min(4,this.lines+1);
      if(this.reduced){this.poseOut=this.poster();return true;}
      this.shots=this.shots.filter(s=>s.clip!==clip);
      this.shots.push({clip,at:this.f,owner:this.acting,beat:true});return true;
    }
    step(dt){
      if(this.reduced){this.poseOut=this.poster();return;}
      dt=clamp(dt,0,1/10);const df=dt*FPS,f0=this.f;this.f+=df;
      const target=this.targetFor(this.acting),sub=Math.max(1,Math.ceil(dt*120)),h=dt/sub;
      for(let s=0;s<sub;s++){
        for(const p of PARAMS){const [w,z]=SPRING[p]||SPRING._;
          const t=this.f-this.changedAt<(this.delays?.[p]||0)?this.fromTarget[p]:target[p];
          this.v[p]+=(w*w*(t-this.x[p])-2*z*w*this.v[p])*h;this.x[p]+=this.v[p]*h;}
        for(const c of this.calves){c.velocity+=(144*(c.target-c.value)-24*c.velocity)*h;c.value=clamp(c.value+c.velocity*h,0,1);}
      }
      const out={};
      if(this.departure){const w=1-clamp((this.f-this.departure.at)/8,0,1);for(const p of PARAMS)out[p]=this.departure.offset[p]*w;if(!w)this.departure=null;}
      this.anchors=R.build(R.DIRECTIONS[this.direction],this.poseOut,0).anchors;
      this.shots=this.shots.filter(s=>this.f-s.at<=s.clip.dur);
      for(const s of this.shots){if(this.f<s.at)continue;evalClip(s.clip,this.f-s.at,out);
        if(!s.exit&&(s.wake||s.owner===this.acting))this.fire(s.clip,f0-s.at,this.f-s.at,s.owner||this.acting);}
      for(const l of this.loops){if(this.f<l.at)continue;const lf=(this.f-l.at)%l.clip.dur,w=clamp((this.f-l.at)/8,0,1);
        evalClip(l.clip,lf,out,w);const pf=(f0-l.at)%l.clip.dur;this.fire(l.clip,pf<=lf?pf:-1,lf,l.owner);}
      this.ambient(out,f0);
      out.fluke=(out.fluke||0)+clamp(-this.v.y*2.2-this.v.rot*.6,-22,22);
      out.fin=(out.fin||0)+clamp(-this.v.y*1.2,-12,12);
      const pose={};
      for(const p of PARAMS){const desired=this.x[p]+(out[p]||0),prior=this.poseOut?.[p]??desired;
        const delta=p==='wrenchSpin'?angleDelta(desired,prior):desired-prior;
        pose[p]=prior+clamp(delta,-(RATE[p]||RATE._)*df,(RATE[p]||RATE._)*df);}
      for(const p of ['lid','lidLow','mouth','yaw','pageFlip'])pose[p]=clamp(pose[p],0,1);
      for(const p of ['page','pad','pencil','lens','wrench','glass','pod','cursor','link'])pose[p]=clamp(pose[p],0,1);
      this.poseOut=pose;
      for(const p of this.particles){p.age+=df;p.vy+=(p.g||0)*df;p.vx*=Math.pow(p.drag||.98,df);p.vy*=Math.pow(p.drag||.98,df);p.x+=p.vx*df;p.y+=p.vy*df;}
      this.particles=this.particles.filter(p=>p.age<p.life&&p.owner===this.acting);
    }
    fire(clip,a,b,owner=this.acting){if(owner!==this.acting)return;for(const [fr,kind,opt] of clip.events||[])if(fr>a&&fr<=b)this.emit(kind,opt||{});}

    ambient(out, f0) {
      if (!this.ambientOn) return;
      const A = ACTS[this.acting].ambient, f = this.f;
      // Breath: a slow inhale, a short hold, a longer exhale.
      const ph = (f % A.period) / A.period;
      const b = ph < 0.4 ? Math.sin(ph / 0.4 * Math.PI / 2) : ph < 0.5 ? 1 : Math.cos((ph - 0.5) / 0.5 * Math.PI / 2);
      const breath = A.breath;
      out.squash = (out.squash || 0) - 0.016 * b * breath;
      out.y = (out.y || 0) - 0.55 * b * breath + Math.sin(f / 210 * TAU) * 1.1 * A.bob;
      out.fluke = (out.fluke || 0) + 3.5 * Math.sin((f - 10) / A.period * TAU) * breath;
      out.fin = (out.fin || 0) + 2.2 * Math.sin((f - 6) / A.period * TAU) * breath;
      if(this.acting==='rest'){
        out.rot=(out.rot||0)+2.3*Math.sin(f/240*TAU);
        out.x=(out.x||0)+1.8*Math.sin(f/330*TAU);
        out.y+=Math.sin((f+33)/210*TAU);
        out.fluke+=8*Math.sin((f-24)/180*TAU);
        out.head=(out.head||0)+1.5*Math.sin((f-12)/250*TAU);
      }
      // Blinks, eye darts, blowhole puffs — at irregular, natural intervals.
      const r = this.rng;
      if (A.blink > 0 && f >= this.amb.blinkAt) {
        this.shots.push({ clip: BLINK, at: f, owner: this.acting, ambient:true });
        if (r() < 0.2) this.shots.push({ clip: BLINK, at: f + 9, owner: this.acting, ambient:true });
        this.amb.blinkAt = f + (90 + r() * 150) / A.blink;
      }
      if (A.dart > 0) {
        if (f >= this.amb.dartAt) { this.amb.look = { x: (r() - 0.5) * 0.9, y: (r() - 0.5) * 0.5 }; this.amb.dartAt = f + 150 + r() * 240; this.amb.lookAt = f; }
        const t = clamp((f - (this.amb.lookAt || 0)) / 4, 0, 1);
        this.amb.cur = this.amb.cur || { x: 0, y: 0 };
        this.amb.cur.x = lerp(this.amb.cur.x, this.amb.look.x, t); this.amb.cur.y = lerp(this.amb.cur.y, this.amb.look.y, t);
        out.lookX = (out.lookX || 0) + this.amb.cur.x; out.lookY = (out.lookY || 0) + this.amb.cur.y;
      }
      if (A.puff > 0 && f >= this.amb.puffAt) { this.shots.push({ clip: PUFF, at: f, owner: this.acting, ambient:true }); this.amb.puffAt = f + 600 + r() * 540; }
      if (A.sleepy && f >= (this.amb.sleepAt || 0)) { this.emit('sleep'); this.amb.sleepAt = f + 170 + r() * 140; }
    }

    // Particles are born at rig anchors, in world design units.
    emit(kind, opt = {}) {
      const start = this.particles.length;
      const a = this.anchors;
      if (!a) return;
      this.emissions.push({kind,owner:this.acting,f:this.f});
      if(this.emissions.length>100)this.emissions.shift();
      const r = this.rng, seed = r();
      if (kind === 'spout') {
        const n = opt.n || 4, big = !!opt.big;
        const source=big?a.spout:a.blowhole;
        for (let i = 0; i < n; i++) {
          const ang = -Math.PI / 2 + (r() - 0.5) * (big ? 1.3 : 0.8);
          const sp = (big ? 1.55 : 0.9) * (0.75 + r() * 0.5);
          this.particles.push({ kind: 'drop', x: source.x+(big?(r()-.5)*26:0), y: source.y-(big?24+r()*3:1), vx: Math.cos(ang)*(big?.3:sp), vy: big?-.35:Math.sin(ang)*sp, g: big?.055:.075, drag: 0.985, r: big ? 1.6 : 1.3, age: 0, life: big ? 35 : 26, seed });
        }
        for (let i = 0; i < (big ? 3 : 2); i++) this.particles.push({ kind: 'mist', x: a.blowhole.x + (r() - 0.5) * 3, y: a.blowhole.y - 5 - r() * 4, vx: (r() - 0.5) * 0.1, vy: -0.12, r: big ? 4.5 : 3.2, age: 0, life: 24, drag: 0.95, seed });
      } else if (kind === 'thought') {
        // Three rising bubbles; the last grows into a cloud, drifts, then pops.
        const bx = a.blowhole.x + 2, by = a.blowhole.y - 3;
        [[0, 1.3, 0], [5, 2.1, 0], [10, 5.2, 1]].forEach(([delay, rad, cloud], i) => this.particles.push({ kind: 'thought', x: bx + [0,6,14][i], y: by - i * 5.5, vx: 0.06, vy: -0.05, r: rad, cloud: !!cloud, age: -delay, life: 64 - delay, popAt: 54 - delay, drag: 0.99, seed, wob: cloud ? 0 : 0.02 }));
      } else if (kind === 'speech') {
        const m = a.mouth;
        for (let i = 0; i < 3; i++) this.particles.push({ kind: 'bubble', x: m.x + 4 + r() * 2, y: m.y - 1, vx: 0.26 + r() * 0.15, vy: -0.3 - r() * 0.2, r: 1.7 + r() * 1.6, age: -i * 6, life: 50, popAt: 42, drag: 0.985, wob: 0.03, seed: r() });
      } else if (kind === 'blub') {
        this.particles.push({ kind: 'bubble', x: a.blowhole.x, y: a.blowhole.y - 2, vx: 0.02, vy: -0.16, r: 2.2, age: 0, life: 64, popAt: 56, drag: 0.995, wob: 0.025, seed });
      } else if (kind === 'sleep') {
        this.particles.push({ kind: 'bubble', x: a.blowhole.x, y: a.blowhole.y - 1.5, vx: 0.03, vy: -0.1, r: 1.6, age: 0, life: 90, popAt: 82, drag: 0.997, wob: 0.02, seed, alpha: 0.85 });
      }
      for (const p of this.particles.slice(start)) p.owner=this.acting;
    }

    // Reduced motion: the state's poster pose — no clock, no ambient, no transitions.
    poster() {
      const act = ACTS[this.acting];
      const pose = { ...this.targetFor(this.acting), ...(act.poster || {}) };
      delete pose.particles;
      pose.padLines = this.lines;
      return pose;
    }
    posterParticles(anchors) {
      const list = (ACTS[this.acting].poster || {}).particles || [];
      const out = [];
      for (const [kind] of list) {
        if (kind === 'thought-cloud') {
          const bx = anchors.blowhole.x + 2, by = anchors.blowhole.y - 3;
          out.push({ kind: 'thought', x: bx, y: by, r: 1.3, age: 10, life: 100 }, { kind: 'thought', x: bx + 6, y: by - 5.5, r: 2.1, age: 10, life: 100 }, { kind: 'thought', x: bx + 14, y: by - 13, r: 5.2, cloud: true, age: 10, life: 100 });
        } else if (kind === 'speech') {
          out.push({ kind: 'bubble', x: anchors.mouth.x + 6, y: anchors.mouth.y - 5, r: 1.9, age: 10, life: 100 }, { kind: 'bubble', x: anchors.mouth.x + 9, y: anchors.mouth.y - 10, r: 1.3, age: 10, life: 100 });
        } else if (kind === 'blub') {
          out.push({ kind: 'bubble', x: anchors.blowhole.x + 1, y: anchors.blowhole.y - 8, r: 2.2, age: 10, life: 100 });
        } else if (kind === 'spout-still') {
          const b = anchors.blowhole;
          [[-3, -7, -0.6, -0.5], [0, -9, 0, -1], [3, -7, 0.6, -0.5], [-5, -4, -0.8, 0.3], [5, -4, 0.8, 0.3]].forEach(([dx, dy, vx, vy]) => out.push({ kind: 'drop', x: b.x + dx, y: b.y + dy, vx, vy, r: 1.4, age: 5, life: 100 }));
        }
      }
      return out;
    }

    pose() { return this.poseOut || this.poster(); }
  }

  // ---------------------------------------------------------------------------
  // Render one view of the director: whale, props, particles.
  // ---------------------------------------------------------------------------
  function scene(director,view={}){
    const dir=R.DIRECTIONS[view.dir||'mark']||R.DIRECTIONS.mark,pose={...director.pose(),...view.pose};
    const podVisibility=Math.max(...director.calves.map(c=>c.value));pose.scale*=1-.16*podVisibility;pose.x+=10*podVisibility;
    const parts=R.build(dir,pose,view.lod||0,{size:view.size||512,dpr:view.dpr||1});
    const calves=[];
    for(let i=0;i<3;i++){
      const vis=director.calves[i].value;if(vis<.002)continue;
      const phase=director.reduced?0:director.f/30*(.8+i*.13)+i*2.1;
      const cp=R.build(dir,{x:-44-(1-vis)*10,y:-30+i*30,scale:.18*vis,
        fluke:6+Math.sin(phase)*8,lid:0},view.lod>=2?2:0,{size:view.size||512,dpr:view.dpr||1});
      calves.push(...cp.shapes.map(s=>({...s,id:`calf-${i+1}-${s.id}`,opacity:s.opacity*vis})));
    }
    parts.shapes=[...calves,...parts.shapes,...PR.shapes(pose,parts,view)];
    const particles=director.reduced?director.posterParticles(parts.anchors):director.particles;
    parts.shapes.push(...PR.particles(particles,view));
    return parts;
  }
  function render(ctx,director,view){const parts=scene(director,view);R.draw(ctx,parts,R.resolveLook(view.theme,view.px,view.dpr,view.lod));return parts;}
  root.WhaleActing = { ACTS, KIND_TO_ACT, actingFor, Director, render, scene, evalClip, track, FPS, PROP_POSE, PARAMS, RATE, SPRING, WAKE, angleDelta };
})(typeof window !== 'undefined' ? window : globalThis);
