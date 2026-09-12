import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { PetNative } from '../dist/core/pet-native.js';
import { PetWorld } from '../dist/core/pet-world.js';
import { PetSim, digest } from '../dist/core/pet-sim.js';
import { compilePetTelemetry, encodePetJSONL } from '../dist/core/pet-telemetry.js';
import { petDemoEvents } from '../dist/core/pet-demo.js';

const points = readFileSync(new URL('../public/whale-points.tsv', import.meta.url), 'utf8').trim().split('\n').map(row => row.split(/\s+/).map(Number));
const tape = compilePetTelemetry(petDemoEvents(), 80_000);

test('native hosts replay checkpoint-free exports from the beginning in their recorded expression version', () => {
  for (const version of [1, 2]) {
    const source = new PetNative(JSON.stringify(points), encodePetJSONL(tape), '[]', false, version);
    for (let i = 0; i < 540; i++) source.step(1 / 30, true);
    const recording = JSON.parse(source.recording());
    if (version === 1) delete recording.expressionVersion;
    const restored = new PetNative(JSON.stringify(points));
    restored.restoreRecording(JSON.stringify(recording));
    assert.equal(JSON.parse(restored.snapshot()).timeMs, 0);
    assert.equal(JSON.parse(restored.recording()).expressionVersion, version);
    const expected = new PetNative(JSON.stringify(points), encodePetJSONL(tape), '[]', false, version);
    for (let i = 0; i < 720; i++) assert.equal(restored.step(1 / 30, true), expected.step(1 / 30, true));
    const before = restored.recording(true);
    recording.checkpoint = null;
    assert.throws(() => restored.restoreRecording(JSON.stringify(recording)));
    assert.equal(restored.recording(true), before);
  }
});

test('recordings without an expression version retain v1 after checkpoint hydration and subsequent work', () => {
  const legacy = new PetNative(JSON.stringify(points), encodePetJSONL(tape), '[]', false, 1);
  for (let i = 0; i < 540; i++) legacy.step(1 / 30, true);
  const saved = JSON.parse(legacy.recording(true));
  delete saved.expressionVersion; delete saved.checkpoint.sim.expressionVersion;
  const restored = new PetNative(JSON.stringify(points));
  restored.restoreRecording(JSON.stringify(saved));
  assert.equal(JSON.parse(restored.recording()).expressionVersion, 1);
  for (let i = 0; i < 240; i++) assert.equal(restored.step(1 / 30, i % 40 < 30), legacy.step(1 / 30, i % 40 < 30));
});

test('unknown or mismatched expression versions are rejected before replacing the current habitat', () => {
  const pet = new PetNative(JSON.stringify(points)); pet.step(1, true);
  const before = pet.recording(true), saved = JSON.parse(before);
  for (const edit of [r => r.expressionVersion = 3, r => r.expressionVersion = null,
    r => r.checkpoint.sim.expressionVersion = 99, r => r.checkpoint.sim.expressionVersion = 1]) {
    const candidate = structuredClone(saved); edit(candidate);
    assert.throws(() => pet.restoreRecording(JSON.stringify(candidate)), /version/);
    assert.equal(pet.recording(true), before);
  }
});

test('field expressions preserve all particle identities and the unchanged home form', () => {
  const old = new PetSim(points, 0xC0FFEE, 1), field = new PetSim(points);
  const state = { activity: .1, coherence: .9, attention: 0, channel: 'reasoning', observed: 1, roamX: 0, roamY: 0, flip: 1, lit: 1 };
  for (let i = 0; i < 60; i++) { old.step(1 / 30, state, { motion: true, sensitivity: 1 }); field.step(1 / 30, state, { motion: true, sensitivity: 1 }); }
  assert.equal(digest(old), digest(field));
  state.activity = .8; state.channel = 'code';
  for (let i = 0; i < 120; i++) { old.step(1 / 30, state, { motion: true, sensitivity: 1 }); field.step(1 / 30, state, { motion: true, sensitivity: 1 }); }
  assert.notEqual(digest(old), digest(field));
  assert.deepEqual(field.checkpoint().body, old.checkpoint().body);
  assert.equal(field.p.length, 980);
});

test('new worlds show a human junction without the legacy approach-to-owner steering', () => {
  const tape = compilePetTelemetry([{ schemaVersion: 1, id: 'request', traceId: 't', startTime: 0, endTime: 40_000,
    agentId: 'parent', category: 'human', name: 'request', status: 'pending', attributes: {} }], 40_000);
  const legacy = new PetWorld(points, tape, [], 1), modern = new PetWorld(points, tape);
  for (let i = 0; i < 900; i++) { legacy.step(1 / 30); modern.step(1 / 30); }
  assert.equal(modern.frame.needs, 'call'); // Recorded request remains visible.
  assert.equal(legacy.checkpoint().targetX, 0);
  assert.notEqual(modern.checkpoint().targetX, 0);
  assert.deepEqual(modern.tape, legacy.tape);
});
