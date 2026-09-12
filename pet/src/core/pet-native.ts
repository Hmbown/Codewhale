import { PetWorld, type PetInteraction } from './pet-world.js';
import { compilePetTelemetry, decodePetJSONL } from './pet-telemetry.js';
import { ARCH_OF, digest, layout } from './pet-sim.js';
import { renderPetPCM, type PetVoice } from './pet-audio.js';
import { PetEngineTelemetry } from './pet-engine.js';

/** Synchronous JSON boundary for JavaScriptCore and embedded JS runtimes.
 * Native hosts share the actual world / score implementation, not a rewrite. */
export class PetNative {
  private world: PetWorld;
  private engine = new PetEngineTelemetry();
  private engineTick = 0;
  constructor(pointsJSON: string, tapeJSONL = '', interactionsJSON = '[]', live = false) {
    const points = JSON.parse(pointsJSON) as [number, number][];
    if (!Array.isArray(points) || points.length !== 980 || points.some(p => !Array.isArray(p) || p.length !== 2 || !p.every(n => Number.isFinite(n) && Math.abs(n) <= 1)))
      throw new Error('Invalid native whale body.');
    this.world = new PetWorld(points, live ? compilePetTelemetry([]) : decodePetJSONL(tapeJSONL), JSON.parse(interactionsJSON) as PetInteraction[]);
  }
  step(dt: number, motion: boolean): string { this.world.step(dt, { motion, sensitivity: 1 }); return this.snapshot(); }
  snapshot(): string { return JSON.stringify({ ...this.world.frame, voices: this.world.voices, digest: digest(this.world.sim) }); }
  interact(kind: PetInteraction['kind'], x: number, y: number): void { this.world.interact(kind, x, y); }
  interactions(): string { return JSON.stringify(this.world.interactions); }
  accept(packet: string): void { this.world.acceptTelemetry(JSON.parse(packet)); }
  recording(withCheckpoint = false): string { return JSON.stringify({ petReplayVersion: 1, tape: this.world.tape,
    interactions: this.world.interactions, ...(withCheckpoint ? { checkpoint: this.world.checkpoint() } : {}) }); }
  checkpoint(): string { return JSON.stringify(this.world.checkpoint()); }
  restoreCheckpoint(text: string): void {
    if (text.length > 512 * 1024) throw new Error('Pet checkpoint exceeds its size limit.');
    this.restoreHistory(this.world.tape, this.world.interactions, JSON.parse(text));
  }
  restoreRecording(text: string): void {
    if (text.length > 8 * 1024 * 1024) throw new Error('Native habitat exceeds 8 MiB.');
    const r = JSON.parse(text);
    if (!r || r.petReplayVersion !== 1 || !Array.isArray(r.tape) || !Array.isArray(r.interactions)) throw new Error('Invalid native habitat.');
    this.restoreHistory(r.tape, r.interactions, r.checkpoint);
  }
  private restoreHistory(tape: PetWorld['tape'], interactions: PetWorld['interactions'], checkpoint: unknown): void {
    const points = this.world.sim.p.map(p => [p.hx, p.hy] as [number, number]);
    this.world = PetWorld.restore(points, tape, interactions, checkpoint);
    this.engineTick = Math.round(this.world.frame.timeMs * 30 / 1000);
    // A restored creature does not prove an Engine operation is still active.
    this.engine = new PetEngineTelemetry();
  }
  /** Resume a live host at the first unrecorded bucket. Keep every accepted
   * interval, but never present its last observed frame as current evidence. */
  resumeEngine(): number {
    const end = this.world.tape.length * 12;
    if (!end || end - this.engineTick > 24) throw new Error('Only a live recording can resume Engine observation.');
    while (this.engineTick < end) { this.world.step(1 / 30, { motion: false, sensitivity: 1 }); this.engineTick++; }
    this.engine = new PetEngineTelemetry();
    this.world.voices = [];
    return this.world.frame.timeMs;
  }
  observeEngine(metadataJSON: string, timeMs: number): void { this.engine.observe(JSON.parse(metadataJSON), timeMs); }
  advanceEngine(timeMs: number, motion: boolean, waiting: boolean): void {
    const target = Math.floor(timeMs * 30 / 1000 + 1e-8);
    if (!Number.isFinite(timeMs) || target < this.engineTick || target - this.engineTick > 300) throw new Error('Engine pet clock jump.');
    this.engine.confirmWaiting(timeMs, waiting);
    const voices: PetVoice[] = [];
    while (this.engineTick < target) {
      if ((this.engineTick + 1) % 12 === 0) this.world.acceptTelemetry(this.engine.bucket(Math.floor(this.engineTick / 12)));
      this.world.step(1 / 30, { motion, sensitivity: 1 });
      voices.push(...this.world.voices);
      this.engineTick++;
    }
    this.world.voices = voices;
  }
  terminal(width: number, height: number): string {
    if (![width, height].every(n => Number.isSafeInteger(n) && n >= 1 && n <= 512)) throw new Error('Invalid pet raster size.');
    const { sim, frame } = this.world, l = layout(width * 2, height * 4, frame.state);
    const cells = Array(width * height).fill(0), bits = [[1, 8], [2, 16], [4, 32], [64, 128]];
    sim.p.forEach((p, i) => {
      if (frame.state.observed < .92 && i % 2 === 1) return;
      const x = Math.round(l.ox + p.x * l.scale * l.flipX), y = Math.round(l.oy + p.y * l.scale);
      if (x >= 0 && x < width * 2 && y >= 0 && y < height * 4) cells[Math.floor(y / 4) * width + Math.floor(x / 2)] |= bits[y % 4][x % 2];
    });
    return JSON.stringify({ width, height, cells, timeMs: frame.timeMs, channel: frame.state.channel, arch: ARCH_OF[frame.state.channel],
      hollow: frame.state.observed < .92, dozing: frame.behaviour === 'doze', lit: frame.state.lit });
  }
  pcm(voicesJSON: string, startSample: number, length: number, rate: number): string {
    const p = renderPetPCM(JSON.parse(voicesJSON) as PetVoice[], startSample, length, rate);
    return JSON.stringify([Array.from(p.left), Array.from(p.right)]);
  }
}
