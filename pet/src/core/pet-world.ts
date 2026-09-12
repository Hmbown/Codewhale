import { clamp, stableHash } from './model.js';
import { PetSim, mulberry32, validatePetState, type PetOpts, type PetState, type PetSimCheckpoint } from './pet-sim.js';
import { validatePetBucket, type PetBucket } from './pet-telemetry.js';
import { PetScore, renderPetPCM, type PetVoice } from './pet-audio.js';

export type Behaviour = 'swim' | 'dive' | 'roll' | 'breathe' | 'drift' | 'doze' | 'wake';
export interface PetInteraction { timeMs: number; kind: 'attention' | 'food'; x: number; y: number }
export interface PodMember { id: string; slot: number; phase: number; present: boolean }
export interface WorldFrame {
  timeMs: number; behaviour: Behaviour; state: PetState; telemetry?: PetBucket;
  needs: 'none' | 'orient' | 'approach' | 'call'; pod: readonly PodMember[];
  surface: number; caustic: number; food: { x: number; y: number; life: number } | null;
}
export interface PetWorldCheckpoint {
  petCheckpointVersion: 1;
  /** Detects accidentally pairing a checkpoint with another recording. This
   * is a consistency checksum, not authentication of imported telemetry. */
  history: number;
  sim: PetSimCheckpoint;
  score: [number, number, boolean];
  accumulator: number; tick: number; bucketIndex: number; interactionIndex: number; branchTick: number;
  random: number; behaviour: Behaviour; until: number;
  targetX: number; targetY: number; x: number; y: number; flip: number; lit: number;
  lastActivity: number; addressedAt: number | null; waitSince: number;
  food: { time: number; x: number; y: number } | null;
  members: PodMember[]; lastStill: string;
  frame: WorldFrame; voices: PetVoice[];
}
const HZ = 30;
const seedFor = (name: string) => (0xC0FFEE ^ stableHash(name)) >>> 0;

/** Fixed-tick creature controller. Wall clocks and pointer APIs belong to drivers.
 * Reconstructing with the same tape and interactions is also the seek operation.
 * Particle, behaviour and identity randomness never consume one another's stream. */
export class PetWorld {
  readonly sim: PetSim;
  private tapeLog: PetBucket[];
  private tapeHashes: number[] = [];
  get tape(): readonly PetBucket[] { return this.tapeLog; }
  private interactionLog: PetInteraction[];
  get interactions(): readonly PetInteraction[] { return this.interactionLog.map(e => ({ ...e })); }
  frame: WorldFrame;
  voices: PetVoice[] = [];
  private score = new PetScore();
  private accumulator = 0;
  private tick = 0;
  private bucketIndex = -1;
  private interactionIndex = 0;
  private branchTick = -1;
  private random = mulberry32(seedFor('behaviour'));
  private behaviour: Behaviour = 'swim';
  private until = 6;
  private targetX = .35;
  private targetY = -.08;
  private x = 0;
  private y = 0;
  private flip = 1;
  private lit = 1;
  private lastActivity = 0;
  private addressedAt = -Infinity;
  private waitSince = -1;
  private food: { time: number; x: number; y: number } | null = null;
  private members = new Map<string, PodMember>();
  private lastStill = '';

  constructor(points: [number, number][], tape: readonly PetBucket[] = [], interactions: readonly PetInteraction[] = [], expressionVersion: 1 | 2 = 2) {
    if (tape.length > 216_000 || interactions.length > 100_000) throw new Error('Pet recording exceeds its input limit.');
    this.sim = new PetSim(points, 0xC0FFEE, expressionVersion);
    this.tapeLog = structuredClone([...tape]);
    this.interactionLog = structuredClone([...interactions]);
    for (let i = 0; i < this.tape.length; i++) {
      const b = this.tape[i];
      validatePetBucket(b);
      if (b.version !== 1 || b.sequence !== i || b.simTimeMs !== i * 400 || b.durationMs !== 400)
        throw new Error('World requires contiguous version 1 pet buckets.');
    }
    for (let i = 0; i < this.interactionLog.length; i++) {
      const e = this.interactionLog[i];
      if (!Number.isFinite(e.timeMs) || e.timeMs < 0 || i > 0 && e.timeMs < this.interactionLog[i - 1].timeMs
        || !['attention', 'food'].includes(e.kind) || !Number.isFinite(e.x) || !Number.isFinite(e.y)
        || Math.abs(e.x) > 1 || Math.abs(e.y) > 1) throw new Error('Invalid pet interaction.');
    }
    this.hashTape(0);
    this.frame = this.makeFrame(0);
    this.voices = this.score.voices(this.frame);
  }

