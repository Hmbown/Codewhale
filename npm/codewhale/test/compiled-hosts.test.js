const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const hosts = require('../scripts/compiled-hosts');
const { run, _internal } = require('../scripts/install');

function fixture(target = 'macos-arm64', marker = 'one') {
  const names = hosts.names(target);
  const payloads = { [names.binary]: Buffer.from(`image-${marker}`), [names.notices]: Buffer.from(`actual-notice-${marker}`), [names.source]: Buffer.from(`source-fixture-${marker}`) };
  const digest = 'a'.repeat(64), source = 'b'.repeat(40);
  const catalog = { schema: 1, version: '0.10.1', source_sha: source, bundle_sha256: digest, hosts: [{
    target, asset: names.binary, sha256: hosts.sha256(payloads[names.binary]),
    notices_asset: names.notices, notices_sha256: hosts.sha256(payloads[names.notices]),
    source_asset: names.source, source_sha256: hosts.sha256(payloads[names.source]),
    runtime_version: '1.4.0', runtime_revision: 'c'.repeat(40), runtime_sha256: 'd'.repeat(64), webkit_revision: 'e'.repeat(40),
    bundle_sha256: digest, source_commit: source, native_platform: hosts.TARGETS[target][0], native_arch: hosts.TARGETS[target][1], libc: target.startsWith('linux-') ? 'glibc' : 'none',
    passed: hosts.compiledMinimum(target), failed: 0, skipped: 0, test_log_sha256: 'f'.repeat(64), native_passed: hosts.nativeMinimum(target), native_failed: 0, native_skipped: 0, native_log_sha256: '1'.repeat(64),
    license_closure: 'complete', relink_source: 'complete',
  }] };
  const text = JSON.stringify(catalog);
  payloads[hosts.HOST_CATALOG] = Buffer.from(text);
  return { catalog, text, payloads, host: catalog.hosts[0] };
}
async function temporary(t) {
  const dir = await fs.promises.mkdtemp(path.join(os.tmpdir(), 'cw-host-delivery-'));
  t.after(() => fs.promises.rm(dir, { recursive: true, force: true }));
  return dir;
}
function installOptions(releaseDir, f, extra = {}) {
  const source = { baseUrl: 'https://fixture.invalid/', checksums: new Map(Object.entries(f.payloads).map(([name, bytes]) => [name, hosts.sha256(bytes)])) };
  return { version: f.catalog.version, releaseDir, source, context: 'install', options: {
    platform: hosts.TARGETS[f.host.target][0], arch: hosts.TARGETS[f.host.target][1],
    fetchText: async () => f.text,
    download: async (url, target) => fs.promises.writeFile(target, f.payloads[path.basename(new URL(url).pathname)]), ...extra,
  } };
}
test('catalog refuses malformed, skipped, cross-native, duplicate and incomplete closure receipts', () => {
  const good = fixture();
  assert.equal(hosts.parseCatalog(Buffer.from(good.text), '0.10.1').hosts.length, 1);
  for (const mutate of [c => c.hosts[0].native_skipped = 1, c => c.hosts[0].passed = 0, c => c.hosts[0].native_arch = 'x64', c => c.hosts[0].license_closure = 'pending', c => c.hosts[0].source_asset = '../source.tar.gz', c => c.hosts.push(c.hosts[0]), c => c.source_sha = 'bad']) {
    const copy = structuredClone(good.catalog); mutate(copy); assert.throws(() => hosts.parseCatalog(copy));
  }
  assert.throws(() => hosts.parseCatalog(good.text, '0.10.2'), /differs/);
  assert.throws(() => hosts.parseCatalog(' '.repeat(65537)), /64 KiB/);
  assert.throws(() => hosts.selectedHost(good.catalog, 'android', 'arm64'), /Android/);
  assert.throws(() => hosts.selectedHost(good.catalog, 'darwin', 'x64'), /no qualified image/);
  assert.equal(hosts.requested({}), false);
});
test('payload directory rejects tampering and symbolic substitutions', async t => {
  const dir = await temporary(t), f = fixture();
  for (const [name, bytes] of Object.entries(f.payloads)) fs.writeFileSync(path.join(dir, name), bytes);
  hosts.verifyDirectory(dir, f.catalog);
  fs.writeFileSync(path.join(dir, f.host.source_asset), 'tampered');
  assert.throws(() => hosts.verifyDirectory(dir, f.catalog), /SHA256/);
  fs.unlinkSync(path.join(dir, f.host.source_asset));
  fs.symlinkSync(path.join(dir, f.host.notices_asset), path.join(dir, f.host.source_asset));
  assert.throws(() => hosts.verifyDirectory(dir, f.catalog), /regular payload/);
});
test('npm stages every companion before publication and keeps canonical file modes', async t => {
  const dir = await temporary(t), f = fixture(), prepared = await _internal.prepareCompiledHost(installOptions(dir, f));
  assert.equal(fs.existsSync(path.join(dir, hosts.HOST_NAME)), false);
  try {
    await prepared.publish();
    assert.deepEqual(fs.readFileSync(path.join(dir, hosts.HOST_NAME)), f.payloads[f.host.asset]);
    assert.equal(hosts.parseCatalog(fs.readFileSync(path.join(dir, hosts.HOST_NAME + '.release.json'))).version, '0.10.1');
    if (process.platform !== 'win32') {
      assert.equal(fs.statSync(path.join(dir, hosts.HOST_NAME)).mode & 0o777, 0o755);
      assert.equal(fs.statSync(path.join(dir, hosts.HOST_NAME + '.LICENSES.txt')).mode & 0o777, 0o644);
    }
  } finally { await prepared.cleanup(); }
  assert.equal(fs.readdirSync(dir).some(name => name.startsWith('.compiled-host-')), false);
});
test('npm rejects foreign companion ownership and destination changes without overwriting them', async t => {
  const dir = await temporary(t), f = fixture();
  fs.writeFileSync(path.join(dir, hosts.HOST_NAME), 'foreign');
  await assert.rejects(_internal.prepareCompiledHost(installOptions(dir, f)), /unclaimed/);
  assert.equal(fs.readFileSync(path.join(dir, hosts.HOST_NAME), 'utf8'), 'foreign');
  fs.unlinkSync(path.join(dir, hosts.HOST_NAME));
  const prepared = await _internal.prepareCompiledHost(installOptions(dir, f));
  fs.writeFileSync(path.join(dir, hosts.HOST_NAME + '.LICENSES.txt'), 'concurrent');
  try { await assert.rejects(prepared.publish(), /changed/); } finally { await prepared.cleanup(); }
  assert.equal(fs.existsSync(path.join(dir, hosts.HOST_NAME)), false);
  assert.equal(fs.readFileSync(path.join(dir, hosts.HOST_NAME + '.LICENSES.txt'), 'utf8'), 'concurrent');
});
test('npm updates an owned companion and rejects modified prior bytes', async t => {
  const dir = await temporary(t), first = fixture(), second = fixture('macos-arm64', 'two');
  await _internal.installCompiledHost(installOptions(dir, first));
  await _internal.installCompiledHost(installOptions(dir, second));
  assert.deepEqual(fs.readFileSync(path.join(dir, hosts.HOST_NAME)), second.payloads[second.host.asset]);
  fs.writeFileSync(path.join(dir, hosts.HOST_NAME + '.LICENSES.txt'), 'changed');
  await assert.rejects(_internal.installCompiledHost(installOptions(dir, first)), /modified/);
  assert.deepEqual(fs.readFileSync(path.join(dir, hosts.HOST_NAME)), second.payloads[second.host.asset]);
});
test('explicit unavailable host fails before any CLI destination or download changes', async t => {
  const dir = await temporary(t), f = fixture(), cli = path.join(dir, 'codewhale'), alias = path.join(dir, 'codew');
  fs.writeFileSync(cli, 'old-cli'); fs.writeFileSync(alias, 'old-alias');
  let downloads = 0;
  const source = 'https://fixture.invalid/';
  const checksums = new Map([['codewhale-macos-x64', 'a'.repeat(64)], ['codew-macos-x64', 'a'.repeat(64)], [hosts.HOST_CATALOG, hosts.sha256(Buffer.from(f.text))]]);
  await assert.rejects(run({ releaseDir: dir, paths: { codewhale: { target: cli, asset: 'codewhale-macos-x64' }, codew: { target: alias, asset: 'codew-macos-x64' } }, platform: 'darwin', arch: 'x64', env: { CODEWHALE_VERSION: '0.10.1', CODEWHALE_RELEASE_BASE_URL: source, CODEWHALE_INSTALL_COMPILED_HOST: '1' }, fetchText: async url => url.endsWith(hosts.HOST_CATALOG) ? f.text : [...checksums].map(([name, hash]) => `${hash}  ${name}`).join('\n'), download: async () => { downloads++; } }), /no qualified image/);
  assert.equal(downloads, 0);
  assert.equal(fs.readFileSync(cli, 'utf8'), 'old-cli'); assert.equal(fs.readFileSync(alias, 'utf8'), 'old-alias');
});
module.exports = { fixture };

