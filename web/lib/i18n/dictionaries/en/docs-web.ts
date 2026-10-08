import type { DocsWebDict } from "../types";

/**
 * English reference dictionary for `app/[locale]/docs/web/page.tsx`
 * ("Open the browser client"). Checked against docs/WEB.md, `WebArgs` in
 * crates/cli/src/lib.rs (`--port`, default 7878), and the `/rc` command in
 * crates/tui/src/commands/groups/session/remote_control.rs.
 */
export const docsWeb: DocsWebDict = {
  metaTitle: "Open the browser client · Codewhale Docs",
  metaDescription:
    "Work with Codewhale in a browser tab on your own machine, or continue a running terminal session from the Codewhale web app with /rc.",
  bodyClassName: "text-ink-soft leading-relaxed",
  title: "Open the browser client",
  lede:
    "Prefer a browser window to a terminal? Codewhale can serve its own client on your machine. It is another view of the same local session — same approvals, same sandbox, no account.",
  sections: [
    {
      id: "start",
      title: "Start it",
      blocks: [
        { p: "Run this from the folder you want Codewhale to work in:" },
        { code: "codewhale web\ncodewhale web --port 8788   # if 7878 is taken", lang: "Terminal" },
        {
          p: "Codewhale starts a local server at `http://127.0.0.1:7878`, prints a one-time link, and opens it in your default browser. If the browser does not open, use the printed link within ten minutes. Press Ctrl+C in the terminal to stop; the browser session ends with it.",
        },
      ],
    },
    {
      id: "use",
      title: "Work in the browser",
      blocks: [
        {
          p: "The browser client lists and searches your threads, shows the transcript with each tool's result, and has a composer. You can start, steer, or interrupt a turn, answer approvals, and rename or archive threads. Your provider keys stay in Codewhale; nothing is copied into browser storage.",
        },
      ],
    },
    {
      id: "local",
      title: "Keep it local",
      blocks: [
        {
          list: [
            "The server only listens on `127.0.0.1`. There is no option to open it to your network, and it cannot run without authentication.",
            "The link carries a single-use code, not your access token. Opening it swaps the code for a cookie tied to this process, and the code stops working.",
            "Do not forward the port through a router, a public proxy, or a tunnel. For a phone or another machine, see the [Runtime API](/docs/runtime-api) and read its authentication rules first.",
          ],
        },
      ],
    },
    {
      id: "remote",
      title: "Continue a session from the web app",
      blocks: [
        {
          p: "This is different: it hands a session already running in your terminal to the signed-in Codewhale web app, so you can keep going from another device. It needs a [Codewhale account](/docs/auth#account).",
        },
        {
          code: `/rc          # in the running session; approve the one-time code in your browser
/rc status   # who controls the session now
/rc link     # print the session link
/rc stop     # hand control back to the terminal`,
          lang: "Codewhale",
        },
        {
          p: "While the web app holds the session, new prompts and approvals come from the browser, and the terminal stays readable. Either side can interrupt. You can also start a session this way with `codewhale rc`.",
        },
      ],
    },
    {
      id: "fix",
      title: "If something goes wrong",
      blocks: [
        {
          rows: [
            ["Port in use", "Pass a free port with `--port`."],
            ["Browser did not open", "Copy the printed link into a browser on the same machine within ten minutes."],
            ["Link expired or already used", "That is expected. Run `codewhale web` again for a new one."],
            ["No model answers", "The web command does not set up providers. Check `codewhale doctor` and `/provider`."],
          ],
        },
      ],
    },
  ],
  next: [
    {
      href: "/docs/runtime-api",
      label: "Automate with the Runtime API",
      note: "The local API the browser client is built on.",
    },
    {
      href: "/docs/auth",
      label: "Connect a provider",
      note: "Give the browser client a model to talk to.",
    },
    {
      href: "/docs/modes",
      label: "Set modes and approvals",
      note: "The same approvals apply in the browser.",
    },
  ],
  sourceNote: "Source document: docs/WEB.md · Update docs-map.ts when changing.",
};
