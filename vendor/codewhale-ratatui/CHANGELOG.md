# Changelog

## Unreleased

- Simplify the native showcase: concise conversation, sparse ambient life,
  muted metrics and optional workflow progress, with the original workbar
  given room for its rows.
- Present a still native preview first and organize every dark/light component
  preview into named collections, with animations available on demand.
- Reduce gallery framing and use native terminal palette colors around the
  unchanged Ratatui buffers.

- Render themed components by reference, and use `Themed::new` with a
  heterogeneous collection of `dyn Paint` components.
- Render lists and pickers with Ratatui's `StatefulWidget`, persisting the
  viewport's scroll offset in application-owned state.
- Keep backend selection with the consuming application; interactive examples
  enable Crossterm separately from the library dependency.
- Add a small native starter app, a standalone consumer check, rendering
  benchmarks, public documentation/package gates and Windows test coverage.
- Fix incremental Unicode editing and grapheme cursor boundaries after
  deletion.
- Keep modified navigation/activation keys with the host and prevent held-key
  repeats from submitting, choosing, cancelling or toggling input controls.
- Reuse contrast calculations within native ocean text runs while preserving
  each cell's original color and contrast policy.
- Enable the previously exempt whale bounds check now that its renderer clips.
- Limit source packages to library sources, examples, documentation and
  required native assets.

## 0.1.0 development baseline

- Native Codewhale workbar, composer, footer, conversation, instrument surfaces
  and session list; 16 source palettes and the Underwater ocean column.
- Input, navigation, decisions, results, motion and marine-life components,
  plus optional workspace compositions and additional ombrés.
- Source view guide, 195-entry gallery across nine terminal profiles, and
  generated dark/light README previews and animations.

This version has not been published to a package registry.
