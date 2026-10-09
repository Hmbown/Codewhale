import type { RoadmapDict } from "../types";

/**
 * English reference dictionary for `app/[locale]/roadmap/page.tsx`. Copy
 * moved verbatim from its `isZh` ternaries. The tracks themselves are content
 * and stay in the page as `tracksEn` / `tracksZh`.
 */
export const roadmap: RoadmapDict = {
  metaTitle: "Roadmap · Codewhale",
  metaDescription:
    "Current Codewhale work grouped by shipped, underway, considered, and deliberately out-of-scope directions.",
  eyebrow: "Project roadmap",
  title: "Roadmap",
  introduction:
    "This page separates published releases from work in progress, proposals still being evaluated, and directions intentionally kept out of scope. Shipped lists the latest GitHub releases, each linked to its release notes. Work in progress — including changes merged in the repository but not yet released — appears under Underway. When the live feed is unavailable, static summaries stand in; the install page and homepage identify the latest published version.",
  sectionTitle: "Work grouped by status",
  browseIssues: "Browse open issues",
  trackCount: "{count} items",
  trackCountOne: "{count} item",
  contributeTitle: "Keep roadmap decisions in the open.",
  contributeBody:
    "Use issues for bugs and well-scoped feature requests, Discussions for ideas that need shaping, and pull requests for concrete changes with tests or documentation. Verification across languages, platforms, and providers helps maintainers judge priority.",
  issuesDetail: "Report a problem or propose scoped work.",
  discussionsDetail: "Explore an early idea before implementation.",
  pullsDetail: "Review existing work or send a focused change.",
};
