import { setTimeout as delay } from 'node:timers/promises';
import { privacyEvent, redact } from '../../dist/core/ingest.js';
import { CodewhaleRuntimeTrace, isCodewhaleRuntimeRecord, observeRuntimeRequests } from '../../dist/core/codewhale.js';

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
  const trace = new CodewhaleRuntimeTrace('Codewhale Runtime', 250_000,
    event => privacyEvent(event, 'metadata'), 64 * 1024 * 1024);
  let cursor = 0, revision = 0, connected = false, fatal = false;
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
        // Also reject an older installed SDK that silently omits the requested
        // progress option; it must not turn historical packets into live state.
        if (response.headers.get('x-codewhale-event-progress') !== '1') {
          await response.body.cancel();
          const error = new Error('Runtime replay progress is unavailable.'); error.status = 501; throw error;
        }
        // Cancel the wrapped pipeline too: the original Response can be collected
        // while its idle body is still being read through the replacement below.
        const body = response.body.pipeThrough(new TransformStream({ transform(chunk, controller) { refresh(); controller.enqueue(chunk); } }), { signal });
        return new Response(body, { status: response.status, headers: response.headers });
      };
      try {
        for await (const record of client.threadEvents(threadId, { sinceSeq: cursor, signal, includeProgress: true })) {
          if (record?.event === 'stream.progress') {
            if (record.thread_id !== threadId || record.seq !== cursor || !['live', 'replaying'].includes(record.state))
              throw new Error('Invalid Runtime replay progress.');
            connected = record.state === 'live';
            continue;
          }
          if (!isCodewhaleRuntimeRecord(record) || record.thread_id !== threadId || !Number.isSafeInteger(record.seq) || record.seq < 0)
            throw new Error('Invalid Runtime envelope.');
          if (record.seq <= cursor) continue;
          // Sequence numbers belong to Runtime, and need not be consecutive.
          // Its predecessor cursor detects loss without inventing a new counter.
          if (record.previous_seq !== undefined && record.previous_seq !== cursor)
            throw new Error('Runtime predecessor cursor does not match.');
          if (record.event !== 'item.delta') {
            // The existing importer retains unfinished lifetimes and a recent
            // recurrence window, not a second copy of the entire raw journal.
            if (revision % 256 === 0) trace.prune(Date.now() - 16_000);
            try { trace.append([redact(record)]); }
            catch (error) { fatal = true; throw error; }
            revision++;
          }
          cursor = record.seq; backoff = 250;
        }
      } catch (error) {
        if ([400, 401, 403, 404, 405, 501].includes(error.status)) fatal = true;
        if (!shutdown.signal.aborted) report(fatal
          ? 'Runtime input stopped: check the thread, authentication, SDK/Runtime replay-progress support, or retained input limit. Recording remains unobserved.'
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
    get retainedEvents() { return trace.retainedEvents; },
    get retainedBytes() { return trace.retainedBytes; },
    snapshot(observedThrough = Date.now()) {
      if (!connected) return undefined;
      trace.prune(observedThrough - 16_000);
      if (!trace.retainedEvents) return undefined;
      return observeRuntimeRequests({ ...trace.snapshot(), privacy: 'metadata' }, observedThrough);
    },
    async close() { shutdown.abort(); await done; },
  };
}
