import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { setTimeout as delay } from 'node:timers/promises';
import { followRuntime } from '../scripts/lib/pet-runtime.mjs';
import { compilePetTelemetry } from '../dist/core/pet-telemetry.js';

// Real HTTP/SSE transport, virtual event timestamps: no provider calls or
// 29-hour wall-clock sleep. This exceeded the old raw-journal lifetime limit.
test('Runtime consumes more than 250000 records across a day while retaining current state and unfinished requests', { timeout: 60_000 }, async t => {
  const total = 260_001, began = Date.now() - total * 400, reports = [];
  let response, transport, serverError;
  const server = createServer(async (_req, res) => {
    response = res;
    const closed = new AbortController(); res.once('close', () => closed.abort());
    res.writeHead(200, { 'content-type': 'text/event-stream' });
    try {
      for (let first = 1; first <= total; first += 128) {
        let block = '';
        for (let seq = first; seq < first + 128 && seq <= total; seq++) {
          block += `data: ${JSON.stringify({ seq, previous_seq: seq - 1, event: seq === 1 ? 'user_input.required' : 'thread.updated',
            thread_id: 'long-fixture', timestamp: new Date(began + seq * 400).toISOString(),
            payload: seq === 1 ? { id: 'still-waiting' } : { description: 'fixture-private-journal'.repeat(12) } })}\n\n`;
        }
        if (!res.write(block)) await once(res, 'drain', { signal: closed.signal });
      }
    } catch (error) { if (!closed.signal.aborted) serverError = error; }
    // Keep the final cursor healthy, including the still-open human request.
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  t.after(async () => {
    await transport?.close(); response?.destroy(); server.closeAllConnections();
    await new Promise(resolve => server.close(resolve));
  });
  transport = await followRuntime({ baseUrl: `http://127.0.0.1:${server.address().port}`, threadId: 'long-fixture', report: text => reports.push(text) });
  const deadline = Date.now() + 50_000;
  while (transport.cursor < total && Date.now() < deadline && !reports.some(text => text.includes('stopped'))) await delay(20);
  assert.equal(serverError, undefined);
  assert.equal(transport.cursor, total, reports.join('\n'));
  assert.equal(transport.connected, true);
  const now = Date.now(), snapshot = transport.snapshot(now);
  assert.ok(snapshot); assert.doesNotMatch(JSON.stringify(snapshot), /fixture-private-journal/);
  assert.ok(transport.retainedEvents < 512); assert.ok(transport.retainedBytes < 256 * 1024);
  const live = compilePetTelemetry(snapshot.events, snapshot.duration, 0, now - 800 - Date.parse(snapshot.originTime));
  assert.equal(live[0].waiting, true); assert.equal(live[0].activeMs[11], 400);
  assert.deepEqual(reports, []);
});
