import type { DigestDict } from "../types";

/**
 * English reference dictionary for `app/[locale]/digest/page.tsx`. Copy moved
 * verbatim from the page's `isZh` ternaries. The digests themselves carry
 * their own en/zh fields and are rendered as stored.
 */
export const digest: DigestDict = {
  metaTitle: "Community Digest · Codewhale",
  metaDescription: "Archive of weekly Codewhale community updates — maintainer-approved summaries.",
  emptyTitle: "Community Digest",
  emptyBody:
    "No maintainer-approved weekly digest exists yet. One appears here only after the cron draft has been reviewed.",
  title: "Weekly Community Updates",
  lead: "Maintainer-approved summaries from Codewhale",
};
