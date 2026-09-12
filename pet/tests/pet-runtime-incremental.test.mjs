import test from 'node:test';
import assert from 'node:assert/strict';
import { importTrace, privacyEvent, redact } from '../dist/core/ingest.js';
import { CodewhaleRuntimeTrace, observeRuntimeRequests } from '../dist/core/codewhale.js';
import { compilePetTelemetry } from '../dist/core/pet-telemetry.js';

const epoch = Date.parse('2026-09-12T00:00:00Z');
const stamp = ms => new Date(epoch + ms).toISOString();
const row = (seq, event, ms, payload, turn_id = 'turn-a') => ({ seq, event, thread_id: 'fixture', turn_id, timestamp: stamp(ms), payload });
const read = rows => importTrace(JSON.stringify(rows), 'fixture', { privacy: 'metadata' })[0];
const incremental = (maxEvents = 250_000, maxBytes = 64 * 1024 * 1024) => new CodewhaleRuntimeTrace('fixture', maxEvents, event => privacyEvent(event, 'metadata'), maxBytes);
const recent = (trace, now) => {
  const observed = observeRuntimeRequests(trace, epoch + now);
  return compilePetTelemetry(observed.events, observed.duration, Math.floor(now / 400) - 6, epoch - Date.parse(observed.originTime));
};

test('pruned incremental Runtime input matches full imports through origin changes, open requests, automatic consent and late failures', () => {
  const source = incremental(), all = [];
  const steps = [
    [12_000, [row(1, 'thread.updated', 10_000, {}), row(2, 'item.completed', 5000, { item: { id: 'old', kind: 'tool_call', started_at: stamp(1000), ended_at: stamp(3000), status: 'failed' }, tool: 'bash' })]],
    [24_000, [row(3, 'user_input.required', 20_000, { id: 'human', request: { questions: ['fixture-private-question'] } }), row(4, 'item.started', 22_000, { item: { id: 'long', kind: 'tool_call', started_at: stamp(0), status: 'running' }, tool: 'bash' })]],
    [40_000, [row(5, 'thread.updated', 39_000, {})]],
    [60_000, [row(6, 'item.completed', 59_000, { item: { id: 'long', status: 'failed', ended_at: stamp(10_000), detail: 'fixture-private-result' } })]],
    [72_000, [row(7, 'user_input.answered', 70_000, { input_id: 'human', answers: ['fixture-private-answer'] }), row(8, 'approval.required', 71_000, { approval_id: 'automatic' }), row(9, 'approval.decided', 71_100, { approval_id: 'automatic', auto: true })]],
    [96_000, [row(10, 'user_input.required', 94_000, { id: 'turn-close' }), row(11, 'turn.completed', 95_000, { turn: { status: 'completed' } })]],
  ];
  let prior, priorText;
  for (const [now, rows] of steps) {
    all.push(...rows); source.append(redact(rows)); source.prune(epoch + now - 16_000);
    if (prior) assert.equal(JSON.stringify(prior), priorText, 'A later receipt cannot mutate an already captured snapshot');
    const snapshot = source.snapshot();
    assert.deepEqual(recent(snapshot, now), recent(read(all), now));
    assert.doesNotMatch(JSON.stringify(snapshot), /fixture-private/);
    if (now === 12_000) assert.equal(snapshot.events.find(e => e.id === 'old').attributes['whalesong.error_onset_ms'], 4000, 'An older negative relative error timestamp is normalized, not dropped');
    if (now === 40_000) assert.ok(snapshot.events.some(e => e.id === 'long' && e.openEnded));
    prior = snapshot; priorText = JSON.stringify(snapshot);
  }
  source.prune(epoch + 120_000);
  assert.equal(source.retainedEvents, 0); assert.equal(source.retainedBytes, 0);
});

test('retained Runtime limits reject excess unfinished work and count terminal mutations without retaining payloads', () => {
  const source = incremental(2, 8192);
  source.append([row(1, 'user_input.required', 0, { id: 'a' }), row(2, 'user_input.required', 1000, { id: 'b' })]);
  source.prune(epoch + 100_000); assert.equal(source.retainedEvents, 2);
  const before = source.retainedBytes;
  // Completion must remain possible at the event limit; it updates the existing
  // lifetime rather than consuming another event slot.
  source.append([row(3, 'user_input.answered', 100_000, { input_id: 'a', answers: ['private'.repeat(10_000)] })]);
  source.prune(epoch + 120_000); assert.equal(source.retainedEvents, 1);
  assert.ok(source.retainedBytes > 0 && source.retainedBytes < before);
  source.append([row(4, 'user_input.required', 120_000, { id: 'c' })]);
  assert.throws(() => source.append([row(5, 'user_input.required', 120_001, { id: 'd' })]), /event limit/);
  assert.equal(source.retainedEvents, 2);
  const bounded = incremental(100, 128);
  assert.throws(() => bounded.append([row(1, 'thread.updated', 0, {})]), /retained input limit/);
  assert.equal(bounded.retainedEvents, 0); assert.equal(bounded.retainedBytes, 0);
});
