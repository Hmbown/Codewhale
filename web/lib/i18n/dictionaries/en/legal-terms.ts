import type { LegalTermsDict } from "../types";

/**
 * English reference dictionary for `app/[locale]/legal/terms/page.tsx`.
 * Copy moved verbatim from the page's `isZh` ternaries — any wording change
 * belongs in its own commit, never mixed into a structural move. The binding
 * legal text itself is English for every locale and stays in `lib/legal-copy`.
 */
export const legalTerms: LegalTermsDict = {
  metaTitle: "Terms of service · Codewhale",
  metaDescription: "Terms that govern your use of Codewhale, a Shannon Labs product.",
  kicker: "Legal",
  title: "Terms of service",
  updated: "Effective and last updated {date}.",
  privacyLink: "Privacy policy",
  homeLink: "Back home",
};
