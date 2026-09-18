// Action trajectories: a local, opt-in JSONL of the tool calls this session
// made — tool name, arguments, outcome — for review and replay. Files live in
// the recordings directory; nothing is uploaded anywhere, and recording stays
// off until a session explicitly starts it. Replay re-enters the normal tool
// pipeline, so every gate (permissions, grants, the kill switch) still applies.
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { stateDir } from "./registry.mjs";

export const trajectoriesDir = () => path.join(process.env.CODEWHALE_CU_RECORDINGS_DIR || path.join(stateDir(), "recordings"), "trajectories");

/** Tools about the recorder itself are never recorded and never replayed. */
export const isTrajectoryTool = (name) => typeof name === "string" && (name === "trajectory" || name.startsWith("trajectory_"));

export function createRecorder() {
  let file = null;
  const turns = () => (file && fs.existsSync(file)) ? fs.readFileSync(file, "utf8").split("\n").filter((line) => line.includes('"call"')).length : 0;
  return {
    get active() { return file; },
    start() {
      fs.mkdirSync(trajectoriesDir(), { recursive: true });
      const stamp = new Date().toISOString().replace(/[:.]/g, "-");
      file = path.join(trajectoriesDir(), `traj-${stamp}-${crypto.randomBytes(3).toString("hex")}.jsonl`);
      fs.writeFileSync(file, JSON.stringify({ type: "start", at: new Date().toISOString(), pid: process.pid }) + "\n");
      return { recording: true, file };
    },
    stop() {
      if (!file) return { recording: false, note: "no trajectory was recording" };
      const stopped = file;
      try { fs.appendFileSync(stopped, JSON.stringify({ type: "stop", at: new Date().toISOString() }) + "\n"); } catch {}
      file = null;
      return { recording: false, file: stopped, turns: countCalls(stopped) };
    },
    status() {
      return { recording: !!file, file, turns: file ? countCalls(file) : 0, dir: trajectoriesDir(), note: "Local JSONL on this machine; arguments are stored verbatim so replay is faithful. Start it only when the person knows it runs." };
    },
    append(entry) {
      if (!file) return;
      try { fs.appendFileSync(file, JSON.stringify({ type: "call", at: new Date().toISOString(), ...entry }) + "\n"); } catch { /* a full disk must not break tool calls */ }
    },
  };
}

const countCalls = (file) => {
  try { return fs.readFileSync(file, "utf8").split("\n").filter((line) => line.trim().endsWith("}") && line.includes('"type":"call"')).length; } catch { return 0; }
};

export function readTrajectory(file) {
  const entries = [];
  for (const line of fs.readFileSync(file, "utf8").split("\n")) {
    if (!line.trim()) continue;
    try { entries.push(JSON.parse(line)); } catch { /* skip a torn last line */ }
  }
  return entries;
}

export function listTrajectories(limit = 5) {
  try {
    return fs.readdirSync(trajectoriesDir())
      .filter((name) => name.startsWith("traj-") && name.endsWith(".jsonl"))
      .sort().reverse().slice(0, limit)
      .map((name) => {
        const full = path.join(trajectoriesDir(), name);
        const stat = fs.statSync(full);
        return { id: name, bytes: stat.size, modified: stat.mtime.toISOString(), turns: countCalls(full) };
      });
  } catch { return []; }
}

/**
 * Resolve an id ("latest" or a traj-*.jsonl basename) to a file inside the
 * trajectories dir. Anything that escapes the directory is refused, not read.
 */
export function resolveTrajectory(id) {
  const dir = trajectoriesDir();
  const bad = (message) => Object.assign(new Error(message), { code: "bad_args" });
  const missing = (message) => Object.assign(new Error(message), { code: "trajectory_not_found" });
  let name = typeof id === "string" && id.trim() && id !== "latest" ? id.trim() : null;
  if (!name) {
    const recent = listTrajectories(1);
    if (!recent.length) throw missing("no trajectories on this machine yet — start one with trajectory {action:\"start\"}");
    name = recent[0].id;
  }
  if (name.includes("/") || name.includes("\\") || name.startsWith(".")) throw bad("trajectory id must be a traj-*.jsonl name from trajectory status");
  const file = path.resolve(dir, name);
  if (path.dirname(file) !== path.resolve(dir)) throw bad("trajectory id must stay inside the trajectories directory");
  if (!fs.existsSync(file)) throw missing(`no trajectory named "${name}" (see trajectory {action:"status"})`);
  return file;
}
