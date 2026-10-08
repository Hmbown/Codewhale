// Temporary mitigation for GHSA-vfj7-8cjw-p6xm; see ../docs/DEPENDENCY_SECURITY.md.
// Preserve upstream version/licence and verify both original and patched bytes.
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
const require = createRequire(import.meta.url);
const digest = value => createHash('sha256').update(value).digest('hex');
const reviewed = [
  {
    "file": "lib/parse.js",
    "original": "e572166565f15fa6ad9865ae49d678218e32aabfd1b3720f6d0d43d39800d310",
    "patched": "eb06a3799e392ea84cabe898f5095a3cad34fd88c78b75bf596e6196738b14b4",
    "edits": [
      {
        "before": "      stack.push(block);",
        "after": "      stack.push(block);\n      if (stack.length > 64) throw new SyntaxError('Brace AST nesting exceeds 64 levels');",
        "count": 2
      }
    ]
  },
  {
    "file": "lib/compile.js",
    "original": "dc98f22eee3d511785d92a00758d5f0d48efed5f5813bdecc2de430c529b5c9f",
    "patched": "3674b9cdb5d23fbe3795ec3083b9cbdbf164f7d586a1f90c1a01de8dcb6b2609",
    "edits": [
      {
        "before": "const walk = (node, parent = {}) => {",
        "after": "const walk = (node, parent = {}, depth = 0) => {\n    if (depth > 64) throw new SyntaxError('Brace AST nesting exceeds 64 levels');",
        "count": 1
      },
      {
        "before": "walk(child, node)",
        "after": "walk(child, node, depth + 1)",
        "count": 1
      }
    ]
  },
  {
    "file": "lib/expand.js",
    "original": "41ccc196ebfa7b7781a634e721eb744e4e7bcb54cba427a7e3d6806a1b9e58f7",
    "patched": "eb2f9dcd071a237d36c7da755ca2cadef3f95d2147b108b1c7fa3c0f1adfdb96",
    "edits": [
      {
        "before": "const walk = (node, parent = {}) => {",
        "after": "const walk = (node, parent = {}, depth = 0) => {\n    if (depth > 64) throw new SyntaxError('Brace AST nesting exceeds 64 levels');",
        "count": 1
      },
      {
        "before": "walk(child, node)",
        "after": "walk(child, node, depth + 1)",
        "count": 1
      }
    ]
  },
  {
    "file": "lib/stringify.js",
    "original": "379f22d77bfa1478341ccd49c5e4267464aabcbba03558bab332aac23fc6f23a",
    "patched": "64ec4fcc359c83ecbd5a3220b028d17f0b60a2645b95271f363b4d443173aba6",
    "edits": [
      {
        "before": "const stringify = (node, parent = {}) => {",
        "after": "const stringify = (node, parent = {}, depth = 0) => {\n    if (depth > 64) throw new SyntaxError('Brace AST nesting exceeds 64 levels');",
        "count": 1
      },
      {
        "before": "stringify(child)",
        "after": "stringify(child, undefined, depth + 1)",
        "count": 1
      }
    ]
  }
];

export function patchBraces({ root = dirname(require.resolve('braces/package.json')), checkOnly = false } = {}) {
  const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
  if (pkg.name !== 'braces' || pkg.version !== '3.0.3') throw new Error('Unreviewed braces version; retire or review the depth mitigation');
  const pending = reviewed.map(entry => {
    const path = join(root, entry.file), source = readFileSync(path, 'utf8'), hash = digest(source);
    if (hash === entry.patched) return { path, source, hash };
    if (checkOnly || hash !== entry.original) throw new Error('Unreviewed or unpatched braces file: ' + entry.file);
    let patched = source;
    for (const edit of entry.edits) {
      if (patched.split(edit.before).length - 1 !== edit.count) throw new Error('braces patch anchor changed: ' + entry.file);
      patched = patched.split(edit.before).join(edit.after);
    }
    if (digest(patched) !== entry.patched) throw new Error('braces patch digest mismatch: ' + entry.file);
    return { path, source: patched, hash: entry.patched };
  });
  // Validate every file before touching any. Re-running after npm ci is idempotent.
  if (!checkOnly) for (const file of pending) writeFileSync(file.path, file.source);
  return { version: pkg.version, files: reviewed.map(entry => ({ file: entry.file, sha256: entry.patched })) };
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  console.log(JSON.stringify({ mitigation: 'bounded AST depth', ...patchBraces() }));
}
