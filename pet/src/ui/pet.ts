import { PetWorld, type PetInteraction } from '../core/pet-world.js';
import { compilePetTelemetry, decodePetJSONL, validatePetBucket, type PetBucket } from '../core/pet-telemetry.js';
import { petDemoEvents } from '../core/pet-demo.js';
import { importTrace } from '../core/ingest.js';
import { layout } from '../core/pet-sim.js';
import { renderPetPCM, type PetVoice } from '../core/pet-audio.js';
import { TraceLibrary, type SavedHabitat } from './storage.js';

const get = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const canvas = get<HTMLCanvasElement>('tank'), ctx = canvas.getContext('2d')!;
const mode = get<HTMLSelectElement>('mode'), seek = get<HTMLInputElement>('seek'), motion = get<HTMLInputElement>('motion');
const message = get('message'), source = get('source');
const media = matchMedia('(prefers-reduced-motion: reduce)'); motion.checked = media.matches;
media.addEventListener('change', () => { motion.checked = media.matches; rebuild(seconds); });
let points: [number, number][] = [], tape: readonly PetBucket[] = [], interactions: PetInteraction[] = [];
let expressionVersion: 1 | 2 = 2;
let world: PetWorld, seconds = 0, last = 0, accumulator = 0, paused = false;
let restoring = false, generation = 0, liveGeneration = 0, liveTimer = 0;
let imported: { tape: readonly PetBucket[]; interactions: PetInteraction[]; name: string; expressionVersion: 1 | 2 } | undefined;
const library = new TraceLibrary();
let savedRevision: number | undefined, saving = false, persistenceReady = false, persistenceFailed = false;
let audio: AudioContext | undefined, anchor = 0, sound = false;
const playing = new Set<AudioBufferSourceNode>();
const duration = () => Number(seek.max);
const timeLabel = (n: number) => `${Math.floor(n / 60)}:${String(Math.floor(n % 60)).padStart(2, '0')}`;

