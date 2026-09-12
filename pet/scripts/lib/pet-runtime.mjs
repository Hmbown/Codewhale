import { setTimeout as delay } from 'node:timers/promises';
import { importTrace } from '../../dist/core/ingest.js';
import { isCodewhaleRuntimeRecord, observeRuntimeRequests } from '../../dist/core/codewhale.js';

/** A read-only transport for the existing Runtime journal. All event meaning
 * remains in Whalesong's importer and canonical pet bucketer. No raw journal,
 * prompt, tool argument or bearer token is written into the pet recording. */
export async function followRuntime({ baseUrl, threadId, token, report = () => {} }) {
  const url = new URL(baseUrl);
  if (url.protocol !== 'http:' || !['127.0.0.1', '[::1]'].includes(url.hostname)
    || url.username || url.password || url.pathname !== '/' || url.search || url.hash)
    throw new Error('Pet Runtime input requires a plain HTTP loopback IP origin, without credentials or a path.');
  if (typeof threadId !== 'string' || !threadId.trim() || threadId.length > 512)
    throw new Error('Choose one Runtime --thread ID.');
  let sdk;
  try { sdk = await import('@codewhale/runtime-sdk'); }
  catch { sdk = await import('../../../npm/runtime-sdk/index.js'); }
  if (typeof sdk.CodeWhaleRuntimeClient.prototype.threadEvents !== 'function')
    throw new Error('The local Runtime SDK needs threadEvents support.');
  const client = new sdk.CodeWhaleRuntimeClient({ baseUrl: url.href, token });
  const shutdown = new AbortController();
  const records = [];
  let cursor = 0, bytes = 0, revision = 0, connected = false, fatal = false;
  let cachedRevision = -1, cachedTrace;
  const done = (async () => {
    let backoff = 250;
    while (!shutdown.signal.aborted && !fatal) {
      // Fifteen-second server heartbeats make a silent, half-open connection
      // distinguishable from an idle journal. The timeout is driver time only.
      const attempt = new AbortController();
      const signal = AbortSignal.any([shutdown.signal, attempt.signal]);
      let idleTimer;
      const refresh = () => { clearTimeout(idleTimer); idleTimer = setTimeout(() => attempt.abort(), 45_000); };
      refresh();
      const fetchImpl = client.fetchImpl;
      client.fetchImpl = async (input, init) => {
        const response = await fetchImpl(input, init);
        if (!response.body || !response.ok) return response;
        const body = response.body.pipeThrough(new TransformStream({ transform(chunk, controller) { refresh(); controller.enqueue(chunk); } }));
        return new Response(body, { status: response.status, headers: response.headers });
      };
      try {
        for await (const record of client.threadEvents(threadId, { sinceSeq: cursor, signal })) {
          if (!isCodewhaleRuntimeRecord(record) || record.thread_id !== threadId || !Number.isSafeInteger(record.seq) || record.seq < 0)
            throw new Error('Invalid Runtime envelope.');
          if (record.seq <= cursor) continue;
          // Sequence numbers belong to Runtime, and need not be consecutive.
          // Its predecessor cursor detects loss without inventing a new counter.
          if (record.previous_seq !== undefined && record.previous_seq !== cursor)
            throw new Error('Runtime predecessor cursor does not match.');
          if (record.event !== 'item.delta') {
            const size = Buffer.byteLength(JSON.stringify(record));
            if (records.length >= 250_000 || bytes + size > 64 * 1024 * 1024) {
              fatal = true; throw new Error('Runtime recording reached its input limit.');
            }
            records.push(record); bytes += size; revision++;
          }
          cursor = record.seq; connected = true; backoff = 250;
        }
      } catch (error) {
        if ([400, 401, 403, 404, 405, 501].includes(error.status)) fatal = true;
        if (!shutdown.signal.aborted) report(fatal
          ? 'Runtime input stopped: check the thread, authentication, SDK support, or recording size. Recording remains unobserved.'
          : 'Runtime input interrupted; recording unobserved gaps while reconnecting from the last cursor.');
      } finally {
        connected = false; clearTimeout(idleTimer); client.fetchImpl = fetchImpl;
      }
      if (!shutdown.signal.aborted && !fatal) {
        await delay(backoff, undefined, { signal: shutdown.signal }).catch(() => {});
        backoff = Math.min(8000, backoff * 2);
      }
    }
  })();
  return {
    get connected() { return connected; },
    get revision() { return revision; },
    get cursor() { return cursor; },
    snapshot(observedThrough = Date.now()) {
      if (!connected || !records.length) return undefined;
      if (cachedRevision !== revision) {
        // Reuse the same validated, metadata-only import path as file replay.
        cachedTrace = importTrace(JSON.stringify(records), 'Codewhale Runtime', { privacy: 'metadata' })[0];
        cachedRevision = revision;
      }
      return observeRuntimeRequests(cachedTrace, observedThrough);
    },
    async close() { shutdown.abort(); await done; },
  };
}
