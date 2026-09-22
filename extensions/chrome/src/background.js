/**
 * Service worker. It does one thing: make the toolbar button open the side panel.
 *
 * Everything else — the runtime connection, the SSE stream, the tool loop —
 * lives in the panel document on purpose. An MV3 service worker is evicted
 * after ~30 seconds idle and terminated on a schedule even while a fetch is
 * open, so a long-lived event stream owned here would drop mid-turn. The side
 * panel is a real document: it lives exactly as long as the user keeps it open,
 * which is also exactly as long as Chromewhale should be able to touch a page.
 */

chrome.sidePanel
  .setPanelBehavior({ openPanelOnActionClick: true })
  .catch((error) => console.error("Chromewhale: could not bind the toolbar button", error));
