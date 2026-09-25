import type { DocsFleetDict } from "../types";

/**
 * English reference dictionary for `app/[locale]/docs/fleet/page.tsx`
 * ("Run a workflow"). Checked against `WorkflowCommand` and `LaneArgs` in
 * crates/cli/src/lib.rs, the Lane runtime backends in
 * crates/lane/src/runtime.rs, `FleetCommand` in crates/tui/src/lib.rs, and
 * docs/FLEET_WORKFLOW_TUTORIAL.md / docs/WORKFLOW_AUTHORING.md.
 */
export const docsFleet: DocsFleetDict = {
  metaTitle: "Run a workflow · Codewhale Docs",
  metaDescription:
    "Save roles and models in a Fleet, write a repeatable Workflow, run it as a Lane you can watch and stop, and run batches of tasks with durable workers.",
  bodyClassName: "text-ink-soft leading-relaxed",
  title: "Run a workflow",
  lede:
    "For most multi-step work you only need to ask: in Operate, Codewhale plans the steps and runs independent ones in parallel. Write a Workflow when you want the same ordered plan every time — phases, parallel branches, and a summary — with a record of each run.",
  sections: [
    {
      id: "fleet",
      title: "Save roles in a Fleet",
      blocks: [
        {
          p: "Your Fleet is the list of roles Codewhale can hand work to, and the model each role uses. Set it up once inside a session:",
        },
        { code: "/fleet setup\n/fleet\n/fleet saved", lang: "Codewhale" },
        {
          p: "`/fleet setup` walks you through a role, its model (or “use the session's model”), and where to save it: this project, or your personal profile for every repository. You review the exact file before it is written. `/fleet` shows the members of the selected Fleet, and `/fleet saved` switches between named Fleets.",
        },
        {
          p: "A Fleet only chooses who does the work. What a worker may read, write, or run still comes from your workspace trust, [approval setting](/docs/modes), and sandbox.",
        },
      ],
    },
    {
      id: "write",
      title: "Write a workflow",
      blocks: [
        {
          p: "A Workflow is a JavaScript file in your repository's `workflows/` folder. It describes steps; it does not do the work itself. This one reviews two areas in parallel, then combines the findings. Save it as `workflows/docs_readiness.workflow.js`:",
        },
        {
          code: `export default workflow({
  "id": "docs-readiness",
  "goal": "Review the docs and code for gaps, then summarize the next edit",
  "nodes": [
    {
      "branch": {
        "id": "parallel-review",
        "parallel": true,
        "children": [
          { "agent": { "id": "code-review", "prompt": "Inspect src/ for undocumented behavior.",
                       "agent_type": "review", "mode": "read_only", "file_scope": ["src"] } },
          { "agent": { "id": "docs-review", "prompt": "Inspect docs/ for stale or missing steps.",
                       "agent_type": "review", "mode": "read_only", "file_scope": ["docs"] } }
        ]
      }
    },
    {
      "reduce": {
        "id": "summary",
        "inputs": ["code-review", "docs-review"],
        "prompt": "Combine the findings into the safest next edit."
      }
    }
  ]
});`,
          lang: "workflows/docs_readiness.workflow.js",
        },
        {
          p: "Steps can be `agent`, `branch`, `sequence`, `reduce`, `loop_until`, `cond`, `expand`, and `teacher_review`. A workflow file has no file, shell, or network access of its own, and `import`, `fetch`, `eval`, and `async` are rejected. The agents it starts do the real work, under your normal permissions.",
        },
        {
          note: "One run can start up to 1,000 agents, with at most 16 working at once; the rest wait for a slot. Loops must declare `max_iterations`.",
        },
      ],
    },
    {
      id: "run",
      title: "Run it",
      blocks: [
        {
          code: `codewhale workflow run docs-readiness --runtime inline
codewhale workflow run docs-readiness --goal "prepare the 1.2 release" --verify`,
          lang: "Terminal",
        },
        {
          p: "Codewhale finds `workflows/docs_readiness.workflow.js` from the name, checks it, and starts it. `--runtime inline` runs it in this terminal. The default, `tmux`, runs it in a detached tmux session that keeps going after you close the terminal. `--verify` runs the verification gates after a successful finish, and `--fleet <name>` uses a named Fleet instead of the built-in roles.",
        },
        {
          p: "To keep the work off your checkout, add `--worktree-repo . --branch <name>`: the run gets its own git worktree and branch.",
        },
        {
          p: "Inside a session, `/workflow` starts a workflow and `/workflows` lists or cancels the runs in that session.",
        },
      ],
    },
    {
      id: "watch",
      title: "Watch and stop a run",
      blocks: [
        { p: "Each run is a Lane. Lanes are saved to disk, so you can check on them from any terminal:" },
        {
          code: `codewhale lane list
codewhale lane status <lane-id>
codewhale lane logs <lane-id>
codewhale lane attach <lane-id>
codewhale lane interrupt <lane-id>`,
          lang: "Terminal",
        },
        {
          p: "`lane list`, `lane status`, and `lane interrupt` accept `--json` and print a machine-readable receipt. In a session, `/lane` offers the same controls with the same results.",
        },
      ],
    },
    {
      id: "batch",
      title: "Run a batch of tasks",
      blocks: [
        {
          p: "When you have a list of separate tasks rather than one plan, write them as a task file and run them as a Fleet run. Each task names its goal, its role, and the paths it may write. [The tutorial](https://github.com/Hmbown/CodeWhale/blob/main/docs/FLEET_WORKFLOW_TUTORIAL.md) has a complete `tasks.json`.",
        },
        {
          code: `codewhale fleet init
codewhale fleet run tasks.json --max-workers 4
codewhale fleet status
codewhale fleet logs <worker-id>
codewhale fleet resume <run-id>
codewhale fleet stop --all`,
          lang: "Terminal",
        },
        {
          p: "`fleet status` counts queued, running, finished, and failed work from this workspace's run record. `fleet resume` picks a run back up after the laptop slept or the manager exited, without starting a new one. For the agents attached to your current session only, use `/fleet workers` (or `/subagents`).",
        },
      ],
    },
  ],
  next: [
    {
      href: "/docs/subagents",
      label: "Run agents in parallel",
      note: "Hand independent pieces of one task to sub-agents without writing a workflow.",
    },
    {
      href: "/docs/review",
      label: "Review what changed",
      note: "Check the diff a run produced and get a review before you push.",
    },
    {
      href: "/docs/vocabulary",
      label: "Product terms",
      note: "Fleet, Workflow, Lane, and Runtime, each in one sentence.",
    },
  ],
  sourceNote:
    "Source documents: docs/FLEET.md, docs/FLEET_WORKFLOW_TUTORIAL.md, docs/WORKFLOW_AUTHORING.md · Update docs-map.ts when changing.",
};
