# Component website

The explorer lives inside the existing Codewhale website at `/ratatui`, with
individual `/ratatui/<gallery-name>` pages. Its source is in
[`Hmbown/CodeWhale/web`](https://github.com/Hmbown/CodeWhale/tree/wave/0.10.1-next/web).
Website deployment follows that repository's manual Cloudflare workflow.

Visitors can search all 199 catalogue entries by name or public API, choose a
collection, compare nine terminal profiles and four column widths, inspect
the exact Rust rendering source, download an SVG, and follow build and release
checks. The native motion player has pause, restart, speed and frame controls.
It starts paused and stops when the page is hidden.

The previews are actual Ratatui buffers with example data. The website does
not run the Engine or dispatch terminal input; use the interactive Cargo
examples for those host interactions. A source fixture can call private
gallery helpers. Each page links the complete source file, and the getting
started example uses the public `NativeComposer` API.

## Update the catalogue

From this repository, export the current library once, then target the owning
website checkout. No new web framework or rendering implementation is needed.

```sh
cargo run --locked --example website -- target/website-buffers
python3 -B tools/export-website.py target/website-buffers /path/to/CodeWhale/web/public/ratatui
python3 -B tools/export-website.py target/website-buffers /path/to/CodeWhale/web/public/ratatui --check
```

The exporter reuses the README collections, source crosswalk and exact gallery
fixtures. Every component has native, 40, 80 and 120 column renders in every
profile. It records the rendering source revision and digest, validates the
geometry, and only replaces files owned by its previous manifest.

## Update motion

The Gallery workflow already creates the source frame directories. For a
local update, run its existing habitat, motion and showcase frame exports,
then regenerate their GIFs when the recorded hashes have changed. The website
exporter checks those hashes before copying any frame.

```sh
python3 -B tools/export-web-motion.py /path/to/CodeWhale/web/public/ratatui/motion
python3 -B tools/export-web-motion.py /path/to/CodeWhale/web/public/ratatui/motion --check
python3 -B -m unittest discover -s tests -p '*website*test.py'
python3 -B -m unittest discover -s tests -p 'web_motion_test.py'
```

The six motion bundles contain 762 native frames. Each stays below Cloudflare's
25 MB asset limit. The website loads only the selected component or motion
bundle; serve JSON with HTTP compression. Its tests verify profile and width
coverage, source links, search, asset boundaries and motion timing. English
and Chinese website copy use the site's existing locale system.
