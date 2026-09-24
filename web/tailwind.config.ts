import type { Config } from "tailwindcss";

export default {
  content: ["./app/**/*.{ts,tsx}", "./components/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // The surface, ink, and accent tokens all resolve through CSS custom
        // properties (app/styles/tokens-roles.css) so the dark subtrees can
        // re-theme themselves; the values are the generated GPUI set_theme
        // tokens. Hover is the primary at 0.9 opacity, as in set_theme.
        paper: "rgb(var(--c-paper) / <alpha-value>)",
        "paper-deep": "rgb(var(--c-paper-deep) / <alpha-value>)",
        "paper-edge": "rgb(var(--c-paper-edge) / <alpha-value>)",
        "paper-card": "var(--paper-card)",
        "paper-line": "var(--paper-line)",
        "paper-line-soft": "var(--paper-line-soft)",
        ink: "rgb(var(--c-ink) / <alpha-value>)",
        "ink-soft": "rgb(var(--c-ink-soft) / <alpha-value>)",
        "ink-mute": "rgb(var(--c-ink-mute) / <alpha-value>)",
        indigo: "rgb(var(--c-indigo) / <alpha-value>)",
        "indigo-deep": "var(--indigo-deep)",
        "indigo-pale": "var(--indigo-pale)",
        ochre: "var(--ochre)",
        jade: "var(--jade)",
        cobalt: "var(--cobalt)",
      },
      fontFamily: {
        // One face, as GPUI set_theme: every family resolves through the
        // role stacks in app/styles/tokens-roles.css (Shannon Sans subsets
        // loaded in app/[locale]/layout.tsx; system mono for code).
        display: ["var(--font-display)"],
        body: ["var(--font-body)"],
        cjk: ["var(--font-cjk)"],
        mono: ["var(--font-mono)"],
      },
      // `transition-colors` and friends use the same motion tokens as
      // app/styles, so reduced motion stills them too.
      transitionDuration: { DEFAULT: "var(--dur-state)" },
      transitionTimingFunction: { DEFAULT: "var(--ease-spring)" },
    },
    // Replaces Tailwind's scale with the GPUI radius grammar
    // (app/styles/tokens-roles.css): 6px controls, 10px surfaces (cards,
    // code, menus), 14px raised sheets, and pills. Nothing in between.
    borderRadius: {
      none: "0",
      sm: "var(--radius-control)",
      DEFAULT: "var(--radius-control)",
      lg: "var(--radius-surface)",
      xl: "var(--radius-sheet)",
      full: "var(--radius-pill)",
    },
    // Replaces Tailwind's scale so wide tracking cannot be generated: labels
    // are sentence case at normal tracking. `wide` stays for Han body copy.
    letterSpacing: {
      tighter: "-0.05em",
      tight: "-0.025em",
      crisp: "-0.018em",
      normal: "0em",
      wide: "0.025em",
    },
  },
  // No all-caps anywhere, as in the GPUI app: the `uppercase` utility and
  // its siblings are not generated.
  corePlugins: { textTransform: false },
  plugins: [],
} satisfies Config;
