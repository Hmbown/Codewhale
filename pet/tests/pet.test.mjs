import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { PetSim, REST_STATE, digest, CHANNELS } from '../dist/core/pet-sim.js';
import { compilePetTelemetry, encodePetJSONL, decodePetJSONL } from '../dist/core/pet-telemetry.js';
import { PetWorld } from '../dist/core/pet-world.js';
import { renderPetPCM } from '../dist/core/pet-audio.js';
import { petDemoEvents } from '../dist/core/pet-demo.js';
import { importTrace } from '../dist/core/ingest.js';

const points = readFileSync(new URL('../public/whale-points.tsv', import.meta.url), 'utf8').trim().split('\n').map(row => row.split(/\s+/).map(Number));
const event = (overrides = {}) => ({ schemaVersion: 1, id: 'e', traceId: 't', startTime: 0, endTime: 400,
  agentId: 'parent', category: 'tool', name: 'work', status: 'success', attributes: {}, ...overrides });

test('span time beats onset count; long tools remain measured; containers are excluded', () => {
  const e = [event({ id: 'root', category: 'orchestration', endTime: 4000 }),
    event({ id: 'long', parentId: 'root', category: 'reasoning', endTime: 4000 }),
    ...Array.from({ length: 8 }, (_, i) => event({ id: String(i), startTime: i * 10, endTime: i * 10 }))];
  const b = compilePetTelemetry(e);
  assert.equal(b[0].channel, 'reasoning'); assert.equal(b[9].channel, 'reasoning');
  assert.equal(b[0].activeMs[9], 0); assert.equal(b[0].activeMs[0], 400);
});
test('half-open bins retain endpoint onsets and distinguish unknown duration from idle', () => {
  const b = compilePetTelemetry([event({ endTime: 800 }), event({ id: 'last', startTime: 800, endTime: 800, category: 'human' })], 1600);
  assert.equal(b[2].channel, 'human'); assert.equal(b[2].onsets[11], 1); assert.equal(b[3].observed, 0); assert.equal(b[3].channel, 'other');
  const open = compilePetTelemetry([event({ openEnded: true, endTime: 3000 })], 1200);
  assert.equal(open[0].onsets[1], 1); assert.equal(open[0].activeMs[1], 0); assert.equal(open[1].observed, 0);
});
test('repeat density is causal; independent future events do not rewrite past buckets', () => {
  const first = [event({ endTime: 80 })], future = Array.from({ length: 7 }, (_, i) => event({ id: `f${i}`, startTime: 400 + i * 400, endTime: 480 + i * 400 }));
  const a = compilePetTelemetry(first), b = compilePetTelemetry([...first, ...future]);
  assert.deepEqual(a[0], b[0]); assert.ok(b[4].coherence < b[0].coherence);
});
test('errors, human spans and concurrent peers select semantic gaits', () => {
  assert.equal(compilePetTelemetry([event({ category: 'reasoning', status: 'error' })])[0].channel, 'error');
  const wait = compilePetTelemetry([event({ category: 'human', status: 'pending' })])[0];
  assert.equal(wait.channel, 'human'); assert.equal(wait.waiting, true);
  const peers = compilePetTelemetry(['a', 'b', 'c'].map(id => event({ id, agentId: id })))[0];
  assert.equal(peers.channel, 'agent'); assert.deepEqual(peers.agentIds, ['a', 'b', 'c']);
});
test('updates deduplicate, multiple traces reject, wire rejects corrupt values', () => {
  assert.equal(compilePetTelemetry([event(), event({ endTime: 800 })])[0].onsets[1], 1);
  assert.throws(() => compilePetTelemetry([event(), event({ traceId: 'another' })]), /one trace/);
  const b = compilePetTelemetry([event()]); assert.deepEqual(decodePetJSONL(encodePetJSONL(b)), b);
  for (const edit of [{ activity: null }, { observed: 2 }, { sequence: 8 }, { channel: 'fake' }, { onsets: [1] }])
    assert.throws(() => decodePetJSONL(JSON.stringify({ ...b[0], ...edit })));
  assert.throws(() => compilePetTelemetry([event({ endTime: Infinity })]));
});
test('reduced motion has no settling, jitter, colour fade or tear decay under constant input', () => {
  for (const ch of CHANNELS) {
    const sim = new PetSim(points), state = { ...REST_STATE, channel: ch.key, activity: .8, coherence: .4 };
    sim.step(1 / 30, state, { motion: false, sensitivity: 1 }); const first = digest(sim);
    for (let i = 0; i < 90; i++) sim.step(1 / 30, state, { motion: false, sensitivity: 1 });
    assert.equal(digest(sim), first, ch.key);
  }
});
test('world and score are invariant to display cadence and repeat replay exactly', () => {
  const tape = compilePetTelemetry(petDemoEvents(), 80_000);
  const run = dt => {
    const world = new PetWorld(points, tape), voices = [...world.voices];
    for (let i = 0; i < Math.round(36 / dt); i++) { world.step(dt); voices.push(...world.voices); }
    return { frame: world.frame, digest: digest(world.sim), voices };
  };
  const a = run(1 / 30); assert.deepEqual(run(1 / 60), a); assert.deepEqual(run(1 / 10), a);
  assert.equal(a.frame.behaviour === 'doze', false);
  assert.ok(a.voices.some(v => v.id.startsWith('tear:'))); assert.ok(a.voices.some(v => v.id.startsWith('address:')));
});
test('sleep remains observed in wild mode; recorded gaps stay hollow; touch wakes without inventing coverage', () => {
  const world = new PetWorld(points, [], [{ timeMs: 80_000, kind: 'attention', x: .2, y: 0 }]);
  for (let i = 0; i < 79 * 30; i++) world.step(1 / 30);
  assert.equal(world.frame.behaviour, 'doze'); assert.equal(world.frame.state.observed, 1); assert.ok(world.frame.state.lit < .3);
  for (let i = 0; i < 30; i++) world.step(1 / 30);
  assert.equal(world.frame.behaviour, 'wake'); assert.equal(world.frame.state.channel, 'human');
  const unknown = new PetWorld(points, compilePetTelemetry([], 90_000), [{ timeMs: 1000, kind: 'attention', x: 0, y: 0 }]);
  unknown.step(1); assert.equal(unknown.frame.state.channel, 'human'); assert.equal(unknown.sim.frame.hollow, true);
});
test('pod identity survives membership changes; needs escalate on actual pending spans', () => {
  const events = ['c', 'a', 'b'].map(agentId => event({ id: agentId, agentId, endTime: 800 }));
  events.push(event({ id: 'b2', agentId: 'b', startTime: 1200, endTime: 1600 }));
  const world = new PetWorld(points, compilePetTelemetry(events, 2000));
  const slots = new Map(world.frame.pod.map(m => [m.id, m.slot])); world.step(1.3);
  assert.equal(world.frame.pod.find(m => m.id === 'b').slot, slots.get('b'));
  assert.equal(world.frame.pod.find(m => m.id === 'a').present, false);
  const waiting = new PetWorld(points, compilePetTelemetry([event({ category: 'human', status: 'pending', endTime: 30_000 })]));
  waiting.step(9); assert.equal(waiting.frame.needs, 'approach'); waiting.step(10); waiting.step(8); assert.equal(waiting.frame.needs, 'call');
});
test('PCM absolute samples match split chunks including tear noise and remain bounded', () => {
  const voices = [{ id: 'tear', start: .05, duration: .3, frequency: 185, gain: .1, pan: -.2, kind: 'noise' },
    { id: 'tone', start: 0, duration: .44, frequency: 130.81, gain: .08, pan: .3, kind: 'tone' }];
  const whole = renderPetPCM(voices, 0, 24_000), a = renderPetPCM(voices, 0, 9123), b = renderPetPCM(voices, 9123, 14_877);
  assert.deepEqual([...a.left, ...b.left], [...whole.left]); assert.deepEqual([...a.right, ...b.right], [...whole.right]);
  assert.ok(whole.left.some(n => n !== 0)); assert.ok(whole.left.every(n => Number.isFinite(n) && Math.abs(n) <= 1));
  assert.throws(() => renderPetPCM(voices, -1, 4));
});

