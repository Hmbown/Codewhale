'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const { spawnSync } = require('node:child_process');
const script = path.resolve(__dirname, '../../../scripts/compiled-host-native-receipt.py');
const identity = 'extension_host::tests::compiled_native_host_cannot_read_secrets_or_write_outside_its_data_dir';
const memoryIdentity = 'extension_host::tests::compiled_native_host_memory_cap_is_enforced';
const digest = (bytes) => crypto.createHash('sha256').update(bytes).digest('hex');
const escape = (text) => text.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
const marker = `compiled-native-containment=passed platform=${{darwin:'macos',win32:'windows',linux:'linux'}[process.platform]} arch=${{x64:'x86_64',arm64:'aarch64'}[process.arch]}`;
const memoryMarker = marker.replace('compiled-native-containment=', 'compiled-native-memory=');
const testcase = (name, body = '') => `<testcase name="${name}" time="0.2">${body}</testcase>`;
function fixture(t, body = `<system-err>${escape(marker)}\n</system-err>`) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'native-receipt-fixture-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const host = path.join(root, 'codewhale-extension-host' + (process.platform === 'win32' ? '.exe' : ''));
  const bundle = path.join(root, 'bundle.mjs'), bun = path.join(root, 'bun-runtime'), junit = path.join(root, 'junit.xml');
  fs.writeFileSync(host, 'synthetic image; no executable or runtime proof');
  fs.writeFileSync(bundle, 'synthetic canonical bundle'); fs.writeFileSync(bun, 'synthetic compiler');
  fs.writeFileSync(junit, `<testsuites><testsuite>${testcase(identity, body)}${testcase(memoryIdentity, `<system-err>${escape(memoryMarker)}\n</system-err>`)}</testsuite></testsuites>`);
  const output = path.join(root, 'receipt');
  const args = [script, '--junit', junit, '--compiled-image', host, '--bundle', bundle, '--bun', bun,
    '--source-sha', '1'.repeat(40), '--image-platform', process.platform, '--image-arch', process.arch, '--host-sha256', digest(fs.readFileSync(host)),
    '--bundle-sha256', digest(fs.readFileSync(bundle)), '--runtime-sha256', digest(fs.readFileSync(bun)), '--output', output];
  return { root, host, bundle, bun, junit, output, args,
    run: () => spawnSync('python3', args, { encoding: 'utf8' }) };
}
function windowsCases(omit = '') {
  const names = ['windows_native_lpac_node_owner_boundary', 'windows_native_lpac_bun_owner_boundary',
    'windows_native_lpac_compiled_owner_boundary', 'windows_native_profiles_are_distinct_and_acl_grant_refuses_junctions',
    'windows_argv_and_environment_keep_exact_values_and_reject_nul',
    'windows_profile_retirement_removes_only_its_grants_and_inherited_data_on_restarts',
    'windows_profile_directory_budget_and_recorded_identity_refuse_before_overwrite'];
  return names.filter(name => name !== omit).map(name => testcase(`extension_host::windows::tests::${name}`)).join('');
}
function includeWindows(f) {
  if (process.platform === 'win32') fs.writeFileSync(f.junit,
    fs.readFileSync(f.junit, 'utf8').replace('</testsuite>', windowsCases() + '</testsuite>'));
}
test('receipt exports exact bytes and real report fields; synthetic fixture is not runtime proof', t => {
  const f = fixture(t); includeWindows(f);
  const result = f.run(); assert.equal(result.status, 0, result.stderr);
  const receipt = JSON.parse(fs.readFileSync(path.join(f.output, 'native-receipt.json'), 'utf8'));
  assert.equal(receipt.scope, 'native-compiled-host'); assert.equal(receipt.platform, process.platform); assert.equal(receipt.arch, process.arch);
  assert.equal(receipt.passed, process.platform === 'win32' ? 9 : 2); assert.equal(receipt.failed, 0); assert.equal(receipt.skipped, 0);
  assert.deepEqual(fs.readFileSync(path.join(f.output, path.basename(f.host))), fs.readFileSync(f.host));
  assert.equal(receipt.host_sha256, digest(fs.readFileSync(f.host)));
  assert.equal(receipt.log_sha256, digest(fs.readFileSync(path.join(f.output, receipt.log))));
  if (process.platform === 'linux') assert.equal(receipt.libc.family, 'glibc');
});
test('receipt refuses missing Native testcase', t => {
  const f = fixture(t); fs.writeFileSync(f.junit, '<testsuites/>');
  const r = f.run(); assert.notEqual(r.status, 0); assert.match(r.stderr, /required Native testcase is missing/); assert.ok(!fs.existsSync(f.output));
});
test('receipt refuses an ordinary successful early return without complete Native marker', t => {
  const f = fixture(t, ''); includeWindows(f);
  const r = f.run(); assert.notEqual(r.status, 0); assert.match(r.stderr, /completion marker is missing/);
});
for (const element of ['failure', 'error', 'skipped', 'flakyFailure']) {
  test(`receipt refuses ${element} rather than upgrading it to a pass`, t => {
    const f = fixture(t, `<${element}/><system-err>${escape(marker)}</system-err>`); includeWindows(f);
    const r = f.run(); assert.notEqual(r.status, 0); assert.match(r.stderr, /did not pass once/);
  });
}
test('receipt refuses duplicate/retried Native results', t => {
  const f = fixture(t); includeWindows(f);
  fs.writeFileSync(f.junit, fs.readFileSync(f.junit, 'utf8').replace('</testsuite>', testcase(identity) + '</testsuite>'));
  const r = f.run(); assert.notEqual(r.status, 0); assert.match(r.stderr, /duplicate\/retried/);
});
test('receipt refuses compiled image mutated after its Native invocation', t => {
  const f = fixture(t); includeWindows(f); fs.appendFileSync(f.host, 'changed');
  const r = f.run(); assert.notEqual(r.status, 0); assert.match(r.stderr, /qualified input changed/);
});
test('receipt refuses prior output directory and noncanonical filename', t => {
  const f = fixture(t); includeWindows(f); fs.mkdirSync(f.output);
  assert.notEqual(f.run().status, 0);
  fs.rmSync(f.output, { recursive: true });
  const other = path.join(f.root, 'host-imposter'); fs.renameSync(f.host, other);
  f.args[f.args.indexOf('--compiled-image') + 1] = other;
  const r = f.run(); assert.notEqual(r.status, 0); assert.match(r.stderr, /canonical image basename/);
});
test('Windows unit extraction requires all actual LPAC/reparse consumers', t => {
  const f = fixture(t);
  const invoke = (report) => spawnSync('python3', ['-c',
    'import runpy,sys; m=runpy.run_path(sys.argv[1]); print(m["extract"](sys.stdin.buffer.read(),"win32","x64")[0])', script],
    { input: report, encoding: 'utf8' });
  const compiled = testcase(identity, '<system-err>compiled-native-containment=passed platform=windows arch=x86_64\n</system-err>');
  const compiledMemory = testcase(memoryIdentity, '<system-err>compiled-native-memory=passed platform=windows arch=x86_64\n</system-err>');
  const r = invoke(`<testsuites><testsuite>${compiled}${compiledMemory}${windowsCases()}</testsuite></testsuites>`);
  assert.equal(r.status, 0, r.stderr); assert.equal(r.stdout.trim(), '9');
  const missing = invoke(`<testsuites><testsuite>${compiled}${compiledMemory}${windowsCases('windows_native_lpac_node_owner_boundary')}</testsuite></testsuites>`);
  assert.notEqual(missing.status, 0); assert.match(missing.stderr, /required Native testcase is missing/);
});
test('receipt refuses XML entities and another OS completion marker', t => {
  const f = fixture(t); fs.writeFileSync(f.junit, '<!DOCTYPE x><testsuites/>');
  assert.match(f.run().stderr, /invalid or oversized/);
  const r = spawnSync('python3', ['-c', 'import runpy,sys; m=runpy.run_path(sys.argv[1]); m["extract"](sys.stdin.buffer.read(),"win32","x64")', script],
    { input: `<testsuites><testsuite>${testcase(identity, '<system-err>compiled-native-containment=passed platform=macos arch=x86_64\n</system-err>')}${testcase(memoryIdentity, '<system-err>compiled-native-memory=passed platform=windows arch=x86_64\n</system-err>')}${windowsCases()}</testsuite></testsuites>`, encoding: 'utf8' });
  assert.notEqual(r.status, 0); assert.match(r.stderr, /another native target/);
});

test('receipt refuses architecture/emulation label transfer even with matching image bytes', t => {
  const f = fixture(t); includeWindows(f);
  f.args[f.args.indexOf('--image-arch') + 1] = process.arch === 'arm64' ? 'x64' : 'arm64';
  const r = f.run(); assert.notEqual(r.status, 0); assert.match(r.stderr, /identity differs from native runner/);
});
