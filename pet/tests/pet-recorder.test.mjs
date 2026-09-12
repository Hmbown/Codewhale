import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, writeFile, stat, readdir, rename } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createPetRecorder } from '../scripts/lib/pet-recorder.mjs';
import { compilePetTelemetry, encodePetJSONL, decodePetJSONL, PetLiveTape } from '../dist/core/pet-telemetry.js';
import { spawnRecorder } from './helpers/recorder-process.mjs';

const empty = compilePetTelemetry([])[0];
const destination = async () => join(await mkdtemp(join(tmpdir(), 'pet-segment-')), 'pet-state');
const part = (path, i) => `${path}.segment-${String(i).padStart(6, '0')}.jsonl`;

test('continuous recorder crosses a day, archives every bucket once, and the live reader resumes after each replacement', async () => {
  const path = await destination(), writer = await createPetRecorder(path, { maxBuckets: 2 });
  const live = new PetLiveTape(), inputs = [];
  try {
    for (let i = 0; i < 7; i++) {
      const state = { ...empty, sequence: 215_999 + i, simTimeMs: (215_999 + i) * 400, activity: i / 10 };
      inputs.push(state); await writer.append(state);
      const accepted = live.readTail(await readFile(path, 'utf8'));
      if (i % 2 === 0) assert.equal(accepted, undefined, 'New segment establishes a baseline');
      else assert.equal(accepted.activity, state.activity, 'Next append is a new observation');
    }
  } finally { await writer.close(); }
  const paths = [part(path, 1), part(path, 2), part(path, 3), path], replay = [];
  for (const file of paths) {
    const rows = decodePetJSONL(await readFile(file, 'utf8'));
    assert.ok(rows.length <= 2); assert.equal(rows[0].sequence, 0);
    replay.push(...rows);
    if (process.platform !== 'win32') assert.equal((await stat(file)).mode & 0o777, 0o600);
  }
  assert.deepEqual(replay.map(b => b.activity), inputs.map(b => b.activity));
  await assert.rejects(createPetRecorder(path), { code: 'EEXIST' });
  await assert.rejects(writer.append(empty), /closed/);
});

test('segment byte limit counts UTF-8 data and rejects an oversized bucket before changing saved history', async () => {
  const path = await destination(), state = { ...empty, agentIds: ['鯨'.repeat(20)] };
  const bytes = Buffer.byteLength(encodePetJSONL([state, { ...state, sequence: 1, simTimeMs: 400 }]));
  const writer = await createPetRecorder(path, { maxBytes: bytes });
  try {
    for (let i = 0; i < 3; i++) await writer.append(state);
    const saved = await readFile(path);
    await assert.rejects(writer.append({ ...empty, agentIds: ['鯨'.repeat(1000)] }), /byte limit/);
    assert.deepEqual(await readFile(path), saved);
  } finally { await writer.close(); }
  assert.equal(decodePetJSONL(await readFile(part(path, 1), 'utf8')).length, 2);
  assert.equal(decodePetJSONL(await readFile(path, 'utf8')).length, 1);
  assert.equal((await stat(part(path, 1))).size, bytes);
});

test('an archive collision preserves both existing files and removes only the new unpublished temporary file', async () => {
  const path = await destination(), writer = await createPetRecorder(path, { maxBuckets: 1 });
  await writer.append(empty);
  await writeFile(part(path, 1), 'existing archive');
  const before = await readFile(path);
  try { await assert.rejects(writer.append({ ...empty, channel: 'code' }), { code: 'EEXIST' }); }
  finally { await writer.close(); }
  assert.deepEqual(await readFile(path), before);
  assert.equal(await readFile(part(path, 1), 'utf8'), 'existing archive');
  assert.deepEqual((await readdir(join(path, '..'))).sort(), ['pet-state', 'pet-state.segment-000001.jsonl']);
});

test('ordinary appends refuse an externally replaced live path without overwriting either recording', async () => {
  const path = await destination(), writer = await createPetRecorder(path, { maxBuckets: 2 });
  await writer.append(empty);
  const before = await readFile(path);
  await rename(path, `${path}.held`); await writeFile(path, 'external replacement');
  try { await assert.rejects(writer.append(empty), /replaced externally/); }
  finally { await writer.close(); }
  assert.equal(await readFile(path, 'utf8'), 'external replacement');
  assert.deepEqual(await readFile(`${path}.held`), before);
  assert.deepEqual((await readdir(join(path, '..'))).sort(), ['pet-state', 'pet-state.held']);
});

test('the actual watch CLI keeps recording at one pathname through several rotations and exits cleanly', { timeout: 15_000 }, async t => {
  const { once } = await import('node:events');
  const { setTimeout: delay } = await import('node:timers/promises');
  const path = await destination(), input = `${path}.source.json`;
  await writeFile(input, JSON.stringify({ schemaVersion: 1, id: 'old', traceId: 'fixture', name: 'bash', category: 'code', startTime: 0, endTime: 1, attributes: {} }));
  const child = spawnRecorder([`--input=${input}`, `--output=${path}`, '--watch', '--segment-buckets=2']);
  let log = ''; child.stdout.on('data', b => log += b); child.stderr.on('data', b => log += b);
  const exited = once(child, 'exit');
  t.after(() => { if (child.exitCode === null) child.kill('SIGTERM'); });
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline && child.exitCode === null) {
    try { if (decodePetJSONL(await readFile(path, 'utf8')).length >= 1 && (await stat(part(path, 3))).size) break; } catch { /* Wait for the next complete segment. */ }
    await delay(25);
  }
  child.stopRecorder(); const [code] = await exited; assert.equal(code, 0, log);
  for (let i = 1; i <= 3; i++) assert.equal(decodePetJSONL(await readFile(part(path, i), 'utf8')).length, 2);
  assert.ok(decodePetJSONL(await readFile(path, 'utf8')).length >= 1);
  assert.match(log, /Archived pet recording:/);
});

test('a post-publication error preserves correct row accounting and allows the next distinct append', async () => {
  const path = await destination(); let reports = 0;
  const writer = await createPetRecorder(path, { maxBuckets: 1, report: () => { if (++reports === 1) throw new Error('report boom'); } });
  try {
    await writer.append(empty);
    await assert.rejects(writer.append({ ...empty, channel: 'code' }), /report boom/);
    assert.equal(decodePetJSONL(await readFile(path, 'utf8'))[0].channel, 'code', 'The new row was already durably published');
    await writer.append({ ...empty, channel: 'human' });
    assert.equal(decodePetJSONL(await readFile(part(path, 1), 'utf8'))[0].channel, 'other');
    assert.equal(decodePetJSONL(await readFile(part(path, 2), 'utf8'))[0].channel, 'code');
    assert.equal(decodePetJSONL(await readFile(path, 'utf8'))[0].channel, 'human');
  } finally { await writer.close(); }
});