  checkpoint(): PetWorldCheckpoint {
    return structuredClone({ petCheckpointVersion: 1, history: this.historyDigest(),
      sim: this.sim.checkpoint(), score: this.score.checkpoint(), accumulator: this.accumulator,
      tick: this.tick, bucketIndex: this.bucketIndex, interactionIndex: this.interactionIndex, branchTick: this.branchTick,
      random: this.random.state(), behaviour: this.behaviour, until: this.until,
      targetX: this.targetX, targetY: this.targetY, x: this.x, y: this.y, flip: this.flip, lit: this.lit,
      lastActivity: this.lastActivity, addressedAt: Number.isFinite(this.addressedAt) ? this.addressedAt : null,
      waitSince: this.waitSince, food: this.food, members: [...this.members.values()], lastStill: this.lastStill,
      frame: this.frame, voices: this.voices });
  }

  /** Lossless version 1 export, including the current pose and score. Drivers
   * consume all chunks synchronously on the world's owner before another tick.
   * Only a small slice is serialized inside an embedded runtime at a time. */
  recordingChunk(index: number): string | null {
    if (!Number.isSafeInteger(index) || index < 0) throw new Error('Invalid pet export cursor.');
    const size = 16, tapes = Math.ceil(this.tapeLog.length / size), inputs = Math.ceil(this.interactionLog.length / size);
    if (index === 0) return `{"petReplayVersion":1,"expressionVersion":${this.sim.expressionVersion},"tape":[`;
    if (index <= tapes) return (index === 1 ? '' : ',') + JSON.stringify(this.tapeLog.slice((index - 1) * size, index * size)).slice(1, -1);
    if (index === tapes + 1) return '],"interactions":[';
    const part = index - tapes - 2;
    if (part < inputs) return (part === 0 ? '' : ',') + JSON.stringify(this.interactionLog.slice(part * size, (part + 1) * size)).slice(1, -1);
    if (part === inputs) return `],"checkpoint":${JSON.stringify(this.checkpoint())}}`;
    return null;
  }

  /** Prefix states preserve the original FNV checksum byte for byte. Live
   * appends and replacements hash only the changed suffix, so checkpointing
   * does not rescan hours of accepted telemetry on the world worker. */
  private hashTape(from: number): void {
    for (let i = from; i < this.tapeLog.length; i++)
      this.tapeHashes[i] = stableHash((i ? ',' : '') + JSON.stringify(this.tapeLog[i]), this.tapeHashes[i - 1] ?? stableHash('[['));
  }
  private historyDigest(): number {
    let hash = stableHash('],[', this.tapeHashes.at(-1) ?? stableHash('[['));
    for (let i = 0; i < this.interactionLog.length; i++)
      hash = stableHash((i ? ',' : '') + JSON.stringify(this.interactionLog[i]), hash);
    return stableHash(']]', hash);
  }

