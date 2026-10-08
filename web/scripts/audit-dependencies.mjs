// Raw upstream findings remain in the report. Only the exact reviewed and
// installed braces mitigation admits its transitive build-tool findings.
import { spawnSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { patchBraces } from './patch-braces.mjs';

const advisory = 'https://github.com/advisories/GHSA-vfj7-8cjw-p6xm';

export function classifyAudit(report) {
  if (report.error || !report.vulnerabilities || !report.metadata?.vulnerabilities) throw new Error('Incomplete dependency audit');
  const entries = report.vulnerabilities;
  if (Object.keys(entries).length !== report.metadata.vulnerabilities.total) throw new Error('Dependency audit count mismatch');
  function mitigated(name, seen = new Set()) {
    if (seen.has(name)) return false;
    const entry = entries[name];
    if (!entry || !Array.isArray(entry.via) || !entry.via.length) return false;
    const next = new Set([...seen, name]);
    return entry.via.every(via => typeof via === 'string'
      ? mitigated(via, next)
      : name === 'braces' && via.name === 'braces' && via.url === advisory);
  }
  // A nested second copy would bypass the single reviewed installation patch.
  const braces = entries.braces;
  if (braces && (braces.nodes?.length !== 1 || braces.nodes[0] !== 'node_modules/braces')) throw new Error('Unreviewed nested braces dependency');
  const blocked = Object.keys(entries).filter(name => !mitigated(name));
  return { upstreamFindings: report.metadata.vulnerabilities.total, mitigated: Object.keys(entries).filter(name => !blocked.includes(name)), blocked };
}

function main() {
  const proof = patchBraces({ checkOnly: true });
  if (!process.env.npm_execpath) throw new Error('Run through npm run audit:dependencies');
  const result = spawnSync(process.execPath, [process.env.npm_execpath, 'audit', '--json'], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 });
  if (result.error || result.signal || ![0, 1].includes(result.status)) throw new Error('Dependency audit failed to complete');
  const report = JSON.parse(result.stdout);
  const at = process.argv.indexOf('--report');
  if (at >= 0) {
    if (!process.argv[at + 1]) throw new Error('Missing audit report path');
    writeFileSync(process.argv[at + 1], JSON.stringify(report, null, 2) + '\n');
  }
  const verdict = classifyAudit(report);
  console.log(JSON.stringify({ ...verdict, reviewedMitigation: proof }));
  if (verdict.blocked.length) process.exitCode = 1;
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
