import type { HomeDict } from "../types";

/**
 * Catalan home dictionary — native copy for the whale-road landing page,
 * translated from the English reference: an open-source coding agent for
 * any model, control over approvals, and availability stated per surface
 * as it is today. Product vocabulary stays literal (Plan / Work / Operate,
 * Ask / Auto-Review / Full Access, Codewhale, codewhale exec, Fleet,
 * /receipts).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale: l’agent de programació de codi obert per a qualsevol model",
  metaDescription:
    "Codewhale és un agent de programació de codi obert per al teu terminal. Llegeix el teu projecte, edita fitxers i executa les teves proves amb el model allotjat o local que triïs.",
  heroTitle: "L’agent de programació de codi obert per a qualsevol model",
  heroIntro:
    "{brand} llegeix el teu projecte, edita fitxers i executa les teves proves des del terminal. Connecta un model allotjat o local i tria quines accions necessiten la teva aprovació.",
  getCodewhale: "Instal·lar Codewhale",
  heroInstallAria: "Ordre d'instal·lació",
  exploreProduct: "Veure com funciona",
  shotPreview: "Vista prèvia del terminal",
  shotBuild: "build de desenvolupament v{version}",
  screenshotAlt:
    "Codewhale v{version}, versió de desenvolupament: balena, sessió nova, camp de missatge, permisos Ask, mode Work i estat del model. Representació de la sortida real d’un terminal aïllat.",
  latestRelease: "Última versió {tag}",
  releaseUnavailable: "Estat de la versió no disponible",
  currentSource: "Font",
  sourceCandidate: "Sense publicar",
  publishedRelease: "publicada",
  figcaptionSourceCandidate: "sense publicar",
  chapterTerminal: "El teu terminal",
  chapterTerminalTitle: "Segueix cada edició i cada ordre mentre s’executen",
  gainHeading: "Delega la tasca i mantén el control",
  gainLede:
    "Demana un resultat: corregir un error, explicar un mòdul o automatitzar una tasca que repeteixes. Comença amb un agent i afegeix-ne més quan la feina creixi.",
  gain: [
    [
      "Canvia el codi i comprova’l",
      "L’agent inspecciona el teu projecte, edita fitxers i executa les teves proves. Segueix cada edició i cada resultat d’ordre mentre treballa."
    ],
    [
      "Automatitza la feina repetitiva",
      "Executa codewhale exec des de scripts i CI. Fes servir un Fleet per dividir una feina més gran entre diversos agents."
    ],
    [
      "Mantén el control",
      "Defineix els permisos abans de començar, respon a les sol·licituds d’aprovació i atura una tasca en qualsevol moment. Executa /receipts per llistar cada fitxer, ordre i aprovació d’una sessió."
    ]
  ],
  chapterModels: "Els teus models",
  modelsHeading: "Tria un model per a cada tasca",
  modelsBody:
    "Per a cada sessió, tria un proveïdor integrat, qualsevol endpoint compatible amb OpenAI o un model local. La connexió amb el teu model es manté separada de qualsevol compte de Codewhale.",
  modelsFacts: [
    ["Allotjat", "La teva pròpia clau d’API, desada amb codewhale auth set --provider <id>"],
    ["Gateway", "Un endpoint per a molts models; el proveïdor el continues triant tu"],
    ["Local", "vLLM, SGLang o Ollama a localhost, normalment sense clau"],
  ],
  modelsLink: "Consulta els models i els proveïdors",
  startHeading: "Instal·la, connecta un model i executa una tasca",
  startLede:
    "Executa la teva primera tasca en tres passos des de la carpeta del projecte. Afegeix un Fleet més endavant si la feina necessita diversos agents.",
  startGuideLink: "Segueix la guia d’inici",
  startVocabularyLink: "Consulta el vocabulari del producte",
  chapterAvailability: "On funciona",
  availabilityHeading: "Fes servir Codewhale al terminal des d’avui",
  availabilityLede:
    "Ja pots fer servir el terminal, el client de navegador local o la interfície CodeWhale GUI de la comunitat. L’aplicació d’escriptori i l’aplicació web allotjada reconstruïda estan en desenvolupament i comparteixen el mateix model de sessió.",
  availability: [
    [
      "Terminal i navegador local",
      "Publicat",
      "Instal·la’l a Linux, macOS o Windows i després executa codewhale, o codewhale web per al client de navegador local. npm i Cargo també funcionen; Android amb Termux és una vista prèvia."
    ],
    [
      "CodeWhale GUI (VS Code)",
      "Disponible",
      "Un projecte independent mantingut per la comunitat: xat, fils i canvis de fitxers en una barra lateral del VS Code sobre el mateix Codewhale Runtime. Instal·la-la des del VS Code Marketplace.",
      "https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode"
    ],
    [
      "Aplicació web allotjada",
      "Vista prèvia de desenvolupament",
      "S’està reconstruint per igualar l’aplicació d’escriptori. Avui pots iniciar sessió i després escriure /rc en una sessió de terminal en execució per continuar-la al web; l’execució de tasques allotjades encara s’està validant."
    ],
    [
      "Escriptori",
      "Build de desenvolupament",
      "L’aplicació nativa que s’està convertint en el client principal de Codewhale: carpetes, converses i connexions de models en una sola finestra. Encara no hi ha cap descàrrega pública."
    ],
    [
      "Ordinadors al núvol",
      "En desenvolupament",
      "Ordinadors allotjats que executen les teves tasques."
    ]
  ],
  availabilityNote:
    "El terminal, el navegador local i la GUI no necessiten cap compte de Codewhale. La web allotjada i l’escriptori fan servir un compte, que no substitueix la connexió amb el teu model; el teu proveïdor factura l’ús amb la teva pròpia clau.",
  accountLink: "Crear un compte",
  surfacesHeading: "Amplia allò a què pot accedir l’agent",
  surfaces: [
    ["Fitxers i ordres", "Llegeix el projecte, edita fitxers, executa proves i revisa la sortida dins dels permisos que defineixis."],
    ["Plugins i MCP", "Connecta més eines i serveis. Cada plugin roman desactivat fins que el revisis i l’activis."],
    ["Computer Use · vista prèvia", "Un plugin que permet a l’agent veure i fer servir altres aplicacions. Tu l’actives i concedeixes els permisos del sistema que demana."],
    ["Sessions desades", "Mantén junts la conversa i els resultats de les eines, i reprèn la feina en lloc de començar de nou. El navegador local obre la mateixa sessió al teu ordinador."],
    ["Fleet", "Assigna parts d’una tasca a agents amb models i rols diferents, i després segueix-ne el progrés."],
  ],
  runtimeLink: "Consulta totes les integracions",
  installBandHeading: "Instal·la a macOS o Linux",
  copy: "Copia",
  copied: "Copiat ✓",
  binaries: "Binaris",
  chinaMirrors: "Mirrors a la Xina",
  installGuideLink: "Llegeix la guia d’instal·lació",
  communityHeading: "Construeix Codewhale amb nosaltres",
  communityBody:
    "Informa d’un error, proposa una funció o envia el teu primer pull request a GitHub. Les correccions petites i provades són benvingudes.",
  communityLinksAria: "Enllaços de la comunitat",
  contribute: "Enviar un pull request",
};
