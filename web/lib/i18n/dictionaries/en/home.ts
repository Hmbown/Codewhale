import type { HomeDict } from "../types";

/** English reference home copy: useful work with chosen models and connected tools. */
export const home: HomeDict = {
  metaTitle: "Codewhale: build with your models and tools",
  metaDescription:
    "Build apps, automate workflows, and work across connected tools with Codewhale. Open source, with your own model APIs or local and self-hosted inference.",

  heroTitle: "Build apps and automate your work.",
  heroIntro:
    "{brand} is an open-source agent that writes code, runs commands and works with the tools you connect. Use your existing model APIs, or run models locally and on your own servers.",
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
    "Describe what you want to build or automate. Codewhale can edit files, run commands and check the result, with access you control.",
  gain: [
    [
      "Build apps and tools",
      "Create an app, add a feature or write a script. Codewhale can work through the project files, run the code and test what it builds."
    ],
    [
      "Automate the work you repeat",
      "Run workflows from your terminal, scripts or CI. For larger tasks, delegate parts of the work to a Fleet of agents with different models."
    ],
    [
      "Connect the tools you use",
      "Add tools through plugins and MCP servers, or use APIs from your own scripts. Each service needs its own setup and authentication."
    ]
  ],

  chapterModels: "Your models",
  modelsHeading: "Use the models you choose",
  modelsBody:
    "Connect your provider accounts, an OpenAI-compatible endpoint, or local and self-hosted models. Pick a model for the session and for each agent in a Fleet.",
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

  startHeading: "Get started",
  startLede:
    "Install Codewhale, connect a model and open a project folder. You can add plugins and more agents as you need them.",
  startGuideLink: "Follow the getting-started guide",
  startVocabularyLink: "Look up a term",

  chapterAvailability: "Where it runs",
  availabilityHeading: "Available now and in development",
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

  surfacesHeading: "Work across files and tools",
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
  runtimeLink: "Explore tools and integrations",

  installBandHeading: "Install on macOS or Linux",
  copy: "Copy",
  copied: "Copied ✓",
  binaries: "Binaries",
  chinaMirrors: "China mirrors",
  installGuideLink: "Read the install guide",

  communityHeading: "Contribute to Codewhale",
  communityBody:
    "Report a bug, improve the docs or contribute code on GitHub. You can also build plugins and share workflows with other users.",
  communityLinksAria: "Community links",
  contribute: "Contribute on GitHub",
};
