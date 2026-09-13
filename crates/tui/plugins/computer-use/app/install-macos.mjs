import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { spawnSync } from "node:child_process";

/** Publish a verified bundle in one rename, retaining the previous install. */
export function replaceMacBundle(source, destination, { prepare = () => {}, verify = verifySignature } = {}) {
  const parent = path.dirname(destination);
  fs.mkdirSync(parent, { recursive: true });
  if (fs.existsSync(destination) && !fs.existsSync(path.join(destination, "Contents", "Resources", "plugin", "app", "daemon.mjs"))) throw new Error("The installation destination contains a different application.");
  if (fs.existsSync(destination) && fs.lstatSync(destination).isSymbolicLink()) throw new Error("The installation destination must not be a symlink.");
  const staging = fs.mkdtempSync(path.join(parent, ".codewhale-cu-update-"));
  const next = path.join(staging, path.basename(destination));
  let backup = null;
  try {
    fs.cpSync(source, next, { recursive: true });
    prepare(next);
    verify(next);
    if (fs.existsSync(destination)) {
      const backups = path.join(parent, ".codewhale-cu-backups");
      fs.mkdirSync(backups, { recursive: true, mode: 0o700 });
      backup = path.join(backups, `${Date.now()}-${crypto.randomUUID()}.app`);
      fs.renameSync(destination, backup);
    }
    try { fs.renameSync(next, destination); }
    catch (error) { if (backup) fs.renameSync(backup, destination); throw error; }
    return { backup };
  } finally { fs.rmSync(staging, { recursive: true, force: true }); }
}

export function verifySignature(bundle) {
  const result=spawnSync("codesign",["--verify","--deep","--strict",bundle],{encoding:"utf8"});
  if(result.status!==0) throw new Error(`The app signature did not verify: ${result.stderr?.trim() ?? "codesign unavailable"}`);
}

export function verifyReleaseBundle(bundle) {
  verifySignature(bundle);
  const requirement='anchor apple generic and identifier "net.codewhale.computer-use" and certificate leaf[subject.OU] = "5RDNSHA5TY"';
  for(const [command,args] of [["/usr/bin/codesign",["--verify","--strict","-R",requirement,bundle]],["/usr/sbin/spctl",["--assess","--type","execute","--verbose=2",bundle]]]) {
    const result=spawnSync(command,args,{encoding:"utf8"});
    // Gatekeeper ships with macOS. Requiring its notarized source also rejects
    // local allow-list overrides; consumer Macs do not need Xcode's stapler.
    if(result.status!==0 || (command.endsWith("/spctl") && !/^source=Notarized Developer ID\r?$/m.test(result.stderr))) throw new Error("The update is not a valid notarized Codewhale release. Your current app has been kept.");
  }
  if(!fs.existsSync(path.join(bundle,"Contents","MacOS","node"))) throw new Error("The release is missing its bundled runtime.");
}
