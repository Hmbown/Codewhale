import type { HomeDict } from "../types";

/** English reference home copy: useful work with chosen models and connected tools. */
export const home: HomeDict = {
  metaTitle: "Codewhale: build with your models and tools",
  metaDescription:
    "Build apps, automate workflows, and work across connected tools with Codewhale. Open source, with your own model APIs or local and self-hosted inference.",

  heroTitle: "An open-source agent for your computer.",
  heroIntro:
    "Build apps, automate workflows, and work with Slack, Gmail and other connected tools. {brand} uses your existing model APIs or local and self-hosted inference.",
  getCodewhale: "Install Codewhale",
  heroInstallAria: "Install command",
  exploreProduct: "Explore Codewhale",

  shotPreview: "Terminal preview",
  screenshotAlt:
    "Codewhale v{version}: a captured terminal view with the conversation, message composer and session controls.",

  latestRelease: "Latest release {tag}",
  releaseUnavailable: "Release status unavailable",
  currentSource: "Source",
  sourceCandidate: "Unreleased",
  publishedRelease: "released",

  gainHeading: "What you can do",
  gainLede:
    "Start with the result you want. Codewhale works through the files, commands and connected tools, while you choose its access and approvals.",
  gain: [
    [
      "Build apps and tools",
      "Go from an idea to a working app, a useful script, or a feature in an existing project. Let the agent write, run and test the pieces with you."
    ],
    [
      "Automate the work you repeat",
      "Turn a recurring task into a workflow. Run it from your terminal, scripts or CI, and bring in a Fleet of agents when the work can happen in parallel."
    ],
    [
      "Connect the tools you use",
      "Connect tools like Gmail and Slack through plugins, MCP servers or APIs. Work across those services alongside your files and commands."
    ]
  ],

  chapterModels: "Your models",
  modelsHeading: "Keep using your models.",
  modelsBody:
    "Connect the model APIs you already pay for, use a compatible gateway, or run inference on your own hardware. Choose a model for each session—and different models for the agents in a Fleet.",
  modelsFacts: [
    [
      "Your API accounts",
      "Connect providers such as OpenAI, Anthropic, Google or DeepSeek with your own keys."
    ],
    [
      "Your gateway",
      "Use an OpenAI-compatible endpoint and choose the models it serves."
    ],
    [
      "Your inference",
      "Run local or self-hosted models with Ollama, vLLM or SGLang."
    ]
  ],
  modelsLink: "Browse models and providers",

  startHeading: "Bring a task. Get started.",
  startLede:
    "Install Codewhale, connect a model and give it something worth doing. Start with one agent; add tools or a team when you need them.",
  startGuideLink: "Follow the getting-started guide",
  startVocabularyLink: "Look up a term",

  chapterAvailability: "Where it runs",
  availabilityHeading: "Start in the terminal.",
  availabilityLede:
    "The terminal and local browser client are available now. A native desktop app and a rebuilt hosted web app are in development.",
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
    "The terminal, local browser, and GUI need no Codewhale account. Hosted web and desktop use an account. When you use your own provider key, your provider bills that usage.",
  accountLink: "Create an account",

  surfacesHeading: "One task. Your files, apps and agents.",
  surfaces: [
    [
      "Files and terminal",
      "Create files, run commands, inspect data and test what you build. You set the working folder and permissions."
    ],
    [
      "Plugins and connected apps",
      "Add skills and tools through plugins and MCP. Review and enable the connections you want the agent to use."
    ],
    [
      "Browser and computer · preview",
      "Use browser tools and the Computer Use plugin for work in apps and websites, with the access you grant."
    ],
    [
      "Sessions you can return to",
      "Keep the conversation, tool results and work history together. Resume the task in the terminal or its local browser client."
    ],
    [
      "Teams of agents",
      "Use Fleet to split a larger job between agents with different roles and models, and follow their progress in one place."
    ]
  ],
  runtimeLink: "See all integrations",

  installBandHeading: "Install on macOS or Linux",
  copy: "Copy",
  copied: "Copied ✓",
  binaries: "Binaries",
  chinaMirrors: "China mirrors",
  installGuideLink: "Read the install guide",

  communityHeading: "Make Codewhale your own.",
  communityBody:
    "Codewhale is open source. Read the code, build a plugin, share a workflow, or help make the next release better.",
  communityLinksAria: "Community links",
  contribute: "Contribute on GitHub",
};
