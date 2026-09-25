"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { currentNavHref, type ChromeLink } from "@/lib/i18n/links";

/**
 * Desktop primary navigation, shown from `lg`. Labels come from the locale's
 * chrome dictionary — no locale branch here and no companion labels: at 2xl,
 * de/pt-BR/id companions once collapsed the home wordmark to a zero-width
 * hit target (#5290).
 *
 * Wrapping is the strip's escape valve for a translated strip that outgrows
 * the row at the `lg` floor. The row gap is tight so a wrapped second row
 * does not double the height of the sticky header.
 */
export function NavLinks({
  links,
  primaryAria,
}: {
  links: ChromeLink[];
  primaryAria: string;
}) {
  const pathname = usePathname();
  // One link is the page; ancestors are not. See currentNavHref.
  const current = currentNavHref(links, pathname);

  return (
    <nav className="hidden lg:flex min-w-0 shrink items-center gap-x-5 gap-y-1 flex-wrap" aria-label={primaryAria}>
      {links.map((l) => {
        const isActive = l.href === current;
        return (
          <Link key={l.href} href={l.href} className="nav-link group inline-flex items-baseline" aria-current={isActive ? "page" : undefined}>
            <span className="leading-none">{l.label}</span>
          </Link>
        );
      })}
    </nav>
  );
}
