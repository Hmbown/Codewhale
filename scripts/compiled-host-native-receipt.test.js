// Synthetic JUnit exercises exporter refusal and target contracts only.
// These source fixtures never qualify a runtime, image, OS or delivery target.
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const exporter = path.join(__dirname, 'compiled-host-native-receipt.py');
const containment = 'extension_host::tests::compiled_native_host_cannot_read_secrets_or_write_outside_its_data_dir';
const memory = 'extension_host::tests::compiled_native_host_memory_cap_is_enforced';
const windows = [
  'windows_native_lpac_node_owner_boundary',
  'windows_native_lpac_bun_owner_boundary',
  'windows_native_lpac_compiled_owner_boundary',
  'windows_native_profiles_are_distinct_and_acl_grant_refuses_junctions',
  'windows_argv_and_environment_keep_exact_values_and_reject_nul',
  'windows_profile_retirement_removes_only_its_grants_and_inherited_data_on_restarts',
  'windows_profile_directory_budget_and_recorded_identity_refuse_before_overwrite',
].map(name => 'extension_host::windows::tests::' + name);
const rustOS = { linux: 'linux', darwin: 'macos', win32: 'windows' };
const rustArch = { x64: 'x86_64', arm64: 'aarch64' };
const xml = value => value.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('"', '&quot;');
function records(platform = 'linux', arch = 'x64') {
  return [containment, memory, ...(platform === 'win32' ? windows : [])].map(name => ({
    name, output: name === containment || name === memory
      ? `compiled-native-${name === containment ? 'containment' : 'memory'}=passed platform=${rustOS[platform]} arch=${rustArch[arch]}` : '',
  }));
}
function report(rows) {
  return `<testsuites><testsuite>${rows.map(row => `<testcase name="${xml(row.name)}" time="0.1">${row.status ? `<${row.status}/>` : ''}<system-out>${xml(row.output)}</system-out></testcase>`).join('')}</testsuite></testsuites>`;
}
function extract(rows, platform = 'linux', arch = 'x64', raw) {
  const source = `import sys, json, importlib.util\nsys.dont_write_bytecode=True\nspec=importlib.util.spec_from_file_location('native_receipt',sys.argv[1])\nmodule=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)\ndata=json.load(sys.stdin)\ntry:\n passed,log=module.extract(data['report'].encode(),data['platform'],data['arch'])\n print(json.dumps({'passed':passed,'log':log.decode()}))\nexcept Exception as error:\n print(str(error),file=sys.stderr);sys.exit(1)\n`;
  return spawnSync(process.env.PYTHON || 'python3', ['-c', source, exporter], {
    input: JSON.stringify({ report: raw ?? report(rows), platform, arch }), encoding: 'utf8', timeout: 10_000,
  });
}
for (const platform of ['linux', 'darwin', 'win32']) test(`${platform} export requires actual matching containment and memory cases`, () => {
  for (const arch of ['x64', 'arm64']) {
    const rows = records(platform, arch), result = extract(rows, platform, arch);
    assert.equal(result.status, 0, result.stderr);
    const receipt = JSON.parse(result.stdout);
    assert.equal(receipt.passed, platform === 'win32' ? 9 : 2);
    assert.match(receipt.log, /compiled-native-containment=passed/);
    assert.match(receipt.log, /compiled-native-memory=passed/);
    assert.equal(extract(rows.filter(row => row.name !== memory), platform, arch).status, 1);
    const noMarker = structuredClone(rows); noMarker.find(row => row.name === memory).output = '';
    assert.equal(extract(noMarker, platform, arch).status, 1);
  }
});
test('a marker from another platform or architecture cannot qualify the image', () => {
  const rows = records();
  for (const output of ['compiled-native-memory=passed platform=windows arch=x86_64', 'compiled-native-memory=passed platform=linux arch=aarch64', 'compiled-native-memory=passed platform=linux arch=x86_64 trailing']) {
    rows.find(row => row.name === memory).output = output;
    assert.equal(extract(rows).status, 1);
  }
});
test('failed, skipped or retried required scenarios are never hidden by later success', () => {
  for (const status of ['failure', 'error', 'skipped', 'flakyFailure', 'flakyError', 'rerunFailure', 'rerunError']) {
    const rows = records(); rows.find(row => row.name === memory).status = status;
    assert.equal(extract(rows).status, 1, status);
  }
  const rows = records(); rows.push(rows[1]);
  assert.equal(extract(rows).status, 1);
});
test('Windows still requires each independent LPAC case alongside compiled memory', () => {
  for (const missing of windows) {
    const result = extract(records('win32').filter(row => row.name !== missing), 'win32');
    assert.equal(result.status, 1); assert.match(result.stderr, /required Native testcase is missing/);
  }
});
test('external entities refuse before parsing or receipt output', () => {
  assert.equal(extract([], 'linux', 'x64', '<!DOCTYPE x><testsuites/>').status, 1);
  assert.equal(extract([], 'linux', 'x64', '<!ENTITY x><testsuites/>').status, 1);
  // The existing 64MiB report limit is checked inside the exporter. Do not
  // allocate that entire fixture in every routine Node source test.
});

test('bounded logs refuse oversized captured output instead of trimming evidence', () => {
  const rows = records(); rows[1].output += '\n' + 'x'.repeat(64 * 1024);
  const result = extract(rows);
  assert.equal(result.status, 1); assert.match(result.stderr, /log exceeds 64 KiB/);
});
test('unrelated successful cases cannot replace either required producing scenario', () => {
  const rows = records().filter(row => row.name !== memory);
  rows.push({ name: 'extension_host::tests::memory_cap_stops_a_bun_host', output: records()[1].output });
  const result = extract(rows);
  assert.equal(result.status, 1); assert.match(result.stderr, /required Native testcase is missing/);
});
