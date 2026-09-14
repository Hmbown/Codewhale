import type { ChromeDict } from "../types";

/**
 * Italian chrome dictionary.
 *
 * Riscrittura nativa nella direzione inglese attuale — «any model, on your
 * machine», non il ritirato posizionamento «local-first». Registro con il
 * tu, lo standard dei tool per sviluppatori in italiano.
 *
 * I modi e le posture di permesso restano letterali (Plan / Work / Operate,
 * Ask / Auto-Review / Full Access); `Runtime`, `fleet` e `TUI` restano nomi
 * di prodotto; «ricevuta» è receipt.
 *
 * Le etichette secondarie di navigazione abbinano l'etichetta italiana a
 * una breve controparte inglese — la coppia Han è il dispositivo editoriale
 * dell'edizione inglese.
 */
export const chrome: ChromeDict = {
  navDocs: "Documentazione",
  navStart: "Inizia",
  navInstall: "Installa",
  navFaq: "FAQ",
  navCommunity: "Comunità",
  navContribute: "Contribuisci",

  navDocsSecondary: "Docs",
  navStartSecondary: "Start",
  navInstallSecondary: "Install",
  navFaqSecondary: "FAQ",
  navCommunitySecondary: "Community",
  navContributeSecondary: "Contribute",

  navProduct: "Prodotto",
  navModels: "Modelli",
  navProductSecondary: "Product",
  navModelsSecondary: "Models",

  skipToContent: "Vai al contenuto principale",

  navPrimaryAria: "Navigazione principale",
  navHomeAria: "Home di Codewhale",

  installCta: "Installa →",

  authSignIn: "Accedi",
  authRegister: "Registrati",
  authGroupAria: "Account",

  wordmarkSeal: "深",
  wordmarkTag: "qualsiasi modello, sulla tua macchina",

  issueLabel: "Edizione del {date}",
  dateLocale: "it-IT",

  starsAria: "Stelle GitHub",
  githubFallback: "GitHub",

  tickerLiveLabel: "In diretta",
  tickerLiveTag: "LIVE",
  tickerMerged: "unito",
  tickerOpened: "aperto",
  tickerClosed: "chiuso",
  tickerReleased: "pubblicato",
  tickerFirstContribution: "prima contribuzione",
  tickerBy: "di {handle}",
  tickerAria: "Attività recente del repository",

  traceLabel: "traccia di ragionamento",
  traceTabsAria: "Stralci di sessione",

  menuOpen: "Apri il menu",
  menuClose: "Chiudi il menu",

  themeAuto: "auto",
  themeLight: "chiaro",
  themeDark: "scuro",
  themeAria: "Tema della documentazione: {mode} (clic per cambiare)",
  themeTitle: "Tema della documentazione · auto / chiaro / scuro",

  footerTagline:
    "Crea quello che vuoi e automatizza il lavoro quotidiano con i modelli che scegli.",
  footerProduct: "Prodotto",
  footerProject: "Progetto",
  footerDocs: "Documentazione",
  footerGuide: "Primi passi",
  footerInstall: "Installazione",
  footerModels: "Modelli",
  footerRuntime: "Runtime",
  footerFaq: "FAQ",
  footerIssues: "Issues",
  footerContribute: "Contribuisci",
  footerLicense: "Licenza MIT",
  footerTerms: "Termini di servizio",
  footerPrivacy: "Privacy",
  footerChangelog: "Registro delle modifiche",
  footerCanonicalSource: "Sorgente canonico: ",
  footerReleases: " · Release: ",
  footerReleasesLink: "Release di GitHub",
  footerSecurity: "Sicurezza",

  switcherLabel: "Lingua",
  switcherSwitchTo: "Passa a {label}",
  partialBadge: "(parziale)",
};
