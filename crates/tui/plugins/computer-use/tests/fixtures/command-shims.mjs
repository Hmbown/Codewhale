// Test-process preload: substitute only explicitly supplied command fixtures.
// Node loads it before production imports, preserving argument arrays, pipes,
// exit codes and cancellation on every OS without POSIX shebang assumptions.
import cp from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { syncBuiltinESMExports } from 'node:module';
const original = cp.spawn;
cp.spawn = function(command, args = [], options) {
  const dir = process.env.CU_COMMAND_FIXTURES;
  if (dir && ['ssh', 'scp', 'hdc'].includes(command)) {
    const script = path.join(dir, `${command}.cjs`);
    if (fs.existsSync(script)) return original(process.execPath, [script, ...args], options);
  }
  return original(command, args, options);
};
syncBuiltinESMExports();
