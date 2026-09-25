#!/usr/bin/env python3
"""Generate the portable GPUI design artifact. No sibling checkout is needed.
Edit tokens.json only in the codewhale-design repository. Consumers vendor the whole folder.
"""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent

# WCAG 2.x contrast every palette must hold, in both modes, before anything is
# generated: text roles at 4.5:1 and control edges at 3:1 on each ground a
# role can sit on, including a hovered or selected row.
GROUNDS = ["background", "surface", "sidebar", "hover", "selected"]
TEXT = ["foreground", "muted_foreground", "primary", "live", "attention", "danger"]
EDGES = ["border_strong"]
FILLED = [("primary_foreground", "primary")]

def luminance(hex6):
    channels = [int(hex6[i:i + 2], 16) / 255 for i in (0, 2, 4)]
    channels = [c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4 for c in channels]
    return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2]

def contrast(a, b):
    hi, lo = sorted([luminance(a), luminance(b)], reverse=True)
    return (hi + 0.05) / (lo + 0.05)

def contrast_violations(colors):
    pairs = [(fg, bg, 4.5) for fg in TEXT for bg in GROUNDS]
    pairs += [(edge, bg, 3.0) for edge in EDGES for bg in GROUNDS]
    pairs += [(fg, bg, 4.5) for fg, bg in FILLED]
    found = []
    for mode, palette in colors.items():
        for fg, bg, floor in pairs:
            ratio = contrast(palette[fg], palette[bg])
            if ratio < floor:
                found.append(f"{mode} {fg} on {bg} is {ratio:.2f}:1, needs {floor}:1")
    return found

def render():
    source = (ROOT / "tokens.json").read_bytes()
    data = json.loads(source)
    for name in ["selection_opacity", "primary_hover_opacity"]:
        value = data[name]
        if isinstance(value, bool) or not isinstance(value, (int, float)) or not 0 <= value <= 1:
            raise ValueError(f"{name} must be a number between 0 and 1")
    digest = hashlib.sha256(source).hexdigest()
    header = f"Generated from Codewhale GPUI design {data['version']}; sha256 {digest}. Do not edit."
    keys = list(data["colors"]["dark"])
    assert set(keys) == set(data["colors"]["light"])
    for palette in data["colors"].values():
        assert all(len(v) == 6 and all(c in "0123456789abcdef" for c in v) for v in palette.values())
    violations = contrast_violations(data["colors"])
    if violations:
        raise ValueError("Contrast below WCAG AA:\n  " + "\n  ".join(violations))
    rust = [f"// {header}", "#![allow(dead_code)]", "", f'pub const VERSION: &str = "{data["version"]}";',
            "#[derive(Clone, Copy, Debug)]", "pub struct Colors {"]
    rust += [f"    pub {k}: u32," for k in keys]
    rust += ["}"]
    for name, palette in data["colors"].items():
        rust += [f"pub const {name.upper()}: Colors = Colors {{"]
        rust += [f"    {k}: 0x{v}," for k, v in palette.items()]
        rust += ["};"]
    rust += ["pub fn colors(dark: bool) -> Colors {", "    if dark { DARK } else { LIGHT }", "}"]
    rust += [f'pub const FONT_FAMILY: &str = "{data["typography"]["family"]}";',
             "pub const FONT_FALLBACKS: &[&str] = &["]
    rust += [f"    {json.dumps(v)}," for v in data["typography"]["fallbacks"]]
    rust += ["];"]
    for section in ["spacing", "radius", "focus", "icons", "motion"]:
        for name, value in data[section].items():
            rust += [f"pub const {section.upper()}_{name.upper()}: f32 = {float(value)};"]
    for name in ["selection_opacity", "primary_hover_opacity"]:
        rust += [f"pub const {name.upper()}: f32 = {float(data[name])};"]
    rust += [f'pub const MONO_PX: f32 = {float(data["typography"]["mono_px"])};']
    for name in ["body_px", "caption_px", "prose_px", "heading_px", "title_px"]:
        if name in data["typography"]:
            rust += [f'pub const TYPE_{name.upper()}: f32 = {float(data["typography"][name])};']
    # Existing web names remain aliases, so every current component migrates together.
    aliases = {"outside":"sidebar","surface":"background","panel":"surface","text":"foreground",
               "muted":"muted_foreground","line":"border","accent":"primary","button":"primary",
               "button-text":"primary_foreground","attention":"attention","hover":"hover","selected":"selected",
               "live":"live","danger":"danger","border-strong":"border_strong"}
    css = [f"/* {header} */"]
    for mode, palette in data["colors"].items():
        css += [":root {" if mode == "dark" else '[data-theme="light"] {']
        css += [f"  --{alias}: #{palette[key]};" for alias, key in aliases.items()]
        css += [f'  --selection: #{palette["primary"]}{round(data["selection_opacity"] * 255):02x};',
                f"  color-scheme: {mode};", "}"]
    css += [":root {"]
    for section in ["spacing", "radius", "focus"]:
        css += [f"  --{section}-{name}: {value}px;" for name, value in data[section].items()]
    css += [f'  --icon-stroke: {data["icons"]["stroke"]};',
            f'  --font-family: {json.dumps(data["typography"]["family"], ensure_ascii=False)};',
            f'  --font-fallbacks: {", ".join(json.dumps(v, ensure_ascii=False) for v in data["typography"]["fallbacks"])};',
            f'  --font-body-size: {data["typography"]["body_px"]}px;',
            f'  --font-code-size: {data["typography"]["mono_px"]}px;']
    for name in ["caption_px", "prose_px", "heading_px", "title_px"]:
        if name in data["typography"]:
            css += [f'  --font-{name[:-3]}-size: {data["typography"][name]}px;']
    css += ["}"]
    return {"tokens.rs":"\n".join(rust) + "\n", "tokens.css":"\n".join(css) + "\n"}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    stale = []
    for name, content in render().items():
        path = ROOT / name
        if args.check:
            if not path.exists() or path.read_text() != content:
                stale.append(name)
        else:
            path.write_text(content)
    if stale:
        raise SystemExit("Stale GPUI design artifact: " + ", ".join(stale))
    print("GPUI design artifact verified" if args.check else "GPUI design artifact generated")

if __name__ == "__main__":
    main()
