import Link from "next/link";
import { getLegalPrivacy, getLegalTerms } from "@/lib/i18n/dictionaries";

/** Terms | Privacy: page-level tabs between the two legal documents. */
export function LegalTabs({ locale, current }: { locale: string; current: "terms" | "privacy" }) {
  const termsLabel = getLegalPrivacy(locale).termsLink;
  const privacyLabel = getLegalTerms(locale).privacyLink;
  return (
    <nav className="tabs legal-tabs" aria-label={`${termsLabel} · ${privacyLabel}`}>
      <Link href={`/${locale}/legal/terms`} className="tab" aria-current={current === "terms" ? "page" : undefined}>
        {termsLabel}
      </Link>
      <Link href={`/${locale}/legal/privacy`} className="tab" aria-current={current === "privacy" ? "page" : undefined}>
        {privacyLabel}
      </Link>
    </nav>
  );
}
