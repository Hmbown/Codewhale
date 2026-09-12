import assert from 'node:assert/strict';
import test from 'node:test';
import { CodeWhaleRuntimeClient } from '../index.js';

function clientFor(chunks, inspect = () => {}) {
  return new CodeWhaleRuntimeClient({ token: 'fixture-token', fetch: async (url, init) => {
    inspect(url, init);
    return new Response(new ReadableStream({ start(controller) {
      for (const chunk of chunks) controller.enqueue(new TextEncoder().encode(chunk));
      controller.close();
    } }), { headers: { 'content-type': 'text/event-stream; charset=utf-8' } });
  } });
}
const collect = async stream => { const out = []; for await (const value of stream) out.push(value); return out; };

test('thread journal reads preserve the Runtime cursor, GET, cancellation and redirect boundary', async () => {
  const signal = new AbortController().signal;
  const record = { seq: 17, previous_seq: 9, event: 'item.completed', thread_id: 't/a', timestamp: '2026-09-12T00:00:00Z', payload: {} };
  const raw = `: keepalive\r\n\r\ndata: ${JSON.stringify(record)}\r\n\r\n`;
  const client = clientFor([...raw], (url, init) => {
    assert.equal(url.pathname, '/v1/threads/t%2Fa/events');
    assert.equal(url.searchParams.get('since_seq'), '9'); assert.equal(url.searchParams.get('replay_limit'), '100');
    assert.equal(init.method, 'GET'); assert.equal(init.redirect, 'error'); assert.equal(init.signal, signal);
    assert.equal(init.headers.get('authorization'), 'Bearer fixture-token');
  });
  assert.deepEqual(await collect(client.threadEvents('t/a', { sinceSeq: 9, replayLimit: 100, signal })), [record]);
});
test('thread stream refuses incomplete, oversized and invalid frames without emitting a partial event', async () => {
  await assert.rejects(collect(clientFor(['data: {"seq":1}']).threadEvents('t')), /inside a frame/);
  await assert.rejects(collect(clientFor(['data: ' + 'x'.repeat(2 * 1024 * 1024)]).threadEvents('t')), /size limit/);
  await assert.rejects(collect(clientFor(['data: not-json\n\n']).threadEvents('t')), SyntaxError);
});
test('thread stream rejects invalid cursor arguments and a JSON response', async () => {
  for (const sinceSeq of [-1, 1.5, Number.MAX_SAFE_INTEGER + 1])
    await assert.rejects(collect(clientFor([]).threadEvents('t', { sinceSeq })), /safe integer/);
  const client = new CodeWhaleRuntimeClient({ fetch: async () => new Response('{}', { headers: { 'content-type': 'application/json' } }) });
  await assert.rejects(collect(client.threadEvents('t')), /not an event stream/);
});
