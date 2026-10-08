import type { DocsVocabularyDict } from "../types";

/**
 * English reference dictionary for `app/[locale]/docs/vocabulary/page.tsx`.
 * Copy moved verbatim from its `isZh` ternaries. The terms and their
 * definitions are content and stay in `lib/content/vocabulary.ts`.
 */
export const docsVocabulary: DocsVocabularyDict = {
  metaTitle: "Vocabulary · Codewhale Docs",
  metaDescription:
    "The exact product nouns: Fleet, Workflow, Lane, Runtime, Advisor, Plan / Work / Operate and permission postures, plus requested→effective reasoning, routing source, and measurement principles.",
  bodyClassName: "text-ink-soft leading-relaxed",
  title: "Vocabulary",
  lead: "These nouns mean the same thing on this site, in the TUI, and in receipts. The nouns themselves are never translated; every definition matches the public fact matrix in the repository verbatim.",
  executionHeading: "Execution nouns",
  controlHeading: "Modes and permission postures",
  controlLead:
    "Modes (Tab, composer idle) decide the visible interaction; permission postures (Shift+Tab) decide how aggressively tools ask before executing. The two are orthogonal.",
  routeHeading: "Route identity: provider · model · requested→effective reasoning · source",
  advisoryHeading: "Advisory role",
  measurementHeading: "Measurement principles",
  leaderboardNote:
    "Benchmark leaderboard: none on this site, and none will appear without route identity and a measurement harness attached.",
  sourceNote:
    "Source documents: docs/FLEET.md, docs/MODES.md, docs/public-surface-facts.json · Vocabulary copy lives in web/lib/content/vocabulary.ts; update docs-map.ts when changing.",
};
