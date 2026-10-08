import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { getEventListeners } from "node:events";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import path from "node:path";

import { ThreadStore } from "../../bridge-core/src/lib.mjs";

import {
  activeTurnBlock,
  apiPost,
  commandAction,
  extractText,
  MessageItemType,
  parseBool,
  parseCommand,
  parseList,
  preservedChatStateFields,
  processUpdateBatch,
  sendMessage,
  splitMessage
} from "../src/lib.mjs";

test("extractText reads text and voice transcript items", () => {
  assert.equal(
    extractText([{ type: MessageItemType.TEXT, text_item: { text: "hello" } }]),
    "hello"
  );
  assert.equal(
    extractText([{ type: MessageItemType.VOICE, voice_item: { text: "voice text" } }]),
    "voice text"
  );
});

test("shared command helpers preserve Weixin bridge command behavior", () => {
  assert.deepEqual(parseList("u1, u2 ,, "), ["u1", "u2"]);
  assert.equal(parseBool("yes"), true);
  assert.deepEqual(parseCommand("/allow ap_1 remember"), {
    name: "allow",
    args: "ap_1 remember"
  });
  assert.deepEqual(commandAction(parseCommand("/model auto")), {
    kind: "set_model",
    modelName: "auto"
  });
  assert.deepEqual(commandAction(parseCommand("/unknown value")), {
    kind: "prompt",
    prompt: "/unknown value"
  });
});

test("shared state and runtime helpers preserve Weixin bridge behavior", () => {
  assert.deepEqual(preservedChatStateFields({ model: "m", activeTurnId: "turn-1" }), {
    model: "m"
  });
  assert.deepEqual(splitMessage("a🧪b", 2), ["a🧪", "b"]);
  assert.deepEqual(activeTurnBlock({ turns: [{ id: "turn-1", status: "in_progress" }] }), {
    turnId: "turn-1",
    message: "Thread already has active turn turn-1. Wait for it to finish or send /interrupt."
  });
});

