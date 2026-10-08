import type { HomeDict } from "../types";

/**
 * English reference home dictionary — the copy contract for the whale-road
 * landing page. Public-copy and public-surface tests assert against these
 * values, not against raw JSX strings.
 *
 * The page leads with what a person gains — their own models, capable
 * agents, and control on their own machine — and states availability per
 * surface as it is today. Nothing here claims cloud execution, and the
 * screenshot is described as the development build it is.
 */
export const home: HomeDict = {
  metaTitle: "Codewhale: the open-source coding agent for any model",
  metaDescription:
    "Codewhale is an open-source coding agent for your terminal. It reads your project, edits files, and runs your tests with the hosted or local model you choose.",

  heroTitle: "The open-source coding agent for any model",
  heroIntro:
    "{brand} reads your project, edits files, and runs your tests from your terminal. Connect a hosted or local model, and choose which actions need your approval.",
  getCodewhale: "Install Codewhale",
  heroInstallAria: "Install command",
  exploreProduct: "See how it works",

  shotPreview: "Terminal preview",
  shotBuild: "v{version} pre-release build",
  screenshotAlt:
    "Codewhale v{version} pre-release build: whale mark, new session, message composer, Ask permissions, Work mode and model status. Rendered from an isolated terminal capture.",

  latestRelease: "Latest release {tag}",
  releaseUnavailable: "Release status unavailable",
  currentSource: "Source",
  sourceCandidate: "Unreleased",
  publishedRelease: "released",
  figcaptionSourceCandidate: "unreleased",
  chapterTerminal: "Your terminal",
  chapterTerminalTitle: "Follow each edit and command as it runs",

  gainHeading: "Delegate the task and keep control",
  gainLede:
    "Ask for a result: fix a bug, explain a module, or automate a task you repeat. Start with one agent, and add more agents when the job grows.",
  gain: [
    [
      "Change code and check it",
      "The agent inspects your project, edits files, and runs your tests. Follow each edit and command result as it works."
    ],
    [
      "Automate repeated work",
      "Run codewhale exec from scripts and CI. Use a Fleet to divide a larger job among several agents."
    ],
    [
      "Stay in control",
      "Set permissions before work starts, answer approval requests, and stop a task at any point. Run /receipts to list every file, command, and approval in a session."
    ]
  ],

  chapterModels: "Your models",
  modelsHeading: "Choose a model for each task",
  modelsBody:
    "Choose a built-in provider, any OpenAI-compatible endpoint, or a local model for each session. Your model connection stays separate from any Codewhale account.",
  modelsFacts: [
    ["Hosted", "Your own API key, saved with codewhale auth set --provider <id>"],
    ["Gateway", "One endpoint for many models; you still choose the provider"],
    ["Local", "vLLM, SGLang, or Ollama on localhost, usually with no key"],
  ],
  modelsLink: "Browse models and providers",

  startHeading: "Install, connect a model, run a task",
  startLede:
    "Run your first task in three steps from your project folder. Add a Fleet later if the work needs several agents.",
  startGuideLink: "Follow the getting-started guide",
  startVocabularyLink: "Look up a term",

  chapterAvailability: "Where it runs",
  availabilityHeading: "Use it in your terminal today",
  availabilityLede:
    "Use the terminal, the local browser client, or the community CodeWhale GUI now. The desktop app and the rebuilt hosted web app are in development and share the same session model.",
  availability: [
    [
      "Terminal and local browser",
      "Released",
      "Install on Linux, macOS, or Windows, then run codewhale, or codewhale web for the local browser client. npm and Cargo also work; Android on Termux is a preview.",
    ],
    [
      "CodeWhale GUI (VS Code)",
      "Available",
      "A separate, community-maintained project: chat, threads, and file changes in a VS Code sidebar over the same Codewhale Runtime. Install it from the VS Code Marketplace.",
      "https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode",
    ],
    [
      "Hosted web app",
      "Development preview",
      "Being rebuilt to match the desktop app. Today you can sign in, then type /rc in a running terminal session to continue it on the web; hosted task execution is still being qualified.",
    ],
    [
      "Desktop",
      "Development build",
      "The native app becoming the main Codewhale client: folders, conversations, and model connections in one window. No public download yet.",
    ],
    [
      "Cloud computers",
      "In development",
      "Hosted computers that run your tasks.",
    ],
  ],
  availabilityNote:
    "The terminal, local browser, and GUI need no Codewhale account. Hosted web and desktop use an account, which does not replace your model connection; your provider bills usage on your own key.",
  accountLink: "Create an account",

  surfacesHeading: "Extend what the agent can reach",
  surfaces: [
    ["Files and commands", "Read the project, edit files, run tests, and inspect output within the permissions you set."],
    ["Plugins and MCP", "Connect more tools and services. Each plugin stays off until you review and enable it."],
    ["Computer Use · preview", "A plugin that lets the agent see and operate other apps. You enable it and grant the system permissions it asks for."],
    ["Saved sessions", "Keep the conversation and tool results together, and resume instead of starting over. The local browser opens the same session on your computer."],
    ["Fleet", "Assign parts of a task to agents with different models and roles, then follow their progress."],
  ],
  runtimeLink: "See all integrations",

  installBandHeading: "Install on macOS or Linux",
  copy: "Copy",
  copied: "Copied ✓",
  binaries: "Binaries",
  chinaMirrors: "China mirrors",
  installGuideLink: "Read the install guide",

  communityHeading: "Build Codewhale with us",
  communityBody:
    "Report a bug, propose a feature, or send your first pull request on GitHub. Small, tested fixes are welcome.",
  communityLinksAria: "Community links",
  contribute: "Send a pull request",
};
