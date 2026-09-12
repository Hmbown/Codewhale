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
    res.writeHead(200, { 'content-type': 'text/event-stream' });
    res.write(`data: ${JSON.stringify({ seq: 1, previous_seq: 0, event: 'thread.updated',
      thread_id: 'fixture', timestamp: new Date().toISOString(), payload: {} })}\n\n`);
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
    res.writeHead(200, { 'content-type': 'text/event-stream' }); res.flushHeaders(); responses.add(res);
    res.on('close', () => responses.delete(res));
    const connection = ++connections;
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
  const child = spawn(process.execPath, ['scripts/pet.mjs', `--runtime=http://127.0.0.1:${server.address().port}`, '--thread=fixture-thread', `--output=${output}`],
    { cwd: new URL('../', import.meta.url), env: { ...process.env, CODEWHALE_RUNTIME_TOKEN: 'fixture-token' }, stdio: ['ignore', 'pipe', 'pipe'] });
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
  await waitForRecordedChannel('browser'); child.kill('SIGINT'); const [code] = await exited; assert.equal(code, 0, log); clearInterval(pulse);
  const text = await readFile(output, 'utf8'), tape = decodePetJSONL(text);
  assert.ok(tape.some(b => b.channel === 'code' && b.observed === 1));
  assert.ok(tape.some(b => b.sequence > 1 && b.observed === 0));
  assert.ok(tape.some(b => b.channel === 'browser' && b.observed === 1));
  assert.equal(requests.length, 3); assert.equal(new URL(requests[1].url, 'http://127.0.0.1').search, new URL(requests[2].url, 'http://127.0.0.1').search);
  assert.ok(requests.every(r => r.method === 'GET' && r.url.startsWith('/v1/threads/fixture-thread/events?') && r.authorization === 'Bearer fixture-token'));
  assert.doesNotMatch(text + log, /fixture-private-text|fixture-token/);
});

test('live Runtime recording retains human waits and a brief late failure exactly once between timer ticks', { timeout: 12_000 }, async t => {
  let sequence = 0, response;
  const timers = [], requests = [];
  const server = createServer((req, res) => {
    requests.push({ method: req.method, url: req.url }); response = res;
    res.writeHead(200, { 'content-type': 'text/event-stream' }); res.flushHeaders();
    const oldStart = new Date(Date.now() - 20_000).toISOString(), oldEnd = new Date(Date.now() - 16_000).toISOString();
    const emit = (event, payload) => {
      const previous_seq = sequence; sequence++;
      res.write(`data: ${JSON.stringify({ seq: sequence, previous_seq, event, thread_id: 'fixture-thread', turn_id: 'turn-a', timestamp: new Date().toISOString(), payload })}\n\n`);
    };
    emit('item.started', { item: { id: 'old-work', kind: 'tool_call', status: 'running', started_at: oldStart }, tool: 'bash' });
    timers.push(setTimeout(() => emit('user_input.required', { id: 'question', request: { questions: ['fixture-private-question'] } }), 110));
    timers.push(setTimeout(() => emit('item.completed', { item: { id: 'old-work', kind: 'tool_call', status: 'failed', started_at: oldStart, ended_at: oldEnd, detail: 'fixture-private-result' }, tool: 'bash' }), 650));
    timers.push(setTimeout(() => emit('user_input.answered', { input_id: 'question', answers: ['fixture-private-answer'] }), 1900));
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const dir = await mkdtemp(join(tmpdir(), 'pet-runtime-lifecycle-')), output = join(dir, 'pet.jsonl');
  const child = spawn(process.execPath, ['scripts/pet.mjs', `--runtime=http://127.0.0.1:${server.address().port}`, '--thread=fixture-thread', `--output=${output}`],
    { cwd: new URL('../', import.meta.url), stdio: ['ignore', 'pipe', 'pipe'] });
  const exited = once(child, 'exit'); let log = '';
  child.stdout.on('data', b => log += b); child.stderr.on('data', b => log += b);
  t.after(async () => { timers.forEach(clearTimeout); if (child.exitCode === null) child.kill('SIGTERM'); response?.destroy(); server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); });
  for (let i = 0; !response && i < 100; i++) await delay(20);
  assert.ok(response, log); await delay(3300);
  child.kill('SIGINT'); const [code] = await exited; assert.equal(code, 0, log);
  const text = await readFile(output, 'utf8'), tape = decodePetJSONL(text);
  assert.equal(tape.reduce((sum, b) => sum + b.errors, 0), 1);
  assert.ok(tape.some(b => b.channel === 'error' && b.observed === 1));
  assert.ok(tape.filter(b => b.waiting).length >= 3);
  assert.ok(tape.slice(-2).every(b => !b.waiting && !b.observed));
  assert.doesNotMatch(text + log, /fixture-private-question|fixture-private-result|fixture-private-answer/);
  assert.ok(requests.every(r => r.method === 'GET' && r.url.startsWith('/v1/threads/fixture-thread/events?')));
});
