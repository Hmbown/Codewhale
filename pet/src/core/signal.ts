import { CATEGORIES, clamp, errorOnsetOf, quantile, type BinLevel, type Metric, type SignalPyramid, type WhaleEvent } from './model.js';

function emptyLevel(length: number, binMs: number): BinLevel {
  const n = CATEGORIES.length * length;
  return { binMs, length, onsets: new Float64Array(n), activeMs: new Float64Array(n),
    outputTokens: new Float64Array(n), cost: new Float64Array(n), errors: new Float64Array(n), peak: new Float64Array(n) };
}
const FIELDS = ['onsets', 'activeMs', 'outputTokens', 'cost', 'errors'] as const;
/** O(events + channels × bins), including long intervals. No span-length inner loop. */
export function buildPyramid(events: readonly WhaleEvent[], requestedDuration?: number, maxBins = 16_384): SignalPyramid {
  if (!Number.isInteger(maxBins) || maxBins < 16 || maxBins > 1_048_576) throw new Error('maxBins must be an integer in [16, 1048576].');
  let duration = requestedDuration ?? 1;
  for (const e of events) duration = Math.max(duration, e.endTime, e.startTime, e.status === 'error' ? errorOnsetOf(e) : 0);
  if (!Number.isFinite(duration) || duration < 0) throw new Error('Signal duration must be finite and nonnegative.');
  duration = Math.max(1, duration);
  const binMs = 2 ** Math.ceil(Math.log2(Math.max(1, duration / (maxBins - 1))));
  const length = Math.floor(duration / binMs) + 1, fine = emptyLevel(length, binMs);
  const stride = length + 1;
  const activeDiff = new Float64Array(CATEGORIES.length * stride), tokenDiff = new Float64Array(CATEGORIES.length * stride);
  for (const e of events) {
    if (!Number.isFinite(e.startTime) || !Number.isFinite(e.endTime) || e.startTime < 0 || e.endTime < e.startTime) throw new Error(`Invalid interval for ${e.id}.`);
    const channel = CATEGORIES.indexOf(e.category);
    if (channel < 0) throw new Error(`Unknown category for ${e.id}.`);
    const a = Math.floor(e.startTime / binMs), b = Math.floor(e.endTime / binMs);
    const at = channel * length, diff = channel * stride;
    fine.onsets[at + a]++;
    fine.cost[at + a] += e.cost ?? 0;
    if (e.status === 'error') fine.errors[at + Math.floor(errorOnsetOf(e) / binMs)]++;
    const d = e.endTime - e.startTime, tokens = e.outputTokens ?? 0;
    if (d === 0) { fine.outputTokens[at + a] += tokens; continue; }
    if (a === b) { fine.activeMs[at + a] += d; fine.outputTokens[at + a] += tokens; }
    else {
      const left = (a + 1) * binMs - e.startTime, right = e.endTime - b * binMs, tokenRate = tokens / d;
      fine.activeMs[at + a] += left;
      fine.activeMs[at + b] += right;
      fine.outputTokens[at + a] += left * tokenRate;
      fine.outputTokens[at + b] += right * tokenRate;
      if (b > a + 1) {
        activeDiff[diff + a + 1] += binMs; activeDiff[diff + b] -= binMs;
        tokenDiff[diff + a + 1] += binMs * tokenRate; tokenDiff[diff + b] -= binMs * tokenRate;
      }
    }
  }
  for (let c = 0; c < CATEGORIES.length; c++) {
    let active = 0, tokens = 0;
    for (let i = 0; i < length; i++) {
      active += activeDiff[c * stride + i]; tokens += tokenDiff[c * stride + i];
      const at = c * length + i;
      fine.activeMs[at] = Math.max(0, fine.activeMs[at] + active);
      fine.outputTokens[at] = Math.max(0, fine.outputTokens[at] + tokens);
      fine.peak[at] = fine.activeMs[at] / binMs;
    }
  }
  const levels = [fine];
  while (levels.at(-1)!.length > 1) {
    const child = levels.at(-1)!, parent = emptyLevel(Math.ceil(child.length / 2), child.binMs * 2);
    for (let c = 0; c < CATEGORIES.length; c++) for (let i = 0; i < parent.length; i++) {
      const a = c * child.length + i * 2, b = a + 1, dst = c * parent.length + i, hasB = i * 2 + 1 < child.length;
      for (const key of FIELDS) parent[key][dst] = child[key][a] + (hasB ? child[key][b] : 0);
      parent.peak[dst] = Math.max(child.peak[a], hasB ? child.peak[b] : 0);
    }
    levels.push(parent);
  }
  const p: SignalPyramid = { duration, channels: CATEGORIES, levels, calibration: { activity: 1, onsets: 1, tokens: 1, cost: 1 } };
  const reference = chooseLevel(p, duration / 1200);
  for (const metric of ['activity', 'onsets', 'tokens', 'cost'] as Metric[]) {
    const positives: number[] = [];
    for (let c = 0; c < CATEGORIES.length; c++) for (let i = 0; i < reference.length; i++) {
      const v = binValue(reference, c, i, metric); if (v > 0) positives.push(v);
    }
    p.calibration[metric] = Math.max(1e-9, quantile(positives, .95));
  }
  return p;
}
/** Choose the coarsest stored level no wider than one requested pixel interval. */
export function chooseLevel(p: SignalPyramid, targetBinMs: number): BinLevel {
  let result = p.levels[0];
  for (const level of p.levels) { if (level.binMs > targetBinMs) break; result = level; }
  return result;
}
export function binValue(level: BinLevel, channel: number, bin: number, metric: Metric): number {
  if (bin < 0 || bin >= level.length) return 0;
  const at = channel * level.length + bin;
  if (metric === 'activity') return level.activeMs[at] / level.binMs;
  if (metric === 'onsets') return level.onsets[at] * 1000 / level.binMs;
  if (metric === 'tokens') return level.outputTokens[at] * 1000 / level.binMs;
  return level.cost[at] * 1000 / level.binMs;
}
export function intensity(value: number, reference: number): number {
  return clamp(Math.log1p(value / Math.max(1e-9, reference) * 8) / Math.log(9), 0, 1);
}
/** Conserved totals: useful for tests and alternate native backends. */
export function totals(level: BinLevel): Record<string, number> {
  return Object.fromEntries(FIELDS.map(k => [k, level[k].reduce((s, x) => s + x, 0)]));
}

