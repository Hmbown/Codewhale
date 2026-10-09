import { stableHash } from './model.js';
import { DOT_GLYPHS } from './dot-glyphs.js';
import { DOT_BODY } from './dot-body.js';
import type { EngineOwnerProjection, OwnerActivityKind } from './pet-engine.js';

export interface DotField { points: number[][]; materials: number[][] }
type Verb = OwnerActivityKind | 'waiting' | 'done';
const TAU = Math.PI * 2;
const clamp = (n: number, lo = 0, hi = 1) => Math.max(lo, Math.min(hi, n));
const ease = (n: number) => { const u = clamp(n); return u * u * (3 - 2 * u); };
const WHALE_DOTS = 640, GLYPH_DOTS = 260;
const GLYPH_POINTS = Object.fromEntries(Object.entries(DOT_GLYPHS).map(([key, rows]) => [key,
  rows.flatMap((row, y) => [...row].flatMap((pixel, x) => pixel === '#' ? [[(x - 23.5) / 38, (y - 23.5) / 38]] : [])),
])) as Record<Verb, number[][]>;
const HUES: Record<Verb, number> = {
  reading: 185, editing: 190, searching: 194, testing: 201, executing: 198,
  browsing: 195, computer: 198, memory: 180, tool: 195, thinking: 185,
  responding: 188, delegating: 192, waiting: 38, done: 180,
};

function verb(a: EngineOwnerProjection, timeMs: number): Verb | null {
  if (a.freshness !== 'fresh' || !a.observed) return null;
  if (a.authoritativePresence === 'needs_you') return 'waiting';
  if (a.authoritativePresence === 'done') return timeMs - (a.observedAtMs ?? timeMs) < 2600 ? 'done' : null;
  if (a.authoritativePresence === 'idle') return null;
  return a.activityKind;
}

export function dotFieldKey(a: EngineOwnerProjection): string {
  return [a.freshness, a.authoritativePresence, a.activityKind, a.actionId,
    a.parallelAgentCount, a.doneEffectId, a.activeSpans.length === 0 && !!a.failedToolAge].join(':');
}

function pigment(hue: number, light: number): number[] {
  const h = ((hue % 360) + 360) % 360 / 60, c = (1 - Math.abs(2 * light - 1)) * .62;
  const x = c * (1 - Math.abs(h % 2 - 1)), m = light - c / 2;
  const rgb = h < 1 ? [c, x, 0] : h < 2 ? [x, c, 0] : h < 3 ? [0, c, x]
    : h < 4 ? [0, x, c] : h < 5 ? [x, 0, c] : [c, 0, x];
  return rgb.map(v => Math.round((v + m) * 255));
}

function stroke(path: number[][], u: number): number[] {
  const lengths = path.slice(1).map((p, i) => Math.hypot(p[0] - path[i][0], p[1] - path[i][1]));
  let at = clamp(u) * lengths.reduce((sum, n) => sum + n, 0);
  for (let i = 0; i < lengths.length; i++) {
    if (at <= lengths[i] || i === lengths.length - 1) {
      const f = lengths[i] ? at / lengths[i] : 0;
      return path[i].map((n, k) => n + (path[i + 1][k] - n) * f);
    }
    at -= lengths[i];
  }
  return path[0];
}

