import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { mkdtemp, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { once } from 'node:events';
import { setTimeout as delay } from 'node:timers/promises';
import { followRuntime } from '../scripts/lib/pet-runtime.mjs';
import { decodePetJSONL } from '../dist/core/pet-telemetry.js';
import { spawnRecorder } from './helpers/recorder-process.mjs';

test('Runtime pet input refuses remote hosts, credentials, paths and missing thread selection before connecting', async () => {
  for (const baseUrl of ['https://127.0.0.1:1', 'http://example.com', 'http://localhost:1', 'http://user:secret@127.0.0.1:1', 'http://127.0.0.1:1/private', 'http://127.0.0.1:1/?token=secret'])
    await assert.rejects(followRuntime({ baseUrl, threadId: 't' }), /loopback IP origin/);
  await assert.rejects(followRuntime({ baseUrl: 'http://127.0.0.1:1', threadId: '' }), /thread/);
});

test('Runtime shutdown closes an idle SSE body after garbage collection', { timeout: 10_000 }, async t => {
  let response, closed = false;
  const server = createServer((req, res) => {
    response = res;
    res.once('close', () => { closed = true; });
    res.writeHead(200, { 'content-type': 'text/event-stream', 'x-codewhale-event-progress': '1' });
    res.write(`data: ${JSON.stringify({ seq: 1, previous_seq: 0, event: 'thread.updated',
      thread_id: 'fixture', timestamp: new Date().toISOString(), payload: {} })}\n\n`);
    res.write(`data: ${JSON.stringify({ event: 'stream.progress', state: 'live', thread_id: 'fixture', seq: 1 })}\n\n`);
    // Stay open without new chunks: cancellation must wake the idle reader.
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const script = `
    import assert from 'node:assert/strict';
    import { setTimeout as delay } from 'node:timers/promises';
    import { followRuntime } from './scripts/lib/pet-runtime.mjs';
    const input = await followRuntime({ baseUrl: 'http://127.0.0.1:${server.address().port}', threadId: 'fixture' });
    for (let i = 0; !input.connected && i < 200; i++) await delay(10);
    assert.equal(input.cursor, 1);
    globalThis.gc(); await delay(20); globalThis.gc();
    const deadline = setTimeout(() => { console.error('Idle Runtime reader did not stop'); process.exit(2); }, 2_000);
    await input.close(); clearTimeout(deadline);
    assert.equal(input.connected, false);
  `;
  const child = spawn(process.execPath, ['--expose-gc', '--input-type=module', '-e', script],
    { cwd: new URL('../', import.meta.url), stdio: ['ignore', 'pipe', 'pipe'] });
  const exited = once(child, 'exit'); let log = '';
  child.stdout.on('data', b => log += b); child.stderr.on('data', b => log += b);
  t.after(async () => {
    if (child.exitCode === null) child.kill('SIGKILL');
    response?.destroy(); server.closeAllConnections();
    await new Promise(resolve => server.close(resolve));
  });
  const [code] = await exited;
  assert.equal(code, 0, log); assert.ok(closed, 'The server must see the reader disconnect');
});

test('the CLI follows real Runtime SSE envelopes through disconnect and cursor recovery, recording no prompt content', { timeout: 20_000 }, async t => {
  let sequence = 0, connections = 0, stream, pulse;
  const requests = [], responses = new Set();
  const emit = (res, tool) => {
    const now = Date.now(), previous = sequence; sequence += 7;
    const event = { seq: sequence, previous_seq: previous, event: 'item.completed', thread_id: 'fixture-thread', item_id: `i${sequence}`,
      timestamp: new Date(now).toISOString(), payload: { item: { id: `i${sequence}`, kind: 'tool_call', status: 'completed',
        started_at: new Date(now - 180).toISOString(), ended_at: new Date(now).toISOString(), summary: `${tool}: fixture-private-text` }, tool } };
    res.write(`data: ${JSON.stringify(event)}\n\n`);
  };
  const server = createServer((req, res) => {
    requests.push({ method: req.method, url: req.url, authorization: req.headers.authorization });
    res.writeHead(200, { 'content-type': 'text/event-stream', 'x-codewhale-event-progress': '1' }); res.flushHeaders(); responses.add(res);
    res.on('close', () => responses.delete(res));
    const connection = ++connections;
    res.write(`data: ${JSON.stringify({ event: 'stream.progress', state: 'live', thread_id: 'fixture-thread', seq: Number(new URL(req.url, 'http://local').searchParams.get('since_seq')) })}\n\n`);
    if (connection === 1) { stream = res; emit(res, 'bash'); pulse = setInterval(() => emit(res, 'bash'), 120); }
    else if (connection === 2) {
      // Reject a hole; the next reconnect must request the same last cursor.
      res.end(`data: ${JSON.stringify({ seq: sequence + 20, previous_seq: sequence + 1, event: 'thread.updated', thread_id: 'fixture-thread', timestamp: new Date().toISOString(), payload: {} })}\n\n`);
    } else {
      const later = setTimeout(() => { emit(res, 'browser'); pulse = setInterval(() => emit(res, 'browser'), 120); }, 700);
      res.once('close', () => clearTimeout(later));
    }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const dir = await mkdtemp(join(tmpdir(), 'pet-runtime-')), output = join(dir, 'pet.jsonl');
  const child = spawnRecorder([`--runtime=http://127.0.0.1:${server.address().port}`, '--thread=fixture-thread', `--output=${output}`],
    { ...process.env, CODEWHALE_RUNTIME_TOKEN: 'fixture-token' });
  const exited = once(child, 'exit'); let log = '';
  child.stdout.on('data', b => log += b); child.stderr.on('data', b => log += b);
  t.after(async () => { clearInterval(pulse); if (child.exitCode === null) child.kill('SIGTERM'); for (const res of responses) res.destroy(); server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); });
  for (let i = 0; !stream && i < 60; i++) await delay(25);
  assert.ok(stream, log);
  const waitForRecordedChannel = async channel => {
    const until = Date.now() + 6_000;
    while (Date.now() < until) {
      try {
        const tape = decodePetJSONL(await readFile(output, 'utf8'));
        if (tape.some(b => b.channel === channel && b.observed === 1)) return;
      } catch { /* The first file or an in-flight final line is not ready. */ }
      await delay(40);
    }
    assert.fail(`The recorder did not persist observed ${channel} work. ${log}`);
  };
  // Assert actual recorder output before moving the fixture to its next phase.
  // A fixed sleep can expire before reconnect + a complete bin on a busy runner.
  await waitForRecordedChannel('code'); clearInterval(pulse); stream.destroy();
  await waitForRecordedChannel('browser'); child.stopRecorder(); const [code] = await exited; assert.equal(code, 0, log); clearInterval(pulse);
  const text = await readFile(output, 'utf8'), tape = decodePetJSONL(text);
  assert.ok(tape.some(b => b.channel === 'code' && b.observed === 1));
  assert.ok(tape.some(b => b.sequence > 1 && b.observed === 0));
  assert.ok(tape.some(b => b.channel === 'browser' && b.observed === 1));
  assert.equal(requests.length, 3); assert.equal(new URL(requests[1].url, 'http://127.0.0.1').search, new URL(requests[2].url, 'http://127.0.0.1').search);
  assert.ok(requests.every(r => r.method === 'GET' && r.url.startsWith('/v1/threads/fixture-thread/events?') && r.authorization === 'Bearer fixture-token'));
  assert.doesNotMatch(text + log, /fixture-private-text|fixture-token/);
});

test('live Runtime recording retains human waits and a brief late failure exactly once between timer ticks', { timeout: 15_000 }, async t => {
  let sequence = 0, response, answer;
  const timers = [], requests = [];
  const server = createServer((req, res) => {
    requests.push({ method: req.method, url: req.url }); response = res;
    res.writeHead(200, { 'content-type': 'text/event-stream', 'x-codewhale-event-progress': '1' }); res.flushHeaders();
    res.write(`data: ${JSON.stringify({ event: 'stream.progress', state: 'live', thread_id: 'fixture-thread', seq: 0 })}\n\n`);
    const oldStart = new Date(Date.now() - 20_000).toISOString(), oldEnd = new Date(Date.now() - 16_000).toISOString();
    const emit = (event, payload) => {
      const previous_seq = sequence; sequence++;
      res.write(`data: ${JSON.stringify({ seq: sequence, previous_seq, event, thread_id: 'fixture-thread', turn_id: 'turn-a', timestamp: new Date().toISOString(), payload })}\n\n`);
    };
    emit('item.started', { item: { id: 'old-work', kind: 'tool_call', status: 'running', started_at: oldStart }, tool: 'bash' });
    timers.push(setTimeout(() => emit('user_input.required', { id: 'question', request: { questions: ['fixture-private-question'] } }), 110));
    timers.push(setTimeout(() => emit('item.completed', { item: { id: 'old-work', kind: 'tool_call', status: 'failed', started_at: oldStart, ended_at: oldEnd, detail: 'fixture-private-result' }, tool: 'bash' }), 650));
    answer = () => emit('user_input.answered', { input_id: 'question', answers: ['fixture-private-answer'] });
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const dir = await mkdtemp(join(tmpdir(), 'pet-runtime-lifecycle-')), output = join(dir, 'pet.jsonl');
  const child = spawnRecorder([`--runtime=http://127.0.0.1:${server.address().port}`, '--thread=fixture-thread', `--output=${output}`]);
  const exited = once(child, 'exit'); let log = '';
  child.stdout.on('data', b => log += b); child.stderr.on('data', b => log += b);
  t.after(async () => { timers.forEach(clearTimeout); if (child.exitCode === null) child.kill('SIGTERM'); response?.destroy(); server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); });
  for (let i = 0; !response && i < 100; i++) await delay(20);
  assert.ok(response, log);
  const waitForTape = async (condition, message) => {
    for (let i = 0; i < 125; i++) {
      let text = '';
      try { text = await readFile(output, 'utf8'); } catch (error) { if (error.code !== 'ENOENT') throw error; }
      const rows = decodePetJSONL(text.slice(0, text.lastIndexOf('\n') + 1));
      if (condition(rows)) return;
      await delay(40);
    }
    assert.fail(message + '\n' + log);
  };
  // Drive the answer after actual recorded coverage. Wall-clock sleeps alone
  // can stop the child before it seals the final unknown bins on a busy runner.
  await waitForTape(rows => rows.filter(b => b.waiting).length >= 3 && rows.some(b => b.errors), 'Waiting/error receipts were not recorded');
  answer();
  await waitForTape(rows => rows.length >= 2 && rows.slice(-2).every(b => !b.waiting && !b.observed), 'Answered input did not expire to unknown');
  child.stopRecorder(); const [code] = await exited; assert.equal(code, 0, log);
  const text = await readFile(output, 'utf8'), tape = decodePetJSONL(text);
  assert.equal(tape.reduce((sum, b) => sum + b.errors, 0), 1);
  assert.ok(tape.some(b => b.channel === 'error' && b.observed === 1));
  assert.ok(tape.filter(b => b.waiting).length >= 3);
  assert.ok(tape.slice(-2).every(b => !b.waiting && !b.observed));
  assert.doesNotMatch(text + log, /fixture-private-question|fixture-private-result|fixture-private-answer/);
  assert.ok(requests.every(r => r.method === 'GET' && r.url.startsWith('/v1/threads/fixture-thread/events?')));
});


test('replayed requests remain unknown until catch-up, including reentry to replay after a live connection', { timeout: 10_000 }, async t => {
  let response, input, sequence = 0;
  const requests = [], reports = [];
  const server = createServer((req, res) => {
    requests.push(req.url); response = res;
    res.writeHead(200, { 'content-type': 'text/event-stream', 'x-codewhale-event-progress': '1' }); res.flushHeaders();
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  t.after(async () => { await input?.close(); response?.destroy(); server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); });
  input = await followRuntime({ baseUrl: `http://127.0.0.1:${server.address().port}`, threadId: 'fixture', report: text => reports.push(text) });
  const wait = async predicate => {
    for (let n = 0; n < 250 && !predicate(); n++) await delay(10);
    assert.ok(predicate(), reports.join('\n'));
  };
  await wait(() => response);
  const write = packet => response.write('data: ' + JSON.stringify(packet) + '\n\n');
  const progress = state => write({ event: 'stream.progress', thread_id: 'fixture', seq: sequence, state });
  const event = (event, payload, age = 0) => write({ seq: ++sequence, previous_seq: sequence - 1,
    event, thread_id: 'fixture', turn_id: 'turn-a', timestamp: new Date(Date.now() - age).toISOString(), payload });
  progress('replaying'); event('user_input.required', { id: 'settled-old-request' }, 60_000);
  await wait(() => input.cursor === 1);
  assert.equal(input.connected, false); assert.equal(input.snapshot(), undefined);
  // Simulate a slow backlog while the 400 ms recorder clock could run.
  await delay(450); assert.equal(input.snapshot(), undefined);
  event('user_input.answered', { id: 'settled-old-request' }, 50_000); progress('live');
  await wait(() => input.connected);
  assert.equal(input.snapshot(), undefined, 'A historical answer must arrive before old pending input can become current');
  event('user_input.required', { id: 'fresh-request' }); await wait(() => input.cursor === 3);
  const { compilePetTelemetry } = await import('../dist/core/pet-telemetry.js');
  let snapshot = input.snapshot(Date.now() + 400);
  assert.ok(compilePetTelemetry(snapshot.events, snapshot.duration).some(b => b.waiting && b.observed === 1));
  progress('replaying'); await wait(() => !input.connected); assert.equal(input.snapshot(), undefined);
  event('user_input.answered', { id: 'fresh-request' }); await wait(() => input.cursor === 4);
  assert.equal(input.snapshot(), undefined, 'Journal data alone cannot establish readiness');
  progress('live'); await wait(() => input.connected);
  snapshot = input.snapshot(Date.now() + 800);
  assert.ok(!snapshot || !compilePetTelemetry(snapshot.events, snapshot.duration).at(-1).waiting);
  assert.equal(requests.length, 1); assert.equal(new URL(requests[0], 'http://local').searchParams.get('progress'), 'true');
  assert.deepEqual(reports, []);
});

test('a Runtime without replay progress stops explicitly before any old request becomes current', { timeout: 5000 }, async t => {
  let response, input, closed = false;
  const reports = [];
  const server = createServer((_req, res) => {
    response = res; res.once('close', () => { closed = true; });
    res.writeHead(200, { 'content-type': 'text/event-stream' });
    res.write('data: ' + JSON.stringify({ seq: 1, event: 'user_input.required', thread_id: 'fixture', timestamp: new Date().toISOString(), payload: { id: 'old' } }) + '\n\n');
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  t.after(async () => { await input?.close(); response?.destroy(); server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); });
  input = await followRuntime({ baseUrl: `http://127.0.0.1:${server.address().port}`, threadId: 'fixture', report: text => reports.push(text) });
  for (let n = 0; n < 200 && !reports.length; n++) await delay(10);
  assert.match(reports.join('\n'), /stopped.*replay-progress support/);
  assert.equal(input.connected, false); assert.equal(input.cursor, 0); assert.equal(input.snapshot(), undefined);
  for (let n = 0; n < 100 && !closed; n++) await delay(10);
  assert.ok(closed);
});
