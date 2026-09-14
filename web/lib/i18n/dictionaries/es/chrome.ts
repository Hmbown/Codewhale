import type { ChromeDict } from "../types";

/**
 * Spanish chrome dictionary — native rewrite in neutral (pan-Hispanic)
 * Spanish, informal `tú`, matching the register of the es-419 TUI pack and
 * mirroring the current English direction (bring-your-own-model, runs on
 * your machine; the old positioning is gone).
 *
 * Terminology is aligned with crates/tui/locales/es-419.json so the website
 * and the terminal name the same things the same way: modes stay literal
 * (Plan / Work / Operate), permission postures stay literal (Ask /
 * Auto-Review / Full Access) under "postura de permisos", `Runtime`,
 * `fleet`, and `Workflow` stay literal product nouns, and "receipt" is
 * "recibo".
 *
 * Secondary nav labels pair the Spanish primary with the short English
 * label, the pattern every non-English locale uses; the Han pair is the
 * English edition's own device.
 */
export const chrome: ChromeDict = {
  navDocs: "Documentación",
  navStart: "Empezar",
  navInstall: "Instalar",
  navFaq: "Preguntas",
  navCommunity: "Comunidad",
  navContribute: "Contribuir",

  navDocsSecondary: "Docs",
  navStartSecondary: "Start",
  navInstallSecondary: "Install",
  navFaqSecondary: "FAQ",
  navCommunitySecondary: "Community",
  navContributeSecondary: "Contribute",

  navProduct: "Producto",
  navModels: "Modelos",
  navProductSecondary: "Product",
  navModelsSecondary: "Models",

  skipToContent: "Saltar al contenido principal",


  navPrimaryAria: "Navegación principal",
  navHomeAria: "Inicio de Codewhale",

  installCta: "Instalar →",

  authSignIn: "Iniciar sesión",
  authRegister: "Crear cuenta",
  authGroupAria: "Cuenta",

  wordmarkSeal: "深",
  wordmarkTag: "cualquier modelo, en tu máquina",

  issueLabel: "Edición {date}",
  dateLocale: "es-419",

  starsAria: "Estrellas en GitHub",
  githubFallback: "GitHub",

  tickerLiveLabel: "En vivo",
  tickerLiveTag: "LIVE",
  tickerMerged: "fusionado",
  tickerOpened: "abierto",
  tickerClosed: "cerrado",
  tickerReleased: "publicado",
  tickerFirstContribution: "primera contribución",
  tickerBy: "por {handle}",
  tickerAria: "Actividad reciente del repositorio",

  traceLabel: "traza de razonamiento",
  traceTabsAria: "Extractos de sesión",

  menuOpen: "Abrir menú",
  menuClose: "Cerrar menú",

  themeAuto: "auto",
  themeLight: "claro",
  themeDark: "oscuro",
  themeAria: "Tema de la documentación: {mode} (clic para alternar)",
  themeTitle: "Tema de la documentación · auto / claro / oscuro",

  footerTagline:
    "Crea lo que quieras y automatiza el trabajo cotidiano con los modelos que elijas.",
  footerProduct: "Producto",
  footerProject: "Proyecto",
  footerDocs: "Documentación",
  footerGuide: "Primeros pasos",
  footerInstall: "Instalación",
  footerModels: "Modelos",
  footerRuntime: "Runtime",
  footerFaq: "Preguntas frecuentes",
  footerIssues: "Incidencias",
  footerContribute: "Contribuir",
  footerLicense: "Licencia MIT",
  footerTerms: "Términos del servicio",
  footerPrivacy: "Privacidad",
  footerChangelog: "Registro de cambios",
  footerCanonicalSource: "Fuente canónica: ",
  footerReleases: " · Lanzamientos: ",
  footerReleasesLink: "Lanzamientos en GitHub",
  footerSecurity: "Seguridad",

  switcherLabel: "Idioma",
  switcherSwitchTo: "Cambiar a {label}",
  partialBadge: "(parcial)",
};