export function projectDotField(activity: EngineOwnerProjection,
  timeMs: number, still = false, expressionAtMs = 0): DotField {
  const kind = verb(activity, timeMs), identity = activity.actionId || activity.activityKind || 'whale';
  const seed = stableHash(identity), variant = (seed % 997) / 997;
  const t = still ? 0 : timeMs / 1000, cycle = still ? .43 : (t / (2.8 + variant * .6)) % 1;
  const sweep = -.63 + clamp(cycle / .82) * 1.26, scanRow = Math.floor(cycle * 4) % 4;
  const glyph = kind ? GLYPH_POINTS[kind] : null;
  const fresh = activity.freshness === 'fresh' && activity.observed;
  const failed = fresh && !activity.activeSpans.length && !!activity.failedToolAge;
  const phase = ((Math.max(0, timeMs - expressionAtMs) / 1000 + 1.7) % 8.4);
  const morph = !kind ? 0 : still ? 1 : kind === 'done'
    ? 1 - ease((timeMs - (activity.observedAtMs ?? timeMs) - 180) / 900)
    : ease(phase / 1.7) * (1 - ease((phase - 5.4) / .9));
  const points: number[][] = [], materials: number[][] = [];
  const lens = [Math.sin(cycle * TAU) * .30, -.25 + scanRow * .16];
  for (let i = 0; i < DOT_BODY.length; i++) {
    const grain = ((i * 37) % 101) / 100;
    const [hx, hy] = DOT_BODY[i];
    const tail = clamp((-hx - hy + .1) / .65);
    const homeX = hx * 1.9 + (still ? 0 : .008 * Math.sin(t * .75 + hy * 5));
    const homeY = hy * 1.9 + (still ? 0 : .009 * Math.sin(t * 1.1 + hx * 4)
      + tail * .012 * Math.sin(t * 1.55 + hx * 7));
    const bodyLight = .61 + .14 * clamp((hx + .45) / .9) + .035 * Math.cos(hy * 7);
    let x = homeX, y = homeY, light = bodyLight, alpha = fresh ? .88 : .42;
    if (glyph && kind && i >= WHALE_DOTS) {
      if (i < WHALE_DOTS + GLYPH_DOTS) {
        [x, y] = glyph[Math.floor((i - WHALE_DOTS) * glyph.length / GLYPH_DOTS)];
        let beam = Math.exp(-Math.pow((y - sweep) * 12, 2));
        if (kind === 'searching') beam = .18 + .35 * (Math.sin(t * 1.3) + 1) / 2;
        if (kind === 'editing' || kind === 'responding') beam = Math.exp(-Math.pow((x - sweep) * 14, 2));
        if (kind === 'executing' || kind === 'tool') beam = Math.exp(-Math.pow((x - sweep) * 10, 2));
        if (kind === 'waiting') beam = .12;
        if (kind === 'done') beam = still ? .35 : clamp(1 - (timeMs - (activity.observedAtMs ?? timeMs)) / 1600);
        light = .64 + beam * .20;
        alpha = .72 + beam * .26;
        if (kind === 'editing' && !still) y += .017 * beam * Math.sin(x * 24);
      } else {
        const count = DOT_BODY.length - WHALE_DOTS - GLYPH_DOTS, j = i - WHALE_DOTS - GLYPH_DOTS, u = j / Math.max(1, count - 1);
        const lane = j % 4, flow = still ? u : (u + cycle) % 1;
        light = .61 + grain * .16;
        alpha = .42 + grain * .24;
        switch (kind) {
          case 'reading': {
            if (u < .7) [x, y] = stroke([[-.76, -.72], [-.08, -.68], [0, -.61], [.08, -.68], [.76, -.72], [.76, .72], [.08, .76], [0, .70], [-.08, .76], [-.76, .72], [-.76, -.72]], u / .7);
            else { x = -.63 + (u - .7) / .3 * 1.26; y = sweep; alpha = .75; }
            break;
          }
          case 'searching': {
            if (u < .76) { const angle = u / .76 * TAU; x = lens[0] + Math.cos(angle) * .26; y = lens[1] + Math.sin(angle) * .26; }
            else [x, y] = stroke([[lens[0] + .18, lens[1] + .18], [lens[0] + .43, lens[1] + .43]], (u - .76) / .24);
            alpha = .92; light = .83; break;
          }
          case 'editing': {
            const tip = [sweep, .65];
            if (u < .8) [x, y] = stroke([tip, [tip[0] + .14, .32], [tip[0] + .23, .37], [tip[0] + .05, .66], tip], u / .8);
            else { x = -.65 + (sweep + .65) * (u - .8) / .2; y = .72 + .018 * Math.sin(x * 28); }
            alpha = .92; break;
          }
          case 'testing': {
            [x, y] = stroke([[-.75, -.70], [.75, -.70], [.75, .70], [-.75, .70], [-.75, -.70]], u);
            alpha = .32 + .65 * Math.exp(-Math.pow((u - cycle) * 14, 2)); break;
          }
          case 'executing': case 'tool': {
            if (u < .35) [x, y] = stroke([[-.89, -.2], [-.71, 0], [-.89, .2]], u / .35);
            else if (u < .7) [x, y] = stroke([[.71, -.2], [.89, 0], [.71, .2]], (u - .35) / .35);
            else { x = -.85 + flow * 1.7; y = .72 + lane * .025; alpha = .3 + .65 * flow; }
            break;
          }
          case 'browsing': {
            if (u < .82) [x, y] = stroke([[-.78, -.75], [.78, -.75], [.78, .75], [-.78, .75], [-.78, -.75], [-.78, -.64], [.78, -.64]], u / .82);
            else { x = .68; y = .58 - flow * 1.08; alpha = .85; }
            break;
          }
          case 'computer': {
            const at = stroke([[-.45, -.40], [.38, -.40], [.38, .35], [-.45, .35], [-.45, -.40]], cycle);
            [x, y] = stroke([[at[0], at[1]], [at[0], at[1] + .39], [at[0] + .10, at[1] + .28], [at[0] + .20, at[1] + .44], [at[0] + .27, at[1] + .40], [at[0] + .16, at[1] + .24], [at[0] + .32, at[1] + .24], [at[0], at[1]]], u);
            alpha = .92; light = .85; break;
          }
          case 'memory': {
            const angle = lane / 4 * TAU + .3, radius = .66 + flow * .22;
            x = Math.cos(angle) * radius; y = Math.sin(angle) * radius;
            alpha = .22 + .75 * (1 - flow); break;
          }
          case 'thinking': {
            const angle = -.8 * Math.PI + u * Math.PI * 1.6, radius = .76;
            x = Math.cos(angle) * radius; y = Math.sin(angle) * radius;
            alpha = .2 + .68 * Math.exp(-Math.pow((u - cycle) * 9, 2)); break;
          }
          case 'responding': {
            x = .62 + flow * .29; y = -.40 + lane * .23;
            alpha = .9 * (1 - flow); break;
          }
          case 'delegating': {
            const peers = Math.max(1, Math.min(6, activity.parallelAgentCount)), branch = j % peers;
            x = -.7 + flow * 1.4; y = -.74 + branch / Math.max(1, peers - 1) * .12 * flow;
            alpha = .3 + .65 * flow; break;
          }
          case 'waiting': {
            [x, y] = stroke([[-.46, -.75], [-.75, -.75], [-.75, .75], [-.46, .75]], u < .5 ? u * 2 : (u - .5) * 2);
            if (u >= .5) x = -x;
            alpha = .65; break;
          }
          case 'done': {
            const angle = u * TAU;
            const radius = .76 + (still ? 0 : clamp((timeMs - (activity.observedAtMs ?? timeMs)) / 1800) * .12);
            x = Math.cos(angle) * radius; y = Math.sin(angle) * radius;
            alpha = still ? .48 : .58 * clamp(1 - (timeMs - (activity.observedAtMs ?? timeMs)) / 2600); break;
          }
        }
      }
      const lettering = i < WHALE_DOTS + GLYPH_DOTS;
      x = lettering ? x * .38 - .20 : x * .24 + .31;
      y = lettering ? y * .38 - .20 : y * .24 - .25;
      if (!still) {
        const px = x, py = y;
        x += .009 * Math.sin(py * 12 - t * 1.3) + .003 * Math.sin(px * 24 + py * 8 + t * 2.1);
        y += .007 * Math.sin(px * 13 + t * 1.05) + .003 * Math.cos(py * 26 - px * 6 - t * 1.7);
      }
      x = homeX + (x - homeX) * morph; y = homeY + (y - homeY) * morph;
      light = bodyLight + (light - bodyLight) * morph;
      alpha = .88 + (alpha - .88) * morph;
    }
    if (!still) light = clamp(light + .025 * Math.sin(hx * 10 - hy * 5 - t * 1.1), .5, .9);
    points.push([clamp(x, -.94, .94), clamp(y, -.94, .94)]);
    const hue = i >= WHALE_DOTS && morph > 0 ? (failed ? 16 : kind ? HUES[kind] : 188) : 188;
    const base = pigment(188 + variant * 6 - 3, light), accent = pigment(hue + variant * 6 - 3, light);
    const color = !fresh ? [145, 166, 178] : base.map((v, k) => Math.round(v + (accent[k] - v) * morph));
    materials.push([...color, alpha]);
  }
  return { points, materials };
}

export function blendDotFields(from: DotField, to: DotField, progress: number): DotField {
  const mix = ease(progress);
  if (mix >= 1) return to;
  return {
    points: to.points.map((p, i) => p.map((v, k) => from.points[i][k] + (v - from.points[i][k]) * mix)),
    materials: to.materials.map((p, i) => p.map((v, k) => from.materials[i][k] + (v - from.materials[i][k]) * mix)),
  };
}