  /** Hydrate a new world without stepping history. Validation finishes before
   * the caller receives it, so a corrupt checkpoint never mutates a live pet. */
  static restore(points: [number, number][], tape: readonly PetBucket[], interactions: readonly PetInteraction[], value: unknown): PetWorld {
    const c = value as PetWorldCheckpoint;
    const range = (n: number, low: number, high: number) => Number.isFinite(n) && n >= low && n <= high;
    const integer = (n: number, low: number, high: number) => Number.isSafeInteger(n) && range(n, low, high);
    if (!c || c.petCheckpointVersion !== 1 || JSON.stringify(c).length > 512 * 1024
      || !integer(c.tick, 0, 2_592_000) || !range(c.accumulator, -1e-8, 1 / HZ + 1e-8)
      || !integer(c.bucketIndex, -1, tape.length - 1) || !integer(c.interactionIndex, 0, interactions.length)
      || !integer(c.branchTick, -1, c.tick) || !integer(c.random, 0, 0xffffffff)
      || !['swim', 'dive', 'roll', 'breathe', 'drift', 'doze', 'wake'].includes(c.behaviour)
      || !range(c.until, 0, 86_410) || ![c.targetX, c.targetY, c.x, c.y, c.flip].every(n => range(n, -1, 1))
      || !range(c.lit, 0, 1) || !range(c.lastActivity, 0, c.tick / HZ)
      || c.addressedAt !== null && !range(c.addressedAt, 0, c.tick / HZ)
      || !range(c.waitSince, -1, c.tick / HZ) || typeof c.lastStill !== 'string' || c.lastStill.length > 2048
      || !Array.isArray(c.members) || c.members.length > 6
      || c.members.some(m => !m || typeof m.id !== 'string' || !m.id || m.id.length > 4096
        || !integer(m.slot, 0, 5) || !range(m.phase, 0, Math.PI * 2) || typeof m.present !== 'boolean')
      || new Set(c.members.map(m => m.id)).size !== c.members.length || new Set(c.members.map(m => m.slot)).size !== c.members.length
      || c.food !== null && (!c.food || !range(c.food.time, 0, c.tick / HZ) || ![c.food.x, c.food.y].every(n => range(n, -1, 1)))
      || !c.frame || c.frame.timeMs !== c.tick * 1000 / HZ || c.frame.behaviour !== c.behaviour
      || !['none', 'orient', 'approach', 'call'].includes(c.frame.needs)
      || !range(c.frame.surface, -.9, -.8) || !range(c.frame.caustic, 0, 1)
      || c.frame.food !== null && (!c.frame.food || !range(c.frame.food.x, -1, 1) || !range(c.frame.food.y, -1, 1.21) || !range(c.frame.food.life, 0, 1))
      || JSON.stringify(c.frame.pod) !== JSON.stringify(c.members)
      || !Array.isArray(c.voices) || c.voices.length > 1024
      || c.voices.some(v => !v || typeof v.id !== 'string' || v.id.length > 256))
      throw new Error('Invalid pet world checkpoint.');
    validatePetState(c.frame.state);
    renderPetPCM(c.voices, 0, 0);
    const world = new PetWorld(points, tape, interactions, c.sim?.expressionVersion ?? 1);
    if (c.history !== world.historyDigest()
      || c.bucketIndex >= 0 && world.tape[c.bucketIndex].simTimeMs > c.frame.timeMs + 1e-7
      // acceptTelemetry may fill past gaps after the most recent fixed tick.
      // Preserve that pending cursor exactly; it may lag only over empty gaps.
      || world.tape.some((b, i) => i > c.bucketIndex && b.simTimeMs <= c.frame.timeMs + 1e-7
        && (b.observed !== 0 || b.channel !== 'other' || b.errors || b.waiting || b.agentIds.length
          || b.onsets.some(Boolean) || b.activeMs.some(Boolean)))
      || world.interactionLog.slice(0, c.interactionIndex).some(e => e.timeMs > c.frame.timeMs + 1e-7)
      || world.interactionLog[c.interactionIndex]?.timeMs <= c.frame.timeMs + 1e-7)
      throw new Error('Pet checkpoint does not match its recording.');
    const candidate = world.tape[c.bucketIndex];
    const telemetry = candidate && c.frame.timeMs < candidate.simTimeMs + candidate.durationMs ? candidate : undefined;
    if (JSON.stringify(c.frame.telemetry) !== JSON.stringify(telemetry)) throw new Error('Pet checkpoint telemetry does not match its clock.');
    world.sim.restore(c.sim); world.score.restore(c.score); world.random.restore(c.random);
    world.accumulator = c.accumulator; world.tick = c.tick; world.bucketIndex = c.bucketIndex;
    world.interactionIndex = c.interactionIndex; world.branchTick = c.branchTick;
    world.behaviour = c.behaviour; world.until = c.until; world.targetX = c.targetX; world.targetY = c.targetY;
    world.x = c.x; world.y = c.y; world.flip = c.flip; world.lit = c.lit;
    world.lastActivity = c.lastActivity; world.addressedAt = c.addressedAt ?? -Infinity; world.waitSince = c.waitSince;
    world.food = structuredClone(c.food); world.members = new Map(c.members.map(m => [m.id, { ...m }]));
    world.lastStill = c.lastStill;
    // JSON omits undefined properties; keep the same frame shape as makeFrame.
    world.frame = { ...structuredClone(c.frame), telemetry: telemetry ? structuredClone(telemetry) : undefined };
    world.voices = structuredClone(c.voices);
    return world;
  }

