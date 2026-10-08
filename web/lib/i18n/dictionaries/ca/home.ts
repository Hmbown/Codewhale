import type { HomeDict } from "../types";

/** Catalan home copy: useful work with chosen models and connected tools. */

export const home: HomeDict = {
  metaTitle: "Codewhale: crea amb els teus models i eines",
  metaDescription:
    "Crea aplicacions, automatitza fluxos de treball i utilitza eines connectades amb Codewhale. Codi obert, amb les teves API de models o inferència local i autoallotjada.",
  heroTitle: "Crea allò que tens al cap.",
  heroIntro:
    "Crea una aplicació, automatitza un flux de treball o transforma una recerca en alguna cosa útil. {brand} treballa amb les API de models que ja fas servir, la teva pròpia inferència i les eines que connectis.",
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
  gainHeading: "Crea alguna cosa útil.",
  gainLede:
    "Comença pel resultat que vols. Codewhale treballa amb fitxers, ordres i eines connectades; tu decideixes els accessos i les aprovacions.",
  gain: [
    [
      "Crea aplicacions i eines",
      "Converteix una idea en una aplicació que funcioni, un script útil o una funció d’un projecte existent. Deixa que l’agent escrigui, executi i provi amb tu."
    ],
    [
      "Automatitza la feina repetitiva",
      "Converteix una tasca recurrent en un flux de treball per al terminal, els scripts o CI. Fes servir un Fleet d’agents quan la feina es pugui fer en paral·lel."
    ],
    [
      "Connecta les eines que fas servir",
      "Connecta eines com Gmail i Slack mitjançant plugins, servidors MCP o API. Treballa amb aquests serveis al costat dels teus fitxers i ordres."
    ]
  ],
  exampleTasks: [
    "Crea una aplicació per reservar cites.",
    "Converteix un CSV de vendes en un informe setmanal que pugui tornar a generar.",
    "Converteix els missatges del correu connectat en una llista de tasques.",
  ],
  // A static example report built from local sample orders.
  reportTitle: "Informe de vendes setmanal",
  reportSampleLabel: "Informe d’exemple · dades de mostra",
  reportDescription: "Demana a Codewhale que agrupi les comandes per setmana i desi el procés per al pròxim CSV.",
  reportSourceLabel: "Dades d’entrada:",
  reportColumns: ["Inici de setmana","Comandes","Vendes (USD)"],
  reportTotalLabel: "Total",
  reportTrend: "Canvi en les vendes, de la primera setmana a l’última: {change}.",
  reportDownloadLabel: "Descarrega l’informe en CSV",
  chapterModels: "Els teus models",
  modelsHeading: "Continua fent servir els teus models.",
  modelsBody:
    "Connecta les API de models que ja pagues, utilitza una passarel·la compatible o executa inferència al teu maquinari. Tria un model per sessió i per cada agent d’un Fleet.",
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
  startHeading: "Porta una tasca. Comença.",
  startLede:
    "Instal·la Codewhale, connecta un model i dona-li una tasca que valgui la pena. Comença amb un agent; afegeix eines o un equip quan calgui.",
  startGuideLink: "Segueix la guia d’inici",
  startVocabularyLink: "Consulta el vocabulari del producte",
  chapterAvailability: "On funciona",
  availabilityHeading: "Comença al terminal.",
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
  surfacesHeading: "Una tasca. Els teus fitxers, aplicacions i agents.",
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
  runtimeLink: "Consulta totes les integracions",
  installBandHeading: "Instal·la a macOS o Linux",
  copy: "Copia",
  copied: "Copiat ✓",
  binaries: "Binaris",
  chinaMirrors: "Mirrors a la Xina",
  installGuideLink: "Llegeix la guia d’instal·lació",
  communityHeading: "Fes teu Codewhale.",
  communityBody:
    "Codewhale és de codi obert. Llegeix el codi, crea un plugin, comparteix un flux de treball o ajuda a millorar la pròxima versió.",
  communityLinksAria: "Enllaços de la comunitat",
  contribute: "Contribueix a GitHub",
};
