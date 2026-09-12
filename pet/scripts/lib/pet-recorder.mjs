import { open, link, rename, unlink, lstat, realpath, opendir } from 'node:fs/promises';
import { constants } from 'node:fs';
import { DatabaseSync } from 'node:sqlite';
import { dirname, basename, resolve } from 'node:path';
import { randomUUID } from 'node:crypto';
import { setTimeout as delay } from 'node:timers/promises';
import { PET_BIN_MS, validatePetBucket, encodePetJSONL, decodePetJSONL } from '../../dist/core/pet-telemetry.js';

async function syncDirectory(path) {
  let directory;
  try { directory = await open(path, 'r'); await directory.sync(); }
  catch (error) {
    // Node cannot open/sync directory handles on Windows. File data is still
    // synced before its atomic replacement on that platform.
    if (process.platform !== 'win32' || !['EPERM', 'EISDIR', 'EINVAL', 'ENOTSUP'].includes(error.code)) throw error;
  } finally { await directory?.close(); }
}

// SQLite's OS lock is released even after process death. This empty sidecar
// contains no events or recorder state; keep its pathname so later processes
// coordinate on the same inode. No PID files or stale-lock deletion are needed.
async function lockRecorder(path) {
  const name = `${path}.writer-lock`;
  try { const created = await open(name, 'wx', 0o600); await created.close(); }
  catch (error) { if (error.code !== 'EEXIST') throw error; }
  const identity = await lstat(name, { bigint: true });
  if (!identity.isFile() || identity.nlink !== 1n || identity.size !== 0n)
    throw new Error('Invalid pet recorder lock; existing files were preserved.');
  let database;
  const check = async () => {
    const current = await lstat(name, { bigint: true });
    if (!current.isFile() || current.nlink !== 1n || current.size !== 0n
      || current.dev !== identity.dev || current.ino !== identity.ino)
      throw new Error('The pet recorder lock was replaced; existing files were preserved.');
  };
  try {
    database = new DatabaseSync(name);
    database.exec('PRAGMA busy_timeout = 0; BEGIN EXCLUSIVE');
    await check();
    return { check, close: () => { database.close(); } };
  } catch (error) {
    database?.close();
    if (error.errcode === 5 || error.errcode === 6) throw new Error('Another pet recorder is using this output.');
    throw error;
  }
}

/** Replaces the CLI's unbounded append-only output. Each complete segment is
 * replayable on its own; the same live pathname always holds the newest one. */
