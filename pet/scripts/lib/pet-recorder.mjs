import { open, link, rename, unlink, lstat } from 'node:fs/promises';
import { dirname, basename, resolve } from 'node:path';
import { randomUUID } from 'node:crypto';
import { PET_BIN_MS, validatePetBucket, encodePetJSONL } from '../../dist/core/pet-telemetry.js';

async function syncDirectory(path) {
  let directory;
  try { directory = await open(path, 'r'); await directory.sync(); }
  catch (error) {
    // Node cannot open/sync directory handles on Windows. File data is still
    // synced before its atomic replacement on that platform.
    if (process.platform !== 'win32' || !['EPERM', 'EISDIR', 'EINVAL', 'ENOTSUP'].includes(error.code)) throw error;
  } finally { await directory?.close(); }
}

/** Replaces the CLI's unbounded append-only output. Each complete segment is
 * replayable on its own; the same live pathname always holds the newest one. */
export async function createPetRecorder(path, { maxBuckets = 216_000, maxBytes = 64 * 1024 * 1024, report = () => {} } = {}) {
  if (!Number.isSafeInteger(maxBuckets) || maxBuckets < 1 || maxBuckets > 216_000
    || !Number.isSafeInteger(maxBytes) || maxBytes < 1 || maxBytes > 64 * 1024 * 1024)
    throw new Error('Invalid pet recording segment limit.');
  path = resolve(path);
  let output = await open(path, 'wx', 0o600), sequence = 0, bytes = 0, segment = 0, busy = false;
  return {
    async append(bucket) {
      if (!output || busy) throw new Error('Pet recorder is closed or already writing.');
      validatePetBucket(bucket);
      busy = true;
      try {
        const held = await output.stat({ bigint: true }), current = await lstat(path, { bigint: true });
        if (!current.isFile() || held.dev !== current.dev || held.ino !== current.ino || held.size !== BigInt(bytes))
          throw new Error('The live pet recording was changed or replaced externally; existing files were preserved.');
        const encode = seq => encodePetJSONL([{ ...bucket, sequence: seq, simTimeMs: seq * PET_BIN_MS }]);
        let row = encode(sequence), size = Buffer.byteLength(row);
        if (sequence >= maxBuckets || bytes + size > maxBytes) {
          row = encode(0); size = Buffer.byteLength(row);
          if (size > maxBytes) throw new Error('Pet bucket exceeds the recording segment byte limit.');
          const temporary = resolve(dirname(path), `.${basename(path)}.next-${randomUUID()}`);
          const archive = `${path}.segment-${String(segment + 1).padStart(6, '0')}.jsonl`;
          let next, installed = false, created = false;
          try {
            next = await open(temporary, 'wx', 0o600); created = true;
            await next.writeFile(row); await next.sync(); await output.sync();
            // link is exclusive: a collision never replaces someone else's
            // archive. Persist this name before replacing the live pathname.
            await link(path, archive); await syncDirectory(dirname(path));
            await rename(temporary, path); installed = true;
            const previous = output; output = next; next = undefined;
            sequence = 0; bytes = 0; segment++;
            await previous.close(); await syncDirectory(dirname(path));
            report(`Archived pet recording: ${archive}`);
          } finally {
            await next?.close();
            if (created && !installed) await unlink(temporary);
          }
        } else {
          await output.writeFile(row);
        }
        bytes += size; sequence++;
      } finally { busy = false; }
    },
    async close() {
      if (busy) throw new Error('Wait for the pet recorder write before closing.');
      if (!output) return;
      const current = output; output = undefined;
      try { await current.sync(); } finally { await current.close(); }
    },
  };
}
