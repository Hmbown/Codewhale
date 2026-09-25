import type { HomeDict } from "../types";

/**
 * Italian home dictionary — native copy for the Tidal Folio landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale — Crea e automatizza con i modelli che scegli",
  metaDescription:
    "Crea software, lavora con i tuoi file e automatizza le attività quotidiane con agenti open source e i modelli di IA ospitati o locali che scegli.",
  heroTitle: "Crea e automatizza con i modelli che scegli",
  heroIntro:
    "{brand} ti offre agenti che possono creare software, lavorare con i tuoi file e trasformare le attività ripetitive in flussi di lavoro riutilizzabili. Di’ loro cosa vuoi realizzare e scegli i modelli ospitati o locali adatti al lavoro, con la libertà di cambiare provider mentre procedi.",
  getCodewhale: "Ottieni Codewhale",
  heroInstallAria: "Comando di installazione",
  exploreProduct: "Esplora il prodotto",
  shotPreview: "Anteprima del terminale",
  shotBuild: "build di sviluppo v{version}",
  screenshotAlt:
    "Codewhale v{version}, build di sviluppo: balena, nuova sessione, campo messaggio, permessi Ask, modalità Work e stato del modello. Rendering dell’output reale di un terminale isolato.",
  latestRelease: "Ultima release {tag}",
  releaseUnavailable: "Stato delle release non disponibile",
  currentSource: "Sorgente",
  sourceCandidate: "Non rilasciata",
  publishedRelease: "rilasciata",
  figcaptionSourceCandidate: "non rilasciata",
  gainHeading: "Cosa puoi fare con Codewhale",
  gainLede:
    "Inizia con un progetto, una domanda o un’attività che vuoi automatizzare, poi lavora con un agente o assegna parti di un lavoro più grande a più agenti.",
  gain: [
    [
      "Crea qualcosa",
      "Descrivi cosa vuoi creare e lavora con agenti che possono leggere il tuo codice, modificare file, eseguire comandi e verificare il risultato."
    ],
    [
      "Automatizza il lavoro quotidiano",
      "Crea script e flussi di lavoro per le attività ricorrenti, così da poterli eseguire di nuovo dal terminale ogni volta che ne hai bisogno."
    ],
    [
      "Lavora con modelli diversi",
      "Usa modelli ospitati o locali per i tuoi agenti, con modelli e ruoli diversi che gestiscano le parti del lavoro a cui sono adatti."
    ]
  ],
  modelsHeading: "Una scelta di modelli per ogni attività",
  modelsBody:
    "Collegati direttamente a un provider di modelli ospitati, usa un gateway per accedere a più provider o esegui un modello in locale, poi scegli quale modello usa ogni sessione mentre lavori.",
  modelsFacts: [
    ["Hosted", "La tua chiave API, salvata con codewhale auth set --provider <id>"],
    ["Gateway", "Un endpoint per molti modelli, il provider lo scegli sempre tu"],
    ["Locale", "vLLM, SGLang, Ollama su localhost — di solito senza chiave"],
  ],
  modelsLink: "Esplora modelli e provider",
  startHeading: "Primi passi con Codewhale",
  startLede:
    "Dopo aver installato Codewhale e collegato un modello, puoi descrivere la tua prima attività nel terminale e aggiungere un Fleet quando vuoi distribuire il lavoro tra più agenti.",
  startGuideLink: "Leggi la guida introduttiva",
  startVocabularyLink: "Vedi il vocabolario del prodotto",
  availabilityHeading: "Dove puoi usare Codewhale",
  availabilityLede:
    "Puoi già usare Codewhale nel tuo terminale mentre sviluppiamo l’app web, l’app desktop e i computer cloud.",
  availability: [
    [
      "Terminale",
      "Rilasciato",
      "Binari delle versioni pubblicate su GitHub per Linux, macOS e Windows; npm e Cargo sono alternative. Android su Termux è disponibile in anteprima."
    ],
    [
      "App web",
      "Anteprima di sviluppo",
      "Accesso all’account e abbinamento con il browser nell’anteprima di sviluppo."
    ],
    [
      "Desktop",
      "Build di sviluppo",
      "L’app per macOS è in sviluppo; il download pubblico arriverà in seguito."
    ],
    [
      "Computer cloud",
      "In sviluppo",
      "Computer ospitati per eseguire le tue attività."
    ]
  ],
  availabilityNote:
    "Puoi usare il terminale senza un account Codewhale, e qualsiasi utilizzo di modelli ospitati viene fatturato dal tuo provider.",
  accountLink: "Crea un account",
  surfacesHeading: "Modi di lavorare con Codewhale",
  surfaces: [
    ["TUI", "Lavoro interattivo nel terminale"],
    ["codewhale exec", "Script e CI"],
    ["Client web locale","Interfaccia su localhost; ambiente di lavoro web ospitato in sviluppo"],
    ["Runtime API + MCP", "Integrazioni locali"],
    ["Fleet","Più agenti su un unico lavoro"],
  ],
  runtimeLink: "Esplora le integrazioni",
  installBandHeading: "Installa Codewhale su macOS o Linux",
  copy: "Copia",
  copied: "Copiato ✓",
  binaries: "Binari",
  chinaMirrors: "Mirror in Cina",
  installGuideLink: "Leggi la guida d'installazione",
  communityHeading: "Aiuta a migliorare Codewhale",
  communityBody:
    "Che tu abbia trovato un bug, abbia un’idea per una funzionalità o voglia inviare la tua prima pull request, ci piacerebbe ascoltarti e lavorare insieme ai prossimi sviluppi.",
  communityLinksAria: "Link della community",
  contribute: "Invia una pull request",
};
