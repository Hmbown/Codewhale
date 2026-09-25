import type { FaqDict } from "../types";

/**
 * English reference dictionary for `app/[locale]/faq/page.tsx` and
 * `components/faq-search.tsx`. Copy moved verbatim from their `isZh`
 * ternaries. The questions and answers themselves are JSX content and stay in
 * the page as `faqEn` / `faqZh`.
 */
export const faq: FaqDict = {
  metaTitle: "FAQ · Codewhale",
  metaDescription:
    "Codewhale frequently asked questions: install, config, providers, models, modes, security, and privacy. Answers sourced from real code, docs, and GitHub issues.",
  eyebrow: "FAQ",
  title: "FAQ",
  titleAside: "常见问题",
  lead: "Answers sourced from real code, docs, release notes, and GitHub issues. Sources are cited below each answer. If your question isn't covered, open an issue on GitHub.",
  notCovered: "Didn't find your question?",
  openIssue: "Open an issue →",
  searchPlaceholder: "Search FAQ… (press / to focus)",
  searchLabel: "Search FAQ",
  searchClear: "Clear",
  searchMatches: "{matched} of {total} questions match \"{query}\"",
  searchNoMatches: "No questions match \"{query}\"",
  answerClassName: "",
  sourcesLabel: "Sources",
  noResultsTitle: "No results found",
  noResultsBody: "Try a different keyword.",
};
