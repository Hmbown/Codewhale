# Library quality

Codewhale supplies the visual system; Ratatui supplies the rendering contracts.
The goal is a library you can adopt one component at a time, with predictable
state, readable fallbacks and a small amount of host code.

## What we learned from the ecosystem

| Reference | Useful standard | Applied here |
| --- | --- | --- |
| [Ratatui widget guidance](https://docs.rs/ratatui/latest/ratatui/widgets/) | Widgets render by reference; application state persists independently | Reusable borrowed `Themed` widgets, `dyn Paint` collections and standard stateful list/picker rendering |
| [Ratatui 0.30](https://ratatui.rs/highlights/v030/) and [tui-widgets](https://github.com/ratatui/tui-widgets) | Libraries avoid selecting a consumer's backend and unused features | Ratatui defaults disabled; the examples select Crossterm separately |
| [rat-widget](https://github.com/thscharler/rat-salsa/tree/master/rat-widget) | Focus, selection and scrolling need explicit state and usable input outcomes | Caller-owned states; repeated navigation, deliberate activation and host-shortcut regression checks |
| [ratatui-textarea](https://github.com/ratatui/ratatui-textarea) | Editing deserves dedicated Unicode, paste and boundary handling | Grapheme-safe incremental editing, bounded paste and safe secret-field behavior |
| [tachyonfx](https://github.com/ratatui/tachyonfx) | Effects compose with a host's clock and target rendered cells | Existing host-clock motion, quiet policies and post-paint native ocean treatments |
| [Cargo packaging](https://doc.rust-lang.org/cargo/reference/manifest.html#the-exclude-and-include-fields) | Consumers receive source and required assets, rather than a repository dump | Explicit package contents and an independent package-build gate |

These are engineering references. Native Codewhale components retain their
source appearance; additional compositions remain optional. This comparison
does not establish a market ranking or a performance advantage over those
libraries.

## Checks that protect adoption

- A standalone consumer uses `TestBackend`, borrows widgets, and chooses no
  terminal backend. Run `cargo run --locked --manifest-path tests/consumer/Cargo.toml`.
- Public documentation compiles and resolves links: `RUSTDOCFLAGS=-Dwarnings
  cargo doc --locked --no-deps`, plus the doctests in the normal test suite.
- Source packages build independently: `cargo package --locked`. Preview
  images and local sessions stay in the repository rather than the package.
  CI also runs the packaged library's tests, so native assets remain complete.
- CI covers Linux, macOS, Windows and the declared Rust minimum, and rejects
  duplicate Ratatui/Crossterm versions.
- Rendering tests cover nonzero origins, partial and empty rectangles, narrow
  terminals, Unicode text, all nine capability profiles and all native themes.
- [Rendering benchmarks](BENCHMARKS.md) separate prepared-widget paint cost
  from full gallery construction and rendering. Timings are measurements on
  your machine, with no flaky wall-clock threshold in CI.

## Integration boundaries

The application owns terminal setup, events, focus, persistence, actions and
time. The kit does not install an event loop or select an async runtime.
`examples/starter.rs` is a small complete app; `examples/showcase.rs` demonstrates
larger compositions and scheduling. Both use the same public components.

The current API is pre-1.0. The root exports remain available; this quality
pass adds reusable adapters without requiring a rewrite of existing callers.
Use the [component guide](COMPONENTS.md) and [view guide](VIEWS.md) to find the
native source and the corresponding public API.
