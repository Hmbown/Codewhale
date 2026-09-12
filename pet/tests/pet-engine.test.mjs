import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { PetEngineTelemetry } from '../dist/core/pet-engine.js';
import { PetNative } from '../dist/core/pet-native.js';
import { compilePetTelemetry, encodePetJSONL } from '../dist/core/pet-telemetry.js';
import { petDemoEvents } from '../dist/core/pet-demo.js';

const points = JSON.stringify(readFileSync(new URL('../public/whale-points.tsv', import.meta.url), 'utf8').trim().split('\n').map(row => row.split(/\s+/).map(Number)));
test('the incremental bucket range uses the same measured projection as full replay', () => {
  const events = petDemoEvents(), full = compilePetTelemetry(events, 80_000);
  for (let i = 0; i < full.length; i++) assert.deepEqual(compilePetTelemetry(events, 80_000, i)[0], full[i]);
});
test('Engine pulses expire; a late failed completion tears at receipt time without rewriting history', () => {
  const engine = new PetEngineTelemetry();
  engine.observe({ event: 'tool_call_started', tool_call_id: 'a', tool_name: 'exec_command' }, 0);
  engine.observe({ event: 'tool_call_heartbeat' }, 300);
  const first = engine.bucket(0);
  assert.equal(first.channel, 'code'); assert.equal(first.activeMs[3], 300);
  assert.equal(engine.bucket(3).observed, 0);
  engine.observe({ event: 'tool_call_complete', tool_call_id: 'a', tool_name: 'exec_command', failed: true }, 5900);
  assert.equal(engine.bucket(13).observed, 0);
  assert.equal(engine.bucket(14).channel, 'error'); assert.equal(engine.bucket(14).errors, 1);
  assert.deepEqual(engine.bucket(0), first);
});
test('an authoritative waiting request escalates, accepted tape replays, and silence stays unknown', () => {
  const pet = new PetNative(points, '', '[]', true);
  pet.observeEngine(JSON.stringify({ event: 'approval_required', id: 'permission' }), 100);
  for (let ms = 400; ms <= 30_000; ms += 400) pet.advanceEngine(ms, true, true);
  assert.equal(JSON.parse(pet.snapshot()).needs, 'call');
  assert.equal(JSON.parse(pet.snapshot()).state.channel, 'human');
  const saved = JSON.parse(pet.recording()), replay = new PetNative(points, encodePetJSONL(saved.tape), JSON.stringify(saved.interactions));
  for (let i = 0; i < 900; i++) replay.step(1 / 30, true);
  assert.deepEqual(JSON.parse(replay.snapshot()).state, JSON.parse(pet.snapshot()).state);
  assert.equal(JSON.parse(replay.snapshot()).digest, JSON.parse(pet.snapshot()).digest);
  pet.observeEngine(JSON.stringify({ event: 'turn_complete' }), 30_100);
  pet.advanceEngine(32_000, true, false);
  assert.equal(JSON.parse(pet.snapshot()).state.observed, 0);
});
test('Engine worker boundary rejects payloads, invalid clocks and oversized active sets', () => {
  const e = new PetEngineTelemetry();
  assert.throws(() => e.observe({ event: 'response_delta', index: 0, delta: 'private' }, 0));
  assert.throws(() => e.observe({ event: 'response_delta', index: 0, channel: 'fabricated' }, 0));
  e.observe({ event: 'thinking_started', index: 0 }, 100);
  assert.throws(() => e.observe({ event: 'thinking_complete', index: 0 }, 90));
  for (let i = 1; i < 256; i++) e.observe({ event: 'thinking_started', index: i }, 100);
  assert.throws(() => e.observe({ event: 'thinking_started', index: 257 }, 100));
});
