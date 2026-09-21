// Spawned computers: task-owned disposable desktops.
//
// A spawned computer is a Docker container running the plugin's Linux desktop
// image (Xvfb + openbox + AT-SPI + the bundled remote agent). It is not the
// user's machine: every existing tool reaches it through the same agent
// protocol as ssh, and the container boundary is the isolation. `owned:true`
// in the registry marks it as ours — remove or session end destroys it.
//
// Safety posture: docker is invoked with argv arrays only (never a shell), the
// container is addressed by a name we generated, and `docker rm` is only ever
// issued against containers carrying our spawn label — a hand-registered
// docker entry pointing at a user's container cannot be destroyed here.
import crypto from "node:crypto";
import path from "node:path";
import { run, ExecError, trim } from "./exec.mjs";
import { PLUGIN_ROOT, SESSION_ID, b64 } from "./transport.mjs";

export const SPAWN_LABEL = "codewhale.cu.spawned";
export const SESSION_LABEL = "codewhale.cu.session";
export const COMPUTER_LABEL = "codewhale.cu.computer";
export const DEFAULT_IMAGE = process.env.CODEWHALE_CU_SPAWN_IMAGE || "codewhale-cu-linux";

const CONTAINER_RE = /^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$/;
const AGENT_EXEC = "/app/docker/agent-exec.sh";

class SpawnError extends ExecError {
  constructor(code, message, result) {
    super(message, result);
    this.code = code;
  }
}

function docker(args, opts = {}) {
  // Docker CLI lives off PATH; on macOS a Colima/Docker-Desktop install puts
  // it in /usr/local/bin or /opt/homebrew/bin which the MCP host's PATH may
  // lack, so try the well-known paths too.
  return run("docker", args, opts);
}

async function dockerOk(args, opts = {}) {
  const r = await docker(args, opts);
  if (r.aborted) throw Object.assign(new ExecError("computer request cancelled", r), { code: "cancelled" });
  if (r.timedOut) throw new SpawnError("spawn_failed", `docker ${args[0]} timed out`, r);
  if (r.code !== 0) throw new SpawnError("spawn_failed", `docker ${args[0]} failed: ${trim(r.stderr || r.stdout)}`, r);
  return r;
}

export async function dockerAvailable(command = docker) {
  const r = await command(["info", "--format", "{{.OSType}}"], { timeoutMs: 10_000 });
  return r.code === 0 && !r.timedOut && !r.aborted && r.stdout.trim() === "linux";
}

/**
 * The image must exist locally. The plugin's own image is built from
 * docker/Dockerfile on first use; any other image name is the caller's
 * responsibility — we never guess a build context for it.
 */
async function ensureImage(image) {
  const inspect = await docker(["image", "inspect", image], { timeoutMs: 15_000 });
  if (inspect.code === 0) return { built: false };
  if (image !== DEFAULT_IMAGE) {
    throw new SpawnError("spawn_image_missing", `docker image "${image}" is not present locally`);
  }
  const r = await docker(["build", "-t", image, "-f", path.join(PLUGIN_ROOT, "docker", "Dockerfile"), PLUGIN_ROOT], { timeoutMs: 15 * 60_000 });
  if (r.aborted) throw Object.assign(new ExecError("computer request cancelled", r), { code: "cancelled" });
  if (r.code !== 0) throw new SpawnError("spawn_failed", `docker build ${image} failed: ${trim(r.stderr || r.stdout, 800)}`, r);
  return { built: true };
}

/**
 * Start a disposable desktop container and verify the agent answers inside
 * its session. On any failure the container is removed — spawn is
 * transactional: either a live computer comes back or nothing was left.
 */
