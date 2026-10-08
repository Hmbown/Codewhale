#!/usr/bin/env python3
"""Split ShannonSans-Variable.woff2 into a Latin face and an "extended" face.

The full variable font is ~531 KB. Nearly every page only needs Latin, so the
site preloads the Latin subset and lets the browser fetch the extended subset
only when a page actually contains a glyph in its unicode-range (latin-ext,
Greek, Cyrillic, Devanagari, ...). Both subsets keep the wght axis and every
OpenType layout feature.

The pinned full font stays in public/brand/fonts/ as the source of truth
(see lib/public-auth-routes.test.ts); rerun this after replacing it:

    python3 web/scripts/subset-shannon-sans.py

Needs fontTools with the brotli module (pip install fonttools brotli). The
ranges printed at the end must match the `unicode-range` declarations in
app/[locale]/layout.tsx.
"""
from __future__ import annotations

from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont

FONTS = Path(__file__).resolve().parent.parent / "public" / "brand" / "fonts"
SOURCE = FONTS / "ShannonSans-Variable.woff2"

# Google Fonts' "latin" range plus the arrows and minus/division signs the
# site's copy uses, so a plain English page never needs the second file.
LATIN = (
    [(0x0000, 0x00FF), (0x0131, 0x0131), (0x0152, 0x0153), (0x02BB, 0x02BC)]
    + [(0x02C6, 0x02C6), (0x02DA, 0x02DA), (0x02DC, 0x02DC), (0x0304, 0x0304)]
    + [(0x0308, 0x0308), (0x0329, 0x0329), (0x2000, 0x206F), (0x20AC, 0x20AC)]
    + [(0x2122, 0x2122), (0x2190, 0x2199), (0x2212, 0x2215), (0xFEFF, 0xFEFF)]
    + [(0xFFFD, 0xFFFD)]
)


def in_ranges(cp: int, ranges: list[tuple[int, int]]) -> bool:
    return any(lo <= cp <= hi for lo, hi in ranges)


def to_ranges(cps: list[int]) -> list[tuple[int, int]]:
    out: list[tuple[int, int]] = []
    for cp in sorted(cps):
        if out and cp == out[-1][1] + 1:
            out[-1] = (out[-1][0], cp)
        else:
            out.append((cp, cp))
    return out


def css_range(ranges: list[tuple[int, int]]) -> str:
    return ", ".join(
        f"U+{lo:04X}" if lo == hi else f"U+{lo:04X}-{hi:04X}" for lo, hi in ranges
    )


def write_subset(unicodes: list[int], dest: Path) -> None:
    options = subset.Options()
    options.flavor = "woff2"
    options.layout_features = ["*"]
    options.name_IDs = ["*"]
    options.name_languages = ["*"]
    options.notdef_outline = True
    options.glyph_names = False
    font = TTFont(SOURCE)
    subsetter = subset.Subsetter(options)
    subsetter.populate(unicodes=unicodes)
    subsetter.subset(font)
    font.save(dest)


def main() -> None:
    cmap = TTFont(SOURCE).getBestCmap()
    latin = [cp for cp in cmap if in_ranges(cp, LATIN)]
    ext = [cp for cp in cmap if not in_ranges(cp, LATIN)]
    write_subset(latin, FONTS / "ShannonSans-Variable-latin.woff2")
    write_subset(ext, FONTS / "ShannonSans-Variable-ext.woff2")
    print("latin unicode-range:", css_range(LATIN))
    print("ext unicode-range:  ", css_range(to_ranges(ext)))


if __name__ == "__main__":
    main()