test("a crash mid-batch replays the batch without losing or re-running prompts", async () => {
  const dir = await mkdtemp(path.join(tmpdir(), "codewhale-weixin-bridge-"));
  try {
    const statePath = path.join(dir, "thread-map.json");
    const batch = [1, 2, 3].map((id) => ({ from_user_id: "u", message_id: id }));
    const keyOf = (msg) => `${msg.from_user_id}:${msg.message_id}`;
    const handled = [];
    const cursors = [];
    const crash = new Error("process killed");
    const first = await ThreadStore.open(statePath, { messageLimit: 50 });
    let afterCrash = null;
    await assert.rejects(processUpdateBatch({
      messages: batch, nextCursor: "buf-2", store: first, keyOf,
      handle: async (msg) => {
        if (msg.message_id === 2) {
          throw crash;
        }
        handled.push(msg.message_id);
      },
      interrupted: async () => assert.fail("nothing is interrupted on the first pass"),
      commitCursor: async (buf) => cursors.push(buf),
    }), crash);
    afterCrash = await ThreadStore.open(statePath, { messageLimit: 50 });
    assert.ok(afterCrash.data.inflight["u:2"], "a failed handler must remain inflight on disk after rejection");
    assert.deepEqual(cursors, [], "the cursor does not move past unhandled messages");

    const reported = [];
    await processUpdateBatch({
      messages: batch, nextCursor: "buf-2", store: afterCrash, keyOf,
      handle: async (msg) => handled.push(msg.message_id),
      interrupted: async (msg) => reported.push(msg.message_id),
      commitCursor: async (buf) => cursors.push(buf),
    });
    assert.deepEqual(handled, [1, 3], "message 1 is not re-run; message 3 is not lost");
    assert.deepEqual(reported, [2], "the in-flight message is reported, not re-run");
    assert.deepEqual(cursors, ["buf-2"]);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("sendMessage checks application responses from a local HTTP server", async (t) => {
  let responseBody;
  let status = 200;
  const requests = [];
  const server = createServer((req, res) => {
    requests.push({ method: req.method, url: req.url });
    req.resume();
    res.writeHead(status, { "Content-Type": "application/json", Connection: "close" });
    res.end(responseBody);
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  t.after(() => new Promise((resolve) => server.close(resolve)));
  const params = {
    baseUrl: `http://127.0.0.1:${server.address().port}`,
    token: "test-token",
    body: { msg: { to_user_id: "test-user", context_token: "test-context" } },
  };
  const cases = [
    { name: "ret zero is API acceptance", body: '{"ret":0}', accepted: { ret: 0 } },
    { name: "errcode zero is API acceptance", body: '{"errcode":0}', accepted: { errcode: 0 } },
    { name: "omitted status matches Tencent reference compatibility", body: '{}', accepted: {} },
    { name: "HTTP 200 with nonzero ret is rejected", body: '{"ret":-1,"errmsg":"response-secret"}', outcome: "rejected", message: /ret=-1/ },
    { name: "HTTP 200 with nonzero errcode is rejected", body: '{"ret":0,"errcode":-14,"errmsg":"response-secret"}', outcome: "rejected", message: /errcode=-14/ },
    { name: "malformed JSON is uncertain", body: '<html>response-secret</html>', outcome: "uncertain", message: /invalid JSON/ },
    { name: "empty response is uncertain", body: '', outcome: "uncertain", message: /invalid JSON/ },
    { name: "null response is uncertain", body: 'null', outcome: "uncertain", message: /non-object/ },
    { name: "array response is uncertain", body: '[]', outcome: "uncertain", message: /non-object/ },
    { name: "string response is uncertain", body: '"response-secret"', outcome: "uncertain", message: /non-object/ },
    { name: "string status is uncertain", body: '{"ret":"response-secret"}', outcome: "uncertain", message: /invalid status/ },
    { name: "null status is uncertain", body: '{"ret":null}', outcome: "uncertain", message: /invalid status/ },
    { name: "boolean errcode is uncertain", body: '{"ret":0,"errcode":false}', outcome: "uncertain", message: /invalid status/ },
    { name: "HTTP failure is uncertain and does not expose its body", status: 503, body: 'response-secret', outcome: "uncertain", message: /HTTP 503/ },
  ];
  for (const row of cases) {
    await t.test(row.name, async () => {
      responseBody = row.body;
      status = row.status ?? 200;
      if (Object.hasOwn(row, "accepted")) {
        assert.deepEqual(await sendMessage(params), row.accepted);
      } else {
        await assert.rejects(sendMessage(params), (error) => {
          assert.equal(error.deliveryStatus, row.outcome);
          assert.match(error.message, row.message);
          assert.doesNotMatch(error.message, /response-secret/);
          return true;
        });
      }
    });
  }
  assert.equal(requests.length, cases.length);
  assert.ok(requests.every((req) => req.method === "POST" && req.url === "/ilink/bot/sendmessage"));
});

test("sendMessage preserves network errors and safely handles non-Error failures", async (t) => {
  let failure = new Error("network unavailable");
  t.mock.method(globalThis, "fetch", async () => { throw failure; });
  const params = { baseUrl: "http://localhost", body: { msg: {} } };
  await assert.rejects(sendMessage(params), (error) => error === failure && error.deliveryStatus === "uncertain");
  failure = "response-secret";
  await assert.rejects(sendMessage(params), (error) => {
    assert.equal(error.deliveryStatus, "uncertain");
    assert.equal(error.message, "iLink sendMessage transport failed");
    return true;
  });
});

test("apiPost does not start a request for an already-aborted signal", async (t) => {
  const controller = new AbortController();
  const reason = new Error("bridge stopped");
  controller.abort(reason);
  t.mock.method(globalThis, "fetch", () => assert.fail("aborted request must not reach fetch"));
  await assert.rejects(apiPost({ baseUrl: "http://localhost", endpoint: "test", body: "{}", signal: controller.signal }), reason);
  assert.equal(getEventListeners(controller.signal, "abort").length, 0);
});

test("apiPost removes external abort listeners after success and failure", async (t) => {
  const controller = new AbortController();
  let fail = false;
  t.mock.method(globalThis, "fetch", async () => {
    assert.equal(getEventListeners(controller.signal, "abort").length, 1);
    if (fail) throw new Error("network unavailable");
    return new Response("{}");
  });
  const params = { baseUrl: "http://localhost", endpoint: "test", body: "{}", signal: controller.signal };
  assert.equal(await apiPost(params), "{}");
  assert.equal(getEventListeners(controller.signal, "abort").length, 0);
  fail = true;
  await assert.rejects(apiPost(params), /network unavailable/);
  assert.equal(getEventListeners(controller.signal, "abort").length, 0);
});

test("apiPost propagates in-flight cancellation and clears its external listener", async (t) => {
  const controller = new AbortController();
  let requestSignal;
  t.mock.method(globalThis, "fetch", (_url, options) => {
    requestSignal = options.signal;
    return new Promise((_resolve, reject) => {
      requestSignal.addEventListener("abort", () => reject(requestSignal.reason), { once: true });
    });
  });
  const pending = apiPost({ baseUrl: "http://localhost", endpoint: "test", body: "{}", signal: controller.signal });
  const reason = new Error("bridge stopped");
  controller.abort(reason);
  await assert.rejects(pending, reason);
  assert.equal(requestSignal.aborted, true);
  assert.equal(getEventListeners(controller.signal, "abort").length, 0);
});
