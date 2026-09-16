#!/usr/bin/env python3
"""Derive the pet body (`crates/tui/src/tui/ambient_life/whale-points.tsv`).

    python3 scripts/brand/whale-points.py            # rewrite the point cloud
    python3 scripts/brand/whale-points.py --check     # exit 1 if it drifted
    python3 scripts/brand/whale-points.py --preview   # print the cloud as ASCII

The pet is a 980-particle body. Before this script the body was hand-authored
and did not follow the product mark: measured against `brand/mark.svg`'s
silhouette only ~19-23% of its points landed inside the mark once the cloud was
scaled to fill it, so the pet read as static rather than a whale.

Source of truth is the same one `trace-brand.py` uses: the hero whale of
`brand/codewhalemarkfinal.png`. `brand/mark.svg` is the kept trace of that hero,
and the founder's app-icon render is the same silhouette (0.93 IoU), so there is
exactly one mark and this script derives from it rather than redrawing it.

Sampling is deliberately contour-only. 980 discs cannot fill a solid
silhouette legibly at pet sizes, so the cloud spends its whole budget on the
mark's outline (outer edge plus internal boundaries), where each dot buys the
most shape. Spacing is even (farthest-point sampling) because clumped sampling
reads as noise even where the underlying silhouette is correct.

Measured against the shipped dot radius, the body this replaced covered ~6% of
the mark's outline and buried the rest under a diffuse interior; this one
covers ~80% of it continuously.

Requires `pillow` and `numpy`. No network. No ImageMagick.
"""

from __future__ import annotations

import argparse
import pathlib
import sys

try:
    from PIL import Image
    import numpy as np
except ImportError:
    raise SystemExit("whale-points.py requires pillow and numpy")

ROOT = pathlib.Path(__file__).resolve().parents[2]
SHEET = ROOT / "brand" / "codewhalemarkfinal.png"
OUT = ROOT / "crates" / "tui" / "src" / "tui" / "ambient_life" / "whale-points.tsv"

# `pet-native.js` rejects any body that is not exactly 980 x 2 finite points in
# [-1, 1]; the sim, the served TSV and the desktop client must agree on a count.
COUNT = 980
# Normalized half-extent of the longer side. The renderers apply one uniform
# scale to x and y, so the cloud must be aspect-true to the mark and this is
# what sets the pet's on-screen size.
HALF_EXTENT = 0.44


def hero_mask(path: pathlib.Path) -> np.ndarray:
    """The hero whale of the brand sheet, as a boolean ink mask.

    The sheet is a multi-panel page (hero mark, size ramp, icon row, wordmark),
    so the hero is found rather than assumed: threshold, then keep the largest
    dark component in the top half, which is the hero mark.
    """
    grey = np.array(Image.open(path).convert("L"), dtype=np.float64)
    h, w = grey.shape
    ink = grey < 128
    ink[int(0.52 * h) :, :] = False  # below the hero band is the size ramp

    # The hero is the topmost ink on the sheet; flood its component with a
    # stack so a caption or a stray rule cannot be mistaken for the mark.
    ys, xs = np.nonzero(ink)
    if len(ys) == 0:
        raise SystemExit(f"no ink found in {path}")
    start = (int(ys[0]), int(xs[np.argmin(ys)]))
    comp = np.zeros_like(ink)
    comp[start] = True
    stack = [start]
    while stack:
        y, x = stack.pop()
        for ny, nx in ((y - 1, x), (y + 1, x), (y, x - 1), (y, x + 1)):
            if 0 <= ny < h and 0 <= nx < w and ink[ny, nx] and not comp[ny, nx]:
                comp[ny, nx] = True
                stack.append((ny, nx))
    return comp


def crop(mask: np.ndarray) -> np.ndarray:
    ys, xs = np.nonzero(mask)
    return mask[ys.min() : ys.max() + 1, xs.min() : xs.max() + 1]


def erode(mask: np.ndarray) -> np.ndarray:
    out = mask.copy()
    out[1:, :] &= mask[:-1, :]
    out[:-1, :] &= mask[1:, :]
    out[:, 1:] &= mask[:, :-1]
    out[:, :-1] &= mask[:, 1:]
    return out


def smooth(mask: np.ndarray, radius: int = 2) -> np.ndarray:
    """Box-blur the edge before thresholding so the contour is not stair-stepped."""
    a = mask.astype(np.float64)
    for _ in range(radius):
        b = a.copy()
        b[1:, :] += a[:-1, :]
        b[:-1, :] += a[1:, :]
        b[:, 1:] += a[:, :-1]
        b[:, :-1] += a[:, 1:]
        a = b / b.max()
    return a > 0.5


def farthest_point(candidates: np.ndarray, seeds: np.ndarray, want: int) -> np.ndarray:
    """Even spacing: repeatedly take the candidate furthest from everything chosen.

    This is what stops the cloud reading as noise: uniform-random sampling
    clumps, and clumps read as speckle at pet sizes no matter how correct the
    underlying silhouette is.
    """
    chosen = list(map(tuple, seeds))
    if not chosen:
        chosen.append(tuple(candidates[0]))
    pts = candidates.astype(np.float64)
    if len(chosen) < want:
        base = np.array(chosen, dtype=np.float64)
        best = np.full(len(pts), np.inf)
        for p in base:
            best = np.minimum(best, ((pts - p) ** 2).sum(1))
        for _ in range(want - len(chosen)):
            i = int(np.argmax(best))
            p = pts[i]
            chosen.append(tuple(candidates[i]))
            best = np.minimum(best, ((pts - p) ** 2).sum(1))
            best[i] = -1.0
    return np.array(chosen, dtype=np.float64)


