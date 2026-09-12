import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { PetWorld } from '../dist/core/pet-world.js';
import { PetNative } from '../dist/core/pet-native.js';
import { compilePetTelemetry, encodePetJSONL } from '../dist/core/pet-telemetry.js';
import { petDemoEvents } from '../dist/core/pet-demo.js';
import { stableHash } from '../dist/core/model.js';

const points = readFileSync(new URL('../public/whale-points.tsv', import.meta.url), 'utf8').trim().split('\n').map(row => row.split(/\s+/).map(Number));
const tape = compilePetTelemetry(petDemoEvents(), 80_000);
const motion = tick => tick % 107 < 71;
const equal = (a, b) => {
  assert.deepEqual(a.frame, b.frame);
  assert.deepEqual(a.sim.checkpoint(), b.sim.checkpoint());
  assert.deepEqual(a.voices, b.voices);
};

test('chunked exports preserve the complete versioned recording and exact checkpoint beyond the autosave bound', () => {
  for (const count of [0, 1, 16, 17, 32_000]) {
    const tape = count ? compilePetTelemetry([], count * 400) : [];
    const world = new PetWorld(points, tape);
    // Cross both kinds of chunk boundary, including pending interactions.
    for (let i = 0; i < 17; i++) world.interact('food', i / 20, -.2);
    world.step(.1); world.interact('attention', -.2, .3);
    const chunks = [];
    for (let i = 0; ; i++) {
      const chunk = world.recordingChunk(i); if (chunk === null) break;
      chunks.push(chunk);
    }
    const text = chunks.join(''), recording = JSON.parse(text);
    assert.equal(text, JSON.stringify({ petReplayVersion: 1, expressionVersion: 2, tape: world.tape,
      interactions: world.interactions, checkpoint: world.checkpoint() }));
    const restored = PetWorld.restore(points, recording.tape, recording.interactions, recording.checkpoint);
    equal(world, restored); world.step(.1); restored.step(.1); equal(world, restored);
    if (count === 32_000) {
      assert.ok(Buffer.byteLength(text) > 8 * 1024 * 1024);
      assert.ok(Math.max(...chunks.map(c => Buffer.byteLength(c))) < 512 * 1024);
    }
    assert.throws(() => world.recordingChunk(-1));
    assert.throws(() => world.recordingChunk(.5));
  }
  const native = new PetNative(JSON.stringify(points), encodePetJSONL(tape));
  const chunks = [];
  for (let i = 0; ; i++) { const part = native.recordingChunk(i); if (part === null) break; chunks.push(part); }
  assert.equal(chunks.join(''), native.recording(true));
});

test('JSON checkpoints continue exact particles, seeded behaviour, pod identities, interactions and score across motion changes', () => {
  for (const input of [[], tape]) {
    const world = new PetWorld(points, input);
    for (let tick = 0; tick < 800; tick++) {
      if (tick === 210) { world.interact('food', .3, -.2); world.interact('attention', -.2, .1); }
      world.step(1 / 30, { motion: motion(tick), sensitivity: 1 });
    }
    // Preserve pending same-tick interactions and a fractional display remainder.
    world.interact('food', -.1, .3); world.interact('attention', .4, .2);
    world.step(.013, { motion: true, sensitivity: 1 });
    const checkpoint = JSON.parse(JSON.stringify(world.checkpoint()));
    const restored = PetWorld.restore(points, world.tape, world.interactions, checkpoint);
    equal(restored, world);
    for (let tick = 800; tick < 1160; tick++) {
      if (tick === 813) { restored.interact('attention', .2, .1); world.interact('attention', .2, .1); }
      const opts = { motion: motion(tick), sensitivity: 1 };
      world.step(1 / 30, opts); restored.step(1 / 30, opts); equal(restored, world);
    }
    checkpoint.sim.particles[0][0] = 7;
    assert.notEqual(restored.sim.p[0].x, 7, 'restoration owns its state instead of aliasing the imported checkpoint');
  }
});

test('checkpoint restore rejects a different recording, authored body, malformed physics and invalid clocks without altering the source', () => {
  const world = new PetWorld(points, tape); world.step(8);
  const checkpoint = world.checkpoint(), before = JSON.stringify(checkpoint);
  for (const edit of [c => c.petCheckpointVersion = 2, c => c.tick = -1, c => c.random = 2 ** 32,
    c => c.frame.timeMs++, c => c.sim.body[0][0] = .99, c => c.sim.particles[0][0] = Infinity,
    c => c.sim.particles[0] = [], c => c.score[0] = -.5, c => c.members = [{ id: 'x', slot: 8, phase: 1, present: true }],
    c => c.voices = [{ id: 'bad', start: 0, duration: 1, frequency: 9e9, gain: .1, pan: 0, kind: 'tone' }]]) {
    const copy = structuredClone(checkpoint); edit(copy);
    assert.throws(() => PetWorld.restore(points, tape, [], copy));
  }
  const other = structuredClone(tape); other[0].activity = .99;
  assert.throws(() => PetWorld.restore(points, other, [], checkpoint), /recording/);
  assert.equal(JSON.stringify(world.checkpoint()), before);
});