function silence() { for (const node of playing) { try { node.stop(); } catch { /* Already ended. */ } } playing.clear(); }
function play(voices: readonly PetVoice[]) {
  if (!audio || !sound || paused) return;
  for (const voice of voices) {
    const start = Math.floor(voice.start * audio.sampleRate), length = Math.ceil(voice.duration * audio.sampleRate) + 2;
    const pcm = renderPetPCM([voice], start, length, audio.sampleRate);
    const buffer = audio.createBuffer(2, length, audio.sampleRate);
    buffer.copyToChannel(pcm.left, 0); buffer.copyToChannel(pcm.right, 1);
    const node = audio.createBufferSource(); node.buffer = buffer; node.connect(audio.destination);
    const at = anchor + start / audio.sampleRate;
    if (at + buffer.duration < audio.currentTime) continue;
    playing.add(node); node.onended = () => { playing.delete(node); node.disconnect(); };
    node.start(Math.max(audio.currentTime, at), Math.max(0, audio.currentTime - at));
  }
}
async function rebuild(to = 0, checkpoint?: import('../core/pet-world.js').PetWorldCheckpoint) {
  const ticket = ++generation;
  silence(); const next = checkpoint ? PetWorld.restore(points, tape, interactions, checkpoint) : new PetWorld(points, tape, interactions, expressionVersion); restoring = true;
  canvas.setAttribute('aria-busy', 'true');
  for (const id of ['save', 'attention', 'feed']) get<HTMLButtonElement>(id).disabled = true;
  const ticks = Math.round(Math.max(0, Math.min(86_400, to)) * 30);
  if (checkpoint && checkpoint.tick !== ticks) throw new Error('Saved pet clock does not match its checkpoint.');
  for (let i = checkpoint?.tick ?? 0; i < ticks; i++) {
    next.step(1 / 30, { motion: !motion.checked, sensitivity: 1 });
    if (i > 0 && i % 600 === 0) { await new Promise(resolve => setTimeout(resolve, 0)); if (ticket !== generation) return; }
  }
  if (ticket !== generation) return;
  world = next; expressionVersion = next.sim.expressionVersion; restoring = false;
  canvas.setAttribute('aria-busy', 'false');
  for (const id of ['save', 'attention', 'feed']) get<HTMLButtonElement>(id).disabled = false;
  seconds = ticks / 30; accumulator = 0; last = 0;
  if (audio) anchor = audio.currentTime - seconds + .08;
  draw();
}
function stopFollowing() { liveGeneration++; clearTimeout(liveTimer); get<HTMLButtonElement>('pause').disabled = false; seek.disabled = false; }
async function persist() {
  if (!world || restoring || saving || !persistenceReady || persistenceFailed) return;
  saving = true;
  try {
    const snapshot: SavedHabitat = { petPersistenceVersion: 1, seconds, source: mode.value === 'live' ? 'replay' : mode.value as SavedHabitat['source'],
      sourceName: mode.value === 'live' ? 'Saved live recording' : source.textContent ?? '', still: motion.checked,
      tape: world.tape, interactions: world.interactions, checkpoint: world.checkpoint(), expressionVersion: world.sim.expressionVersion };
    savedRevision = await library.saveHabitat(snapshot, savedRevision); get('persistence').textContent = 'Habitat saved on this device.';
  }
  catch (error) { persistenceFailed = true; get('persistence').textContent = error instanceof Error ? error.message : 'Unable to save the habitat. Save a replay file to keep it.'; }
  finally { saving = false; }
}
function draw() {
  if (!world) return;
  const w = canvas.clientWidth, h = canvas.clientHeight, dpr = Math.min(2, devicePixelRatio || 1);
  if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) { canvas.width = Math.round(w * dpr); canvas.height = Math.round(h * dpr); }
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0); ctx.clearRect(0, 0, w, h);
  const frame = world.frame, state = frame.state, sim = world.sim, l = layout(w, h - 65, state);
  const t = motion.checked ? 0 : seconds;
  ctx.lineWidth = 1;
  for (let i = 0; i < 6; i++) {
    ctx.strokeStyle = `rgba(98,169,190,${.025 + .016 * frame.caustic})`; ctx.beginPath();
    for (let x = 0; x <= w; x += 8) { const y = h * .82 + Math.sin(x / 110 + i + t * .18) * 8 + i * 6; if (x === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y); } ctx.stroke();
  }
  ctx.strokeStyle = '#24424f'; ctx.beginPath(); ctx.moveTo(24, h * (.08 + frame.surface * .008)); ctx.lineTo(w - 24, h * (.08 + frame.surface * .008)); ctx.stroke();
  const f = sim.frame;
  ctx.fillStyle = ctx.strokeStyle = `rgba(${Math.round(f.r)},${Math.round(f.g)},${Math.round(f.b)},${f.alpha})`;
  for (const q of sim.p) {
    ctx.beginPath(); ctx.arc(l.ox + q.x * l.scale * l.flipX, l.oy + q.y * l.scale, Math.max(.7, l.dot * .31), 0, Math.PI * 2);
    if (state.observed < .92) ctx.stroke(); else ctx.fill();
  }
  if (frame.food) { ctx.fillStyle = `rgba(210,198,146,${frame.food.life})`; ctx.beginPath(); ctx.arc(w * (.5 + frame.food.x * .3), h * (.5 + frame.food.y * .3), 3, 0, Math.PI * 2); ctx.fill(); }
  get('channel').textContent = `${f.channel} · ${f.arch}${state.observed < .92 ? ' · unobserved' : ''}`;
  get('behaviour').textContent = `${frame.behaviour}${frame.needs !== 'none' ? ' · awaiting input' : ''}${frame.pod.length >= 3 ? ` · ${frame.pod.filter(p => p.present).length} peers present` : ''}`;
  canvas.setAttribute('aria-label', `Dot whale: ${f.channel}, ${f.arch}, ${frame.behaviour}${state.observed < .92 ? ', telemetry unobserved' : ''}.`);
  seek.value = String(seconds); get('clock').textContent = timeLabel(seconds); get('end').textContent = timeLabel(duration());
}
function interact(kind: PetInteraction['kind'], x = .2, y = -.15) {
  if (restoring) return;
  world.interact(kind, x, y); interactions = [...world.interactions];
  if (mode.value === 'replay' && imported) imported.interactions = [...interactions];
}
canvas.addEventListener('pointerdown', event => { const r = canvas.getBoundingClientRect(); interact('attention', Math.max(-1, Math.min(1, (event.clientX - r.left) / r.width * 2 - 1)), Math.max(-1, Math.min(1, (event.clientY - r.top) / r.height * 2 - 1))); });
get('attention').onclick = () => interact('attention'); get('feed').onclick = () => interact('food');
get('pause').onclick = () => { paused = !paused; get('pause').textContent = paused ? 'Play' : 'Pause'; get('pause').setAttribute('aria-pressed', String(paused)); silence(); if (audio) anchor = audio.currentTime - seconds + .08; last = 0; };
get('sound').onclick = async () => {
  try { if (!audio) audio = new AudioContext(); await audio.resume(); sound = !sound; silence(); anchor = audio.currentTime - seconds + .08;
    get('sound').textContent = sound ? 'Sound on' : 'Sound off'; get('sound').setAttribute('aria-pressed', String(sound));
  } catch { message.textContent = 'Audio is unavailable in this browser. The visual replay remains available.'; }
};
motion.onchange = () => rebuild(seconds); seek.oninput = () => rebuild(Number(seek.value));
mode.onchange = () => {
  stopFollowing();
  if (mode.value === 'replay' && imported) {
    expressionVersion = imported.expressionVersion; tape = imported.tape; interactions = [...imported.interactions]; source.textContent = imported.name;
    seek.max = String(Math.max(90, tape.length * .4)); rebuild(); return;
  }
  expressionVersion = 2;
  tape = mode.value === 'demo' ? compilePetTelemetry(petDemoEvents(), 80_000) : [];
  interactions = []; seek.max = mode.value === 'demo' ? '80' : '120';
  source.textContent = mode.value === 'demo' ? 'Event demo · synthetic telemetry' : 'Wild · simulated creature'; rebuild();
  message.textContent = mode.value === 'demo' ? 'Synthetic event-v1 telemetry uses the same derivation as imported traces.' : 'Wild mode is a simulated creature. Import event-v1, OTLP, or Codewhale telemetry to see work.';
};
get<HTMLInputElement>('file').onchange = async event => {
  const file = (event.target as HTMLInputElement).files?.[0]; if (!file) return;
  try {
    if (file.size > 64 * 1024 * 1024) throw new Error('File exceeds 64 MiB.');
    const text = await file.text(); let replay: unknown;
    try { replay = JSON.parse(text); } catch { /* JSONL is decoded below. */ }
    let nextInteractions: PetInteraction[] = [], nextTape: PetBucket[], nextExpressionVersion: 1 | 2 = 2;
    if (replay && typeof replay === 'object' && 'petReplayVersion' in replay) {
      const r = replay as { petReplayVersion: number; expressionVersion?: 1 | 2; tape: unknown; interactions: PetInteraction[] };
      if (r.petReplayVersion !== 1 || !Array.isArray(r.tape) || !Array.isArray(r.interactions)) throw new Error('Invalid pet replay.');
      nextExpressionVersion = r.expressionVersion === undefined ? 1 : r.expressionVersion;
      if (![1, 2].includes(nextExpressionVersion)) throw new Error('Unsupported pet expression version.');
      nextTape = decodePetJSONL(r.tape.map(b => JSON.stringify(b)).join('\n')); nextInteractions = r.interactions;
    } else {
      const first = replay ?? JSON.parse(text.split(/\r?\n/).find(line => line.trim()) || '{}');
      if (first && typeof first === 'object' && 'version' in first && first.version === 1 && 'simTimeMs' in first) nextTape = decodePetJSONL(text);
      else { const traces = importTrace(text, file.name, { privacy: 'metadata' });
        if (traces.length !== 1) throw new Error('Import a single trace. Select and export it in Whalesong first.');
        nextTape = compilePetTelemetry(traces[0].events, traces[0].duration); }
    }
    // Validate before replacing the currently playing world.
    new PetWorld(points, nextTape, nextInteractions, nextExpressionVersion);
    stopFollowing();
    expressionVersion = nextExpressionVersion; tape = nextTape; interactions = nextInteractions; mode.value = 'replay'; seek.max = String(Math.max(90, tape.length * .4));
    imported = { tape, interactions: [...interactions], name: `Local replay · ${file.name}`, expressionVersion };
    mode.querySelector<HTMLOptionElement>('[value="replay"]')!.disabled = false;
    source.textContent = `Local replay · ${file.name}`; message.textContent = 'Replay loaded locally. Saved replays include interactions and audio onsets.'; rebuild();
  } catch (error) { message.textContent = error instanceof Error ? error.message : 'Unable to import this file.'; }
};
get('save').onclick = () => {
  const blob = new Blob([JSON.stringify({ petReplayVersion: 1, expressionVersion: world.sim.expressionVersion, source: mode.value, tape: world.tape, interactions: world.interactions })], { type: 'application/json' });
  const url = URL.createObjectURL(blob), a = document.createElement('a'); a.href = url; a.download = 'codewhale-pet-replay.json'; a.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
};
get('follow').onclick = async () => {
  // A browser-granted read handle is the same local JSONL seam native hosts use.
  // It is never persisted: reloading restores the accepted recording, not access.
  const picker = (window as Window & { showOpenFilePicker?: () => Promise<{ getFile(): Promise<File> }[]> }).showOpenFilePicker;
  if (!picker) { message.textContent = 'This browser cannot follow local files. Import a tape to replay it, or use the native pet for live telemetry.'; return; }
  try {
    const [handle] = await picker.call(window); if (!handle) return;
    const file = await handle.getFile();
    stopFollowing(); const ticket = liveGeneration;
    mode.value = 'live'; paused = false; get('pause').textContent = 'Pause'; get('pause').setAttribute('aria-pressed', 'false');
    get<HTMLButtonElement>('pause').disabled = true; seek.disabled = true;
    expressionVersion = 2; tape = compilePetTelemetry([]); interactions = []; seek.max = '120';
    source.textContent = `Live local tape · ${file.name}`; await rebuild();
    let lastSequence = -1, lastSize = -1;
    const poll = async () => {
      if (ticket !== liveGeneration) return;
      try {
        const current = await handle.getFile();
        const tail = await current.slice(Math.max(0, current.size - 262_144)).text();
        if (ticket !== liveGeneration) return;
        if (restoring) { liveTimer = window.setTimeout(poll, 400); return; }
        if (current.size < lastSize) lastSequence = -1;
        lastSize = current.size;
        if (tail.endsWith('\n')) {
          const packet = JSON.parse(tail.trimEnd().split('\n').at(-1) ?? '{}'); validatePetBucket(packet);
          if (packet.sequence > lastSequence) { world.acceptTelemetry(packet); tape = world.tape; lastSequence = packet.sequence; }
          message.textContent = 'Following local telemetry. Unchanged or unavailable input becomes an unobserved gap.';
        }
      } catch { if (ticket === liveGeneration) message.textContent = 'Local tape unavailable or invalid · unobserved. Select the file again if it was replaced.'; }
      if (ticket === liveGeneration) liveTimer = window.setTimeout(poll, 400);
    };
    await poll();
  } catch (error) { if (!(error instanceof DOMException && error.name === 'AbortError')) message.textContent = 'Unable to open this local tape.'; }
};
document.addEventListener('visibilitychange', () => { last = 0; silence(); void persist(); if (audio) anchor = audio.currentTime - seconds + .08; });
window.addEventListener('pagehide', () => { void persist(); });
function animate(now: number) {
  if (world && !restoring && !paused && !document.hidden) {
    accumulator += last ? Math.min(.1, (now - last) / 1000) : 0;
    while (accumulator >= 1 / 30 && (mode.value === 'live' || !tape.length || seconds < duration())) {
      world.step(1 / 30, { motion: !motion.checked, sensitivity: 1 }); seconds = world.frame.timeMs / 1000; play(world.voices); accumulator -= 1 / 30;
    }
    if ((mode.value === 'live' || !tape.length) && seconds >= duration()) seek.max = String(Math.ceil(seconds / 30) * 30 + 30);
    draw();
  }
  last = now; requestAnimationFrame(animate);
}
try {
  const response = await fetch('./whale-points.tsv'); if (!response.ok) throw new Error('Whale point asset is unavailable.');
  points = (await response.text()).trim().split('\n').map(row => row.split(/\s+/).map(Number) as [number, number]);
  if (points.length !== 980 || points.some(p => p.length !== 2 || !p.every(Number.isFinite))) throw new Error('Invalid whale point asset.');
  try {
    const saved = await library.getHabitat();
    if (saved) {
      savedRevision = saved.revision; const h = saved.habitat;
      if (h.petPersistenceVersion !== 1 || !Number.isFinite(h.seconds) || h.seconds < 0 || h.seconds > 86_400
        || !['wild', 'demo', 'replay'].includes(h.source) || typeof h.still !== 'boolean' || typeof h.sourceName !== 'string') throw new Error('Saved habitat is invalid. Save a replay file before replacing it.');
      expressionVersion = h.expressionVersion === undefined ? 1 : h.expressionVersion;
      if (![1, 2].includes(expressionVersion) || h.checkpoint && (h.checkpoint.sim?.expressionVersion ?? 1) !== expressionVersion) throw new Error('Saved expression version does not match its checkpoint.');
      tape = h.tape; interactions = [...h.interactions]; mode.value = h.source; motion.checked = h.still || media.matches;
      source.textContent = h.sourceName;
      seek.max = h.source === 'demo' ? '80' : String(Math.max(h.source === 'replay' ? 90 : 120, h.seconds, tape.length * .4));
      if (h.source === 'replay') {
        imported = { tape, interactions, name: h.sourceName, expressionVersion }; mode.querySelector<HTMLOptionElement>('[value="replay"]')!.disabled = false;
      }
      message.textContent = 'Restoring the saved habitat…';
      await rebuild(h.seconds, h.still === motion.checked ? h.checkpoint : undefined);
      message.textContent = 'Habitat restored locally. Sound starts only when you enable it.';
    } else await rebuild();
    persistenceReady = true;
  } catch (error) { persistenceFailed = true; get('persistence').textContent = error instanceof Error ? error.message : 'Local persistence is unavailable.'; await rebuild(); }
  setInterval(() => { void persist(); }, 5000); requestAnimationFrame(animate);
} catch (error) { message.textContent = error instanceof Error ? error.message : 'Unable to start the habitat.'; }
