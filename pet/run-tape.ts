// Generate tape.tsv (maketape) or run it (run). Usage:
//   node run-tape.ts maketape > tape.tsv
//   node run-tape.ts run < tape.tsv > digests.txt
import { readFileSync } from 'node:fs';
import { PetSim, runTape, REST_STATE } from './PetSim.ts';
import type { PetState } from './PetSim.ts';

const DIR = new URL('.', import.meta.url).pathname;
const points = readFileSync(DIR + 'whale-points.tsv', 'utf8')
  .trim().split('\n').map(l => l.split('\t').map(Number) as [number, number]);

const lerp = (a: number, b: number, t: number) => a + (b - a) * t;

// The tape: the same beats as the demo session, as keyframes eased at 30fps.
const BEATS: Array<[number, Partial<PetState>]> = [
  [6,  { channel: 'reasoning',  activity: .12, coherence: .95, attention: 0,   observed: 1 }],
  [5,  { channel: 'tool',       activity: .62, coherence: .90 }],
  [7,  { channel: 'code',       activity: .70, coherence: .90 }],
  [5,  { channel: 'filesystem', activity: .64, coherence: .90 }],
  [5,  { channel: 'network',    activity: .60, coherence: .88 }],
  [3,  { channel: 'error',      activity: .80, coherence: .50 }],
  [5,  { channel: 'code',       activity: .72, coherence: .58 }],
  [5,  { channel: 'agent',      activity: .58, coherence: .86 }],
  [5,  { channel: 'agent',      activity: .50, coherence: .86, observed: .15 }],
  [7,  { channel: 'human',      activity: .50, coherence: .90, attention: .95, observed: 1 }],
  [8,  { channel: 'reasoning',  activity: .12, coherence: .95, attention: 0, roamX: .4, roamY: -.2, flip: -1, lit: .55 }],
  [6,  { channel: 'reasoning',  activity: .12, coherence: .95, lit: 1, roamX: 0, roamY: 0, flip: 1 }],
];

function maketape() {
  const lines = ['dt\tactivity\tcoherence\tattention\tchannel\tobserved\troamX\troamY\tflip\tlit'];
  const dt = 1 / 30;
  let cur: PetState = { ...REST_STATE, activity: .12, coherence: .95 };
  for (const [dur, patch] of BEATS) {
    const from = { ...cur };
    cur = { ...cur, ...patch };
    const frames = Math.round(dur * 30);
    for (let i = 0; i < frames; i++) {
      const e = Math.min(1, i / 42), m = e * e * (3 - 2 * e);   // 1.4s morph
      const s: PetState = { ...cur };
      for (const k of ['activity','coherence','attention','observed','roamX','roamY','flip','lit'] as const)
        s[k] = lerp((from as any)[k], (cur as any)[k], m);
      lines.push(`${dt}\t${s.activity}\t${s.coherence}\t${s.attention}\t${s.channel}\t${s.observed}\t${s.roamX}\t${s.roamY}\t${s.flip}\t${s.lit}`);
    }
  }
  console.log(lines.join('\n'));
}

if (process.argv[2] === 'maketape') {
  maketape();
} else {
  const rows = readFileSync(0, 'utf8').trim().split('\n');
  const sim = new PetSim(points, 0xC0FFEE, process.argv.includes('--legacy') ? 1 : 2);
  console.log(runTape(sim, rows, { motion: !process.argv.includes('--reduced-motion'), sensitivity: 1 }).join('\n'));
}
