import type { ComputerUseDict } from "../types";

/**
 * English reference dictionary for `app/[locale]/computer-use/page.tsx` and
 * the Computer Use section of the install page. Product names (Codewhale,
 * Computer Use), the menu items Pause, Stop and Check for updates, and the
 * macOS setting names stay as the app shows them.
 */
export const computerUse: ComputerUseDict = {
  metaTitle: "Computer Use for Mac · Codewhale",
  metaDescription: "Download and set up Codewhale Computer Use for Mac. Background app control, permission setup, and human Pause and Stop controls.",
  title: "Computer Use",
  lead: "Let Codewhale work in your apps while you keep working. The Mac helper brings permissions, background app control, and a way to pause or stop input to your menu bar.",
  publisher: "By Codewhale",
  download: "Download for Mac",
  downloadZip: "ZIP archive (used by the in-app updater)",
  requirements: "macOS 13.5 or later · Apple silicon and Intel",
  included: "One app download. No separate Node installation or compiler required.",
  pendingTitle: "Mac download in preparation",
  pendingBody: "The public installer will appear here after Apple notarization and release checks are complete.",
  unavailableTitle: "Download availability could not be checked",
  unavailableBody: "Refresh this page to try again, or check the published releases below.",
  releases: "Published releases",
  receipt: "Download verification details",
  setup: "Set up your Mac",
  steps: [
    { title: "Install the app", body: "Open the disk image and drag Codewhale Computer Use into Applications. Open it from Applications, then choose Computer Use from the whale icon in your menu bar." },
    { title: "Review permissions", body: "Use the setup buttons to open Accessibility and Screen Recording in System Settings. You choose which permissions to grant." },
    { title: "Run the background check", body: "The helper opens a disposable practice window, enters text, and captures that window. It checks whether the pointer or active app changed during the run." },
    { title: "Connect it to Codewhale", body: "Review, trust, and enable Computer Use in Codewhale’s plugin marketplace. Use plugin 0.3.1 or later so local actions go through the helper’s Pause and Stop controls." },
  ],
  controlsTitle: "Keep working. Keep control.",
  controlsBody: "Supported actions operate on the selected app in the background. Apps and gestures that need foreground control require your authorization. The menu shows the target and input mode; Pause suspends helper input, and Stop ends its existing sessions.",
  updateTitle: "Updates when you choose",
  updateBody: "Choose Check for updates from the app. Before installing an update, it checks the download, Codewhale signature, and Apple notarization, and keeps the previous app for recovery.",
  help: "Setup and troubleshooting",
  notes: "Release notes",
  demo: "See the background check",
  source: "Source and other platforms",
  platforms: "This download is for Mac. Windows and Linux currently use the source plugin and host-side setup.",
};
