// codewhale-cu remote agent — runs on an ssh-registered computer.
// One-shot: `node agent.mjs <base64(json)>` prints exactly one JSON receipt
// line. The request is {"tool": "...", "args": {...}}; the shared handler
// enforces the tool allow-list, so the transport can never become a shell.
import { handle } from "./src/app-handler.mjs";

function reply(obj) {
  process.stdout.write(JSON.stringify(obj) + "\n");
  process.exit(0);
}

const argv = process.argv.slice(2);
if (!argv[0]) reply({ ok: false, error: { code: "missing_payload", message: "usage: node agent.mjs <base64 payload>" } });

let req;
try {
  req = JSON.parse(Buffer.from(argv[0], "base64").toString("utf8"));
} catch {
  reply({ ok: false, error: { code: "bad_payload", message: "payload is not base64 JSON" } });
}

reply(await handle(req, { computerId: "remote" }));
