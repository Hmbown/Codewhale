import type { HomeDict } from "../types";

/** Catalan home copy: useful work with chosen models and connected tools. */

export const home: HomeDict = {
  metaTitle: "Codewhale: crea amb els teus models i eines",
  metaDescription:
    "Crea aplicacions, automatitza fluxos de treball i utilitza eines connectades amb Codewhale. Codi obert, amb les teves API de models o inferència local i autoallotjada.",
  heroTitle: "Crea aplicacions i automatitza la teva feina.",
  heroIntro:
    "{brand} és un agent de codi obert que escriu codi, executa ordres i treballa amb les eines que connectes. Fes servir les API de models que ja utilitzes o executa models localment i als teus propis servidors.",
  getCodewhale: "Instal·lar Codewhale",
  heroInstallAria: "Ordre d'instal·lació",
  exploreProduct: "Explora Codewhale",
  shotPreview: "Vista prèvia del terminal",
  screenshotAlt:
    "Codewhale v{version}: captura del terminal amb la conversa, el camp de missatge i els controls de sessió.",
  latestRelease: "Última versió {tag}",
  releaseUnavailable: "Estat de la versió no disponible",
  currentSource: "Font",
  sourceCandidate: "Sense publicar",
  publishedRelease: "publicada",
  gainHeading: "Què pots fer",
  gainLede:
    "Descriu què vols crear o automatitzar. Codewhale pot editar fitxers, executar ordres i comprovar el resultat, amb un accés que tu controles.",
  gain: [
    [
      "Crea aplicacions i eines",
      "Crea una aplicació, afegeix una funció o escriu un script. Codewhale pot treballar amb els fitxers del projecte, executar el codi i provar el que crea."
    ],
    [
      "Automatitza la feina repetitiva",
      "Executa fluxos de treball des del terminal, scripts o CI. Per a tasques més grans, delega parts de la feina a una Fleet d’agents amb models diferents."
    ],
    [
      "Connecta les eines que fas servir",
      "Afegeix eines mitjançant plugins i servidors MCP, o utilitza API des dels teus propis scripts. Cada servei requereix la seva pròpia configuració i autenticació."
    ]
  ],
  chapterModels: "Els teus models",
  modelsHeading: "Fes servir els models que triïs",
  modelsBody:
    "Connecta els teus comptes de proveïdors, un endpoint compatible amb OpenAI o models locals i autoallotjats. Tria un model per a la sessió i per a cada agent d’una Fleet.",
  modelsFacts: [
    [
      "Els teus comptes d’API",
      "Connecta OpenAI, Anthropic, Google o DeepSeek amb les teves pròpies claus."
    ],
    [
      "La teva passarel·la",
      "Utilitza un endpoint compatible amb OpenAI i tria els models que ofereix."
    ],
    [
      "La teva inferència",
      "Executa models locals o autoallotjats amb Ollama, vLLM o SGLang."
    ]
  ],
  modelsLink: "Consulta els models i els proveïdors",
  startHeading: "Primers passos",
  startLede:
    "Instal·la Codewhale, connecta un model i obre una carpeta de projecte. Pots afegir plugins i més agents quan els necessitis.",
  startGuideLink: "Segueix la guia d’inici",
  startVocabularyLink: "Consulta el vocabulari del producte",
  chapterAvailability: "On funciona",
  availabilityHeading: "Disponible ara i en desenvolupament",
  availabilityLede:
    "El terminal i el client de navegador local ja estan disponibles. L’aplicació nativa d’escriptori i la nova aplicació web allotjada estan en desenvolupament.",
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
    "El terminal, el navegador local i la GUI no necessiten cap compte de Codewhale. La web allotjada i l’aplicació d’escriptori fan servir un compte. Si fas servir la teva pròpia clau de proveïdor, aquest et factura l’ús.",
  accountLink: "Crear un compte",
  surfacesHeading: "Treballa amb fitxers i eines",
  surfaces: [
    [
      "Fitxers i terminal",
      "Crea fitxers, executa ordres, examina dades i prova allò que construeixes. Tu defineixes la carpeta de treball i els permisos."
    ],
    [
      "Plugins i aplicacions connectades",
      "Afegeix habilitats i eines amb plugins i MCP. Revisa i activa les connexions que vols que utilitzi l’agent."
    ],
    [
      "Navegador i ordinador · vista prèvia",
      "Utilitza les eines de navegador i el plugin Computer Use en aplicacions i webs amb l’accés que concedeixis."
    ],
    [
      "Sessions que pots reprendre",
      "Conserva junts la conversa, els resultats de les eines i l’historial. Reprèn la tasca al terminal o al client de navegador local."
    ],
    [
      "Equips d’agents",
      "Reparteix una feina gran entre agents amb rols i models diferents mitjançant Fleet i segueix-ne el progrés en un sol lloc."
    ]
  ],
  runtimeLink: "Explora eines i integracions",
  installBandHeading: "Instal·la a macOS o Linux",
  copy: "Copia",
  copied: "Copiat ✓",
  binaries: "Binaris",
  chinaMirrors: "Mirrors a la Xina",
  installGuideLink: "Llegeix la guia d’instal·lació",
  communityHeading: "Contribueix a Codewhale",
  communityBody:
    "Informa d’un error, millora la documentació o contribueix amb codi a GitHub. També pots crear plugins i compartir fluxos de treball amb altres usuaris.",
  communityLinksAria: "Enllaços de la comunitat",
  contribute: "Contribueix a GitHub",
};
