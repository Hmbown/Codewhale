/**
 * `text/event-stream` framing for `GET /v1/threads/{id}/events`.
 *
 * Pure and dependency-free: raw chunks in, runtime events out.
 *
 * This is a deliberate port of `extensions/vscode/src/sse.ts`, which parses the
 * same wire contract. The two are not shared because they consume different
 * transports — that client reads a `node:http` response, this one reads a
 * `fetch` `ReadableStream` in a browser — and a shared module would mean adding
 * a bundler to an extension that otherwise needs no build step at all. If the
 * runtime's envelope changes, both copies change: the fields read here are
 * `seq`, `previous_seq`, `event`, `thread_id`, `turn_id`, `item_id`, and
 * `payload`.
 */

export class SseParser {
  #buffer = "";

  /**
   * Feed one raw chunk; returns the complete events it finished.
   *
   * @param {string} chunk
   * @returns {Array<{seq: number, previousSeq?: number, event: string,
   *   threadId?: string, turnId?: string, itemId?: string, payload: unknown}>}
   */
  push(chunk) {
    this.#buffer += chunk;
    const events = [];
    let boundary = this.#nextBoundary();
    while (boundary !== undefined) {
      const frame = this.#buffer.slice(0, boundary.index);
      this.#buffer = this.#buffer.slice(boundary.index + boundary.length);
      const event = parseFrame(frame);
      if (event) {
        events.push(event);
      }
      boundary = this.#nextBoundary();
    }
    return events;
  }

  #nextBoundary() {
    const lf = this.#buffer.indexOf("\n\n");
    const crlf = this.#buffer.indexOf("\r\n\r\n");
    if (lf === -1 && crlf === -1) {
      return undefined;
    }
    if (crlf === -1 || (lf !== -1 && lf < crlf)) {
      return { index: lf, length: 2 };
    }
    return { index: crlf, length: 4 };
  }
}

/** @param {string} frame */
export function parseFrame(frame) {
  let data = "";
  for (const rawLine of frame.split(/\r?\n/)) {
    if (rawLine === "" || rawLine.startsWith(":")) {
      continue;
    }
    const colon = rawLine.indexOf(":");
    const field = colon === -1 ? rawLine : rawLine.slice(0, colon);
    let value = colon === -1 ? "" : rawLine.slice(colon + 1);
    if (value.startsWith(" ")) {
      value = value.slice(1);
    }
    if (field === "data") {
      data += (data ? "\n" : "") + value;
    }
    // `event`, `id`, and `retry` are ignored: the runtime envelope carries the
    // event name in its own `event` field, which is the stable contract.
  }
  return data ? readEvent(data) : undefined;
}

/** @param {string} data */
function readEvent(data) {
  let body;
  try {
    body = JSON.parse(data);
  } catch {
    return undefined;
  }
  if (!body || typeof body !== "object") {
    return undefined;
  }
  const seq = readNumber(body.seq);
  const event = readString(body.event);
  if (seq === undefined || !event) {
    return undefined;
  }
  return {
    seq,
    previousSeq: readNumber(body.previous_seq),
    event,
    threadId: readString(body.thread_id),
    turnId: readString(body.turn_id),
    itemId: readString(body.item_id),
    payload: body.payload,
  };
}

/** @param {unknown} value */
function readString(value) {
  return typeof value === "string" ? value : undefined;
}

/** @param {unknown} value */
function readNumber(value) {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}