  /** Branch at the current playhead; input is journalled for the next fixed tick.
   * A live touch never needs to re-simulate the creature's entire lifetime. */
  interact(kind: PetInteraction['kind'], x: number, y: number): void {
    if (!['attention', 'food'].includes(kind) || !Number.isFinite(x) || !Number.isFinite(y) || Math.abs(x) > 1 || Math.abs(y) > 1)
      throw new Error('Invalid pet interaction.');
    if (this.branchTick !== this.tick) this.interactionLog.splice(this.interactionIndex);
    this.branchTick = this.tick;
    this.interactionLog.push({ timeMs: (this.tick + 1) * 1000 / HZ, kind, x, y });
  }

  /** Accept a live source packet at the next 400ms boundary. The accepted tape,
   * including any missing intervals, is the exact replay authority for this host. */
  acceptTelemetry(input: PetBucket): void {
    validatePetBucket(input);
    const sequence = Math.floor(this.tick / 12) + 1;
    if (sequence >= 216_000) throw new Error('Start a new pet recording after 24 hours.');
    const changedFrom = Math.min(sequence, this.tapeLog.length);
    while (this.tapeLog.length <= sequence) {
      const at = this.tapeLog.length;
      this.tapeLog.push({ version: 1, sequence: at, simTimeMs: at * 400, durationMs: 400,
        activity: .12, coherence: .25, attention: 0, channel: 'other', observed: 0, roamX: 0, roamY: 0, flip: 1, lit: 1,
        onsets: Array(13).fill(0), activeMs: Array(13).fill(0), errors: 0, agentIds: [], waiting: false });
    }
    this.tapeLog[sequence] = { ...structuredClone(input), sequence, simTimeMs: sequence * 400 };
    this.hashTape(changedFrom);
  }

  /** dt is bounded so suspending a surface cannot cause an unbounded catch-up. */
  step(dt: number, opts: PetOpts = { motion: true, sensitivity: 1 }): WorldFrame {
    if (!Number.isFinite(dt) || dt < 0 || dt > 10) throw new Error('World dt must be in [0, 10] seconds.');
    this.accumulator += dt;
    this.voices = [];
    while (this.accumulator + 1e-10 >= 1 / HZ) {
      this.accumulator -= 1 / HZ;
      const time = ++this.tick / HZ;
      this.frame = this.makeFrame(time);
      this.voices.push(...this.score.voices(this.frame));
      if (!opts.motion) {
        this.frame.state = { ...this.frame.state, roamX: 0, roamY: 0, flip: 1, lit: this.behaviour === 'doze' ? .18 : 1 };
        this.frame.surface = -.86; this.frame.caustic = .5;
        if (this.frame.food) this.frame.food.y = this.food!.y;
      }
      const signature = JSON.stringify(this.frame.state);
      const peers = this.frame.pod.filter(p => p.present);
      const podSlots = peers.length >= 3 ? peers.map(p => [[0, 2, 4, 1, 3, 5][p.slot], p.phase] as const) : undefined;
      if (opts.motion || signature !== this.lastStill || podSlots) this.sim.step(1 / HZ, this.frame.state, { ...opts, podSlots });
      this.lastStill = signature;
    }
    return this.frame;
  }