export async function createPetRecorder(path, { maxBuckets = 216_000, maxBytes = 64 * 1024 * 1024, report = () => {}, resume = false } = {}) {
  if (!Number.isSafeInteger(maxBuckets) || maxBuckets < 1 || maxBuckets > 216_000
    || !Number.isSafeInteger(maxBytes) || maxBytes < 1 || maxBytes > 64 * 1024 * 1024)
    throw new Error('Invalid pet recording segment limit.');
  path = resolve(await realpath(dirname(resolve(path))), basename(path));
  let lock = await lockRecorder(path), output, sequence = 0, bytes = 0, segment = 0, busy = false, restart = false, expectedMtime;
  try {
    try { output = await open(path, 'wx', 0o600); }
    catch (error) {
      if (!resume || error.code !== 'EEXIST') throw error;
      const original = await lstat(path, { bigint: true });
      if (!original.isFile() || original.size > 64n * 1024n * 1024n)
        throw new Error('The previous pet recording is not a bounded regular file; it was preserved.');
      output = await open(path, constants.O_RDWR | constants.O_APPEND | constants.O_NOFOLLOW | constants.O_NONBLOCK);
      const held = await output.stat({ bigint: true });
      if (held.dev !== original.dev || held.ino !== original.ino || held.size !== original.size)
        throw new Error('The previous pet recording changed while opening; it was preserved.');
      // Read at most the size already checked, including a single growth byte.
      const contents = Buffer.alloc(Number(held.size) + 1);
      let length = 0;
      while (length < contents.length) {
        const { bytesRead } = await output.read(contents, length, contents.length - length, length);
        if (!bytesRead) break;
        length += bytesRead;
      }
      if (length !== Number(held.size)) throw new Error('The previous pet recording changed while reading; it was preserved.');
      const text = new TextDecoder('utf-8', { fatal: true }).decode(contents.subarray(0, length));
      if (text && !text.endsWith('\n')) throw new Error('The previous pet recording has an incomplete final row; it was preserved.');
      decodePetJSONL(text);
      const unchanged = await output.stat({ bigint: true });
      if (unchanged.size !== original.size || unchanged.mtimeNs !== original.mtimeNs)
        throw new Error('The previous pet recording changed while reading; it was preserved.');
      bytes = length; restart = true; expectedMtime = original.mtimeNs;
      // Continue archive numbering without collecting a growing directory list.
      const prefix = `${basename(path)}.segment-`;
      for await (const entry of await opendir(dirname(path))) {
        if (!entry.name.startsWith(prefix)) continue;
        const suffix = entry.name.slice(prefix.length);
        if (!/^[0-9]{6,}\.jsonl$/.test(suffix)) continue;
        const number = Number(suffix.slice(0, -6));
        if (!Number.isSafeInteger(number) || number >= Number.MAX_SAFE_INTEGER)
          throw new Error('Pet archive numbering is exhausted; existing files were preserved.');
        segment = Math.max(segment, number);
      }
    }
    expectedMtime ??= (await output.stat({ bigint: true })).mtimeNs;
  } catch (error) { try { await output?.close(); } finally { lock.close(); } throw error; }
  return {
    async append(bucket) {
      if (!output || busy) throw new Error('Pet recorder is closed or already writing.');
      validatePetBucket(bucket);
      busy = true;
      try {
        await lock.check();
        const held = await output.stat({ bigint: true }), current = await lstat(path, { bigint: true });
        if (!current.isFile() || held.dev !== current.dev || held.ino !== current.ino || held.size !== BigInt(bytes) || held.mtimeNs !== expectedMtime)
          throw new Error('The live pet recording was changed or replaced externally; existing files were preserved.');
        const encode = seq => encodePetJSONL([{ ...bucket, sequence: seq, simTimeMs: seq * PET_BIN_MS }]);
        let row = encode(sequence), size = Buffer.byteLength(row);
        if (restart || sequence >= maxBuckets || bytes + size > maxBytes) {
          row = encode(0); size = Buffer.byteLength(row);
          if (size > maxBytes) throw new Error('Pet bucket exceeds the recording segment byte limit.');
          const temporary = resolve(dirname(path), `.${basename(path)}.next-${randomUUID()}`);
          const archive = `${path}.segment-${String(segment + 1).padStart(6, '0')}.jsonl`;
          let next, installed = false, created = false;
          try {
            next = await open(temporary, 'wx', 0o600); created = true;
            await next.writeFile(row); await next.sync();
            const nextIdentity = await next.stat({ bigint: true });
            await next.close(); next = undefined;
            await output.sync();
            // link is exclusive: a collision never replaces someone else's
            // archive. Persist this name before replacing the live pathname.
            await link(path, archive); await syncDirectory(dirname(path));
            // Windows can reject replacement while either writer handle is
            // open. Both files are synced and the old file is archived first.
            await output.close(); output = undefined;
            for (let attempt = 0; ; attempt++) {
              await lock.check();
              const destination = await lstat(path, { bigint: true });
              if (!destination.isFile() || destination.dev !== held.dev || destination.ino !== held.ino
                || destination.size !== held.size || destination.mtimeNs !== held.mtimeNs)
                throw new Error('The live pet recording changed while rotating; existing files were preserved.');
              try { await rename(temporary, path); break; }
              catch (error) {
                // A reader or file scanner can briefly deny replacement on
                // Windows. Retry for under two seconds, checking identity each
                // time; persistent denial still stops without deleting history.
                if (process.platform !== 'win32' || !['EPERM', 'EBUSY'].includes(error.code) || attempt >= 20) throw error;
                await delay(Math.min(100, (attempt + 1) * 25));
              }
            }
            installed = true;
            // The first row is already published. Account for it before any
            // fallible cleanup/report so a later append sees the actual file.
            sequence = 1; bytes = size; segment++; restart = false;
            output = await open(path, constants.O_WRONLY | constants.O_APPEND);
            const reopened = await output.stat({ bigint: true });
            if (reopened.dev !== nextIdentity.dev || reopened.ino !== nextIdentity.ino)
              throw new Error('The live pet recording was replaced externally after rotation.');
            expectedMtime = reopened.mtimeNs;
            await syncDirectory(dirname(path));
            report(`Archived pet recording: ${archive}`);
          } finally {
            await next?.close();
            if (created && !installed) await unlink(temporary);
          }
          return;
        } else {
          await output.writeFile(row);
          expectedMtime = (await output.stat({ bigint: true })).mtimeNs;
        }
        bytes += size; sequence++;
      } finally { busy = false; }
    },
    async close() {
      if (busy) throw new Error('Wait for the pet recorder write before closing.');
      const current = output, heldLock = lock; output = undefined; lock = undefined;
      try { if (current) { try { await current.sync(); } finally { await current.close(); } } }
      finally { heldLock?.close(); }
    },
  };
}