def cloud(mask: np.ndarray) -> np.ndarray:
    """Spend the whole budget on the contour, evenly spaced.

    Measured at the shipped dot radius (1.55px where the pet is rendered),
    spreading points through the interior instead leaves most of the mark's
    outline undrawn and scatters loose specks inside it - which is what made
    the pet read as static. A contour-only cloud draws a continuous outline.
    """
    rim = mask & ~erode(mask)
    rys, rxs = np.nonzero(rim)
    rimp = np.stack([rxs, rys], axis=1).astype(np.float64)
    if len(rimp) == 0:
        raise SystemExit("no contour found")
    # Seeds must be spread across the whole contour. Taking a prefix instead
    # leaves everything past it undrawn, and farthest-point sampling cannot
    # recover a region it has no seed near.
    seed = rimp[np.linspace(0, len(rimp) - 1, min(64, len(rimp))).astype(int)]
    return farthest_point(rimp, seed, COUNT)


def normalize(pts: np.ndarray, mask: np.ndarray) -> np.ndarray:
    ys, xs = np.nonzero(mask)
    cx = (xs.min() + xs.max()) / 2.0
    cy = (ys.min() + ys.max()) / 2.0
    span = max(xs.max() - xs.min(), ys.max() - ys.min())
    scale = (HALF_EXTENT * 2.0) / (span + 1.0)
    out = np.empty_like(pts)
    out[:, 0] = (pts[:, 0] - cx) * scale
    # Screen space is y-down and the mask is y-down, so this keeps the whale
    # the right way up in both renderers.
    out[:, 1] = (pts[:, 1] - cy) * scale
    return out


def render() -> str:
    mask = smooth(crop(hero_mask(SHEET)))
    pts = normalize(cloud(mask), mask)
    return "".join(f"{x:.6f}\t{y:.6f}\n" for x, y in pts)


def rasterize(pts: np.ndarray, w: int, h: int, dot_scale: float = 1.0) -> np.ndarray:
    """Emulate the shipped paint path so legibility is judged on real output.

    Mirrors `src/workspace/pet.rs` / `pet_watch/graphics.rs`: one uniform scale
    for both axes, a disc per point, the same radius rule and clamp.
    """
    scale = min(w * 0.52, h * 0.85)
    radius = max(0.68, min(1.55, min(w, h) * 0.00285)) * dot_scale
    ox, oy = w * 0.5, h * 0.47
    canvas = np.zeros((h, w), dtype=np.float64)
    reach = int(radius) + 2
    for px, py in pts:
        cx = ox + px * scale
        cy = oy + py * scale
        x0, x1 = int(cx) - reach, int(cx) + reach + 1
        y0, y1 = int(cy) - reach, int(cy) + reach + 1
        if x1 < 0 or y1 < 0 or x0 >= w or y0 >= h:
            continue
        ys, xs = np.mgrid[max(0, y0) : min(h, y1), max(0, x0) : min(w, x1)]
        d = np.hypot(xs - cx, ys - cy)
        np.maximum(
            canvas[max(0, y0) : min(h, y1), max(0, x0) : min(w, x1)],
            np.clip(radius + 0.5 - d, 0.0, 1.0),
            out=canvas[max(0, y0) : min(h, y1), max(0, x0) : min(w, x1)],
        )
    return canvas


def preview(text: str, label: str, w: int, h: int) -> None:
    pts = np.array([list(map(float, line.split("\t"))) for line in text.strip().splitlines()])
    canvas = rasterize(pts, w, h)
    # Terminal cells are about twice as tall as wide, so the sample grid is
    # twice as fine vertically as horizontally.
    cols = 96
    rows = max(1, int(h / w * cols * 0.5))
    ramp = " .:-=+*#%@"
    print(f"# {label}  {w}x{h}px -> {cols}x{rows} cells")
    for r in range(rows):
        line = ""
        for c in range(cols):
            ys = slice(int(r * h / rows), max(int(r * h / rows) + 1, int((r + 1) * h / rows)))
            xs = slice(int(c * w / cols), max(int(c * w / cols) + 1, int((c + 1) * w / cols)))
            v = canvas[ys, xs].mean()
            line += ramp[min(len(ramp) - 1, int(v * len(ramp) * 2.2))]
        print(line)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail if the file drifted")
    parser.add_argument("--preview", action="store_true", help="render at the shipped sizes")
    args = parser.parse_args()

    text = render()
    if args.preview:
        preview(text, "ambient backdrop", 960, 560)
        preview(text, "pet panel", 420, 260)
        return 0
    if args.check:
        if not OUT.exists() or OUT.read_text() != text:
            print(f"{OUT} is stale; run scripts/brand/whale-points.py", file=sys.stderr)
            return 1
        print(f"{OUT} matches the mark")
        return 0
    OUT.write_text(text)
    print(f"wrote {OUT} ({COUNT} points)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