test('Windows delivery requires containment and memory plus all seven LPAC cases', () => {
  const f = fixture('windows-x64');
  f.catalog.hosts[0].native_passed = 8;
  assert.throws(() => hosts.parseCatalog(f.catalog), /Native compiled-image/);
  f.catalog.hosts[0].native_passed = 9;
  assert.equal(hosts.parseCatalog(f.catalog).hosts[0].passed, 5);
  // Cross-source contract proof; actual PowerShell installation is separate.
  const installer = fs.readFileSync(path.resolve(__dirname, '../../../scripts/release/install.ps1'), 'utf8');
  for (const target of ['windows-x64', 'windows-arm64']) {
    assert.match(installer, new RegExp(`\\$hostEntry\\.passed -ne ${hosts.compiledMinimum(target)}(?: |\\))`));
    assert.match(installer, new RegExp(`\\$hostEntry\\.native_passed -lt ${hosts.nativeMinimum(target)}(?: |\\))`));
  }
});

test('npm fresh publication never overwrites a file created after its identity check', async t => {
  const dir = await temporary(t), f = fixture(), prepared = await _internal.prepareCompiledHost(installOptions(dir, f));
  const original = fs.promises.link;
  fs.promises.link = async (source, destination) => {
    await fs.promises.writeFile(destination, 'another writer');
    return original(source, destination);
  };
  try { await assert.rejects(prepared.publish(), /EEXIST/); }
  finally { fs.promises.link = original; await prepared.cleanup(); }
  assert.equal(fs.readFileSync(path.join(dir, hosts.HOST_NAME), 'utf8'), 'another writer');
});

