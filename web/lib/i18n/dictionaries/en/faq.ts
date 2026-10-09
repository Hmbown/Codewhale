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
    "Answers about installing and configuring Codewhale, providers, models, modes, security, and privacy, each sourced from code, docs, or GitHub issues.",
  eyebrow: "FAQ",
  title: "Frequently asked questions",
  titleAside: "常见问题",
  titleAsideLang: "zh",
  lead: "Each answer cites its sources: code, docs, release notes, or GitHub issues. If your question is missing, open an issue on GitHub.",
  notCovered: "Didn't find your question?",
  openIssue: "Open an issue",
  searchPlaceholder: "Search the FAQ",
  searchLabel: "Search FAQ",
  searchClear: "Clear",
  searchMatches: "{matched} of {total} questions match \"{query}\"",
  searchNoMatches: "No questions match \"{query}\"",
  answerClassName: "",
  sourcesLabel: "Sources",
  noResultsTitle: "No results found",
  noResultsBody: "Try a different keyword.",
};
