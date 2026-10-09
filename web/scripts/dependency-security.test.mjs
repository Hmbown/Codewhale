import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { tmpdir } from 'node:os';
import { patchBraces } from './patch-braces.mjs';
import { classifyAudit } from './audit-dependencies.mjs';

const require = createRequire(import.meta.url);
const root = dirname(require.resolve('braces/package.json'));
const braces = require('braces');
const advisory = 'https://github.com/advisories/GHSA-vfj7-8cjw-p6xm';

test('reviewed installed patch preserves ordinary expansion, escaping and all walkers', () => {
  assert.equal(patchBraces({ checkOnly: true }).files.length, 4);
  assert.deepEqual(braces.expand('{a,{b,c}}'), ['a', 'b', 'c']);
  assert.deepEqual(braces.expand('x{1..3}'), ['x1', 'x2', 'x3']);
  assert.deepEqual(braces.expand('x\\{a,b\\}'), ['x{a,b}']);
  assert.equal(braces.stringify(braces.parse('{a,b}')), '{a,b}');
  assert.equal(braces.compile('{a,b}'), '(a|b)');
  assert.throws(() => braces.parse('abc', { maxLength: 2 }), /max characters/);
});

test('parser bounds structural nesting without treating quotes or escaped braces as groups', () => {
  const groups = '{'.repeat(64) + 'x' + '}'.repeat(64);
  assert.throws(() => braces.parse(groups), /nesting exceeds 64 levels/);
  assert.throws(() => braces.parse('('.repeat(64) + 'x' + ')'.repeat(64)), /nesting exceeds 64 levels/);
  assert.doesNotThrow(() => braces.parse('"' + groups + '"'));
  assert.doesNotThrow(() => braces.parse('\\{'.repeat(64)));
});

test('direct AST walker inputs have their own finite depth guard', () => {
  const ast = braces.parse('{a,b}');
  const inner = ast.nodes.find(node => node.type === 'brace');
  let cursor = inner;
  for (let i = 0; i < 66; i++) {
    const child = { type: 'brace', parent: cursor, nodes: [] };
    cursor.nodes = [child];
    cursor = child;
  }
  cursor.nodes = [{ type: 'text', value: 'a' }];
  for (const walk of [braces.compile, braces.expand, braces.stringify]) {
    assert.throws(() => walk(ast), /nesting exceeds 64 levels/);
  }
});

test('patch verification is idempotent and refuses changed source or an unknown version', t => {
  const copy = mkdtempSync(join(tmpdir(), 'cw-braces-'));
  t.after(() => rmSync(copy, { recursive: true, force: true }));
  cpSync(root, copy, { recursive: true });
  assert.deepEqual(patchBraces({ root: copy }), patchBraces({ root: copy, checkOnly: true }));
  const file = join(copy, 'lib/compile.js');
  writeFileSync(file, readFileSync(file, 'utf8') + '\n');
  assert.throws(() => patchBraces({ root: copy }), /Unreviewed or unpatched/);
  const pkg = JSON.parse(readFileSync(join(copy, 'package.json'), 'utf8'));
  pkg.version = '3.0.4';
  writeFileSync(join(copy, 'package.json'), JSON.stringify(pkg));
  assert.throws(() => patchBraces({ root: copy }), /Unreviewed braces version/);
});

function report(extra = {}) {
  const vulnerabilities = {
    braces: { nodes: ['node_modules/braces'], via: [{ name: 'braces', url: advisory }] },
    micromatch: { via: ['braces'] },
    ...extra,
  };
  return { vulnerabilities, metadata: { vulnerabilities: { total: Object.keys(vulnerabilities).length } } };
}

test('audit preserves the exact upstream findings while classifying their complete chains', () => {
  const verdict = classifyAudit(report());
  assert.equal(verdict.upstreamFindings, 2);
  assert.deepEqual(verdict.mitigated, ['braces', 'micromatch']);
  assert.deepEqual(verdict.blocked, []);
});

test('another advisory, an incomplete chain or a cycle is blocked', () => {
  assert.deepEqual(classifyAudit(report({ other: { via: [{ name: 'other', url: 'https://github.com/advisories/new' }] } })).blocked, ['other']);
  assert.deepEqual(classifyAudit(report({ other: { via: ['missing'] } })).blocked, ['other']);
  assert.deepEqual(classifyAudit(report({ alias: { via: [{ name: 'braces', url: advisory }], nodes: ['node_modules/alias'] } })).blocked, ['alias']);
  assert.deepEqual(classifyAudit(report({ first: { via: ['second'] }, second: { via: ['first'] } })).blocked, ['first', 'second']);
});

test('missing audit data and an unpatched nested dependency cannot be admitted', () => {
  assert.throws(() => classifyAudit({ error: { message: 'registry unavailable' } }), /Incomplete/);
  const mismatch = report(); mismatch.metadata.vulnerabilities.total++;
  assert.throws(() => classifyAudit(mismatch), /count mismatch/);
  const nested = report(); nested.vulnerabilities.braces.nodes.push('node_modules/other/node_modules/braces');
  assert.throws(() => classifyAudit(nested), /Unreviewed nested/);
});
