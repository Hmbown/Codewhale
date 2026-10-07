// Actual Windows Terminal clipboard acceptance on disposable GitHub runners.
// Uses a synthetic home and loopback provider; no credentials or paid inference.
// This qualifies clipboard delivery in the runner's interactive session, not
// every Windows desktop, IME, remote-desktop client or terminal distribution.
import assert from 'node:assert/strict';
import { createHash, randomUUID } from 'node:crypto';
import fs from 'node:fs/promises';
import http from 'node:http';
import path from 'node:path';
import { spawn, execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';

assert.equal(process.platform, 'win32', 'requires a real Windows desktop');
assert.equal(process.env.GITHUB_ACTIONS, 'true', 'disposable GitHub runner only');
const [binaryArg, evidenceArg] = process.argv.slice(2);
assert.ok(binaryArg && evidenceArg, 'usage: node windows-terminal-clipboard.mjs BINARY EVIDENCE_DIR');
const binary = path.resolve(binaryArg);
const evidence = path.resolve(evidenceArg);
await fs.mkdir(evidence, { recursive: true });
const root = await fs.mkdtemp(path.join(process.env.RUNNER_TEMP, 'cw-clipboard-'));
const qaHome = path.join(root, 'home');
const workspace = path.join(root, 'workspace');
const terminalRoot = path.join(root, 'terminal');
const title = `Codewhale clipboard ${randomUUID()}`;
const model = 'gpt-4o-mini';
const expected = Array.from({ length: 24 }, (_, n) => `cw-clipboard-${String(n + 1).padStart(2, '0')} 漢字 🐳 line ${n + 1}`).join('\n');
const requests = [];
const receipt = { source: process.env.GITHUB_SHA, binary, result: 'failed', lineCount: 24, provider: 'loopback fixture', title };
// One budget per desktop stage, shared with windows-terminal-clipboard.ps1.
// The deadlines below are backstops for a desktop script that is stuck and
// cannot report: each is the sum of the stages it covers plus slack, so a
// stage that runs out always fails first, with its own name and screenshot.
const stageSeconds = 45;
// Terminal window, focus, composer, focus; slack for PowerShell start-up.
const pasteBackstop = (4 * stageSeconds + 60) * 1000;
// Submit marker seen, focus, rendered reply.
const completedBackstop = (3 * stageSeconds + 30) * 1000;
const script = fileURLToPath(new URL('./windows-terminal-clipboard.ps1', import.meta.url));
let desktop;
let desktopEnv;
let desktopExit;
let desktopError = '';
const server = http.createServer(async (req, res) => {
  if (req.method === 'GET' && req.url === '/v1/models') {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ object: 'list', data: [{ id: model, object: 'model', owned_by: 'fixture' }] }));
    return;
  }
  if (req.method !== 'POST' || req.url !== '/v1/chat/completions') {
    res.writeHead(404); res.end(); return;
  }
  try {
    const chunks = []; let size = 0;
    for await (const chunk of req) {
      size += chunk.length;
      assert.ok(size <= 2 * 1024 * 1024, 'fixture request exceeds limit');
      chunks.push(chunk);
    }
    const body = JSON.parse(Buffer.concat(chunks));
    const users = (body.messages ?? []).filter(m => m.role === 'user').map(m => typeof m.content === 'string'
      ? m.content : (m.content ?? []).filter(c => c.type === 'text').map(c => c.text).join('\n'));
    requests.push(users);
    // Retain only synthetic user input; never dump headers or system prompts.
    await fs.writeFile(path.join(evidence, 'provider-user-messages.json'), JSON.stringify(requests, null, 2));
    const response = { id: 'clipboard-fixture', object: 'chat.completion', created: 1, model,
      choices: [{ index: 0, message: { role: 'assistant', content: 'cw-clipboard-ok' }, finish_reason: 'stop' }],
      usage: { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 } };
    if (body.stream) {
      res.writeHead(200, { 'Content-Type': 'text/event-stream', 'Cache-Control': 'no-cache' });
      res.write(`data: ${JSON.stringify({ ...response, object: 'chat.completion.chunk', choices: [{ index: 0, delta: { role: 'assistant', content: 'cw-clipboard-ok' }, finish_reason: null }] })}\n\n`);
      res.write(`data: ${JSON.stringify({ ...response, object: 'chat.completion.chunk', choices: [{ index: 0, delta: {}, finish_reason: 'stop' }] })}\n\n`);
      res.end('data: [DONE]\n\n');
    } else {
      res.writeHead(200, { 'Content-Type': 'application/json' }); res.end(JSON.stringify(response));
    }
  } catch (error) { desktopError ||= String(error); res.writeHead(500); res.end(); }
});

