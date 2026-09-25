import type { DocsComputersDict } from "../types";

/**
 * English reference dictionary for `app/[locale]/docs/computers/page.tsx`
 * ("Send a task to the cloud"). Checked against docs/DAYTONA_CLOUD_DISPATCH.md
 * and `DispatchArgs` / `run_with` in crates/cli/src/dispatch.rs. The sandbox
 * vendor is Codewhale-operated infrastructure and is deliberately not named
 * in user copy (DAYTONA_CLOUD_DISPATCH.md: no provider brand on any surface).
 * The live network path is not yet smoke-tested, so the page says preview.
 */
export const docsComputers: DocsComputersDict = {
  metaTitle: "Send a task to the cloud · Codewhale Docs",
  metaDescription:
    "Hand a coding task to a Codewhale cloud agent that works on a branch and opens a pull request — proposed first, started only when you confirm.",
  bodyClassName: "text-ink-soft leading-relaxed",
  title: "Send a task to the cloud",
  lede:
    "A cloud agent takes a task off your machine: it works in a fresh cloud sandbox, pushes a branch, and opens a pull request while you keep working locally. Nothing starts, costs money, or pushes until you confirm it.",
  sections: [
    {
      id: "status",
      title: "Know what is ready",
      blocks: [
        {
          note: "Cloud agents are a preview. The full lifecycle is covered by offline tests, but the live path — a real sandbox and a real pull request on each forge — has not been verified end to end yet. Private repositories are not supported yet.",
        },
      ],
    },
    {
      id: "before",
      title: "Before you start",
      blocks: [
        {
          list: [
            "Sign in to your Codewhale account with `codewhale login`. Without it, a task can be proposed but not confirmed.",
            "Make an account API key available as `CODEWHALE_API_KEY`, so the agent in the sandbox runs as your account. Without it, confirming is refused before anything is spent.",
            "For GitHub, be signed in to the `gh` command-line tool; Codewhale uses that login to open the pull request.",
          ],
        },
        { code: "codewhale dispatch --status", lang: "Terminal" },
        {
          p: "`--status` shows the forges found in your git remotes and whether the required credentials are present. It never prints a secret.",
        },
      ],
    },
    {
      id: "send",
      title: "Propose, then confirm",
      blocks: [
        {
          code: `codewhale dispatch "fix the flaky login test and open a PR" --remote github
codewhale dispatch --confirm cloud_<id>`,
          lang: "Terminal",
        },
        {
          p: "The first command only writes a proposal and prints its id. The second starts it. Codewhale may propose a cloud task on its own, but it never confirms one. In a session, use `/dispatch <task>` and `/dispatch confirm <id>`.",
        },
        {
          p: "Once confirmed, the agent clones the repository into a new sandbox, does the work, pushes a new branch (never a force-push), and opens the pull request. The sandbox is deleted when the job finishes, fails, or is cancelled.",
        },
      ],
    },
    {
      id: "track",
      title: "Track or cancel a job",
      blocks: [
        {
          code: `codewhale dispatch --list
codewhale dispatch --show cloud_<id>
codewhale dispatch --cancel cloud_<id>`,
          lang: "Terminal",
        },
        {
          p: "Cloud jobs also appear in `/jobs`. A job shows its progress, the branch, the pull request link once one exists, and how many minutes it ran — a runtime figure, not a bill. Cancelling tears the sandbox down right away.",
        },
      ],
    },
    {
      id: "forge",
      title: "Choose where the pull request goes",
      blocks: [
        {
          p: "Codewhale supports GitHub, CNB, and Gitee, and never assumes `origin` is GitHub. A remote named `github`, `cnb`, or `gitee` is that forge; any other remote is identified by its host. If your repository has more than one forge, pass `--remote`.",
        },
        {
          p: "If a pull request cannot be opened — for example, a forge token is missing — the job is marked failed after the push and says that no pull request was opened. It never reports a link it does not have.",
        },
      ],
    },
  ],
  next: [
    {
      href: "/docs/auth",
      label: "Connect a provider",
      note: "Sign in to your Codewhale account and manage keys.",
    },
    {
      href: "/docs/review",
      label: "Review what changed",
      note: "Review the agent's pull request before you merge it.",
    },
    {
      href: "/docs/fleet",
      label: "Run a workflow",
      note: "Run longer, multi-step work on your own machine instead.",
    },
  ],
  sourceNote:
    "Source documents: docs/DAYTONA_CLOUD_DISPATCH.md, docs/CODEWHALE_AGENT.md · Update docs-map.ts when changing.",
};
