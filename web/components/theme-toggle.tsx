"use client";

/**
 * <ThemeToggle> — a compact System / Light / Dark control in the site nav,
 * shown on every page.
 *
 * The whole site follows the OS appearance by default, the way the GPUI
 * client follows its `set_theme` light/dark pair: with no `data-theme` on
 * <html>, the stylesheet's `prefers-color-scheme` rules pick the scheme and
 * track OS changes live. "light" and "dark" pin the scheme through
 * `data-theme`; "system" removes the pin.
 *
 * One storage contract, shared with the web app: the `cw-theme` key holds
 * `system | light | dark`. A stored `auto` (this toggle's former name for
 * system) reads as `system`. localStorage is per-origin, so the choice made
 * here does not carry to another Codewhale host. The inline boot script in
 * the locale layout applies a stored pin before paint, so there is no theme
 * flash on reload.
 */

import { useEffect, useState } from "react";
import { fill } from "@/lib/i18n/dictionaries";

type Mode = "system" | "light" | "dark";
const ORDER: Mode[] = ["system", "light", "dark"];
const KEY = "cw-theme";

function load(): Mode {
  try {
    const stored = localStorage.getItem(KEY);
    return stored === "light" || stored === "dark" ? stored : "system";
  } catch {
    return "system";
  }
}

function apply(mode: Mode) {
  const el = document.documentElement;
  if (mode === "system") el.removeAttribute("data-theme");
  else el.setAttribute("data-theme", mode);
  try {
    localStorage.setItem(KEY, mode);
  } catch {
    /* private mode / storage disabled — the choice applies until reload */
  }
}

export function ThemeToggle({
  autoLabel,
  lightLabel,
  darkLabel,
  ariaTemplate,
  titleLabel,
}: {
  /** Label for the "system" mode (follow the OS). */
  autoLabel: string;
  lightLabel: string;
  darkLabel: string;
  /** "Theme: {mode} (click to cycle)" — interpolated with fill(). */
  ariaTemplate: string;
  titleLabel: string;
}) {
  const [mode, setMode] = useState<Mode>("system");
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
    setMode(load());
  }, []);

  const cycle = () => {
    const next = ORDER[(ORDER.indexOf(mode) + 1) % ORDER.length];
    setMode(next);
    apply(next);
  };

  const labels: Record<Mode, string> = {
    system: autoLabel,
    light: lightLabel,
    dark: darkLabel,
  };
  const glyph: Record<Mode, string> = { system: "◐", light: "☀", dark: "☾" };
  const shown = mounted ? mode : "system";

  return (
    <button
      type="button"
      onClick={cycle}
      className="inline-flex items-center gap-1.5 px-1.5 py-0.5 hairline-l hairline-r hairline-t hairline-b hover:text-indigo transition-colors"
      aria-label={fill(ariaTemplate, { mode: labels[shown] })}
      title={titleLabel}
      suppressHydrationWarning
    >
      <span aria-hidden>{glyph[shown]}</span>
      <span className="hidden 2xl:inline" suppressHydrationWarning>
        {labels[shown]}
      </span>
    </button>
  );
}
