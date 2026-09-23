#!/usr/bin/env python3
"""Export the Codewhale palettes to the other Codewhale clients.

`crates/palette/src/tokens.rs` is the single source for the product colors.
This script parses its `WHALE_*_RGB`, `LIGHT_*_RGB`, `SHORELINE_*_RGB`, and
`SHORELINE_LIGHT_*_RGB` consts (aliases included) and writes the same values
as CSS custom properties so the web app stops hand-copying hexes. The
Shoreline set is the Shoreline redesign's dark + light pair; `--whale-*` and
`--light-*` stay until the components that use them migrate.

Target: <repo>/web/app/tokens.css. This script writes nothing outside this
repository.

Usage:
    scripts/export-design-tokens.py            # write
    scripts/export-design-tokens.py --check    # exit 1 if any target is stale
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
TOKENS_RS = REPO / "crates/palette/src/tokens.rs"
SOURCE_LABEL = "crates/palette/src/tokens.rs"

CONST_RE = re.compile(
    r"^pub const ((?:SHORELINE_LIGHT|SHORELINE|WHALE|LIGHT)_[A-Z0-9_]+)_RGB: \(u8, u8, u8\) = "
    r"(?:\((\d+), (\d+), (\d+)\)|((?:SHORELINE_LIGHT|SHORELINE|WHALE|LIGHT)_[A-Z0-9_]+)_RGB);",
    re.MULTILINE,
)



def parse_tokens(text: str) -> list[tuple[str, tuple[int, int, int] | str]]:
    """Return [(NAME, (r, g, b) | alias-NAME)] in source order."""
    tokens: list[tuple[str, tuple[int, int, int] | str]] = []
    known: set[str] = set()
    for m in CONST_RE.finditer(text):
        name = m.group(1)
        if m.group(5) is not None:
            target = m.group(5)
            if target not in known:
                raise SystemExit(f"{name} aliases unknown token {target}")
            tokens.append((name, target))
        else:
            tokens.append((name, (int(m.group(2)), int(m.group(3)), int(m.group(4)))))
        known.add(name)
    if not tokens:
        raise SystemExit(f"no palette RGB consts found in {TOKENS_RS}")
    return tokens


def css_name(name: str) -> str:
    """`WHALE_*` exports as `--whale-*`; the Blue Stage light preset's
    `LIGHT_*` consts export as `--light-*` so the website's paper surface can
    reference the same light-mode ink and border values the TUI ships. The
    Shoreline dark pair exports as `--shoreline-*` and its light pair as
    `--shoreline-light-*`, matching the theme the TUI and GPUI clients open
    on."""
    if name.startswith("SHORELINE_LIGHT_"):
        return "--shoreline-light-" + name.removeprefix("SHORELINE_LIGHT_").lower().replace(
            "_", "-"
        )
    if name.startswith("SHORELINE_"):
        return "--shoreline-" + name.removeprefix("SHORELINE_").lower().replace("_", "-")
    if name.startswith("LIGHT_"):
        return "--light-" + name.removeprefix("LIGHT_").lower().replace("_", "-")
    return "--whale-" + name.removeprefix("WHALE_").lower().replace("_", "-")


def render_css(tokens) -> str:
    lines = [
        f"/* generated from {SOURCE_LABEL} — do not edit */",
        "/* regenerate: scripts/export-design-tokens.py (in the codewhale repo) */",
        ":root {",
    ]
    for name, value in tokens:
        prop = css_name(name)
        if isinstance(value, str):
            ref = css_name(value)
            lines.append(f"  {prop}: var({ref});")
            lines.append(f"  {prop}-rgb: var({ref}-rgb);")
        else:
            r, g, b = value
            lines.append(f"  {prop}: #{r:02x}{g:02x}{b:02x};")
            lines.append(f"  {prop}-rgb: {r} {g} {b};")
    lines.append("}")
    return "\n".join(lines) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true", help="verify instead of write")
    args = ap.parse_args()

    tokens = parse_tokens(TOKENS_RS.read_text(encoding="utf-8"))
    css = render_css(tokens)

    targets: list[tuple[Path, str]] = [(REPO / "web/app/tokens.css", css)]

    stale = []
    for path, content in targets:
        current = path.read_text(encoding="utf-8") if path.exists() else None
        if current == content:
            continue
        if args.check:
            stale.append(path)
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
            print(f"wrote {path}")

    if stale:
        for path in stale:
            print(f"stale: {path}", file=sys.stderr)
        print(
            "run scripts/export-design-tokens.py to regenerate from "
            f"{SOURCE_LABEL}",
            file=sys.stderr,
        )
        return 1
    if args.check:
        print(f"design tokens up to date ({len(targets)} file(s), {len(tokens)} tokens)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
