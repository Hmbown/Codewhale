import { spawn } from 'node:child_process';

/** Windows child.kill('SIGINT') force-terminates the process. This test-only
 * preload delivers the handler notification over IPC; POSIX uses a real signal.
 * The IPC channel is unreferenced so startup rejection can exit naturally.
 * No test control channel is added to the shipped recorder. */
export function spawnRecorder(args, env = process.env, ipc = process.platform === 'win32') {
  const control = `process.once('message', () => { process.disconnect(); process.emit('SIGINT'); }); process.channel.unref();`;
  const preload = ipc ? ['--import', `data:text/javascript,${encodeURIComponent(control)}`] : [];
  const child = spawn(process.execPath, [...preload, 'scripts/pet.mjs', ...args], {
    cwd: new URL('../../', import.meta.url), env, stdio: ipc ? ['ignore', 'pipe', 'pipe', 'ipc'] : ['ignore', 'pipe', 'pipe'],
  });
  child.stopRecorder = () => {
    if (child.exitCode !== null || child.signalCode !== null) return false;
    return ipc ? child.send('stop') : child.kill('SIGINT');
  };
  return child;
}
