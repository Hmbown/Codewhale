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
    "This page separates completed repository work from work in progress, proposals still being evaluated, and directions intentionally kept out of scope. Roadmap Shipped can include work implemented in a source candidate; the install page and homepage separately identify the latest published package. Release records and GitHub issues refresh these categories when available.",
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
