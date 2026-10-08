# Adding a component

Several packages add components at once. Each package owns its own files and
nothing else, so work never collides.

## The contract

1. **`Paint`, `Theme` and `Role` only.** A component implements
   `Paint::paint(&self, area, buf, &Theme)` and styles every cell with
   `theme.fg(Role::..)` / `theme.bg(Role::..)`. Never a raw color: no
   `Color::..`, `Rgb(..)` or `Indexed(..)` in `src/components`
   (`tests/contract.rs` fails on it). Where grounds do not paint
   (`!theme.paints_grounds()`), draw an edge or a mark instead of relying on
   a fill; `theme.grounds_differ(a, b)` says when two grounds look alike.
2. **Every state has a mark and a word.** Color is never the only carrier.
   Marks come from `glyphs` with their ASCII form (`glyphs::pick(mark,
   theme.ascii())`); caller text goes through `text::display_safe` before it
   is measured or painted.
3. **Words are parameters.** The kit owns no copy beyond English defaults. A
   component that shows its own words takes a plain struct of
   `Cow<'static, str>` fields with `impl Default` (English), by reference:
   `StateWords` and `StatusMark::with_words` are the model. Words the host
   already has go in as `impl Into<Cow<'static, str>>` arguments.
4. **State is a plain struct, keys become an outcome.** A stateful component
   exposes a `Copy`/`Clone` state struct, an outcome enum, and
   `fn handle_key(&mut self, key: crossterm::event::KeyEvent, ..) -> Outcome`
   that ignores `KeyEventKind::Release`. The host decides what an outcome
   does. `PickerState::handle_key` and `PickerOutcome` are the model.
5. **No new runtime dependency without the lead.** `ratatui`, `ratatui-core`, `crossterm`,
   `unicode-width`, `unicode-segmentation`, native character decoding through
   `serde`/`serde_json` (and `libc` on unix). `tests/contract.rs` and CI
   (`cargo tree` must show one ratatui and one crossterm) enforce it.
6. **Names are unique crate-wide.** Everything `pub` in your module is
   re-exported from the crate root, so prefix it: `TextInputWords`, not
   `Words`.
7. **At most two native surface hairlines per frame; no `═`; preserve source
   punctuation (native lists use `...`);** ASCII
   output stays ASCII. `testing::assert_rules` checks all of it.

## Your files

| Package | Component files (`src/components/`) | Gallery (`src/gallery/`) | Tests (`tests/`) |
|---|---|---|---|
| Input | `text_input.rs`, `form.rs` | `input.rs` | `input.rs` |
| Lists | `list.rs`, `fuzzy.rs`, `empty.rs`, and `picker.rs` | `lists.rs` | `lists.rs` |
| Chrome | `heading.rs`, `tabs.rs`, `toggle.rs`, `segmented.rs`, `keymap.rs`, and `surface.rs`, `hints.rs` | `chrome.rs` | `chrome.rs` |
| Display | `receipt.rs`, `diff.rs`, `tree.rs`, `progress.rs`, and `toast.rs` | `display.rs` | `display.rs` |
| Approval | `approval.rs` | `approval.rs` | `approval.rs` |
| Motion | `motion.rs` | `motion.rs` | `motion.rs` |

Make an item `pub` in your module and it is exported; no other file changes.
**Never edit** `src/components/mod.rs`, `src/lib.rs`, `src/gallery/mod.rs`,
`src/testing.rs`, `Cargo.toml`, `Cargo.lock`, `src/roles.rs`, `tests/contract.rs`,
`tests/generated.rs`, `tests/snapshots.rs` or another package's files without
the lead. If you need a new `Role`, a testing helper or a dependency, ask.
Stage only the paths you changed.

## Gallery entries

In your `src/gallery/<package>.rs`, add one `Entry` per state worth seeing
(normal, empty, error, narrow, disabled, long and CJK text):

```rust
pub(crate) fn entries() -> Vec<Entry> {
    vec![Entry { name: "text-input", width: 40, height: 3, draw: text_input }]
}
// fn text_input(area: Rect, buf: &mut Buffer, theme: &Theme)
```

`draw` paints inside the `Rect` it is given and uses only the `Theme`. The
gallery draws it at its own size and at 40, 80 and 120 columns
(`cargo run --example gallery`: `p` profile, `w` width).

## Tests

In your `tests/<package>.rs`, for each component at its real heights:

```rust
use codewhale_ratatui::{Paint, Theme, testing};
use ratatui::{buffer::Buffer, layout::Rect};

fn paint(area: Rect, buf: &mut Buffer, theme: &Theme) {
    MyComponent::new(/* fixtures */).paint(area, buf, theme);
}

#[test]
fn my_component() {
    testing::assert_rules(4, paint);                              // 9 profiles x 40/80/120
    insta::assert_snapshot!("my-component", testing::snapshot(4, paint));
}
```

- `testing::frames(height, paint) -> Vec<Frame>`: every `Profile` at every
  width in `testing::WIDTHS` (40, 80, 120), painted on `Background` like the
  gallery. `Frame` has `text()`, `styled()`, `label()`, `violations()`.
- `testing::assert_rules(height, paint)`: the rule check over `frames`.
  `testing::assert_frames_keep_the_rules(&frames)` for frames you built.
- `testing::snapshot(height, paint) -> String`: `DarkTrue` styled (roles per
  run, never hex) plus `NoColor` and `Ascii` as text, each at every width.
  `snapshot_all_profiles` records every profile; prefer `snapshot`.
- Also test the state: keys in, outcomes and state out.

Review snapshot changes (`cargo insta review`, or `INSTA_UPDATE=always cargo
test` and read `git diff tests/snapshots/`); accept only what you meant.

## Worked example

Abridged from `src/components/picker.rs`: words as parameters, state plus
outcome plus `handle_key`, paint through roles.

```rust
pub struct PickerItem {
    pub label: Cow<'static, str>,                 // words come in; none live here
    pub detail: Option<Cow<'static, str>>,
    pub disabled: Option<Cow<'static, str>>,      // a disabled row says why
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerOutcome { Ignored, Moved, Chose(usize), Toggled(usize), Cancelled }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PickerState { pub selected: usize, pub offset: usize }

impl PickerState {
    pub fn handle_key(&mut self, key: KeyEvent, len: usize, rows: u16) -> PickerOutcome {
        if key.kind == KeyEventKind::Release || len == 0 { return PickerOutcome::Ignored; }
        match key.code {
            KeyCode::Up => self.prev(len),
            KeyCode::Down => self.next(len),
            KeyCode::Enter => return PickerOutcome::Chose(self.selected),
            KeyCode::Esc => return PickerOutcome::Cancelled,
            _ => return PickerOutcome::Ignored,
        }
        self.scroll_into_view(len, rows);
        PickerOutcome::Moved
    }
}

impl Paint for Picker<'_> {
    fn paint(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        // The selected row carries a marker, bold and a ground: three cues, so
        // it still shows where grounds do not paint.
        let marker = glyphs::pick(glyphs::selection_marker(selected), theme.ascii());
        row.push(format!("{marker} "), theme.fg(Role::Primary));
        row.push(text::display_safe(&item.label), theme.fg(Role::Foreground).add_modifier(Modifier::BOLD));
        buf.set_style(rect, theme.bg(Role::Selected));
    }
}
```

## Before you hand back

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings
cargo test --test <package>        # your own file; CI runs the whole suite
```

Format only your files (`rustfmt --edition 2024 <path>`), not the crate: other
packages' files are in flight.
