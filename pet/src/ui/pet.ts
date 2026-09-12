import { PetWorld, PET_MAX_SECONDS, type PetInteraction, type PetWorldCheckpoint } from '../core/pet-world.js';
import { compilePetTelemetry, decodePetJSONL, PetLiveTape, type PetBucket } from '../core/pet-telemetry.js';
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
media.addEventListener('change', () => { motion.checked = media.matches; if (world) rebuild(seconds, undefined, world.recording(false).start); });
let points: [number, number][] = [], tape: readonly PetBucket[] = [], interactions: PetInteraction[] = [];
let expressionVersion: 1 | 2 = 2;
let worldMode = 'wild';
let world: PetWorld, seconds = 0, last = 0, accumulator = 0, paused = false;
let restoring = false, generation = 0, liveGeneration = 0, liveTimer = 0;
let imported: { world: PetWorld; name: string } | undefined;
const library = new TraceLibrary();
const liveTape = new PetLiveTape();
let liveObservation = 0;
let savedRevision: number | undefined, saving: Promise<boolean> | undefined, persistenceReady = false, persistenceFailed = false;
let audio: AudioContext | undefined, anchor = 0, sound = false;
const playing = new Set<AudioBufferSourceNode>();
const duration = () => Number(seek.max);
const timeLabel = (n: number) => {
  const minutes = Math.floor(n / 60), seconds = String(Math.floor(n % 60)).padStart(2, '0');
  return minutes < 60 ? `${minutes}:${seconds}` : `${Math.floor(minutes / 60)}:${String(minutes % 60).padStart(2, '0')}:${seconds}`;
};

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
async function rebuild(to = 0, checkpoint?: PetWorldCheckpoint, start?: PetWorldCheckpoint) {
  const ticket = ++generation;
  const next = start ? PetWorld.fromRecording(points, { petReplayVersion: 2, expressionVersion, tape, interactions, start, checkpoint })
    : checkpoint ? PetWorld.restore(points, tape, interactions, checkpoint) : new PetWorld(points, tape, interactions, expressionVersion, true);
  const ticks = Math.round(Math.max(next.startTimeMs / 1000, Math.min(PET_MAX_SECONDS, to)) * 30);
  if (checkpoint && checkpoint.tick !== ticks) throw new Error('Saved pet clock does not match its checkpoint.');
  silence(); restoring = true;
  canvas.setAttribute('aria-busy', 'true');
  for (const id of ['save', 'attention', 'feed']) get<HTMLButtonElement>(id).disabled = true;
  for (let i = Math.round(next.frame.timeMs * 30 / 1000); i < ticks; i++) {
    next.step(1 / 30, { motion: !motion.checked, sensitivity: 1 });
    if (i > 0 && i % 600 === 0) { await new Promise(resolve => setTimeout(resolve, 0)); if (ticket !== generation) return; }
  }
  if (ticket !== generation) return;
  adoptWorld(next);
}
function adoptWorld(next: PetWorld) {
  world = next; worldMode = mode.value; tape = world.tape; interactions = [...world.interactions];
  seek.min = String(next.startTimeMs / 1000); expressionVersion = next.sim.expressionVersion; restoring = false;
  if (mode.value === 'replay') imported = { world: next, name: imported?.name ?? source.textContent ?? 'Imported replay' };
  canvas.setAttribute('aria-busy', 'false');
  for (const id of ['save', 'attention', 'feed']) get<HTMLButtonElement>(id).disabled = false;
  seconds = next.frame.timeMs / 1000; accumulator = 0; last = 0;
  if (audio) anchor = audio.currentTime - seconds + .08;
  draw();
}
function stopFollowing() { liveGeneration++; clearTimeout(liveTimer); liveTape.reset(); get<HTMLButtonElement>('pause').disabled = false; seek.disabled = false; }
async function refreshArchives() {
  const list = get<HTMLSelectElement>('archive');
  const entries = await library.petArchives();
  list.replaceChildren(new Option('Earlier recordings…', ''));
  for (const entry of entries) list.add(new Option(`${entry.name} · ${timeLabel(entry.startSeconds)}–${timeLabel(entry.seconds)}`, String(entry.key)));
}
async function persist(archiveCurrent = false): Promise<boolean> {
  if (saving) {
    if (!archiveCurrent) return saving;
    await saving;
    return persist(true);
  }
  if (!world || restoring || !persistenceReady || persistenceFailed) return false;
  const task = saveCurrent(archiveCurrent); saving = task;
  try { return await task; }
  finally { if (saving === task) saving = undefined; }
}
async function saveCurrent(archiveCurrent: boolean): Promise<boolean> {
  try {
    const owner = world;
    const snapshot: SavedHabitat = { petPersistenceVersion: 1, seconds: owner.frame.timeMs / 1000,
      source: worldMode === 'live' ? 'replay' : worldMode as SavedHabitat['source'],
      sourceName: worldMode === 'live' ? 'Saved live recording' : source.textContent ?? '', still: motion.checked,
      ...owner.recording() };
    const segment = archiveCurrent || owner.needsSegment ? owner.prepareSegment() : undefined;
    savedRevision = await library.saveHabitat(segment ? { ...snapshot, ...segment.recording } : snapshot, savedRevision,
      archiveCurrent ? snapshot : segment ? { ...snapshot, ...segment.archive } : undefined);
    if (segment) {
      segment.commit();
      if (world === owner) { tape = owner.tape; interactions = [...owner.interactions]; seek.min = String(owner.startTimeMs / 1000); }
      void refreshArchives().catch(() => {});
    }
    get('persistence').textContent = 'Habitat saved on this device. Earlier recordings remain available.';
    return true;
  }
  catch (error) { persistenceFailed = true; get('persistence').textContent = error instanceof Error ? error.message : 'Unable to save the habitat. Save a replay file to keep it.'; return false; }
}
async function mayLeave(): Promise<boolean> {
  return await persist(true) || window.confirm('This visit could not be saved. Cancel to keep it and export a replay, or leave without saving its latest progress.');
}
function adoptImported(next: PetWorld, name: string) {
  stopFollowing(); mode.value = 'replay';
  imported = { world: next, name }; source.textContent = name;
  mode.querySelector<HTMLOptionElement>('[value="replay"]')!.disabled = false;
  seek.max = String(Math.max(90, next.endTimeMs / 1000));
  ++generation; silence(); adoptWorld(next);
}
get<HTMLSelectElement>('archive').onchange = async event => {
  const picker = event.target as HTMLSelectElement, key = Number(picker.value); picker.value = '';
  if (!key) return;
  try {
    const saved = await library.petArchive(key);
    const next = PetWorld.fromRecording(points, { ...saved, petReplayVersion: saved.petReplayVersion ?? 1 });
    if (!await mayLeave()) return;
    adoptImported(next, saved.sourceName); message.textContent = 'Earlier recording opened. Its original remains in local storage.';
  } catch (error) { message.textContent = error instanceof Error ? error.message : 'Unable to open this recording.'; }
};

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
}
canvas.addEventListener('pointerdown', event => { const r = canvas.getBoundingClientRect(); interact('attention', Math.max(-1, Math.min(1, (event.clientX - r.left) / r.width * 2 - 1)), Math.max(-1, Math.min(1, (event.clientY - r.top) / r.height * 2 - 1))); });
get('attention').onclick = () => interact('attention'); get('feed').onclick = () => interact('food');
get('pause').onclick = () => { paused = !paused; get('pause').textContent = paused ? 'Play' : 'Pause'; get('pause').setAttribute('aria-pressed', String(paused)); silence(); if (audio) anchor = audio.currentTime - seconds + .08; last = 0; };
get('sound').onclick = async () => {
  try { if (!audio) audio = new AudioContext(); await audio.resume(); sound = !sound; silence(); anchor = audio.currentTime - seconds + .08;
    get('sound').textContent = sound ? 'Sound on' : 'Sound off'; get('sound').setAttribute('aria-pressed', String(sound));
  } catch { message.textContent = 'Audio is unavailable in this browser. The visual replay remains available.'; }
};
motion.onchange = () => rebuild(seconds, undefined, world.recording(false).start); seek.oninput = () => rebuild(Number(seek.value), undefined, world.recording(false).start);
mode.onchange = async () => {
  if (!await mayLeave()) { mode.value = worldMode; return; }
  stopFollowing();
  if (mode.value === 'replay' && imported) {
    tape = imported.world.tape; interactions = [...imported.world.interactions]; source.textContent = imported.name;
    seek.max = String(Math.max(90, imported.world.endTimeMs / 1000));
    ++generation; silence(); adoptWorld(imported.world);
    message.textContent = 'Returned to the imported world at its current pose.'; return;
  }
  expressionVersion = 2;
  tape = mode.value === 'demo' ? compilePetTelemetry(petDemoEvents(), 80_000) : [];
  interactions = []; seek.max = mode.value === 'demo' ? '80' : '120';
  source.textContent = mode.value === 'demo' ? 'Event demo · synthetic telemetry' : 'Wild · simulated creature'; rebuild();
  message.textContent = mode.value === 'demo' ? 'Synthetic event-v1 telemetry uses the same derivation as imported traces.' : 'Wild mode is a simulated creature. Import event-v1, OTLP, or Codewhale telemetry to see work.';
};
get<HTMLInputElement>('file').onchange = async event => {
  const input = event.target as HTMLInputElement, file = input.files?.[0]; if (!file) return;
  try {
    if (file.size > 64 * 1024 * 1024) throw new Error('Pet import exceeds 64 MiB.');
    const text = await file.text(); let replay: unknown;
    try { replay = JSON.parse(text); } catch { /* JSONL and trace imports follow below. */ }
    let next: PetWorld;
    if (replay && typeof replay === 'object' && 'petReplayVersion' in replay) next = PetWorld.fromRecording(points, replay);
    else {
      let nextTape: readonly PetBucket[];
      const first = replay ?? JSON.parse(text.split(/\r?\n/).find(line => line.trim()) || '{}');
      if (first && typeof first === 'object' && 'version' in first && first.version === 1 && 'simTimeMs' in first) nextTape = decodePetJSONL(text);
      else {
        const traces = importTrace(text, file.name, { privacy: 'metadata' });
        if (traces.length !== 1) throw new Error('Choose one recording to import.');
        nextTape = compilePetTelemetry(traces[0].events, traces[0].duration);
      }
      next = new PetWorld(points, nextTape, [], 2, true);
    }
    if (!await mayLeave()) return;
    adoptImported(next, `Local replay · ${file.name}`);
    message.textContent = 'Recording loaded locally, including its current pose, interactions and score.';
  } catch (error) { message.textContent = error instanceof Error ? error.message : 'Unable to import this file.'; }
  finally { input.value = ''; }
};
get('save').onclick = () => {
  try {
    const chunks: string[] = []; let bytes = 0;
    for (let index = 0; ; index++) {
      const chunk = world.recordingChunk(index); if (chunk === null) break;
      bytes += new TextEncoder().encode(chunk).length;
      if (bytes > 64 * 1024 * 1024) throw new Error('Recording exceeds the 64 MiB export limit. The current world was kept.');
      chunks.push(chunk);
    }
    const blob = new Blob(chunks, { type: 'application/json' });
    const url = URL.createObjectURL(blob), a = document.createElement('a'); a.href = url; a.download = 'codewhale-pet-replay.json'; a.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
  } catch (error) { message.textContent = error instanceof Error ? error.message : 'Unable to export the recording. The current world was kept.'; }
};
get('follow').onclick = async () => {
  // A browser-granted read handle is the same local JSONL seam native hosts use.
  // It is never persisted: reloading restores the accepted recording, not access.
  const picker = (window as Window & { showOpenFilePicker?: () => Promise<{ getFile(): Promise<File> }[]> }).showOpenFilePicker;
  if (!picker) { message.textContent = 'This browser cannot follow local files. Import a tape to replay it, or use the native pet for live telemetry.'; return; }
  try {
    const [handle] = await picker.call(window); if (!handle) return;
    const file = await handle.getFile();
    if (!await mayLeave()) return;
    stopFollowing(); const ticket = liveGeneration;
    mode.value = 'live'; paused = false; get('pause').textContent = 'Pause'; get('pause').setAttribute('aria-pressed', 'false');
    get<HTMLButtonElement>('pause').disabled = true; seek.disabled = true;
    expressionVersion = 2; tape = compilePetTelemetry([]); interactions = []; seek.max = '120';
    source.textContent = `Live local tape · ${file.name}`; await rebuild();
    const poll = async () => {
      if (ticket !== liveGeneration) return;
      if (document.hidden) { liveTimer = window.setTimeout(poll, 400); return; }
      const observation = liveObservation;
      try {
        const current = await handle.getFile();
        const tail = await current.slice(Math.max(0, current.size - 262_144)).text();
        if (ticket !== liveGeneration) return;
        if (restoring || document.hidden || observation !== liveObservation) { liveTimer = window.setTimeout(poll, 400); return; }
        const packet = liveTape.readTail(tail);
        if (packet) { world.acceptTelemetry(packet); tape = world.tape; }
        message.textContent = 'Following local telemetry. Existing or unchanged input stays unobserved until new packets arrive.';
      } catch { if (ticket === liveGeneration && observation === liveObservation) { liveTape.reset(); message.textContent = 'Local tape unavailable or invalid · unobserved. Select the file again if it was replaced.'; } }
      if (ticket === liveGeneration) liveTimer = window.setTimeout(poll, 400);
    };
    await poll();
  } catch (error) { if (!(error instanceof DOMException && error.name === 'AbortError')) message.textContent = 'Unable to open this local tape.'; }
};
document.addEventListener('visibilitychange', () => {
  last = 0; silence();
  if (worldMode === 'live') {
    liveObservation++;
    liveTape.reset();
    if (!document.hidden && world && !restoring) { world.resumeObservation(); seconds = world.frame.timeMs / 1000; }
  }
  void persist(); if (audio) anchor = audio.currentTime - seconds + .08;
});
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
      if (h.petPersistenceVersion !== 1 || !Number.isFinite(h.seconds) || h.seconds < 0 || h.seconds > PET_MAX_SECONDS
        || !['wild', 'demo', 'replay'].includes(h.source) || typeof h.still !== 'boolean' || typeof h.sourceName !== 'string') throw new Error('Saved habitat is invalid. Save a replay file before replacing it.');
      expressionVersion = h.expressionVersion === undefined ? 1 : h.expressionVersion;
      if (![1, 2].includes(expressionVersion) || h.checkpoint && (h.checkpoint.sim?.expressionVersion ?? 1) !== expressionVersion) throw new Error('Saved expression version does not match its checkpoint.');
      const savedWorld = PetWorld.fromRecording(points, { ...h, petReplayVersion: h.petReplayVersion === undefined ? 1 : h.petReplayVersion });
      if (h.checkpoint && savedWorld.frame.timeMs / 1000 !== h.seconds) throw new Error('Saved pet clock does not match its checkpoint.');
      tape = h.tape; interactions = [...h.interactions]; mode.value = h.source; motion.checked = h.still || media.matches;
      source.textContent = h.sourceName;
      seek.max = String(Math.max(h.source === 'demo' ? 80 : h.source === 'replay' ? 90 : 120, savedWorld.endTimeMs / 1000));
      if (h.source === 'replay') {
        mode.querySelector<HTMLOptionElement>('[value="replay"]')!.disabled = false;
      }
      message.textContent = 'Restoring the saved habitat…';
      if (h.checkpoint && h.still === motion.checked) adoptWorld(savedWorld);
      else await rebuild(h.seconds, undefined, savedWorld.recording(false).start);
      message.textContent = 'Habitat restored locally. Sound starts only when you enable it.';
    } else await rebuild();
    persistenceReady = true; void refreshArchives().catch(() => {});
  } catch (error) { persistenceFailed = true; get('persistence').textContent = error instanceof Error ? error.message : 'Local persistence is unavailable.'; await rebuild(); }
  void refreshArchives().catch(() => {});
  setInterval(() => { void persist(); }, 5000); requestAnimationFrame(animate);
} catch (error) { message.textContent = error instanceof Error ? error.message : 'Unable to start the habitat.'; }
