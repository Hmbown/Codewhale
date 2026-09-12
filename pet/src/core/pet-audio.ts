import { CHANNELS } from './pet-sim.js';
import { clamp, stableHash } from './model.js';
import type { WorldFrame } from './pet-world.js';

export interface PetVoice {
  id: string; start: number; duration: number; frequency: number;
  gain: number; pan: number; kind: 'tone' | 'noise';
}

/** Core emits score events; WebAudio / AVAudioEngine only present their PCM.
 * Calling at any display cadence produces the same score when all world ticks
 * are supplied. Calling twice for a world tick cannot retrigger a voice. */
export class PetScore {
  private lastWindow = -1;
  private lastSequence = -1;
  private lastAddress = false;
  checkpoint(): [number, number, boolean] { return [this.lastWindow, this.lastSequence, this.lastAddress]; }
  restore(value: unknown): void {
    if (!Array.isArray(value) || value.length !== 3
      || !value.slice(0, 2).every(n => Number.isSafeInteger(n) && n >= -1 && n <= 216_000) || typeof value[2] !== 'boolean')
      throw new Error('Invalid pet score checkpoint.');
    [this.lastWindow, this.lastSequence, this.lastAddress] = value as [number, number, boolean];
  }
  voices(frame: WorldFrame): PetVoice[] {
    const time = frame.timeMs / 1000, window = Math.floor((frame.timeMs + 1e-7) / 400);
    const out: PetVoice[] = [];
    const add = (id: string, frequency: number, duration: number, gain: number, pan = 0, delay = 0, kind: PetVoice['kind'] = 'tone') =>
      out.push({ id, start: time + delay, duration, frequency, gain, pan, kind });
    const t = frame.telemetry, fresh = t !== undefined && t.sequence !== this.lastSequence;
    if (fresh) {
      this.lastSequence = t.sequence;
      for (let c = 0; c < CHANNELS.length; c++) {
        const channel = CHANNELS[c], n = t.onsets[c];
        if (!n || channel.sustained || ['human', 'error'].includes(channel.key)) continue;
        add(`onset:${t.sequence}:${c}`, channel.freq, .24, .035 * Math.min(2, Math.sqrt(n)), (c / 12 - .5) * .7);
      }
      if (t.errors) add(`tear:${t.sequence}`, CHANNELS.find(c => c.key === 'error')!.freq, .22, .05, 0, 0, 'noise');
    }
    const address = frame.state.channel === 'human' && frame.state.attention > .5;
    if (address && (!this.lastAddress || fresh && t!.onsets[11] > 0)) {
      const frequency = CHANNELS[11].freq;
      for (let i = 0; i < 3; i++) add(`address:${frame.timeMs}:${i}`, frequency * [1, 1.25, 1.5][i], .23, .032, 0, i * .14);
    }
    this.lastAddress = address;
    if (window !== this.lastWindow) {
      this.lastWindow = window;
      if (frame.state.observed >= .92) {
        const channel = CHANNELS.find(c => c.key === frame.state.channel)!;
        if (channel.sustained && frame.behaviour !== 'doze') {
          const peers = frame.state.channel === 'agent' ? Math.max(1, frame.pod.filter(p => p.present).length) : 1;
          for (let i = 0; i < peers; i++) add(`sustain:${window}:${i}`, channel.freq * (1 + (i - (peers - 1) / 2) * .004),
            .44, (.02 + frame.state.activity * .02) / Math.sqrt(peers), peers === 1 ? 0 : i / (peers - 1) - .5);
        }
        if (frame.behaviour === 'doze' && window % 5 === 0) {
          add(`heart:${window}:0`, 49, .3, .035); add(`heart:${window}:1`, 49, .24, .023, 0, .33);
        }
        if (frame.needs === 'call' && window % 10 === 0) add(`call:${window}`, CHANNELS[11].freq * 1.5, .38, .03);
      }
    }
    return out;
  }
}

/** Sample-addressed noise: no global RNG, identical samples when chunked/seeking. */
function noise(seed: number, sample: number): number {
  let x = (seed + Math.imul(sample, 0x6D2B79F5)) >>> 0;
  x = Math.imul(x ^ x >>> 15, x | 1); x ^= x + Math.imul(x ^ x >>> 7, x | 61);
  return ((x ^ x >>> 14) >>> 0) / 2147483648 - 1;
}

export function renderPetPCM(voices: readonly PetVoice[], startSample: number, length: number, sampleRate = 48_000): { left: Float32Array; right: Float32Array } {
  if (!Number.isInteger(sampleRate) || sampleRate < 8000 || sampleRate > 96000
    || !Number.isSafeInteger(startSample) || startSample < 0 || !Number.isInteger(length) || length < 0 || length > sampleRate * 120)
    throw new Error('Invalid pet PCM range.');
  const left = new Float32Array(length), right = new Float32Array(length);
  for (const v of voices) {
    if (![v.start, v.duration, v.frequency, v.gain, v.pan].every(Number.isFinite)
      || v.start < 0 || v.duration <= 0 || v.duration > 10 || v.frequency <= 0 || v.frequency > sampleRate / 2
      || v.gain < 0 || v.gain > 1 || Math.abs(v.pan) > 1 || !['tone', 'noise'].includes(v.kind)) throw new Error('Invalid pet voice.');
    const first = Math.max(startSample, Math.ceil(v.start * sampleRate));
    const last = Math.min(startSample + length, Math.ceil((v.start + v.duration) * sampleRate));
    const pan = (v.pan + 1) * Math.PI / 4, seed = (0xC0FFEE ^ stableHash(v.id)) >>> 0;
    for (let absolute = first; absolute < last; absolute++) {
      const age = absolute / sampleRate - v.start;
      const envelope = Math.min(1, age / .015, (v.duration - age) / .045);
      const sample = v.kind === 'noise' ? noise(seed, absolute) * Math.exp(-age * 14)
        : Math.sin(2 * Math.PI * v.frequency * age) * .88 + Math.sin(4 * Math.PI * v.frequency * age) * .12;
      const value = sample * Math.max(0, envelope) * v.gain, at = absolute - startSample;
      left[at] += value * Math.cos(pan); right[at] += value * Math.sin(pan);
    }
  }
  // Limiting is a presentation operation and cannot perturb voice scheduling.
  for (let i = 0; i < length; i++) { left[i] = clamp(left[i], -1, 1); right[i] = clamp(right[i], -1, 1); }
  return { left, right };
}