export async function spawnDockerComputer({ id, image = DEFAULT_IMAGE } = {}) {
  if (!await dockerAvailable()) {
    throw new SpawnError("docker_unavailable", "A Linux Docker engine is required — start Docker Desktop in Linux-container mode (or Colima) and spawn again");
  }
  const { built } = await ensureImage(image);
  const container = `cu-spawn-${id}-${crypto.randomBytes(3).toString("hex")}`;
  const cleanup = async () => {
    await docker(["rm", "-f", container], { timeoutMs: 15_000, signal: null }).catch(() => {});
  };
  try {
    // --init reaps the desktop's children; --ipc=host keeps Chromium off the
    // 64MB default /dev/shm, same as docker/run.sh. "sleep infinity" is the
    // payload — the image entrypoint stands up Xvfb/openbox/the session bus
    // first, and the agent is exec'd in per request.
    await dockerOk([
      "run", "-d", "--name", container,
      "--init", "--ipc=host",
      "--label", `${SPAWN_LABEL}=1`,
      "--label", `${SESSION_LABEL}=${SESSION_ID}`,
      "--label", `${COMPUTER_LABEL}=${id}`,
      image, "sleep", "infinity",
    ], { timeoutMs: 30_000 });
    const deadline = Date.now() + 30_000;
    let lastErr = "no reply";
    for (;;) {
      // list_windows is the honest readiness probe: it needs the session env,
      // the X display, and a window manager managing the root window — the
      // whole stack a caller is about to drive, not just a live agent.
      const probe = await docker(["exec", container, "/bin/sh", AGENT_EXEC, b64({ tool: "list_windows", args: {}, nonce: "spawn" })], { timeoutMs: 10_000, signal: null });
      if (probe.code === 0 && /"ok"\s*:\s*true/.test(probe.stdout)) {
        return { container, image, built };
      }
      lastErr = trim(probe.stderr || probe.stdout) || `exited ${probe.code}`;
      if (Date.now() >= deadline) break;
      await new Promise((resolve) => setTimeout(resolve, 500));
    }
    throw new SpawnError("spawn_failed", `spawned desktop did not come up: ${lastErr}`);
  } catch (err) {
    await cleanup();
    throw err;
  }
}

/**
 * Destroy a spawned container — but only one this plugin created. A docker
 * computer whose container lacks our spawn label is left running and reported
 * not_spawned; removing its registry entry is still the caller's choice.
 */
export async function destroyDockerComputer(computer) {
  const container = computer?.container;
  if (!container || !CONTAINER_RE.test(container)) {
    throw new SpawnError("invalid_container", "docker computer has no valid container name");
  }
  const insp = await docker(["container", "inspect", "--format", `{{index .Config.Labels "${SPAWN_LABEL}"}}`, container], { timeoutMs: 10_000, signal: null });
  if (insp.code !== 0) return { destroyed: false, reason: "container_gone" };
  if (insp.stdout.trim() !== "1") return { destroyed: false, reason: "not_spawned" };
  const r = await docker(["rm", "-f", container], { timeoutMs: 20_000, signal: null });
  if (r.code !== 0) throw new SpawnError("cleanup_failed", `docker rm -f ${container} failed: ${trim(r.stderr)}`, r);
  return { destroyed: true };
}

/**
 * Session teardown: destroy every spawned container this MCP process owns.
 * Other sessions' spawns are left alone — the registry is shared but a
 * container belongs to the process that created it.
 */
export async function destroySessionSpawns() {
  const listed = await docker(["ps", "-aq", "--filter", `label=${SPAWN_LABEL}=1`, "--filter", `label=${SESSION_LABEL}=${SESSION_ID}`], { timeoutMs: 10_000, signal: null });
  if (listed.code !== 0) return { destroyed: [], error: trim(listed.stderr) };
  const ids = listed.stdout.split("\n").map((s) => s.trim()).filter(Boolean);
  const destroyed = [];
  for (const container of ids) {
    const r = await docker(["rm", "-f", container], { timeoutMs: 15_000, signal: null });
    destroyed.push({ container, ok: r.code === 0 });
  }
  return { destroyed };
}