/** Sorted-start, max-end segment tree. Long root spans do not force a reverse
 * scan through every earlier event. Results are chronological and honor limits. */
export class IntervalIndex {
  readonly events: WhaleEvent[];
  private readonly maxEnd: Float64Array;
  private readonly leafCount: number;
  constructor(events: readonly WhaleEvent[]) {
    this.events = [...events].sort((a, b) => a.startTime - b.startTime || a.id.localeCompare(b.id));
    this.leafCount = 2 ** Math.ceil(Math.log2(Math.max(1, this.events.length)));
    this.maxEnd = new Float64Array(this.leafCount * 2).fill(-Infinity);
    for (let i = 0; i < this.events.length; i++) this.maxEnd[this.leafCount + i] = this.events[i].endTime;
    for (let i = this.leafCount - 1; i; i--) this.maxEnd[i] = Math.max(this.maxEnd[i * 2], this.maxEnd[i * 2 + 1]);
  }
  query(start: number, end: number, limit = Infinity): WhaleEvent[] {
    if (!Number.isFinite(start) || !Number.isFinite(end) || end < start || limit <= 0) return [];
    let lo = 0, hi = this.events.length;
    while (lo < hi) { const mid = (lo + hi) >>> 1; if (this.events[mid].startTime <= end) lo = mid + 1; else hi = mid; }
    const bound = lo, found: WhaleEvent[] = [];
    const visit = (node: number, left: number, right: number): void => {
      if (left >= bound || this.maxEnd[node] < start || found.length >= limit) return;
      if (right - left === 1) {
        const e = this.events[left];
        if (e && (e.endTime > start || e.startTime === e.endTime && e.startTime >= start)) found.push(e);
        return;
      }
      const mid = (left + right) >>> 1;
      visit(node * 2, left, mid); visit(node * 2 + 1, mid, right);
    };
    visit(1, 0, this.leafCount);
    return found;
  }
}
/** Sample an onset train into equal-width bins; nothing is inferred between impulses. */
export function onsetSeries(events: readonly WhaleEvent[], start: number, end: number, n = 256, category?: string): Float64Array {
  const out = new Float64Array(n), span = Math.max(1e-6, end - start);
  for (const e of events) if ((!category || e.category === category) && e.startTime >= start && e.startTime < end) {
    const at = Math.floor((e.startTime - start) / span * n); if (at >= 0 && at < n) out[at]++;
  }
  return out;
}
/** Centered, variance-normalized autocorrelation; lag-zero is one unless constant. */
export function autocorrelation(input: ArrayLike<number>, maxLag = 64): number[] {
  const n = input.length;
  if (!n) return [];
  let mean = 0; for (let i = 0; i < n; i++) mean += input[i]; mean /= n;
  const centered = Array.from(input, x => x - mean), energy = centered.reduce((s, v) => s + v * v, 0);
  const result: number[] = [];
  for (let lag = 0; lag <= Math.min(maxLag, n - 1); lag++) {
    let sum = 0; for (let i = 0; i < n - lag; i++) sum += centered[i] * centered[i + lag];
    result.push(energy > 1e-12 ? sum / energy : 0);
  }
  return result;
}
export interface Spectrum { frequencies: number[]; power: number[]; entropy: number; peakHz: number; sampleHz: number; resolutionHz: number }
/** Hann-window periodogram of an actual uniformly sampled onset signal, DC removed. */
export function periodogram(input: ArrayLike<number>, sampleHz: number): Spectrum {
  const n = input.length, frequencies: number[] = [], power: number[] = [];
  if (n < 4 || sampleHz <= 0) return { frequencies, power, entropy: 0, peakHz: 0, sampleHz, resolutionHz: 0 };
  let mean = 0; for (let i = 0; i < n; i++) mean += input[i]; mean /= n;
  const windowed = Array.from(input, (x, i) => (x - mean) * (.5 - .5 * Math.cos(2 * Math.PI * i / (n - 1))));
  const windowEnergy=Array.from({length:n},(_,i)=>(.5-.5*Math.cos(2*Math.PI*i/(n-1)))**2).reduce((a,b)=>a+b,0);
  for (let k = 1; k <= Math.floor(n / 2); k++) {
    let re = 0, im = 0;
    for (let t = 0; t < n; t++) { const angle = 2 * Math.PI * k * t / n; re += windowed[t] * Math.cos(angle); im -= windowed[t] * Math.sin(angle); }
    frequencies.push(k * sampleHz / n); power.push((re * re + im * im) / (windowEnergy * sampleHz) * (n%2===0&&k===n/2?1:2));
  }
  const sum = power.reduce((s, x) => s + x, 0);
  let entropy = 0; for (const x of power) if (x > 0 && sum > 0) { const p = x / sum; entropy -= p * Math.log2(p); }
  entropy = power.length > 1 ? entropy / Math.log2(power.length) : 0;
  const max = Math.max(...power);
  return { frequencies, power, entropy, peakHz: max > 1e-12 ? frequencies[power.indexOf(max)] : 0, sampleHz, resolutionHz: sampleHz / n };
}
export function unionDuration(events: readonly WhaleEvent[], start = 0, end = Infinity): number {
  const intervals = events.filter(e => e.endTime > e.startTime && e.endTime > start && e.startTime < end)
    .map(e => [Math.max(start, e.startTime), Math.min(end, e.endTime)]).sort((a, b) => a[0] - b[0]);
  let total = 0, left = 0, right = 0;
  for (const [a, b] of intervals) {
    if (a > right) { total += right - left; left = a; right = b; } else right = Math.max(right, b);
  }
  return total + right - left;
}
