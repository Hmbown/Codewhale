#!/usr/bin/env node
// codewhale-cu-turn-hold — hold a Sprite awake for exactly one turn.
//
// The Engine spawns this at turn start with a piped stdin and ends it at turn
// end by closing stdin (or SIGTERM). It registers a Sprite Task (5 min
// expiry), refreshes it every 60 s, and deletes it on stdin EOF, SIGTERM,
// SIGINT or SIGHUP. If the Engine dies, stdin closes and the hold is released;
// if this process is SIGKILLed, the Task lapses within its expiry.
// Receipts are JSON lines on stdout.
//
//   codewhale-cu-turn-hold --name turn-<id> [--expire 5m] [--refresh 60] [--socket /.sprite/api.sock]
import { createTaskHold, DEFAULT_SOCKET } from "../src/sprite-task.mjs";

function arg(flag, fallback) {
  const i = process.argv.indexOf(flag);
  return i >= 0 && i + 1 < process.argv.length ? process.argv[i + 1] : fallback;
}
const emit = (obj) => process.stdout.write(`${JSON.stringify(obj)}\n`);

let hold;
try {
  hold = createTaskHold({
    name: arg("--name", null),
    expire: arg("--expire", "5m"),
    refreshMs: Number(arg("--refresh", "60")) * 1000,
    socket: arg("--socket", process.env.CODEWHALE_SPRITE_API_SOCKET || DEFAULT_SOCKET),
    onEvent: emit,
  });
} catch (error) {
  emit({ event: "refused", error: error.message, code: error.code ?? "bad_args" });
  process.exit(2);
}

let ending = false;
async function end(code = 0) {
  if (ending) return;
  ending = true;
  await hold.release();
  process.exit(code);
}

try {
  await hold.acquire();
} catch (error) {
  emit({ event: "acquire_failed", error: error.message, code: error.code ?? "task_api_error" });
  process.exit(1);
}
process.stdin.on("end", () => end(0));
process.stdin.on("error", () => end(0));
process.stdin.resume();
for (const signal of ["SIGTERM", "SIGINT", "SIGHUP"]) process.on(signal, () => end(0));
