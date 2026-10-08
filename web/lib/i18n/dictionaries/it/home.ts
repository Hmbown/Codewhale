import type { HomeDict } from "../types";

/** Italian home copy: useful work with chosen models and connected tools. */

export const home: HomeDict = {
  metaTitle: "Codewhale: crea con i tuoi modelli e strumenti",
  metaDescription:
    "Crea app, automatizza flussi di lavoro e usa strumenti connessi con Codewhale. Open source, con le tue API di modelli o inferenza locale e autogestita.",
  heroTitle: "Un agente open source per il tuo computer.",
  heroIntro:
    "Crea app, automatizza i flussi di lavoro e lavora con Slack, Gmail e altri strumenti connessi. {brand} usa le API di modelli che già utilizzi oppure l’inferenza locale o su infrastruttura gestita da te.",
  getCodewhale: "Installa Codewhale",
  heroInstallAria: "Comando di installazione",
  exploreProduct: "Esplora Codewhale",
  shotPreview: "Anteprima del terminale",
  screenshotAlt:
    "Codewhale v{version}: una cattura del terminale con conversazione, campo del messaggio e controlli della sessione.",
  latestRelease: "Ultima release {tag}",
  releaseUnavailable: "Stato delle release non disponibile",
  currentSource: "Sorgente",
  sourceCandidate: "Non rilasciata",
  publishedRelease: "rilasciata",
  gainHeading: "Cosa puoi fare",
  gainLede:
    "Parti dal risultato che vuoi. Codewhale lavora con file, comandi e strumenti connessi; accessi e approvazioni li decidi tu.",
  gain: [
    [
      "Crea app e strumenti",
      "Trasforma un’idea in un’app funzionante, uno script utile o una funzionalità per un progetto esistente. L’agente scrive, esegue e testa insieme a te."
    ],
    [
      "Automatizza il lavoro ripetitivo",
      "Trasforma un’attività ricorrente in un flusso per terminale, script o CI. Usa Fleet quando più agenti possono lavorare in parallelo."
    ],
    [
      "Collega gli strumenti che usi",
      "Collega strumenti come Gmail e Slack tramite plugin, server MCP o API. Usa questi servizi insieme ai tuoi file e comandi."
    ]
  ],
  chapterModels: "I tuoi modelli",
  modelsHeading: "Continua a usare i tuoi modelli.",
  modelsBody:
    "Collega le API di modelli che già paghi, usa un gateway compatibile o esegui l’inferenza sul tuo hardware. Scegli un modello per ogni sessione e per ciascun agente di un Fleet.",
  modelsFacts: [
    [
      "I tuoi account API",
      "Collega OpenAI, Anthropic, Google o DeepSeek con le tue chiavi."
    ],
    [
      "Il tuo gateway",
      "Usa un endpoint compatibile con OpenAI e scegli i modelli che offre."
    ],
    [
      "La tua inferenza",
      "Esegui modelli locali o autogestiti con Ollama, vLLM o SGLang."
    ]
  ],
  modelsLink: "Sfoglia modelli e provider",
  startHeading: "Porta un’attività. Comincia.",
  startLede:
    "Installa Codewhale, collega un modello e affidagli qualcosa di utile. Parti con un agente; aggiungi strumenti o un team quando serve.",
  startGuideLink: "Segui la guida introduttiva",
  startVocabularyLink: "Vedi il vocabolario del prodotto",
  chapterAvailability: "Dove funziona",
  availabilityHeading: "Comincia dal terminale.",
  availabilityLede:
    "Il terminale e il client browser locale sono disponibili. L’app desktop nativa e la nuova app web ospitata sono in sviluppo.",
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
    "Il terminale, il browser locale e la GUI non richiedono un account Codewhale. Il web ospitato e l’app desktop usano un account. Se usi la tua chiave del provider, è quel provider a fatturarti l’utilizzo.",
  accountLink: "Crea un account",
  surfacesHeading: "Un’attività. I tuoi file, app e agenti.",
  surfaces: [
    [
      "File e terminale",
      "Crea file, esegui comandi, analizza dati e testa ciò che realizzi. Tu imposti la cartella di lavoro e i permessi."
    ],
    [
      "Plugin e app connesse",
      "Aggiungi competenze e strumenti con plugin e MCP. Esamina e abilita le connessioni che vuoi far usare all’agente."
    ],
    [
      "Browser e computer · anteprima",
      "Usa gli strumenti browser e il plugin Computer Use per lavorare su app e siti con l’accesso che concedi."
    ],
    [
      "Sessioni da riprendere",
      "Conserva insieme conversazione, risultati degli strumenti e cronologia. Riprendi l’attività nel terminale o nel client browser locale."
    ],
    [
      "Team di agenti",
      "Dividi un lavoro più grande con Fleet tra agenti con ruoli e modelli diversi e seguine i progressi in un solo posto."
    ]
  ],
  runtimeLink: "Vedi tutte le integrazioni",
  installBandHeading: "Installa su macOS o Linux",
  copy: "Copia",
  copied: "Copiato ✓",
  binaries: "Binari",
  chinaMirrors: "Mirror in Cina",
  installGuideLink: "Leggi la guida d'installazione",
  communityHeading: "Fai tuo Codewhale.",
  communityBody:
    "Codewhale è open source. Leggi il codice, crea un plugin, condividi un flusso di lavoro o aiuta a migliorare la prossima versione.",
  communityLinksAria: "Link della community",
  contribute: "Contribuisci su GitHub",
};
