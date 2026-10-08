import test from 'node:test';
import assert from 'node:assert/strict';
import { importTrace } from '../dist/core/ingest.js';
import { compilePetTelemetry } from '../dist/core/pet-telemetry.js';

const session = entries => JSON.stringify({ metadata: { id: 'session-1' }, journal: { entries } });
const at = seconds => new Date(Date.UTC(2026, 0, 1, 0, 0, seconds)).toISOString();

test('Codewhale session import bounds emitted events, keeps call identities and never ends a call before it starts', () => {
  // One entry, many blocks: the limit applies per event, not per entry.
  const blocks = Array.from({ length: 5 }, (_, i) => ({ type: 'text', text: `block ${i}` }));
  assert.throws(() => importTrace(session([{ id: 'e1', message: { role: 'assistant', content: blocks } }]), 's', { maxEvents: 3 }),
    /3 event limit/);
  // A repeated tool call id would otherwise silently replace the first call.
  const call = id => ({ id, message: { role: 'assistant', content: [{ type: 'tool_use', id: 'call-1', name: 'read_file', input: {} }] } });
  assert.throws(() => importTrace(session([call('e1'), call('e2')]), 's'), /Duplicate event identity \(session-1, call-1\)/);
  // A result timestamped before its call is clamped, reported, and still compiles.
  const [trace] = importTrace(session([
    { id: 'e1', created_at: at(10), message: { role: 'assistant', content: [{ type: 'tool_use', id: 'call-1', name: 'read_file', input: {} }] } },
    { id: 'e2', created_at: at(4), message: { role: 'user', content: [{ type: 'tool_result', tool_use_id: 'call-1', content: 'ok' }] } },
  ]), 's');
  const tool = trace.events.find(e => e.id === 'call-1');
  assert.equal(tool.openEnded, false);
  assert.ok(tool.endTime >= tool.startTime);
  assert.ok(trace.warnings.some(w => w.includes('timestamped before its call')));
  assert.ok(compilePetTelemetry(trace.events, trace.duration).length > 0);
});

test('Codewhale runtime import never ends a turn before its kept start', () => {
  // turn.completed carries wall times earlier than the turn.started record that fixed the start.
  const rec = (seq, event, s, turn) => ({ seq, event, thread_id: 'th', turn_id: 't1', timestamp: at(s), payload: { turn } });
  const [trace] = importTrace([rec(0, 'turn.started', 10, {}),
    rec(1, 'turn.completed', 12, { started_at: at(0), ended_at: at(5), status: 'completed' })].map(r => JSON.stringify(r)).join('\n'), 'r');
  const turn = trace.events.find(e => e.id === 'turn:t1');
  assert.equal(turn.openEnded, false);
  assert.ok(turn.endTime >= turn.startTime, `${turn.startTime} > ${turn.endTime}`);
  assert.ok(compilePetTelemetry(trace.events, trace.duration).length > 0);
});
