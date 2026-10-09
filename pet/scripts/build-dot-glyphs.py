import argparse
import hashlib
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

parser = argparse.ArgumentParser()
parser.add_argument("font", type=Path)
parser.add_argument("--output", type=Path, default=Path(__file__).resolve().parents[1] / "src/core/dot-glyphs.ts")
args = parser.parse_args()
font_hash = hashlib.sha256(args.font.read_bytes()).hexdigest()
if font_hash != "ca094f6b0001fb048ca39ddd797a0cdb0179e1e55c6561e111c49c3e6a61d7b7":
    raise ValueError("Expected the pinned Noto Sans CJK SC Medium font in dot-glyphs-LICENSE.txt")
characters = dict(zip(
    ("reading", "editing", "searching", "testing", "executing", "browsing", "computer", "memory", "tool", "thinking", "responding", "delegating", "waiting", "done"),
    "读写搜试行览控记用思答协待成",
    strict=True,
))
for size in range(48, 0, -1):
    font = ImageFont.truetype(str(args.font), size)
    bounds = [font.getbbox(char) for char in characters.values()]
    if all(right - left <= 44 and bottom - top <= 44 for left, top, right, bottom in bounds):
        break
lines = ["import type { OwnerActivityKind } from './pet-engine.js';", "", "export const DOT_GLYPHS: Record<OwnerActivityKind | 'waiting' | 'done', string[]> = {"]
for key, char in characters.items():
    image = Image.new("L", (48, 48), 0)
    left, top, right, bottom = font.getbbox(char)
    ImageDraw.Draw(image).text(((48 - right + left) // 2 - left, (48 - bottom + top) // 2 - top), char, font=font, fill=255)
    rows = ["".join("#" if image.getpixel((x, y)) >= 128 else "." for x in range(48)) for y in range(48)]
    lines.append(f"  {key}: [")
    lines.extend(f"    '{row}'," for row in rows)
    lines.append("  ],")
lines.append("};")
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text("\n".join(lines) + "\n", encoding="utf-8")
print(f"Generated {len(characters)} masks at 48 × 48, font size {size}, threshold 128: {args.output}")