  private makeFrame(time: number): WorldFrame {
    const timeMs = this.tick * 1000 / HZ;
    while (this.bucketIndex + 1 < this.tape.length && this.tape[this.bucketIndex + 1].simTimeMs <= timeMs + 1e-7)
      this.bucketIndex++;
    const candidate = this.tape[this.bucketIndex];
    const telemetry = candidate && timeMs < candidate.simTimeMs + candidate.durationMs ? candidate : undefined;
    while (this.interactionIndex < this.interactionLog.length && this.interactionLog[this.interactionIndex].timeMs <= timeMs + 1e-7) {
      const e = this.interactionLog[this.interactionIndex++];
      this.addressedAt = time; this.lastActivity = time;
      this.targetX = e.x * .65; this.targetY = e.y * .65;
      if (e.kind === 'food') this.food = { time, x: e.x, y: e.y };
    }
    const busy = telemetry && telemetry.observed >= .92 && telemetry.activity > .2;
    if (busy) this.lastActivity = time;
    if (telemetry?.waiting) { if (this.waitSince < 0) this.waitSince = time; }
    else this.waitSince = -1;
    const waitingFor = this.waitSince < 0 ? 0 : time - this.waitSince;
    const needs = this.waitSince < 0 ? 'none' : waitingFor < 8 ? 'orient' : waitingFor < 25 ? 'approach' : 'call';
    const addressed = time - this.addressedAt < 2;
    if (this.behaviour === 'doze' && (busy || addressed)) { this.behaviour = 'wake'; this.until = time + 2; }
    else if (!busy && !addressed && time - this.lastActivity >= 75) this.behaviour = 'doze';
    else if (time >= this.until) {
      const choices: Behaviour[] = ['swim', 'swim', 'dive', 'roll', 'breathe', 'drift'];
      this.behaviour = choices[Math.floor(this.random() * choices.length)];
      this.until = time + 3 + this.random() * 7;
      this.targetX = (this.random() * 2 - 1) * .75;
      this.targetY = this.behaviour === 'dive' ? .6 : this.behaviour === 'breathe' ? -.65 : (this.random() * 2 - 1) * .4;
    }
    const sleeping = this.behaviour === 'doze';
    if (sleeping) { this.targetY = .65; this.targetX = .15; }
    if (this.sim.expressionVersion === 1 && (needs === 'approach' || needs === 'call')) { this.targetX = 0; this.targetY = .3; }
    const move = sleeping ? .004 : this.behaviour === 'drift' ? .006 : .018;
    const dx = this.targetX - this.x;
    this.x += dx * move; this.y += (this.targetY - this.y) * move;
    this.flip += ((Math.abs(dx) < .015 ? this.flip < 0 ? -1 : 1 : dx < 0 ? -1 : 1) - this.flip) * .035;
    this.lit += ((sleeping ? .18 : 1) - this.lit) * .03;
    const wild = !this.tape.length;
    const state: PetState = {
      activity: telemetry?.activity ?? (wild ? sleeping ? .05 : .18 : .12),
      coherence: telemetry?.coherence ?? (wild ? .94 : .25),
      attention: Math.max(telemetry?.attention ?? 0, addressed ? .85 : 0, this.sim.expressionVersion === 1 && needs === 'call' ? 1 : 0),
      // A touch changes orientation, never hides an instrumentation gap.
      channel: addressed ? 'human' : telemetry?.channel ?? 'other',
      observed: telemetry?.observed ?? (wild ? 1 : 0),
      roamX: this.x, roamY: this.y, flip: this.flip, lit: this.lit,
    };
    const activeIds = new Set(telemetry?.agentIds ?? []);
    for (const member of this.members.values()) member.present = activeIds.has(member.id);
    for (const id of activeIds) {
      if (!this.members.has(id)) {
        const vacant = [...this.members.values()].find(m => !m.present);
        const slot = this.members.size < 6 ? this.members.size : vacant?.slot;
        if (slot !== undefined) {
          if (this.members.size >= 6 && vacant) this.members.delete(vacant.id);
          this.members.set(id, { id, slot, phase: mulberry32(seedFor(`pod:${id}`))() * Math.PI * 2, present: true });
        }
      }
      const member = this.members.get(id); if (member) member.present = true;
    }
    return { timeMs, behaviour: this.behaviour, state, telemetry, needs,
      pod: [...this.members.values()].map(m => ({ ...m })),
      surface: -.86 + .012 * Math.sin(time * .8), caustic: .5 + .5 * Math.sin(time * .31),
      food: this.food && time - this.food.time < 5
        ? { x: this.food.x, y: this.food.y + (time - this.food.time) * .04, life: clamp(1 - (time - this.food.time) / 5, 0, 1) } : null };
  }
}
