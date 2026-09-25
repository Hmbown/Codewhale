import type { HomeDict } from "../types";

/**
 * English reference home dictionary — the copy contract for the Tidal Folio
 * landing page. Public-copy and public-surface tests assert against these
 * values, not against raw JSX strings.
 *
 * The page leads with what a person gains — their own models, capable
 * agents, and control on their own machine — and states availability per
 * surface as it is today. Nothing here claims cloud execution, and the
 * screenshot is described as the development build it is.
 */
export const home: HomeDict = {
  metaTitle: "Codewhale — Build and automate with the models you choose",
  metaDescription:
    "Build software, work with your files, and automate everyday tasks using open-source agents and your choice of hosted or local AI models.",

  heroTitle: "Build and automate with the models you choose",
  heroIntro:
    "{brand} is an open-source agent that reads files, edits code, runs commands, and checks its work. Use it in your terminal or local browser with a hosted or local model. You choose the tools and permissions; the session keeps the conversation and tool results.",
  getCodewhale: "Get Codewhale",
  heroInstallAria: "Install command",
  exploreProduct: "Explore the product",

  shotPreview: "Terminal preview",
  shotBuild: "v{version} development build",
  screenshotAlt:
    "Codewhale v{version} development build: whale mark, new session, message composer, Ask permissions, Work mode and model status. Rendered from an isolated terminal capture.",

  latestRelease: "Latest release {tag}",
  releaseUnavailable: "Release status unavailable",
  currentSource: "Source",
  sourceCandidate: "Unreleased",
  providerRoutes: "{count} providers",
  publishedRelease: "released",
  figcaptionSourceCandidate: "unreleased",

  chapterTerminal: "Your terminal",
  chapterTerminalTitle: "Start with something you want to make",

  gainHeading: "What you can do with Codewhale",
  gainLede:
    "Ask for a concrete result: fix a bug, understand a project, or turn a repeated task into a workflow. Start with one agent and split larger jobs when useful.",
  gain: [
    [
      "Build and check a project",
      "Ask the agent to inspect a project, make a change, then run its tests. Follow the file edits and command results as it works."
    ],
    [
      "Reuse work that repeats",
      "Turn a repeated task into a script or saved workflow. Use codewhale exec in scripts and CI, or give parts of a larger job to several agents."
    ],
    [
      "Stay in control",
      "Set permissions before work starts, respond to approval requests, and interrupt a running task. Review the conversation and tool results before continuing."
    ]
  ],

  chapterModels: "Your models",
  modelsHeading: "A choice of models for every task",
  modelsBody:
    "Choose the provider and model for each session: connect with an API key, use a supported provider sign-in, or run a local model. Your Codewhale account and your model connection serve different purposes.",
  modelsFacts: [
    ["Hosted", "Your own API key, saved with codewhale auth set --provider <id>"],
    ["Gateway", "One endpoint for many models, provider still chosen by you"],
    ["Local", "vLLM, SGLang, Ollama on localhost — usually no key"],
  ],
  modelsLink: "Explore models and providers",

  startHeading: "Getting started with Codewhale",
  startLede:
    "Install the published release, connect a model, then try one task in your project folder. A team of agents is optional; start with one and add more when the work can be split.",
  startGuideLink: "Read the getting-started guide",
  startVocabularyLink: "Look up a term",

  chapterAccount: "Get Codewhale",
  availabilityHeading: "Where you can use Codewhale",
  availabilityLede:
    "The terminal and local browser client are available now. Desktop and hosted web apps are being developed around the same session model; their availability is listed separately below.",
  availability: [
    [
      "Terminal and local browser",
      "Released",
      "Install on Linux, macOS, or Windows. Run codewhale in your terminal, or codewhale web for the local browser client. npm and Cargo are alternatives; Android on Termux is a preview.",
    ],
    [
      "Hosted web app",
      "Development preview",
      "Sign in with a Codewhale account and pair a computer in the development preview. Hosted task execution is still being qualified.",
    ],
    [
      "Desktop",
      "Development build",
      "The macOS app brings folders, conversations, and model connections into a desktop window. A public download is coming later.",
    ],
    [
      "Cloud computers",
      "In development",
      "Hosted computers for running your tasks.",
    ],
  ],
  availabilityNote:
    "The terminal and local browser do not require a Codewhale account. An account is used for hosted web and desktop access; it does not replace your model connection. Hosted model usage with your own key is billed by that provider.",
  accountLink: "Create an account",

  surfacesHeading: "Tools, connected apps, and saved work",
  surfaces: [
    ["Files and commands", "Read a project, edit files, run tests, and inspect command output within the permissions you set."],
    ["Plugins and MCP", "Connect additional tools and services. Review and enable plugins before the agent can use them."],
    ["Computer Use · source preview", "The current source includes a plugin for seeing and interacting with other applications. Enable it explicitly and grant the required system permissions."],
    ["Saved sessions", "Keep the conversation and tool results together. The local browser connects to the same Codewhale session on your computer; resume saved work instead of starting over."],
    ["Fleet", "Assign parts of a task to agents with different models and roles, and follow their progress."],
  ],
  runtimeLink: "Explore integrations",

  installBandHeading: "Install Codewhale on macOS or Linux",
  copy: "Copy",
  copied: "Copied ✓",
  binaries: "Binaries",
  chinaMirrors: "China mirrors",
  installGuideLink: "Read the install guide",

  communityHeading: "Help make Codewhale better",
  communityBody:
    "Whether you have found a bug, have an idea for a feature, or want to send your first pull request, we would like to hear from you and work together on what comes next.",
  communityLinksAria: "Community links",
  contribute: "Send a pull request",
};
