import type { ComputerUseDict } from "../types";

/**
 * Catalan dictionary for `app/[locale]/computer-use/page.tsx` and the
 * Computer Use section of the install page. Product names, the menu items
 * Pause, Stop and Check for updates, and the macOS setting names stay as
 * the app shows them; the macOS settings and the Applications folder use
 * the names the Catalan system UI shows.
 */
export const computerUse: ComputerUseDict = {
  metaTitle: "Computer Use per a Mac · Codewhale",
  metaDescription: "Baixa i configura Codewhale Computer Use per a Mac. Control d’aplicacions en segon pla, configuració de permisos i controls humans de Pause i Stop.",
  title: "Computer Use",
  lead: "Deixa que Codewhale treballi a les teves aplicacions mentre tu continues treballant. L’ajudant per a Mac posa els permisos, el control d’aplicacions en segon pla i una manera de pausar o aturar l’entrada a la barra de menús.",
  publisher: "De Codewhale",
  download: "Baixa per a Mac",
  downloadZip: "Arxiu ZIP (el fa servir l’actualitzador integrat)",
  requirements: "macOS 13.5 o posterior · Apple silicon i Intel",
  included: "Una sola baixada. No cal instal·lar Node ni cap compilador a part.",
  pendingTitle: "Baixada per a Mac en preparació",
  pendingBody: "L’instal·lador públic apareixerà aquí quan s’hagin completat la notarització d’Apple i les comprovacions de publicació.",
  unavailableTitle: "No s’ha pogut comprovar la disponibilitat de la baixada",
  unavailableBody: "Actualitza la pàgina per tornar-ho a provar o consulta les versions publicades a sota.",
  releases: "Versions publicades",
  receipt: "Detalls de verificació de la baixada",
  setup: "Configura el teu Mac",
  steps: [
    { title: "Instal·la l’aplicació", body: "Obre la imatge de disc i arrossega Codewhale Computer Use a «Aplicacions». Obre-la des d’«Aplicacions» i tria Computer Use a la icona de la balena de la barra de menús." },
    { title: "Revisa els permisos", body: "Fes servir els botons de configuració per obrir «Accessibilitat» i «Gravació de pantalla» a Configuració del Sistema. Tu decideixes quins permisos concedeixes." },
    { title: "Executa la comprovació en segon pla", body: "L’ajudant obre una finestra de pràctica d’un sol ús, hi escriu text i la captura. Comprova si el punter o l’aplicació activa han canviat durant l’execució." },
    { title: "Connecta’l amb Codewhale", body: "Revisa, marca com a fiable i activa Computer Use al mercat de connectors de Codewhale. Fes servir el connector 0.3.1 o posterior perquè les accions locals passin pels controls Pause i Stop de l’ajudant." },
  ],
  controlsTitle: "Continua treballant. Mantén el control.",
  controlsBody: "Les accions compatibles operen en segon pla sobre l’aplicació seleccionada. Les aplicacions i els gestos que necessiten el control en primer pla requereixen la teva autorització. El menú mostra l’aplicació de destinació i el mode d’entrada; Pause (pausa) suspèn l’entrada de l’ajudant i Stop (atura) tanca les seves sessions actives.",
  updateTitle: "Actualitzacions quan tu vulguis",
  updateBody: "Tria Check for updates (Cerca actualitzacions) a l’aplicació. Abans d’instal·lar una actualització, comprova la baixada, la signatura de Codewhale i la notarització d’Apple, i conserva l’aplicació anterior per si cal recuperar-la.",
  help: "Configuració i resolució de problemes",
  notes: "Notes de la versió",
  demo: "Mira la comprovació en segon pla",
  source: "Codi font i altres plataformes",
  platforms: "Aquesta baixada és per a Mac. Windows i Linux fan servir de moment el connector des del codi font i la configuració a l’equip amfitrió.",
};
