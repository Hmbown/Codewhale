"use client";

import { useRouter, usePathname } from "next/navigation";
import { ALL_LOCALES } from "@/lib/i18n/config";
import { fill, getChrome } from "@/lib/i18n/dictionaries";
import { replacePathLocale } from "@/lib/i18n/path";
import { Icon } from "./icon";

/** Labels for the dropdown. Keyed by locale code, displayed in native script. */
const LOCALE_LABELS: Record<string, string> = {};
for (const l of ALL_LOCALES) {
  LOCALE_LABELS[l.code] = l.label;
}

/** Routed locales that appear in the switcher (shipped + partial). */
const ROUTED = ALL_LOCALES.filter((l) => l.status === "shipped" || l.status === "partial");

export function LocaleSwitcher({ current }: { current: string }) {
  const router = useRouter();
  const pathname = usePathname();
  const chrome = getChrome(current);

  const switchLocale = (code: string) => {
    if (code === current) return;
    document.cookie = `NEXT_LOCALE=${code};path=/;max-age=${60 * 60 * 24 * 365}`;
    router.push(replacePathLocale(pathname, code));
  };

  // If only 1 routed locale, no switcher needed.
  if (ROUTED.length <= 1) return null;

  // If exactly 2 routed locales, show a simple toggle.
  if (ROUTED.length === 2) {
    const other = ROUTED.find((l) => l.code !== current);
    if (!other) return null;
    return (
      <button
        type="button"
        onClick={() => switchLocale(other.code)}
        className="nav-icon-button"
        aria-label={fill(chrome.switcherSwitchTo, { label: other.label })}
        title={other.label}
      >
        <Icon name="globe" className="nav-icon" />
      </button>
    );
  }

  // 3+ routed locales: a globe icon over a native <select>, which keeps the
  // platform picker, keyboard and screen-reader behavior. The select is
  // transparent and covers the icon, so the whole icon is the hit target.
  // Partial packs sit under a visible group label so the incomplete scope is
  // honest at the point of selection. Each option names its language in its
  // own script, so it carries that language's `lang` (and `dir`); the badge
  // stays in the page language on the <optgroup> instead of being mixed into
  // the option text.
  const option = (l: (typeof ROUTED)[number]) => (
    <option key={l.code} value={l.code} lang={l.code} dir={l.dir === "rtl" ? "rtl" : undefined}>
      {l.label}
    </option>
  );
  const partial = ROUTED.filter((l) => l.status === "partial");
  // The badge is written as an inline "(partial)"; as a group heading it reads
  // better without its (ASCII or full-width) brackets.
  const partialHeading = chrome.partialBadge.replace(/^[(（]\s*|\s*[)）]$/g, "");
  return (
    <span className="nav-icon-button nav-locale">
      <Icon name="globe" className="nav-icon" />
      <select
        value={current}
        onChange={(e) => switchLocale(e.target.value)}
        aria-label={chrome.switcherLabel}
      >
        {ROUTED.filter((l) => l.status !== "partial").map(option)}
        {partial.length > 0 && <optgroup label={partialHeading}>{partial.map(option)}</optgroup>}
      </select>
    </span>
  );
}
