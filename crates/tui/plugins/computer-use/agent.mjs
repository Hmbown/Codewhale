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
if (!argv[0]) reply({ ok: false, error: { code: "missing_payload", message: "usage: node agent.mjs <base64 payload> | node agent.mjs --serve" } });

if (argv[0] === "--serve") {
  // Persistent mode: each stdin line is base64 {id, tool, args}; each stdout
  // line is the JSON receipt {id, ok, ...}. The process stays alive, so its
  // backend retains the open_application binding and owned input between
  // calls. The allow-list still applies — the channel cannot become a shell.
  let buf = "";
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (chunk) => {
    buf += chunk;
    let i;
    while ((i = buf.indexOf("\n")) !== -1) {
      const line = buf.slice(0, i).trim();
      buf = buf.slice(i + 1);
      if (line) void serve(line);
    }
  });
  process.stdin.on("end", () => process.exit(0));
} else {
  let req;
  try {
    req = JSON.parse(Buffer.from(argv[0], "base64").toString("utf8"));
  } catch {
    reply({ ok: false, error: { code: "bad_payload", message: "payload is not base64 JSON" } });
  }

  if (["left_mouse_down", "recordingStart"].includes(req?.tool)) {
    reply({ ok: false, error: { code: "persistent_session_required", message: "This operation needs a persistent computer session; the one-shot SSH agent exits after each request. Upgrade the plugin so it serves the agent over one connection, or use a complete drag gesture." } });
  }

  reply(await handle(req, { computerId: "remote" }));
}

async function serve(line) {
  let req;
  try {
    req = JSON.parse(Buffer.from(line, "base64").toString("utf8"));
  } catch {
    process.stdout.write(JSON.stringify({ id: null, ok: false, error: { code: "bad_payload", message: "line is not base64 JSON" } }) + "\n");
    return;
  }
  const receipt = await handle(req, { computerId: "remote", sessionId: "ssh-serve", persistentInputOwner: true });
  process.stdout.write(JSON.stringify({ id: req?.id ?? null, ...receipt }) + "\n");
}
