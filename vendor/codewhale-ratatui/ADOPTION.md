# Pinned terminal components

Source files in this directory are byte-for-byte from the upstream commit
recorded in UPSTREAM.json. Keep the upstream MIT licence and component
attribution with this source. Preview media and local sessions are excluded.

The Engine consumes the kit through a local path dependency, with no Git
dependency and no package publication. Binary builds need only this checkout.
Publishing codewhale-tui to crates.io requires this kit version on crates.io;
that publication remains a separate release gate requiring founder approval.

Adoption is deliberately incremental: each migrated renderer deletes its
predecessor. Workflow progress, working/verification indicators, the metrics
row and posture row use the kit. MetricsLine and PostureBar own their complete
shed, projection, render and pointer geometry; the Engine supplies measured
facts, localized labels and hints, clock lifecycle, live/custom theme ink and
action dispatch. The current kit drops the pinned right PostureFact's optional
ink during layout. The host carries every fact's exact ink in a distinct role
and uses the shared live-theme adapter, so permission and right-notice colors
remain independent without modifying pinned source. Dock tabs, body layout
and scrollbar paint use the accepted shared plans. Engine still composes live
rows, hover text and typed actions. Character state and timing remain in the
Engine; packed Braille painting uses the shared raster plan.

The Ocean facade adopts complete pure sampling and guarded painting. The
kit owns matching-cell iteration, proposed visible-ink contrast checks,
absolute-row cached samples, explicit semantic-surface projection and sparse
caustic paint/math. Engine keeps live UiTheme colors, actual backend depth and
ASCII facts, monotonic clocks, phase/context/presence/completion policy,
actual-ramp cache identity, frame composition and ambient character lifecycle.
Backend palette adaptation supplies the exact proposed visible ink; source
symbols, inks and modifiers stay untouched. Main and focused transcripts
publish explicit styled-ground masks, preserving blank semantic padding and
custom RGB aliases through the final whole-shell pass. A composer selection
whose custom ground aliases its base conservatively protects its mounted area.

This deliberately reconciles prior unguarded host paint: lower color depths
keep the flat selected pane; light, Reset and unknown named base grounds never
supply dark-water evidence. An actually painted opaque dark pane can supply
known-ground evidence when the terminal default is unknown, while actual
truecolor capability is still required. Unreadable or unknown backend inks,
reversed cells, different grounds and semantic regions are spared. Caustics
only touch already painted ordinary water and share reduced-motion/capability
guards. Frozen safe-painter comparisons retain exact ordinary RGB geometry,
phase/cache samples and travelling caustic rounding; intentional guard
improvements have explicit cases. Character paint and its occupancy/scheduler
remain outside this background finishing adoption.

PendingCard routes the complete composer pending-input facade through the
kit's measured row plan, shared with its existing generic PendingInputPreview.
Engine-owned queue/context/child-request facts, localized copy, exact palette
styles and terminal backend remain authoritative. The host compositor/wrapping
helpers are deleted; decision settlement remains outside this rendering slice. The existing context action suffixes retain their current English
copy; other native card labels use the Engine localization catalogue.

DecisionBand adopts the complete bottom-anchored ApprovalWidget paint and
geometry through one kit plan: wrapped controls, full/compact validated rule
coverage, persistent-action visibility, body truncation and option-order
mouse rectangles. The host band compositor and save-preview fitting are
deleted. The existing ElevationWidget row measurements also use the kit's
shared Ratatui wrapping helper. Engine request facts, badges, dossier/command
preview formatting, localization, exact palette styles and keyboard/mouse
decisions remain authoritative. Default Enter and timeout still deny; parent
Escape aborts, child Escape hides, and modified/non-press keys cannot grant.
Persistent keys and mouse targets follow the last painted save coverage;
collapsed and empty frames retain canonical empty slots and withdraw stale
geometry. Every styled span and rule-coverage string is display-safe before
measurement and painting. The generic bordered ApprovalCard keeps
its separate verbatim-subject/input contract, and the native band does not
claim its invisible-token encoding. ElevationWidget painting
remains outside this approval slice.

NativeComposerFrame adopts the complete mounted composer: shell/titles,
raw scalar source row projection, display-safe selection, final caret/prompt,
IME empty row, menu reservation/centering/columns, wrapped option rectangles
and hover bounds. Existing NativeComposer painting/caret also uses this same
pure plan. Engine editor state, history/completion/filtering, localized fact
copy, exact live/custom styles, paste/submit predicates and key/IME dispatch
remain authoritative. Frame viewport records the painter's actual centered
padding and scroll; mouse/keyboard/wheel source projection uses the same kit
rows while retaining keyboard display-column versus wheel scalar-column
policy. Old mounted shell, wrapping, selection and menu geometry are deleted;
frozen old code is test-only. Generated gallery/catalogue assets and native
input/provider acceptance require separate evidence after this candidate.


TranscriptViewport adopts every mounted cached-row painter: the main
ChatWidget, focused-child transcript and LiveTranscriptOverlay use the kit's
pure content/viewport/chrome plan. The old paragraph/scrollbar/jump compositor,
link-column clipping and span selection painter are deleted from production.
Engine parsing, cache/source receipts and revisions, original clipboard source,
streaming/session state, scroll intent and selection endpoints remain
unchanged. The host supplies exact live style facts and its existing CJK/keycap
column grammar; the kit preserves styled spans and guards display content.
Opaque hyperlink targets remain only in Engine metadata, with existing URI
validation and OSC emission; final geometry excludes rails and the opaque
jump button. The kit guarded Ocean finishing runs between transcript content
and chrome using the same source-derived semantic regions; ambient character
policy remains host-owned. Counterparts are private cfg(test) fixtures. Generated
previews, compiled/native accessibility/input checks and hosted CI require
separate acceptance evidence.

DockTabRow supplies fitting, painting and typed target geometry for all strip
tabs. WorkbarLayout::for_body and WorkbarScrollbar share body reservation,
clipping and rail paint with the gallery. Engine keeps the selected caller,
live row facts, localized labels, custom ink, viewport intent and action
dispatch. Exact buffer and target counterparts are private test fixtures;
source adoption alone does not qualify installed-terminal behavior.

BrailleFrame paints the existing packed character raster. Engine retains its
external pose digest, session cursor, animation/audio ownership and scheduler.
This avoids a second character clock while using the same measured pixels.
