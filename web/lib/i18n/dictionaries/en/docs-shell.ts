import type { DocsShellDict } from "../types";

/**
 * English reference dictionary for the docs shell: the portal hero in
 * `app/[locale]/docs/layout.tsx`, the hub metadata, the task/topic search,
 * the sidebar and breadcrumb chrome, the release-truth band, the contextual
 * help band that closes every docs page, the shared page-body chrome, and the
 * session recording panel.
 */
export const docsShell: DocsShellDict = {
  metaTitle: "Docs · Codewhale",
  metaDescription:
    "Install Codewhale, connect a provider, and get work done: modes and approvals, reviewing changes, workflows, sub-agents, MCP tools, hooks, the Runtime API, and troubleshooting.",
  portalMark: "Codewhale documentation",
  heroTitle: "Get something done with Codewhale.",
  heroLead:
    "Start from what you want to do. Each page says what you need, shows commands you can run today, and points to the next step.",
  installCta: "Install Codewhale",

  releaseLabel: "Release",
  releasePublished: "Latest release {tag} · {date}",
  releaseCandidate:
    "These pages describe the {version} source candidate, which is not published yet.",
  releaseMatches: "These pages describe {tag}, the published release.",
  releaseChangelog: "Changelog →",

  searchLabel: "Search the documentation",
  searchPlaceholder: "Search by task or topic… (press / to focus)",
  searchClear: "Clear",
  searchMatches: "{matched} of {total} entries match “{query}”",
  searchNoMatches: "Nothing matches “{query}”",
  tasksHeading: "By task",
  tasksLead: "Start from what you are trying to do.",
  topicsHeading: "By topic",
  webGuideTag: "Web guide",
  sourceDocTag: "Source doc",
  emptyTitle: "No matching entry",
  emptyBody:
    "Try a different word — searches match English and Chinese — or browse the complete docs directory on GitHub.",
  emptyCta: "GitHub docs directory ↗",
  indexNote:
    "Web guides open on codewhale.net. Source docs open the full reference in the GitHub repository.",

  sidebarHeading: "Documentation",
  sidebarAria: "Documentation index",
  breadcrumbAria: "Breadcrumb",
  breadcrumbHome: "Home",
  breadcrumbDocs: "Docs",

  helpTitle: "Need more than this page?",
  helpLead:
    "Every guide is checked against a document in the repository. If something is wrong or missing, say so where the maintainer will see it.",
  helpSource: "Source: {name}",
  helpTroubleshooting: "Fix a problem",
  helpFaq: "FAQ",
  helpDiscord: "Ask on Discord ↗",
  helpIssue: "Report a docs problem ↗",

  nextHeading: "Next",
  noteLabel: "Note:",
  onThisPage: "On this page",

  mediaPendingNote:
    "There is no recording yet. When there is, it goes here with captions, a transcript, and an optional GIF download.",
  mediaPlanLink: "Recording plan and acceptance checklist ↗",
  mediaGifFallback: "GIF fallback download (no-video environments)",
  mediaTranscript: "Transcript ↗",
};