// Fixture receipts verify contract refusal only; they are not Native OS proof.
test('every target requires its exact direct-image cases and separate Native memory proof without skips', () => {
  for (const target of Object.keys(hosts.TARGETS)) {
    const f = fixture(target), host = f.catalog.hosts[0];
    const expectedDirect = target.startsWith('macos-') ? 6 : 5;
    const expectedNative = target.startsWith('windows-') ? 9 : 2;
    assert.equal(hosts.compiledMinimum(target), expectedDirect);
    assert.equal(hosts.nativeMinimum(target), expectedNative);
    assert.equal(hosts.parseCatalog(f.catalog).hosts[0].passed, expectedDirect);
    for (const count of [expectedDirect - 1, expectedDirect + 1]) {
      host.passed = count;
      assert.throws(() => hosts.parseCatalog(f.catalog), /compiled-image qualification/);
    }
    host.passed = expectedDirect;
    host.native_passed = expectedNative - 1;
    assert.throws(() => hosts.parseCatalog(f.catalog), /Native compiled-image/);
    host.native_passed = expectedNative;
    host.skipped = 1;
    assert.throws(() => hosts.parseCatalog(f.catalog), /compiled-image qualification/);
    host.skipped = 0; host.native_skipped = 1;
    assert.throws(() => hosts.parseCatalog(f.catalog), /Native compiled-image/);
  }
});
