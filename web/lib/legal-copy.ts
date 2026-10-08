/** Legal policy and publication metadata come verbatim from the pinned platform source. */
export { LEGAL_DOCUMENTS, formatLegalDocumentStatus } from "../vendor/legal-documents/legal-documents.js";
export { PrivacyContent, TermsContent } from "../vendor/legal-documents/legal-content";

/** Website-specific usage disclosure; the browser preference control is owned by this site. */
export const WEBSITE_USAGE_DISCLOSURE = {
  title: "Anonymous usage counting",
  body: "Codewhale counts anonymous product usage by default. On this website that means plain totals of page views, documentation views, install-command copies, sign-in and sign-up link clicks, and error pages shown, sent with a random install identifier that rotates every 90 days to Codewhale’s own endpoint; Codewhale may pass those totals to PostHog as a processor. No page addresses, referrers, account, or content are included. In the Codewhale runtime and app the same rule covers aggregate version, platform, session, feature, and error counts, which never include conversations, code, prompts, files, repository or branch names, model content, or credentials. You can turn counting off at any time: for this browser on this page, in the app under Settings, or in the runtime with `codewhale config set telemetry false` or CODEWHALE_TELEMETRY=0. An opt-out is kept and never silently reversed, and presenting this notice does not record any acceptance on your behalf.",
} as const;
