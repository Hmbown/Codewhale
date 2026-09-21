# Status-bar color grammar

Status-bar ink goes through `crates/palette/src/grammar.rs` (`SemanticFamily`
and `ChromeInk`). Widgets use existing theme slots; they do not invent RGB.
The 0.10.0 direction uses five visual families per theme, with shades for
contrast rather than an unrelated hue for every mode and state.

| Family | Role | Existing theme slots |
| --- | --- | --- |
| Surface | Field, plate, selection and depth | Background and selection shades |
| Neutral | Body, values, labels and secondary context | Body/soft/muted/hint/dim text |
| Action | Identity, navigation, mode, effort and context | `accent_primary` |
| Live | Active work and settled outcomes | `status_working` |
| Attention | Human decisions, permissions, warnings and failure | Existing permission, warning and danger shades |

Five families are not five literal RGB values. In particular, warnings and
failures keep their existing distinct safety inks, words and symbols. Ask,
Auto-Review and Full Access preserve their permission ramp. A mode selection
never borrows failure red. Completed work shares the live hue but changes its
glyph and label; the display never relies on color alone to distinguish them.
Underwater keeps its atmospheric field while its controls follow this grammar.

`Identity`, `Info` and `PolicyAct/Plan/Operate` resolve to the action slot.
`Active` and `Outcome` resolve to the live slot. Metadata retains four weights:
`MetadataValue`, `Metadata`, `MetadataHint` and `MetadataDim`. `Failure` always
resolves to the exact theme error slot; the visual grouping with Attention does
not turn a failure into a warning or change any permission authority.

All selectable themes are covered by grammar and rendered selection checks.
Terminal-owned colors remain host-defined: an unknown contrast pair is not
reported as passing. ASCII symbols and reduced/still motion retain explicit
state labels independently of these color choices.

## Repo / worktree honesty

Repository chrome is derived from Git's common directory and the cached
`GitStatusSnapshot` (`crates/tui/src/tui/git_status.rs`). The render path
never probes. The label is:

- main checkout: `repo · branch*`
- linked worktree: `repo/worktree · branch*`
- unknown branch, known location: `repo` or `repo/worktree` (no invented
  ref)
- not a git repository: omit the segment

`*` is dirtiness. Ahead / behind stay on the same metadata string. Narrow
widths truncate the label by `ShellTier` and drop the segment rather than
wrap.

## Adding chrome

1. Use one of these families. If a fact needs a new distinction, prefer a word
   or glyph rather than another unrelated hue.
2. Reuse an existing `ChromeInk` and active-theme slot where possible.
3. Check the actual foreground/background pair on the rendered surface,
   including selection, light themes and terminal-owned backgrounds.

Shoreline pairs glacial action blue (`#67B8D6`) with warm charcoal; its light
variant uses deep ocean blue (`#006684`) on warm paper. Selection surfaces
use the same blue family. These replace the earlier periwinkle/cobalt pair;
Underwater retains its existing ocean ramp and accent palette.
