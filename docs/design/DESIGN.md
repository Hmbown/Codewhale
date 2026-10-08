# Public website design

The GPUI app is the product client and the website follows its visual language:
warm paper or charcoal, Shannon Sans, quiet navigation, distinct raised inputs,
and restrained blue for actions, selection and keyboard focus. This supersedes
the earlier serif “Tidal Folio” palette and typography description in this file.
The source already made that visual transition; this document now names its
actual authority.

## Source and updates

`vendor/codewhale-design/` is the complete versioned GPUI design artifact.
Its `tokens.json` owns semantic colors, font fallbacks, spacing, control/panel
radii, focus geometry and native motion constants. Update it from the app's
design package as one folder; never edit consumer copies by hand.

Run `python3 scripts/export-design-tokens.py` to regenerate
`web/app/tokens.css`. The exporter checks the portable artifact's own generated
files and maps its semantic names to the website's existing aliases. Run the
same command with `--check` to reject drift. A source digest identifies the
snapshot. Builds require no sibling checkout or network lookup for tokens.

The terminal's Whale, Blue Stage and Shoreline presets remain owned by
`crates/palette/src/rgb.rs`. They are distinct themes; their copied GPUI
constants no longer supply the website's colors.

## Theme and components

The website respects the OS appearance and the person's theme choice. The
footer, terminal code plates and existing homepage water section retain dark
surfaces. `web/app/styles/tokens-roles.css` maps each surface to the shared
palette; components use roles rather than hex values.

Use the official whale assets and locally loaded Shannon Sans. Code uses the
system monospace stack at the artifact's size. CJK uses the shared fallback
order. The website keeps its larger reading measure and responsive heading
scale; a desktop workbench's density is not a requirement for a reading page.

Controls and rows use the shared 6px radius, panels and code surfaces use 10px.
The native composer's 14px sheet radius and small pills remain explicit layout
choices. The common page gutter uses the shared page spacing; the responsive
section rhythm remains specific to the website.

Reuse the existing navigation, command-copy control, sidebar, disclosure,
button and state components. Status needs words as well as color. Keep
permissions, costs and recovery understandable without implementation jargon.

## Interaction and evidence

Keep one visible focus ring, using the artifact's width and offset. Body text,
secondary text, links and button labels must meet WCAG AA in both appearances.
Phone layouts must support keyboard navigation and avoid page overflow.

The existing CSS easing fits the native 420/42/1 panel spring. Revisit that fit
if the shared spring changes. Motion responds to actions; no new decorative
animation. Reduced motion removes transition duration and looping animation.

Verify the changed surface in a real browser at desktop and phone widths,
including focus, theme controls and reduced motion. Screenshots prove the
rendered source; they do not prove deployment or release acceptance. Public
claims continue to come from repository facts and the media manifest.