test('the native JSON boundary restores exact state and leaves its old world intact after a rejected checkpoint', () => {
  const a = new PetNative(JSON.stringify(points), encodePetJSONL(tape));
  for (let i = 0; i < 970; i++) a.step(1 / 30, motion(i));
  const b = new PetNative(JSON.stringify(points), encodePetJSONL(tape));
  b.restoreCheckpoint(a.checkpoint()); assert.equal(b.snapshot(), a.snapshot());
  for (let i = 970; i < 1050; i++) assert.equal(b.step(1 / 30, motion(i)), a.step(1 / 30, motion(i)));
  const before = b.snapshot();
  assert.throws(() => b.restoreCheckpoint('{"petCheckpointVersion":99}'));
  assert.equal(b.snapshot(), before);
});

test('doze and wake survive a checkpoint without consuming another random choice or retriggering the old score', () => {
  const world = new PetWorld(points);
  for (let i = 0; i < 9; i++) world.step(10, { motion: false, sensitivity: 1 });
  assert.equal(world.frame.behaviour, 'doze');
  const copy = PetWorld.restore(points, world.tape, world.interactions, JSON.parse(JSON.stringify(world.checkpoint())));
  world.interact('attention', .2, .1); copy.interact('attention', .2, .1);
  for (let i = 0; i < 100; i++) {
    world.step(1 / 30); copy.step(1 / 30); equal(copy, world);
    if (i === 0) assert.equal(copy.frame.behaviour, 'wake');
  }
});

test('a native live habitat keeps accepted history but resumes with an explicit gap and no stale human request', () => {
  const live = new PetNative(JSON.stringify(points), '', '[]', true);
  live.observeEngine(JSON.stringify({event: 'approval_required', id: 'request-a'}), 0);
  live.advanceEngine(3200, true, true);
  const before = JSON.parse(live.snapshot());
  assert.equal(before.state.channel, 'human');
  const saved = live.recording(true), recording = JSON.parse(saved);
  const restored = new PetNative(JSON.stringify(points), '', '[]', true);
  restored.restoreRecording(saved);
  assert.equal(restored.snapshot(), live.snapshot());
  const offset = restored.resumeEngine();
  assert.ok(offset >= before.timeMs && offset <= before.timeMs + 800);
  assert.equal(JSON.parse(restored.snapshot()).state.observed, 0);
  assert.deepEqual(JSON.parse(restored.recording()).tape, recording.tape);
  // Shell waiting alone cannot resurrect a historical approval request.
  restored.advanceEngine(offset + 1200, false, true);
  assert.equal(JSON.parse(restored.snapshot()).state.observed, 0);
  assert.equal(JSON.parse(restored.snapshot()).needs, 'none');
  restored.observeEngine(JSON.stringify({event: 'approval_required', id: 'request-b'}), offset + 1300);
  restored.advanceEngine(offset + 2000, false, true);
  assert.equal(JSON.parse(restored.snapshot()).state.channel, 'human');
  const current = restored.snapshot();
  assert.throws(() => restored.restoreRecording('{"petReplayVersion":2}'));
  assert.equal(restored.snapshot(), current);
});

test('the initial native live checkpoint and a source that has already expired can both resume', () => {
  for (const at of [0, 9200]) {
    const a = new PetNative(JSON.stringify(points), '', '[]', true);
    a.advanceEngine(at, false, false);
    const b = new PetNative(JSON.stringify(points), '', '[]', true);
    b.restoreRecording(a.recording(true));
    assert.equal(b.snapshot(), a.snapshot());
    const offset = b.resumeEngine();
    b.advanceEngine(offset + 400, false, false);
    assert.equal(JSON.parse(b.snapshot()).state.observed, 0);
  }
});

test('incremental history checksums retain the version-one wire value across appended gaps, replacements and Unicode identities', () => {
  const packet = structuredClone(tape[0]); packet.agentIds = ['alpha', '鲸', '🌊'];
  for (const input of [compilePetTelemetry([]), tape]) {
    const world = new PetWorld(points, input);
    const check = () => assert.equal(world.checkpoint().history, stableHash(JSON.stringify([world.tape, world.interactions])));
    check(); world.step(2); world.acceptTelemetry(packet); check();
    world.interact('food', .2, -.1); world.interact('attention', -.3, .2); check();
    world.acceptTelemetry({ ...packet, attention: .7 }); check();
    world.step(1); world.acceptTelemetry(packet); check();
    world.interact('food', .1, -.2); check();
    const restored = PetWorld.restore(points, world.tape, world.interactions, world.checkpoint());
    assert.equal(restored.checkpoint().history, world.checkpoint().history);
  }
});
