export class BodyReadError extends Error {
  constructor(readonly status: 400 | 413, message: string) {
    super(message);
    this.name = "BodyReadError";
  }
}

/** Count raw bytes while reading; Content-Length is only an early rejection. */
export async function readBoundedBody(request: { body: ReadableStream<Uint8Array> | null; headers: Headers }, maxBytes: number): Promise<Uint8Array> {
  const reject = async (status: 400 | 413, message: string): Promise<never> => {
    try { await request.body?.cancel(message); } catch { /* Keep the original rejection. */ }
    throw new BodyReadError(status, message);
  };
  const rawLength = request.headers.get("content-length");
  if (rawLength !== null) {
    if (!/^\d+$/.test(rawLength)) return reject(400, "invalid Content-Length");
    if (Number(rawLength) > maxBytes) return reject(413, "payload too large");
  }
  if (!request.body) return new Uint8Array();

  const reader = request.body.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      total += value.byteLength;
      if (total > maxBytes) throw new BodyReadError(413, "payload too large");
      chunks.push(value);
    }
  } catch (cause) {
    try { await reader.cancel("body rejected"); } catch { /* Preserve the read/size error. */ }
    throw cause instanceof BodyReadError ? cause : new BodyReadError(400, "body read failed");
  } finally {
    reader.releaseLock();
  }
  const bytes = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return bytes;
}

/** Budget for background (cron) reads of other hosts. */
export interface OutboundBudget {
  /** Deadline for the whole exchange, body included. */
  timeoutMs?: number;
  /** Largest body read; a larger one is an error, never a silent truncation. */
  maxBytes?: number;
}

export const OUTBOUND_TIMEOUT_MS = 15_000;
export const OUTBOUND_MAX_BYTES = 2 * 1024 * 1024;

/**
 * One outbound GET-style read under a deadline and a byte cap. A slow or
 * endless peer cannot hold a cron invocation until the platform kills it, and
 * a huge body cannot exhaust isolate memory. Non-2xx answers return
 * `{ ok: false }` with the body discarded; transport, timeout and size
 * failures throw, so callers keep their own fallbacks.
 */
export async function fetchBoundedText(
  url: string,
  init: RequestInit = {},
  { timeoutMs = OUTBOUND_TIMEOUT_MS, maxBytes = OUTBOUND_MAX_BYTES }: OutboundBudget = {},
): Promise<{ ok: boolean; status: number; text: string }> {
  const response = await fetch(url, { ...init, signal: AbortSignal.timeout(timeoutMs) });
  if (!response.ok) {
    try { await response.body?.cancel(); } catch { /* The status is the answer. */ }
    return { ok: false, status: response.status, text: "" };
  }
  const bytes = await readBoundedBody(response, maxBytes);
  return { ok: true, status: response.status, text: new TextDecoder().decode(bytes) };
}
