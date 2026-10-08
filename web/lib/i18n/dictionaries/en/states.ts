import type { StatesDict } from "../types";

/**
 * English reference dictionary for shared surface states: empty, loading,
 * error, retry, recovery, not-found, and the connection banner. Every
 * data-bearing page renders these through `components/surface-state.tsx`
 * rather than inventing its own wording.
 */
export const states: StatesDict = {
  loadingLabel: "Loading…",
  emptyTitle: "Nothing here yet",
  emptyBody: "There are no records to show yet.",
  errorTitle: "This page did not finish loading",
  errorBody:
    "Something went wrong. Try again, or report the problem if it continues.",
  retry: "Try again",
  reload: "Reload the page",
  homeLink: "Back to the home page",
  docsIndexLink: "Open the documentation index",
  notFoundTitle: "We all make typos.",
  notFoundBody:
    "This page doesn’t exist yet.\nNeither does this game.",
  notFoundHomeLink: "Return to base",
  notFoundPosterAlt:
    "A blue whale in tactical gear on the fictional Codwhale: Modern Whalefare game poster.",
  unavailableTitle: "The live record has not loaded",
  unavailableBody:
    "The latest information is unavailable. Try refreshing this page.",

  offlineTitle: "You are offline",
  offlineBody: "Actions are paused until the connection returns. Nothing shown here is refreshing.",
  reconnectingTitle: "Reconnecting…",
  reconnectingBody: "Checking the connection (attempt {attempt}).",
  degradedTitle: "The connection is unstable",
  degradedBody: "The server did not answer the last check. What you see may be stale.",
  onlineTitle: "Back online",
  onlineBody: "The connection is restored.",
  retryNow: "Retry now",
  dismiss: "Dismiss",
  lastChecked: "Last checked {time}",
};
