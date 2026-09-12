// web.ts — the TypeScript/Canvas renderer for the pet.
//
// The renderer owns no simulation: the caller steps `sim` and passes the
// PetState that produced the frame — the same contract as the ratatui widget,
// the SwiftUI view, and the Compose composable. One pass:
//
//   * filled dot per particle, colour and alpha from sim.frame
//   * hollow frames stroke the dots instead of filling them
//   * the caller (or the DOM caption) carries the non-colour label
//
// Reduced motion: step the sim with motion:false — the same contract as every
// other port.

import { PetSim, PetState, layout } from './PetSim.ts';

export function drawPet(ctx: CanvasRenderingContext2D, sim: PetSim, state: PetState, w: number, h: number): void {
  const lay = layout(w, h, state);
  const f = sim.frame;
  ctx.globalAlpha = f.alpha;
  const rgb = `rgb(${Math.round(f.r)},${Math.round(f.g)},${Math.round(f.b)})`;
  ctx.lineWidth = 1;
  const r = lay.dot / 2;
  if (f.hollow) {
    ctx.strokeStyle = rgb;
    for (const q of sim.p) {
      const x = lay.ox + q.x * lay.scale * lay.flipX;
      const y = lay.oy + q.y * lay.scale;
      ctx.beginPath();
      ctx.arc(x, y, r, 0, 6.283);
      ctx.stroke();
    }
  } else {
    ctx.fillStyle = rgb;
    for (const q of sim.p) {
      const x = lay.ox + q.x * lay.scale * lay.flipX;
      const y = lay.oy + q.y * lay.scale;
      ctx.beginPath();
      ctx.arc(x, y, r, 0, 6.283);
      ctx.fill();
    }
  }
  ctx.globalAlpha = 1;
}
