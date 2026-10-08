import type { HomeDict } from "../types";

/**
 * Italian home dictionary — native copy for the whale-road landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale: l’agente di programmazione open source per qualsiasi modello",
  metaDescription:
    "Codewhale è un agente di programmazione open source per il terminale. Legge il tuo progetto, modifica i file ed esegue i tuoi test con il modello ospitato o locale che scegli.",
  heroTitle: "L’agente di programmazione open source per qualsiasi modello",
  heroIntro:
    "{brand} legge il tuo progetto, modifica i file ed esegue i tuoi test dal terminale. Collega un modello ospitato o locale e scegli quali azioni richiedono la tua approvazione.",
  getCodewhale: "Installa Codewhale",
  heroInstallAria: "Comando di installazione",
  exploreProduct: "Scopri come funziona",
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
  chapterTerminal: "Il tuo terminale",
  chapterTerminalTitle: "Segui ogni modifica e ogni comando mentre viene eseguito",
  gainHeading: "Delega l’attività e mantieni il controllo",
  gainLede:
    "Chiedi un risultato: correggere un bug, spiegare un modulo o automatizzare un’attività che ripeti. Inizia con un agente e aggiungine altri quando il lavoro cresce.",
  gain: [
    [
      "Modifica il codice e verificalo",
      "L’agente esamina il tuo progetto, modifica i file ed esegue i tuoi test. Segui ogni modifica e ogni risultato dei comandi mentre lavora."
    ],
    [
      "Automatizza il lavoro ripetitivo",
      "Esegui codewhale exec da script e CI. Usa un Fleet per dividere un lavoro più grande tra più agenti."
    ],
    [
      "Mantieni il controllo",
      "Imposta i permessi prima che il lavoro inizi, rispondi alle richieste di approvazione e interrompi un’attività in qualsiasi momento. Esegui /receipts per elencare ogni file, comando e approvazione di una sessione."
    ]
  ],
  chapterModels: "I tuoi modelli",
  modelsHeading: "Scegli un modello per ogni attività",
  modelsBody:
    "Per ogni sessione scegli un provider integrato, qualsiasi endpoint compatibile con OpenAI o un modello locale. La connessione al modello resta separata da qualsiasi account Codewhale.",
  modelsFacts: [
    ["Hosted", "La tua chiave API, salvata con codewhale auth set --provider <id>"],
    ["Gateway", "Un endpoint per molti modelli; il provider lo scegli sempre tu"],
    ["Locale", "vLLM, SGLang o Ollama su localhost, di solito senza chiave"],
  ],
  modelsLink: "Sfoglia modelli e provider",
  startHeading: "Installa, collega un modello ed esegui un’attività",
  startLede:
    "Esegui la tua prima attività in tre passaggi dalla cartella del progetto. Aggiungi un Fleet in seguito se il lavoro richiede più agenti.",
  startGuideLink: "Segui la guida introduttiva",
  startVocabularyLink: "Vedi il vocabolario del prodotto",
  chapterAvailability: "Dove funziona",
  availabilityHeading: "Usa Codewhale nel terminale già da oggi",
  availabilityLede:
    "Puoi già usare il terminale, il client browser locale o l’interfaccia CodeWhale GUI mantenuta dalla comunità. L’app desktop e l’app web ospitata ricostruita sono in sviluppo e condividono lo stesso modello di sessione.",
  availability: [
    [
      "Terminale e browser locale",
      "Rilasciato",
      "Installa su Linux, macOS o Windows, poi esegui codewhale, oppure codewhale web per il client browser locale. Funzionano anche npm e Cargo; Android su Termux è in anteprima."
    ],
    [
      "CodeWhale GUI (VS Code)",
      "Disponibile",
      "Un progetto separato mantenuto dalla comunità: chat, thread e modifiche ai file in una barra laterale di VS Code sullo stesso Codewhale Runtime. Installala dal VS Code Marketplace.",
      "https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode"
    ],
    [
      "App web ospitata",
      "Anteprima di sviluppo",
      "In fase di ricostruzione per corrispondere all’app desktop. Oggi puoi accedere e poi digitare /rc in una sessione di terminale in esecuzione per continuarla sul web; l’esecuzione di attività ospitate è ancora in fase di qualificazione."
    ],
    [
      "Desktop",
      "Build di sviluppo",
      "L’app nativa che sta diventando il client principale di Codewhale: cartelle, conversazioni e connessioni ai modelli in un’unica finestra. Non è ancora disponibile un download pubblico."
    ],
    [
      "Computer cloud",
      "In sviluppo",
      "Computer ospitati che eseguono le tue attività."
    ]
  ],
  availabilityNote:
    "Il terminale, il browser locale e la GUI non richiedono un account Codewhale. Il web ospitato e il desktop usano un account, che non sostituisce la connessione al modello; il tuo provider fattura l’utilizzo della tua chiave.",
  accountLink: "Crea un account",
  surfacesHeading: "Estendi ciò a cui l’agente può accedere",
  surfaces: [
    ["File e comandi", "Leggi il progetto, modifica i file, esegui i test e controlla l’output entro i permessi che imposti."],
    ["Plugin e MCP", "Collega altri strumenti e servizi. Ogni plugin resta disattivato finché non lo esamini e lo attivi."],
    ["Computer Use · anteprima", "Un plugin che permette all’agente di vedere e usare altre app. Sei tu ad attivarlo e a concedere i permessi di sistema che richiede."],
    ["Sessioni salvate", "Mantieni insieme la conversazione e i risultati degli strumenti e riprendi il lavoro invece di ricominciare. Il browser locale apre la stessa sessione sul tuo computer."],
    ["Fleet", "Assegna parti di un’attività ad agenti con modelli e ruoli diversi, poi segui i loro progressi."],
  ],
  runtimeLink: "Vedi tutte le integrazioni",
  installBandHeading: "Installa su macOS o Linux",
  copy: "Copia",
  copied: "Copiato ✓",
  binaries: "Binari",
  chinaMirrors: "Mirror in Cina",
  installGuideLink: "Leggi la guida d'installazione",
  communityHeading: "Costruisci Codewhale con noi",
  communityBody:
    "Segnala un bug, proponi una funzionalità o invia la tua prima pull request su GitHub. Le correzioni piccole e testate sono benvenute.",
  communityLinksAria: "Link della community",
  contribute: "Invia una pull request",
};
