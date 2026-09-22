import assert from "node:assert/strict";
import test from "node:test";

import { SseParser, parseFrame } from "../src/sse.js";

test("a complete frame yields the runtime envelope", () => {
  const event = parseFrame('data: {"seq":4,"event":"item.delta","item_id":"i1","payload":{"delta":"hi"}}');
  assert.deepEqual(event, {
    seq: 4,
    previousSeq: undefined,
    event: "item.delta",
    threadId: undefined,
    turnId: undefined,
    itemId: "i1",
    payload: { delta: "hi" },
  });
});

test("an event split across network chunks is reassembled once, not twice", () => {
  const parser = new SseParser();
  assert.deepEqual(parser.push('data: {"seq":1,"eve'), []);
  assert.deepEqual(parser.push('nt":"turn.completed"}'), []);
  const events = parser.push("\n\n");
  assert.equal(events.length, 1);
  assert.equal(events[0].event, "turn.completed");
});

test("CRLF and LF frame boundaries both terminate a frame", () => {
  const parser = new SseParser();
  const events = parser.push(
    'data: {"seq":1,"event":"a"}\r\n\r\ndata: {"seq":2,"event":"b"}\n\n',
  );
  assert.deepEqual(
    events.map((event) => event.event),
    ["a", "b"],
  );
});

test("heartbeats, comments, and unparseable frames yield nothing", () => {
  const parser = new SseParser();
  assert.deepEqual(parser.push(": keep-alive\n\n"), []);
  assert.deepEqual(parser.push("\n\n"), []);
  assert.deepEqual(parser.push("data: not json\n\n"), []);
  assert.deepEqual(parser.push('data: {"event":"no-seq"}\n\n'), []);
  assert.deepEqual(parser.push('data: {"seq":9}\n\n'), []);
});

test("multi-line data fields join with newlines before parsing", () => {
  const event = parseFrame('data: {"seq":2,\ndata: "event":"item.completed"}');
  assert.equal(event.seq, 2);
  assert.equal(event.event, "item.completed");
});

test("previous_seq travels so a client can tell that replay skipped history", () => {
  const event = parseFrame('data: {"seq":90,"previous_seq":12,"event":"item.started"}');
  assert.equal(event.previousSeq, 12);
});
