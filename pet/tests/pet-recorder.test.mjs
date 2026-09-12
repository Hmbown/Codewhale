import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, writeFile, stat, readdir, rename, symlink, utimes } from 'node:fs/promises';
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
  assert.deepEqual((await readdir(join(path, '..'))).sort(), ['pet-state', 'pet-state.segment-000001.jsonl', 'pet-state.writer-lock']);
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
  assert.deepEqual((await readdir(join(path, '..'))).sort(), ['pet-state', 'pet-state.held', 'pet-state.writer-lock']);
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


test('resume preserves previous bytes, continues archive numbering, and the follower accepts only fresh appends', async () => {
  const path = await destination(), before = encodePetJSONL([empty, { ...empty, sequence: 1, simTimeMs: 400, channel: 'human', waiting: true }]);
  await writeFile(path, before);
  await writeFile(part(path, 9), 'earlier archive retained');
  const live = new PetLiveTape(); live.readTail(before);
  const writer = await createPetRecorder(path, { resume: true, maxBuckets: 2 });
  try {
    await writer.append(empty);
    assert.equal(live.readTail(await readFile(path, 'utf8')), undefined, 'Restart establishes a baseline without replaying the old request');
    assert.equal(await readFile(part(path, 10), 'utf8'), before);
    await writer.append({ ...empty, channel: 'code', observed: 1 });
    assert.equal(live.readTail(await readFile(path, 'utf8')).channel, 'code');
    await writer.append(empty);
    assert.equal(decodePetJSONL(await readFile(part(path, 11), 'utf8')).length, 2);
    assert.equal(await readFile(part(path, 9), 'utf8'), 'earlier archive retained');
  } finally { await writer.close(); }
  const again = await createPetRecorder(path, { resume: true });
  try { await again.append(empty); } finally { await again.close(); }
  assert.equal(decodePetJSONL(await readFile(part(path, 12), 'utf8')).length, 1);
});

test('a resumed recorder rejects malformed, incomplete, and non-file sources without changing them', async () => {
  for (const text of ['not a pet tape\n', encodePetJSONL([empty]).trimEnd(), encodePetJSONL([{ ...empty, sequence: 2, simTimeMs: 800 }])]) {
    const path = await destination(); await writeFile(path, text);
    await assert.rejects(createPetRecorder(path, { resume: true }));
    assert.equal(await readFile(path, 'utf8'), text);
    assert.deepEqual((await readdir(join(path, '..'))).sort(), ['pet-state', 'pet-state.writer-lock']);
    // Failed validation must release the lock, too.
    await writeFile(path, encodePetJSONL([empty]));
    const writer = await createPetRecorder(path, { resume: true }); await writer.close();
  }
  const path = await destination();
  const { mkdir } = await import('node:fs/promises');
  await mkdir(path);
  await assert.rejects(createPetRecorder(path, { resume: true }));
  if (process.platform !== 'win32') {
    const linked = await destination(), target = `${linked}.original`;
    await writeFile(target, encodePetJSONL([empty])); await symlink(target, linked);
    await assert.rejects(createPetRecorder(linked, { resume: true }), /regular file/);
    assert.equal(await readFile(target, 'utf8'), encodePetJSONL([empty]));
  }
});

test('the recorder rejects another writer, a replaced writer lock, and same-size external edits', async () => {
  const path = await destination(), writer = await createPetRecorder(path);
  try {
    await writer.append(empty);
    await assert.rejects(createPetRecorder(path, { resume: true }), /Another pet recorder/);
    await writer.append(empty); // Closing the rejected contender must not release this writer's lock.
    await assert.rejects(createPetRecorder(path, { resume: true }), /Another pet recorder/);
    const original = await readFile(path, 'utf8'), rewritten = original.replace('"channel":"other"', '"channel":"human"');
    assert.equal(Buffer.byteLength(rewritten), Buffer.byteLength(original));
    await writeFile(path, rewritten);
    await utimes(path, new Date(0), new Date(0));
    await assert.rejects(writer.append(empty), /changed or replaced externally/);
    assert.equal(await readFile(path, 'utf8'), rewritten);
  } finally { await writer.close(); }
  const other = await destination(), active = await createPetRecorder(other);
  try {
    await active.append(empty);
    try { await rename(`${other}.writer-lock`, `${other}.old-lock`); }
    catch (error) {
      // Some Windows filesystems deny renaming the held SQLite lock outright.
      // In that case the original writer must remain exclusive and usable.
      if (process.platform !== 'win32' || !['EPERM', 'EBUSY', 'EACCES'].includes(error.code)) throw error;
      await assert.rejects(createPetRecorder(other, { resume: true }), /Another pet recorder/);
      await active.append(empty);
      return;
    }
    await writeFile(`${other}.writer-lock`, '');
    await assert.rejects(active.append(empty), /lock was replaced/);
    assert.equal(decodePetJSONL(await readFile(other, 'utf8')).length, 1);
  } finally { await active.close(); }
});

test('the actual CLI resumes after process death at the same path without reclaiming a live writer or deleting history', { timeout: 20_000 }, async t => {
  const { once } = await import('node:events');
  const { setTimeout: delay } = await import('node:timers/promises');
  const path = await destination(), input = `${path}.source.json`;
  await writeFile(input, JSON.stringify({ schemaVersion: 1, id: 'old', traceId: 'fixture', name: 'bash', category: 'code', startTime: 0, endTime: 1, attributes: {} }));
  const args = [`--input=${input}`, `--output=${path}`, '--watch'];
  const launch = extra => {
    const child = spawnRecorder([...args, ...extra]);
    child.log = ''; child.stdout.on('data', b => child.log += b); child.stderr.on('data', b => child.log += b);
    child.exited = once(child, 'exit');
    t.after(() => { if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL'); });
    return child;
  };
  const waitFor = async predicate => {
    const deadline = Date.now() + 7000;
    while (Date.now() < deadline) { try { if (await predicate()) return; } catch { /* Atomic publication or the next tick is pending. */ } await delay(25); }
    assert.fail('Recorder did not reach the requested state');
  };
  const first = launch([]);
  await waitFor(async () => decodePetJSONL(await readFile(path, 'utf8')).length >= 2);
  const rejected = launch(['--resume']);
  assert.equal((await rejected.exited)[0], 1); assert.match(rejected.log, /Another pet recorder/);
  first.kill('SIGKILL'); await first.exited;
  const previous = await readFile(path, 'utf8');
  const second = launch(['--resume']);
  await waitFor(async () => (await stat(part(path, 1))).size && decodePetJSONL(await readFile(path, 'utf8')).length >= 2);
  second.stopRecorder(); assert.equal((await second.exited)[0], 0, second.log);
  assert.equal(await readFile(part(path, 1), 'utf8'), previous);
  const buckets = decodePetJSONL(await readFile(path, 'utf8'));
  assert.equal(buckets[0].observed, 0); assert.equal(buckets[0].waiting, false); assert.equal(buckets[0].errors, 0);
  assert.ok(buckets[0].onsets.every(n => n === 0));
  const third = launch(['--resume']);
  await waitFor(async () => (await stat(part(path, 2))).size && decodePetJSONL(await readFile(path, 'utf8')).length >= 2);
  third.stopRecorder(); assert.equal((await third.exited)[0], 0, third.log);
  assert.equal(decodePetJSONL(await readFile(part(path, 2), 'utf8')).length, buckets.length);
});
