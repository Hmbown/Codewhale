#!/usr/bin/env node
/** Read-only adapter: existing Whalesong ingestion is the only event parser. */
import { readFile, stat, open } from 'node:fs/promises';
import { watch } from 'node:fs';
import { basename, dirname } from 'node:path';
import { importTrace } from '../dist/core/ingest.js';
import { compilePetTelemetry, encodePetJSONL, encodePetTSV } from '../dist/core/pet-telemetry.js';
import { petDemoEvents } from '../dist/core/pet-demo.js';
import { followRuntime } from './lib/pet-runtime.mjs';

const args = process.argv.slice(2);
const option = name => args.find(a => a.startsWith(`--${name}=`))?.slice(name.length + 3);
if (args.includes('--help')) {
  console.log('node scripts/pet.mjs --input=trace.jsonl --output=pet.jsonl [--trace=ID] [--format=jsonl|tsv] [--watch]\nnode scripts/pet.mjs --runtime=http://127.0.0.1:7878 --thread=ID --output=pet.jsonl\nUse --demo instead of --input for synthetic telemetry. Output must not already exist.\nRuntime reads only the existing local event journal. Optional authentication comes from CODEWHALE_RUNTIME_TOKEN; never put a token in the URL. No agent or provider is started.');
  process.exit(0);
}
let output, monitor, timer, runtime;
try {
  for (const a of args) if (!['--demo', '--watch'].includes(a) && !/^--(input|output|trace|format|runtime|thread)=.+/.test(a)) throw new Error('Unknown or empty option. Use --help.');
  const input = option('input'), runtimeURL = option('runtime'), path = option('output'), format = option('format') ?? 'jsonl', live = args.includes('--watch') || !!runtimeURL;
  if (!path || [!!input, args.includes('--demo'), !!runtimeURL].filter(Boolean).length !== 1
    || !['jsonl', 'tsv'].includes(format) || live && format !== 'jsonl' || args.includes('--watch') && !input
    || !!runtimeURL !== !!option('thread') || option('trace') && !input)
    throw new Error('Choose one input source, an unused --output path, and JSONL for live recording. Runtime requires --thread.');
  const load = async () => {
    if (!input) return { events: petDemoEvents(), duration: 80_000 };
    if ((await stat(input)).size > 64 * 1024 * 1024) throw new Error('Input exceeds 64 MiB.');
    const traces = importTrace(await readFile(input, 'utf8'), input, { privacy: 'metadata' });
    const trace = option('trace') ? traces.find(t => t.id === option('trace')) : traces.length === 1 ? traces[0] : undefined;
    if (!trace) throw new Error('Select an existing --trace ID when input contains multiple traces.');
    return trace;
  };
  let trace = runtimeURL ? undefined : await load(), buckets = compilePetTelemetry(trace?.events ?? [], trace?.duration ?? 0);
  output = await open(path, 'wx', 0o600);
  if (runtimeURL) runtime = await followRuntime({ baseUrl: runtimeURL, threadId: option('thread'),
    token: process.env.CODEWHALE_RUNTIME_TOKEN, report: text => console.error(text) });
  if (!live) {
    await output.writeFile(format === 'tsv' ? encodePetTSV(buckets) : encodePetJSONL(buckets));
    await output.close(); output = undefined;
    console.log(`Wrote ${buckets.length} pet buckets (${args.includes('--demo') ? 'demo' : 'trace replay'}).`);
  } else {
    // The driver owns wall time. The core only sees recorded relative timestamps.
    const started = performance.now(), startedWall = Date.now();
    const origin = trace && 'originTime' in trace && trace.originTime ? Date.parse(trace.originTime) : NaN;
    const offset = Number.isFinite(origin) ? Math.max(0, Date.now() - origin) : trace?.duration ?? 0;
    let dirty = false, running = false, sequence = 0, failed = false, stopping = false, lastBin = -1;
    const empty = compilePetTelemetry([])[0];
    if (input) {
      monitor = watch(dirname(input), (_event, filename) => { if (!filename || String(filename) === basename(input)) dirty = true; });
      monitor.on('error', () => { failed = true; dirty = true; });
    }
    const tick = async () => {
      if (running || stopping) return;
      running = true;
      try {
        const elapsed = performance.now() - started, target = Math.floor(elapsed / 400);
        if (sequence > target) return;
        if (target >= 216_000) throw new Error('Start a new pet recording after 24 hours.');
        if (runtime) {
          failed = !runtime.connected;
          if (!failed) {
            try {
              trace = runtime.snapshot(startedWall + elapsed);
            } catch { failed = true; console.error('Runtime snapshot is invalid; recording an unobserved gap.'); }
          }
        }
        if (dirty) {
          dirty = false;
          try { trace = await load(); buckets = compilePetTelemetry(trace.events, trace.duration); failed = false; }
          catch { failed = true; console.error('Source unavailable or invalid; recording an unobserved gap.'); }
        }
        // A stalled host records skipped intervals as unknown instead of silently
        // compressing time. Never repeat onsets when timer jitter hits a source bin twice.
        while (sequence < target) {
          await output.write(encodePetJSONL([{ ...empty, sequence, simTimeMs: sequence * 400 }])); sequence++;
        }
        let state = empty;
        if (runtime) {
          // Seal the preceding observation interval before recording its state.
          // A fixed recorder origin survives imports discovering older starts.
          // Accepting this state one bucket later matches the foreground host.
          if (!failed && trace && sequence > 0) {
            try { state = compilePetTelemetry(trace.events, trace.duration,
              sequence - 1, startedWall - Date.parse(trace.originTime))[0] ?? empty; }
            catch { console.error('Runtime snapshot is invalid; recording an unobserved gap.'); }
          }
        } else {
          const bin = Math.floor((offset + elapsed) / 400);
          state = failed ? empty : buckets[bin] ?? empty;
          if (bin === lastBin) state = { ...state, onsets: Array(13).fill(0), errors: 0 };
          lastBin = bin;
        }
        await output.write(encodePetJSONL([{ ...state, sequence, simTimeMs: sequence * 400 }]));
        sequence++;
      } finally { running = false; }
    };
    await tick();
    timer = setInterval(() => { tick().catch(async error => { console.error(error.message); process.exitCode = 1; await stop(); }); }, 400);
    const stop = async () => {
      stopping = true; clearInterval(timer); monitor?.close(); await runtime?.close();
      while (running) await new Promise(resolve => setTimeout(resolve, 5));
      if (output) { await output.sync(); await output.close(); output = undefined; }
    };
    process.once('SIGINT', stop); process.once('SIGTERM', stop);
    console.log('Recording local live pet states. Ctrl+C to stop.');
  }
} catch (error) {
  console.error(error.message); process.exitCode = 1;
  clearInterval(timer); monitor?.close(); await runtime?.close(); if (output) await output.close();
}