const desktopStopped = () => Boolean(desktop) && (desktop.exitCode !== null || desktop.signalCode !== null);
async function until(check, label, milliseconds) {
  const deadline = Date.now() + milliseconds;
  while (Date.now() < deadline) {
    const result = await check();
    if (result) return result;
    if (desktopStopped()) {
      await desktopExit; // resolves on 'close', after stderr has been read in full
      throw new Error(`desktop stopped (exit ${desktop.exitCode}) while waiting for ${label}: ${desktopError.trim()}`);
    }
    await delay(100);
  }
  throw new Error(`backstop deadline (${milliseconds / 1000}s) waiting for ${label}; the desktop script was still running: ${desktopError.trim()}`);
}
const exists = async name => fs.access(path.join(evidence, name)).then(() => true, () => false);
const readEvidence = async name => (await fs.readFile(path.join(evidence, name), 'utf8')).replace(/^\uFEFF/, '');
// Runs before the desktop script is stopped, so a stuck desktop is captured
// as it is. Nothing here can turn a failure into a pass.
async function diagnose() {
  try {
    receipt.desktopStages = (await readEvidence('stages.log')).trim().split(/\r?\n/);
    receipt.desktopStage = receipt.desktopStages.at(-1);
  } catch { receipt.desktopStage = 'none reported'; }
  try { receipt.desktopFailure = JSON.parse(await readEvidence('failure.json')); } catch { /* the script did not get to report */ }
  if (desktopStopped() && await exists('failure-desktop.png')) return;
  try {
    execFileSync('powershell.exe', ['-NoProfile', '-STA', '-File', script, '-Evidence', evidence, '-Title', title, '-DiagnoseOnly', 'backstop'],
      { env: desktopEnv, timeout: 60_000, stdio: 'ignore' });
    receipt.backstopDiagnostics = 'backstop-desktop.png, backstop-windows.json';
  } catch (error) { receipt.backstopDiagnostics = `not captured: ${error}`; }
}
try {
  receipt.binarySha256 = createHash('sha256').update(await fs.readFile(binary)).digest('hex');
  receipt.version = execFileSync(binary, ['--version'], { encoding: 'utf8' }).trim();
  for (const dir of [workspace, path.join(qaHome, '.codewhale'), path.join(qaHome, '.deepseek'), path.join(qaHome, 'AppData', 'Roaming'), path.join(qaHome, 'AppData', 'Local')]) await fs.mkdir(dir, { recursive: true });
  const config = 'telemetry = false\n[notifications]\nmethod = "off"\ncompletion_sound = "off"\n';
  for (const folder of ['.codewhale', '.deepseek']) await fs.writeFile(path.join(qaHome, folder, 'config.toml'), config);
  await fs.writeFile(path.join(evidence, 'expected.txt'), expected.replaceAll('\n', '\r\n'));
  // Official Microsoft ZIP, pinned by release and GitHub's asset SHA-256.
  const terminalUrl = 'https://github.com/microsoft/terminal/releases/download/v1.25.2733.0/Microsoft.WindowsTerminal_1.25.2733.0_x64.zip';
  const terminalHash = 'bf3ef2012f6c44d8340a4c58125acc9498d19b580f9890dc043cdf831852e796';
  const download = await fetch(terminalUrl, { signal: AbortSignal.timeout(60_000) });
  assert.equal(download.status, 200, 'official Terminal download');
  const bytes = Buffer.from(await download.arrayBuffer());
  assert.equal(createHash('sha256').update(bytes).digest('hex'), terminalHash, 'Terminal asset digest');
  const zip = path.join(root, 'terminal.zip'); await fs.writeFile(zip, bytes);
  await fs.mkdir(terminalRoot);
  execFileSync('tar.exe', ['-xf', zip, '-C', terminalRoot], { timeout: 60_000 });
  const files = await fs.readdir(terminalRoot, { recursive: true });
  const executables = files.filter(f => path.basename(f).toLowerCase() === 'windowsterminal.exe');
  assert.equal(executables.length, 1, 'unique portable Terminal executable');
  const terminal = path.join(terminalRoot, executables[0]);
  const terminalDir = path.dirname(terminal);
  await fs.writeFile(path.join(terminalDir, '.portable'), '');
  await fs.mkdir(path.join(terminalDir, 'settings'), { recursive: true });
  const profile = '{9e4b713f-d69c-4206-85fb-98663b729f17}';
  await fs.writeFile(path.join(terminalDir, 'settings', 'settings.json'), JSON.stringify({
    defaultProfile: profile, initialCols: 110, initialRows: 38, confirmCloseAllTabs: false,
    // The payload is trusted fixture text; this tests Codewhale's paste path,
    // excluding Terminal's own optional multiline confirmation dialog.
    multiLinePasteWarning: false, largePasteWarning: false,
    profiles: { defaults: { font: { size: 12 } }, list: [{ guid: profile, name: 'Codewhale fixture', commandline: 'cmd.exe' }] },
    actions: [{ command: 'paste', keys: 'ctrl+shift+v' }],
  }, null, 2));
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const baseUrl = `http://127.0.0.1:${server.address().port}/v1`;
  const env = Object.fromEntries(Object.entries(process.env).filter(([k]) => /^(PATH|SYSTEMROOT|WINDIR|COMSPEC|TEMP|TMP|PATHEXT|SYSTEMDRIVE|PROGRAMFILES|PROGRAMFILES\(X86\)|PROGRAMW6432|PROCESSOR_ARCHITECTURE|NUMBER_OF_PROCESSORS)$/i.test(k)));
  Object.assign(env, {
    HOME: qaHome, USERPROFILE: qaHome, APPDATA: path.join(qaHome, 'AppData', 'Roaming'), LOCALAPPDATA: path.join(qaHome, 'AppData', 'Local'),
    XDG_CONFIG_HOME: path.join(qaHome, '.config'), XDG_DATA_HOME: path.join(qaHome, '.local', 'share'), XDG_CACHE_HOME: path.join(qaHome, '.cache'),
    CODEWHALE_CONFIG_PATH: path.join(qaHome, '.codewhale', 'config.toml'), DEEPSEEK_CONFIG_PATH: path.join(qaHome, '.deepseek', 'config.toml'),
    CODEWHALE_TELEMETRY: '0', CODEWHALE_NO_UPDATE_CHECK: '1', CODEWHALE_DISABLE_MODELS_DEV_FETCH: '1', CODEWHALE_DISABLE_LOCAL_OLLAMA_PROBE: '1', NO_ANIMATIONS: '1',
  });
  receipt.terminal = { version: '1.25.2733.0', sha256: terminalHash, distribution: 'official portable x64' };
  desktopEnv = env;
  desktop = spawn('powershell.exe', ['-NoProfile', '-STA', '-File', script, terminal, binary, workspace, baseUrl, title, evidence, '-StageSeconds', String(stageSeconds)], { env, windowsHide: false, stdio: ['ignore', 'pipe', 'pipe'] });
  desktop.stdout.on('data', data => { void fs.appendFile(path.join(evidence, 'desktop.log'), data); });
  desktop.stderr.on('data', data => { desktopError += data; void fs.appendFile(path.join(evidence, 'desktop-error.log'), data); });
  desktopExit = new Promise((resolve, reject) => { desktop.once('error', reject); desktop.once('close', resolve); });
  await until(() => exists('pasted.json'), 'actual clipboard paste', pasteBackstop);
  assert.equal(requests.length, 0, 'pasting must not submit any partial turn');
  receipt.requestsBeforeEnter = 0;
  await fs.writeFile(path.join(evidence, 'submit'), 'enter once');
  await until(() => exists('completed.json'), 'rendered fixture reply', completedBackstop);
  await delay(1000);
  assert.equal(requests.length, 1, 'one Enter must submit exactly one turn');
  const matching = requests[0].filter(text => text.includes('cw-clipboard-01'));
  assert.equal(matching.length, 1, 'one user message contains the pasted input');
  const normalized = matching[0].replaceAll('\r\n', '\n');
  assert.ok(normalized.includes(expected), 'all24 Unicode lines arrive contiguously and unchanged');
  for (let line = 1; line <= 24; line++) assert.equal(normalized.split(`cw-clipboard-${String(line).padStart(2, '0')}`).length - 1, 1, `line${line} occurs once`);
  assert.equal(await desktopExit, 0, desktopError);
  assert.equal(desktopError, '', 'no desktop or provider fixture errors');
  receipt.requestsAfterEnter = 1;
  receipt.result = 'passed';
} catch (error) {
  receipt.error = String(error); process.exitCode = 1;
  if (desktop) await diagnose();
} finally {
  if (desktop && !desktopStopped()) { desktop.kill(); await desktopExit.catch(() => {}); }
  server.closeAllConnections(); await new Promise(resolve => server.close(resolve));
  await fs.writeFile(path.join(evidence, 'receipt.json'), JSON.stringify(receipt, null, 2));
  console.log(JSON.stringify(receipt));
}
