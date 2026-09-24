import type { LegalPrivacyDict } from "../types";

/**
 * English reference dictionary for `app/[locale]/legal/privacy/page.tsx`.
 * Copy moved verbatim from the page's `isZh` ternaries — any wording change
 * belongs in its own commit, never mixed into a structural move. The binding
 * legal text itself is English for every locale and stays in `lib/legal-copy`.
 */
export const legalPrivacy: LegalPrivacyDict = {
  metaTitle: "Privacy policy · Codewhale",
  metaDescription: "How Shannon Labs handles information when you use Codewhale.",
  kicker: "Legal",
  title: "Privacy policy",
  updated: "Effective and last updated {date}.",
  termsLink: "Terms of service",
  homeLink: "Back home",
};
