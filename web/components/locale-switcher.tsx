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
  // Partial packs carry a visible badge in the list so the incomplete scope
  // is honest at the point of selection.
  return (
    <span className="nav-icon-button nav-locale">
      <Icon name="globe" className="nav-icon" />
      <select
        value={current}
        onChange={(e) => switchLocale(e.target.value)}
        aria-label={chrome.switcherLabel}
      >
        {ROUTED.map((l) => (
          <option key={l.code} value={l.code}>
            {l.status === "partial" ? `${l.label} ${chrome.partialBadge}` : l.label}
          </option>
        ))}
      </select>
    </span>
  );
}
