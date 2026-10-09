import type { ContributeDict } from "../types";

/**
 * English reference dictionary for `app/[locale]/contribute/page.tsx`. Copy
 * moved verbatim from its `isZh` ternaries. The contribution paths, workflow
 * steps and review notes are content and stay in the page.
 */
export const contribute: ContributeDict = {
  metaTitle: "Contribute · Codewhale",
  metaDescription:
    "File issues, improve translations and documentation, and send pull requests to the international Codewhale community.",
  kicker: "Contribute to an international open-source project",
  title: "Start with one concrete improvement.",
  lede: "Contribute from any country, language, platform, or experience level. Clear bug reports, reproductions, docs fixes, translations, and small complete patches all count.",
  fileIssue: "File an issue",
  browsePulls: "Browse pull requests",
  fullGuide: "Open the full contributor guide",
  pathsTitle: "Choose the contribution that fits.",
  workflowTitle: "From a problem to a reviewable patch.",
  reviewTitle: "Make the change easy to verify.",
  reviewScope:
    "Give reviewers the problem, the reason for the change, test evidence, and remaining risk. A tight scope gets clearer feedback.",
  devTitle: "Build and run the relevant checks.",
  devScope:
    "The repository uses stable Rust. Run the focused test for your change first, then formatting, Clippy, and the workspace suite.",
};
