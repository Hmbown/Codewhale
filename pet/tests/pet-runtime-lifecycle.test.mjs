import test from 'node:test';
import assert from 'node:assert/strict';
import { importTrace } from '../dist/core/ingest.js';
import { observeRuntimeRequests } from '../dist/core/codewhale.js';
import { compilePetTelemetry } from '../dist/core/pet-telemetry.js';
import { buildPyramid } from '../dist/core/signal.js';

const epoch = Date.parse('2026-09-12T00:00:00Z');
const stamp = ms => new Date(epoch + ms).toISOString();
const row = (seq, event, ms, payload, turn_id = 'turn-a') => ({ seq, event, thread_id: 'fixture', turn_id, timestamp: stamp(ms), payload });
const read = rows => importTrace(JSON.stringify(rows), 'fixture', { privacy: 'metadata' })[0];

test('a late Runtime failure keeps the real operation interval and tears once at its receipt time', () => {
  const trace = read([
    row(1, 'item.started', 0, { item: { id: 'work', kind: 'tool_call', started_at: stamp(0), status: 'running' }, tool: 'bash' }),
    row(2, 'item.completed', 2000, { item: { id: 'work', kind: 'tool_call', started_at: stamp(0), ended_at: stamp(1000), status: 'failed', detail: 'fixture-private-result' }, tool: 'bash' }),
  ]);
  const item = trace.events.find(e => e.id === 'work');
  assert.equal(item.status, 'error'); assert.equal(item.endTime, 1000);
  assert.equal(item.attributes['whalesong.error_onset_ms'], 2000);
  assert.equal(trace.duration, 2000); assert.doesNotMatch(JSON.stringify(trace), /fixture-private-result/);
  const tape = compilePetTelemetry(trace.events, 2400);
  assert.equal(tape[0].channel, 'code'); assert.equal(tape[0].errors, 0);
  assert.equal(tape[3].observed, 0);
  assert.equal(tape[5].channel, 'error'); assert.equal(tape[5].observed, 1);
  assert.equal(tape.reduce((sum, b) => sum + b.errors, 0), 1);
  const level = buildPyramid(trace.events).levels[0], offset = 3 * level.length;
  assert.equal(level.errors[offset], 0);
  assert.equal(level.errors[offset + Math.floor(2000 / level.binMs)], 1);
});

test('Runtime approval and input identities retain waiting intervals through their matching terminal receipts', () => {
  const trace = read([
    row(1, 'approval.required', 0, { approval_id: 'shared', description: 'fixture-private-question' }),
    row(2, 'user_input.required', 1000, { id: 'shared', request: { questions: ['fixture-private-question'] } }),
    row(3, 'user_input.answered', 2000, { input_id: 'shared' }, 'different-turn'),
    row(4, 'approval.decided', 4000, { approval_id: 'shared', decision: 'deny' }),
    row(5, 'user_input.canceled', 8000, { input_id: 'shared' }),
  ]);
  const approval = trace.events.find(e => e.name === 'approval.required');
  const input = trace.events.find(e => e.name === 'user_input.required');
  assert.equal(approval.endTime, 4000); assert.equal(input.endTime, 8000);
  assert.equal(approval.openEnded, false); assert.equal(input.openEnded, false);
  const tape = compilePetTelemetry(trace.events, 8800);
  assert.ok(tape.slice(0, 20).every(b => b.waiting && b.channel === 'human'));
  assert.equal(tape[20].waiting, false); assert.equal(tape[20].observed, 0);
  assert.equal(tape.reduce((sum, b) => sum + b.errors, 0), 0);
  assert.doesNotMatch(JSON.stringify(trace), /fixture-private-question/);
});

test('automatic consent never becomes a human wait and turn completion closes only its own requests', () => {
  const automatic = read([
    row(1, 'approval.required', 0, { id: 'auto' }),
    row(2, 'approval.decided', 500, { approval_id: 'auto', decision: 'allow', auto: true }),
  ]);
  assert.ok(compilePetTelemetry(automatic.events, 1200).every(b => !b.waiting && !b.observed));
  const trace = read([
    row(1, 'user_input.required', 0, { id: 'same' }),
    row(2, 'user_input.required', 100, { id: 'same' }, 'turn-b'),
    row(3, 'turn.completed', 2000, { turn: { status: 'completed' } }),
  ]);
  const requests = trace.events.filter(e => e.name === 'user_input.required');
  assert.equal(requests[0].openEnded, false); assert.equal(requests[0].endTime, 2000);
  assert.equal(requests[1].openEnded, true);
  assert.throws(() => read([row(1, 'user_input.required', 0, {})]), /identity/);
});

test('only a healthy live observation extends a witnessed request; static imports and open tools stay unknown', () => {
  const trace = read([
    row(1, 'user_input.required', 0, { id: 'wait' }),
    row(2, 'item.started', 100, { item: { id: 'work', kind: 'tool_call', status: 'running' }, tool: 'bash' }),
  ]);
  const snapshot = JSON.stringify(trace);
  assert.equal(compilePetTelemetry(trace.events, 30_000)[70].observed, 0);
  const live = observeRuntimeRequests(trace, epoch + 30_000);
  const tape = compilePetTelemetry(live.events, live.duration);
  assert.equal(tape[70].waiting, true); assert.equal(tape[70].activeMs[11], 400);
  assert.equal(tape[70].activeMs[3], 0); assert.equal(JSON.stringify(trace), snapshot);
  assert.throws(() => observeRuntimeRequests(trace, NaN), /horizon/);
});

test('a fixed recorder clock survives older source origins without replaying historical failures or shifting onsets', () => {
  const event = { schemaVersion: 1, id: 'work', traceId: 't', startTime: 0, endTime: 1000,
    agentId: 'parent', name: 'bash', category: 'code', status: 'error', attributes: { 'whalesong.error_onset_ms': 3000 } };
  const tape = compilePetTelemetry([event], 3600, 0, 400);
  const shifted = { ...event, startTime: 5000, endTime: 6000, attributes: { 'whalesong.error_onset_ms': 8000 } };
  assert.deepEqual(compilePetTelemetry([shifted], 8600, 0, 5400), tape);
  assert.equal(tape[6].errors, 1); assert.equal(tape[0].onsets[3], 0);
  assert.equal(compilePetTelemetry([event], 4400, 0, 4000).reduce((sum, b) => sum + b.errors, 0), 0);
  const normalized = read([row(1, 'item.completed', 3000, { item: { id: 'done', kind: 'tool_call', started_at: stamp(1000), ended_at: stamp(2000), status: 'failed' } })]);
  assert.equal(normalized.originTime, stamp(1000));
  assert.equal(normalized.events[0].attributes['whalesong.error_onset_ms'], 2000);
  const generic = importTrace(JSON.stringify([shifted]), 'event-v1', { privacy: 'metadata' })[0];
  assert.equal(generic.events[0].attributes['whalesong.error_onset_ms'], 3000);
  assert.throws(() => compilePetTelemetry([{ ...event, attributes: { 'whalesong.error_onset_ms': -1 } }]), /failure observation/);
});
