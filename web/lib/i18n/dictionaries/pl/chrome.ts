import type { ChromeDict } from "../types";

/**
 * Polish chrome dictionary.
 *
 * Natywny przekład w aktualnym kierunku angielskiego — „any model, on your
 * machine", bez wycofanego pozycjonowania „local-first". Rejestr bezpośredni
 * (ty), standard polskich narzędzi deweloperskich.
 *
 * Tryby i postawy uprawnień zostają dosłowne (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access); `Runtime`, `fleet` i `TUI` to nazwy produktu;
 * „receipt" to potwierdzenie.
 *
 * Drugie etykiety nawigacji parują polską etykietę z krótkim angielskim
 * odpowiednikiem — para han to zabieg redakcyjny wydania angielskiego.
 */
export const chrome: ChromeDict = {
  navDocs: "Dokumentacja",
  navStart: "Pierwsze kroki",
  navInstall: "Instalacja",
  navFaq: "FAQ",
  navCommunity: "Społeczność",
  navContribute: "Współtwórz",

  navProduct: "Produkt",
  navModels: "Modele",
  navPlugins: "Wtyczki",

  skipToContent: "Przejdź do treści głównej",

  navPrimaryAria: "Nawigacja główna",
  navHomeAria: "Strona główna Codewhale",

  installCta: "Instaluj →",

  authSignIn: "Zaloguj się",

  dateLocale: "pl-PL",

  menuOpen: "Otwórz menu",
  menuClose: "Zamknij menu",

  themeAuto: "auto",
  themeLight: "jasny",
  themeDark: "ciemny",
  themeAria: "Motyw: {mode} (kliknij, aby przełączyć)",
  themeTitle: "Motyw · auto / jasny / ciemny",

  footerTagline:
    "Twórz to, co chcesz, i automatyzuj codzienną pracę z wybranymi przez siebie modelami.",
  footerProduct: "Produkt",
  footerProject: "Projekt",
  footerDocs: "Dokumentacja",
  footerGuide: "Pierwsze kroki",
  footerInstall: "Instalacja",
  footerModels: "Modele",
  footerRuntime: "Runtime",
  footerFaq: "FAQ",
  footerIssues: "Issues",
  footerContribute: "Współtwórz",
  footerLicense: "Licencja MIT",
  footerTerms: "Warunki usługi",
  footerPrivacy: "Prywatność",
  footerChangelog: "Dziennik zmian",
  footerCanonicalSource: "Repozytorium źródłowe: ",
  footerReleases: " · Wydania: ",
  footerReleasesLink: "Wydania na GitHubie",
  footerSecurity: "Bezpieczeństwo",

  switcherLabel: "Język",
  switcherSwitchTo: "Przełącz na: {label}",
  partialBadge: "(częściowo)",
};
