// One set of ssh rules for every ssh this plugin starts (the remote agent,
// its persistent channel, and the agent install copy). Codewhale's Fleet SSH
// host applies the same rules; tests/fixtures/ssh-destinations.json holds the
// vectors both implementations are tested against.
//  - host keys must already be known: StrictHostKeyChecking=yes, no updates;
//    a computer may name its own known-hosts file, which is then the only one
//  - a host or user is a destination, never an option: neither may start
//    with "-", and "--" ends the options before the destination

import path from "node:path";

export const SSH_HOST_RE = /^[A-Za-z0-9_][A-Za-z0-9._-]*$/;
export const SSH_USER_RE = /^[A-Za-z0-9_][A-Za-z0-9._-]*$/;

export class SshTargetError extends Error {
  constructor(code, message) { super(message); this.name = "SshTargetError"; this.code = code; }
}

// OpenSSH accepts forward slashes on Windows. Validate the native absolute
// path first, then avoid interpreting its backslashes as config escapes.
function knownHostsFile(value) {
  if (typeof value !== "string" || !path.isAbsolute(value)) {
    throw new SshTargetError("invalid_known_hosts", "knownHosts must be an absolute path without spaces or quotes");
  }
  const file = process.platform === "win32" ? value.replaceAll("\\", "/") : value;
  if (/[\s"\\]/.test(file)) {
    throw new SshTargetError("invalid_known_hosts", "knownHosts must be an absolute path without spaces or quotes");
  }
  return file;
}

/** Throw when an ssh computer entry cannot be used as a destination. */
export function validateSshTarget({ host, user, port, knownHosts } = {}) {
  if (typeof host !== "string" || !SSH_HOST_RE.test(host)) {
    throw new SshTargetError("invalid_host", "ssh computers need a valid host (letters, digits, dot, dash, underscore; not starting with '-')");
  }
  if (user != null && (typeof user !== "string" || !SSH_USER_RE.test(user))) {
    throw new SshTargetError("invalid_user", "user must be a plain name (not starting with '-')");
  }
  if (port != null && (!Number.isInteger(port) || port < 1 || port > 65535)) {
    throw new SshTargetError("invalid_port", "port must be an integer in 1..65535");
  }
  if (knownHosts != null) knownHostsFile(knownHosts);
}

/** ssh options (no destination) for a computer entry. */
export function sshOptions(computer, { portFlag = "-p" } = {}) {
  const options = [
    "-o", "BatchMode=yes",
    "-o", "ConnectTimeout=8",
    "-o", "StrictHostKeyChecking=yes",
    "-o", "UpdateHostKeys=no",
  ];
  if (computer.knownHosts) {
    options.push("-o", `UserKnownHostsFile=${knownHostsFile(computer.knownHosts)}`, "-o", `GlobalKnownHostsFile=${process.platform === "win32" ? "NUL" : "/dev/null"}`);
  }
  if (computer.port) options.push(portFlag, String(computer.port));
  return options;
}

export function sshDestination(computer) {
  return computer.user ? `${computer.user}@${computer.host}` : computer.host;
}

/** Full ssh argv prefix: options, "--", destination. The remote command follows. */
export function sshArgv(computer) {
  validateSshTarget(computer);
  return [...sshOptions(computer), "--", sshDestination(computer)];
}
