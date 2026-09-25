import { defaultLocale, locales } from "./config";

/** Routes whose page-body translation coverage differs from English/Chinese. */
const ROUTE_CONTENT_LOCALES: Readonly<Record<string, readonly string[]>> = {
  "/": locales,
  "/install": ["en"],
  "/docs/guide": ["en", "zh", "fr", "de", "ca", "hi", "tr", "it", "pl", "ar"],
};

/** Most first-party page bodies currently ship in English and Chinese. */
const DEFAULT_CONTENT_LOCALES = ["en", "zh"] as const;

function normalizedPath(path: string): string {
  if (path === "" || path === "/") return "/";
  return `/${path.replace(/^\/+|\/+$/g, "")}`;
}

/** Locales with a genuine page-body translation for an indexable route. */
export function contentLocalesForPath(path: string): readonly string[] {
  return ROUTE_CONTENT_LOCALES[normalizedPath(path)] ?? DEFAULT_CONTENT_LOCALES;
}

/**
 * Canonical locale for a rendered route.
 *
 * Partial locale routes stay accessible in the product, but an English-body
 * fallback points crawlers at the English source instead of presenting a
 * duplicate as a translated page.
 */
export function canonicalLocaleForPath(path: string, locale: string): string {
  return contentLocalesForPath(path).includes(locale) ? locale : defaultLocale;
}
