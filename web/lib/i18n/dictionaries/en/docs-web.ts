import type { DocsWebDict } from "../types";

/**
 * English reference dictionary for `app/[locale]/docs/web/page.tsx`.
 * Keep local-client behavior aligned with docs/WEB.md and the Runtime.
 */
export const docsWeb: DocsWebDict = {
  metaTitle: "Browser Client · Codewhale Docs",
  metaDescription:
    "The loopback-only embedded browser client — one-time bootstrap, session cookie, the local trust boundary — and remote control of a running local session from the signed-in web app with /rc.",
  bodyClassName: "text-ink-soft leading-relaxed",
  overviewTitle: "Browser Client",
  overviewLead:
    "{webCommand} opens Codewhale's embedded browser client over the canonical Runtime API. It is a local surface: the server always binds to {loopbackHost}, cannot be rebound to a LAN address, and cannot run with Runtime authentication disabled. The default address is {defaultUrl}; on a port collision, pick another loopback port with {portExample}. Stop the process with Ctrl+C and the browser session ends with it.",
  overviewBody:
    "The current client provides a responsive thread and search rail, Runtime-owned session facts, transcript and tool receipts, and a composer. It can create, select, rename, and archive threads; start or steer turns; interrupt work; resolve approvals; and answer Runtime user-input requests. The browser is another view of the same local Runtime — it does not create a second cloud account, copy provider credentials into browser storage, or weaken the configured approval and sandbox policies.",
  workflowTitle: "Working in the browser",
  workflowLead:
    "Search recent threads or preview saved sessions from the history rail. Saved previews are read-only until you choose to resume. When creating a thread, choose a provider, then search its model catalog. Starter prompts fill the composer for review before you send. Finished fenced code blocks have a Copy button; other message text remains literal. The sidebar switches light and dark themes, and Back to latest returns to the newest message after you scroll back.",
  draftsBody:
    "Unsent drafts are kept per thread in this tab's session storage so they survive a reload. Storage retains at most the 50 most recent drafts and is best effort; if unavailable, full, or given an oversized draft payload, drafts remain in memory and cannot be guaranteed after a reload. Only the theme preference uses local storage. The client does not put Runtime tokens or configured provider credentials in either store.",
  shortcutsBody:
    "Use Cmd/Ctrl+K to search, Cmd/Ctrl+N to create a thread, Enter to send, and Shift+Enter for a new line. Input-method composition does not send a partially composed message.",
  authTitle: "Authentication boundary",
  authLead:
    "The browser-launch URL carries a random, short-lived, one-time bootstrap capability — never the Runtime bearer token. A loopback request exchanges it for an HttpOnly, SameSite=Strict, process-local session cookie and immediately invalidates the capability. Reused, expired, malformed, and non-loopback bootstrap attempts fail closed. The Runtime token is never placed in rendered HTML, browser storage, URL queries or fragments, or browser-launch arguments. Cookie-authenticated state-changing requests must also present the exact local web origin; cross-origin browser requests are rejected.",
  localTitle: "Local means local",
  localLead:
    "{webCommand} accepts only {portFlag} — there is no {hostFlag} and no insecure-auth option on this command. Do not treat it as a public website or expose its port through router forwarding, a public reverse proxy, or a tunnel. The separate {mobileCommand} and {httpFlag} modes carry different deployment and authentication contracts; read the Runtime API documentation before operating either one, especially before selecting a non-loopback bind.",
  remoteTitle: "Remote control from the web app",
  remoteLead:
    "Available now. To continue the exact running local session from the signed-in Codewhale web app, type /rc in that session or launch with codewhale rc, then approve the one-time code in your browser. While the lease is active the browser owns new prompts and approvals and the terminal stays a readable safety surface; interrupt remains available from both.",
  remoteBody:
    "Once connected, the banner and a transcript note show the live session link. /rc open opens it in your browser, /rc link prints it, /rc status shows who owns the session, and /rc stop returns it to the terminal. A dropped connection keeps local input locked until the last web lease expires, so two controllers never race. Every folder you enroll from one terminal shares a single stable device id, so the web app lists one computer per machine, not one per session. This is different from the loopback browser client above: /rc pairs a local session with your account; codewhale web serves a local page with no account at all.",
  troubleshootingTitle: "Troubleshooting",
  troubleshootingLead:
    "If port 7878 is occupied, pass an unused --port. If the browser does not open, use the printed single-use launch URL on the same machine within ten minutes. Start again if it has been used or expired. If a provider is unavailable, inspect codewhale doctor and /provider; the web command does not configure or move credentials. For a temporary connection failure while the Runtime is running, choose Reconnect to reload its state while keeping your draft. If the browser session expired, restart codewhale web to mint a new session; Reconnect cannot renew it.",
  sourceNote: "Source document: docs/WEB.md · Update docs-map.ts when changing.",
};
