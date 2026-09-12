import { PetWorld, type PetInteraction, type PetSegment } from './pet-world.js';
import { compilePetTelemetry, decodePetJSONL, PetLiveTape } from './pet-telemetry.js';
import { ARCH_OF, digest, layout } from './pet-sim.js';
import { renderPetPCM, type PetVoice } from './pet-audio.js';
import { PetEngineTelemetry } from './pet-engine.js';

/** Synchronous native boundary: JSON state and directly transferable PCM.
 * Native hosts share the actual world / score implementation, not a rewrite. */
export class PetNative {
  private world: PetWorld;
  private engine = new PetEngineTelemetry();
  private engineTick = 0;
  private segment?: PetSegment;
  private liveTape = new PetLiveTape();
  constructor(pointsJSON: string, tapeJSONL = '', interactionsJSON = '[]', live = false, expressionVersion: 1 | 2 = 2) {
    const points = JSON.parse(pointsJSON) as [number, number][];
    if (!Array.isArray(points) || points.length !== 980 || points.some(p => !Array.isArray(p) || p.length !== 2 || !p.every(n => Number.isFinite(n) && Math.abs(n) <= 1)))
      throw new Error('Invalid native whale body.');
    this.world = new PetWorld(points, live ? compilePetTelemetry([]) : decodePetJSONL(tapeJSONL), JSON.parse(interactionsJSON) as PetInteraction[], expressionVersion, true);
  }
  step(dt: number, motion: boolean): string { this.world.step(dt, { motion, sensitivity: 1 }); return this.snapshot(); }
  snapshot(): string { return JSON.stringify({ ...this.world.frame, voices: this.world.voices, digest: digest(this.world.sim) }); }
  interact(kind: PetInteraction['kind'], x: number, y: number): void { this.world.interact(kind, x, y); }
  interactions(): string { return JSON.stringify(this.world.interactions); }
  accept(packet: string): void { this.world.acceptTelemetry(JSON.parse(packet)); }
  acceptLiveTail(text: string): boolean {
    const packet = this.liveTape.readTail(text); if (!packet) return false;
    this.world.acceptTelemetry(packet); return true;
  }
  resetLiveInput(): number { this.liveTape.reset(); return this.resumeEngine(); }
  recording(withCheckpoint = false): string { return JSON.stringify(this.world.recording(withCheckpoint)); }
  needsSegment(): boolean { return this.world.needsSegment; }
  prepareSegment(): string { this.segment = this.world.prepareSegment(); return JSON.stringify(this.segment.recording); }
  commitSegment(): void { if (!this.segment) throw new Error('No pet segment was prepared.'); this.segment.commit(); this.segment = undefined; }
  checkpoint(): string { return JSON.stringify(this.world.checkpoint()); }
  recordingChunk(index: number, completed = false): string | null { return this.world.recordingChunk(index, completed); }
  restoreCheckpoint(text: string): void {
    if (text.length > 512 * 1024) throw new Error('Pet checkpoint exceeds its size limit.');
    this.restoreRecording(JSON.stringify({ ...this.world.recording(false), checkpoint: JSON.parse(text) }));
  }
  restoreRecording(text: string): void {
    if (text.length > 8 * 1024 * 1024) throw new Error('Native habitat exceeds 8 MiB.');
    const points = this.world.sim.p.map(p => [p.hx, p.hy] as [number, number]);
    this.world = PetWorld.fromRecording(points, JSON.parse(text));
    this.liveTape.reset();
    this.segment = undefined; this.engineTick = Math.round(this.world.frame.timeMs * 30 / 1000); this.engine = new PetEngineTelemetry();
  }
  /** Resume a live host at the first unrecorded bucket. Keep every accepted
   * interval, but never present its last observed frame as current evidence. */
  resumeEngine(): number {
    this.world.resumeObservation();
    this.engineTick = Math.round(this.world.frame.timeMs * 30 / 1000);
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
  pcmChannels(voicesJSON: string, startSample: number, length: number, rate: number): [Float32Array, Float32Array] {
    const p = renderPetPCM(JSON.parse(voicesJSON) as PetVoice[], startSample, length, rate);
    return [p.left, p.right];
  }
  pcm(voicesJSON: string, startSample: number, length: number, rate: number): string {
    return JSON.stringify(this.pcmChannels(voicesJSON, startSample, length, rate).map(channel => Array.from(channel)));
  }
}