test('live accepted packets and interactions replay the same frames and audio including expired gaps', () => {
  const live = new PetWorld(points, compilePetTelemetry([])), voices = [...live.voices], frames = [];
  const packets = compilePetTelemetry(petDemoEvents(), 80_000);
  for (let tick = 0; tick < 180; tick++) {
    if ([4, 29, 92].includes(tick)) live.acceptTelemetry(packets[tick]);
    if (tick === 73) live.interact('attention', .4, -.2);
    live.step(1 / 30); voices.push(...live.voices); frames.push(digest(live.sim));
  }
  const replay = new PetWorld(points, live.tape, live.interactions), replayVoices = [...replay.voices];
  for (let tick = 0; tick < 180; tick++) {
    replay.step(1 / 30); replayVoices.push(...replay.voices); assert.equal(digest(replay.sim), frames[tick], `tick ${tick}`);
  }
  assert.deepEqual(replayVoices, voices); assert.equal(live.frame.state.observed, 0);
});
test('metadata privacy preserves only boolean container markers needed by every live and replay driver', () => {
  const events = [event({ id: 'container', category: 'reasoning', attributes: { 'whalesong.container': true, 'codewhale.container': 'private text' } }), event({ id: 'work' })];
  const [trace] = importTrace(JSON.stringify(events), 'fixture', { privacy: 'metadata' });
  assert.deepEqual(trace.events[0].attributes, { 'whalesong.container': true });
  assert.equal(compilePetTelemetry(trace.events)[0].channel, 'tool');
});

test('successive peer groups reuse vacant slots while continuously present identities keep their slot and phase', () => {
  const events = ['a', 'b', 'c', 'd', 'e', 'f'].map(id => event({ id, agentId: id, endTime: 800 }));
  events.push(...['b', 'g', 'h', 'i', 'j', 'k', 'l'].map(id => event({ id: `next-${id}`, agentId: id, startTime: 800, endTime: 1600 })));
  const world = new PetWorld(points, compilePetTelemetry(events)), before = world.frame.pod.find(p => p.id === 'b');
  world.step(1);
  assert.deepEqual(world.frame.pod.find(p => p.id === 'b'), before);
  assert.equal(world.frame.pod.filter(p => p.present).length, 6);
  assert.ok(world.frame.pod.some(p => p.id === 'k')); assert.ok(!world.frame.pod.some(p => p.id === 'a'));
  assert.equal(new Set(world.frame.pod.map(p => p.slot)).size, 6);
});
test('multiple inputs before one fixed tick are all journalled and replay together', () => {
  const world = new PetWorld(points);
  world.interact('food', -.4, .3); world.interact('attention', .2, .1); world.step(1 / 30);
  assert.equal(world.interactions.length, 2); assert.equal(world.frame.food.x, -.4);
  const replay = new PetWorld(points, [], world.interactions); replay.step(1 / 30);
  assert.deepEqual(replay.frame, world.frame); assert.equal(digest(replay.sim), digest(world.sim));
});
