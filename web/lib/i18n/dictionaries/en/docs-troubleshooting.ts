import type { DocsTroubleshootingDict } from "../types";

/**
 * English reference dictionary for
 * `app/[locale]/docs/troubleshooting/page.tsx`. Copy moved verbatim from the
 * page's `isZh` ternaries — any wording change belongs in its own commit,
 * never mixed into a structural move.
 */
export const docsTroubleshooting: DocsTroubleshootingDict = {
  metaTitle: "Troubleshooting · Codewhale Docs",
  metaDescription:
    "Quick triage for common issues: hung turns, the offline queue, crash recovery, schema errors, MCP failures, and Docker notes.",
  bodyClassName: "text-ink-soft leading-relaxed",
  overviewTitle: "Troubleshooting",
  overviewLead:
    "Start with quick triage: confirm the binary and config (codewhale --version, ~/.codewhale/config.toml), enable verbose logs with RUST_LOG=codewhale_tui=debug when needed (RUST_LOG=codewhale_tui::client=debug for HTTP retries/reconnects; logs land in ~/.codewhale/logs/), and capture the current state of ~/.codewhale/sessions and ~/.codewhale/tasks.",
  incidents: [
    [
      "Turn hangs or the stream stops",
      "If a foreground shell command is still running, press Ctrl+B to move it to the background (the turn keeps running and the command becomes a background job under /jobs); use Esc or Ctrl+C to cancel the turn itself. Inspect codewhale_tui::client retry logs and endpoint connectivity, and after a restart confirm the previously in-flight turn shows as interrupted rather than running.",
    ],
    [
      "Network outage / offline behavior",
      "New prompts queue while offline, persisted per session to ~/.codewhale/sessions/checkpoints/<session-id>.offline_queue.json (a legacy global offline_queue.json is adopted once on upgrade). Inspect with /queue list, restore connectivity, then re-send queued entries (/queue edit <n> plus Enter, or the normal input flow); the queue file clears when the queue empties.",
    ],
    [
      "Crash recovery",
      "Each session checkpoints to ~/.codewhale/sessions/checkpoints/<session-id>.json (a legacy latest.json is still read but no longer written); startup begins a fresh session unless --resume/--continue is supplied. Resume explicitly with codewhale --resume <id> or Ctrl+R in the TUI; if the checkpoint schema is newer than the binary supports, upgrade the binary or remove the stale checkpoint.",
    ],
    [
      "Persistent state schema errors",
      "Errors like schema vX is newer than supported vY affect sessions, runtime thread/turn/item records, and tasks. Confirm the binary version, back up the state directory before editing, then either run a newer compatible binary or archive the incompatible records and regenerate state.",
    ],
    [
      "MCP / tool execution failures",
      "Validate the ~/.codewhale/mcp.json schema and server command paths, confirm the server process starts manually, and check sandbox denials in TUI history/logs. Use /mcp validate for diagnostics, temporarily disable a failing server to isolate the issue, and re-enable after verification.",
    ],
  ],
  dockerTitle: "Docker notes",
  dockerLead:
    "Each release publishes a multi-arch Linux image to GitHub Container Registry. The default image is a conservative runtime image: it runs as the non-root codewhale user (UID/GID 1000:1000), grants no passwordless sudo, and keeps user state in a volume mounted at /home/codewhale/.codewhale. Pin a release tag instead of latest for reproducible installs.",
  dockerToolboxNote:
    "When a project needs apt-get, compiler toolchains, or package managers inside the container, do not change the default image contract — build an explicit toolbox image from docs/examples/Dockerfile.toolbox, and use one named state volume per project so sessions, config, and the offline queue do not bleed across workspaces. Never bake API keys or SSH private keys into custom images.",
  sourceNote:
    "Source documents: docs/OPERATIONS_RUNBOOK.md, docs/DOCKER.md · Update docs-map.ts when changing.",
};
