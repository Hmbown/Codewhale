import type { HomeDict } from "../types";

/** Italian home copy: useful work with chosen models and connected tools. */

export const home: HomeDict = {
  metaTitle: "Codewhale: crea con i tuoi modelli e strumenti",
  metaDescription:
    "Crea app, automatizza flussi di lavoro e usa strumenti connessi con Codewhale. Open source, con le tue API di modelli o inferenza locale e autogestita.",
  heroTitle: "Crea app e automatizza il tuo lavoro.",
  heroIntro:
    "{brand} è un agente open source che scrive codice, esegue comandi e lavora con gli strumenti che colleghi. Usa le API di modelli che già utilizzi oppure esegui modelli in locale e sui tuoi server.",
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
    "Descrivi cosa vuoi creare o automatizzare. Codewhale può modificare file, eseguire comandi e verificare il risultato, con un accesso che controlli tu.",
  gain: [
    [
      "Crea app e strumenti",
      "Crea un’app, aggiungi una funzionalità o scrivi uno script. Codewhale può lavorare sui file del progetto, eseguire il codice e testare ciò che crea."
    ],
    [
      "Automatizza il lavoro ripetitivo",
      "Esegui flussi di lavoro dal terminale, da script o dalla CI. Per le attività più grandi, delega parte del lavoro a una Fleet di agenti con modelli diversi."
    ],
    [
      "Collega gli strumenti che usi",
      "Aggiungi strumenti tramite plugin e server MCP, oppure usa le API dai tuoi script. Ogni servizio richiede una configurazione e un’autenticazione proprie."
    ]
  ],
  chapterModels: "I tuoi modelli",
  modelsHeading: "Usa i modelli che preferisci",
  modelsBody:
    "Collega i tuoi account dei provider, un endpoint compatibile con OpenAI oppure modelli locali e su infrastruttura gestita da te. Scegli un modello per la sessione e per ogni agente di una Fleet.",
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
  startHeading: "Per iniziare",
  startLede:
    "Installa Codewhale, collega un modello e apri una cartella di progetto. Puoi aggiungere plugin e altri agenti quando ti servono.",
  startGuideLink: "Segui la guida introduttiva",
  startVocabularyLink: "Vedi il vocabolario del prodotto",
  chapterAvailability: "Dove funziona",
  availabilityHeading: "Disponibile ora e in sviluppo",
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
  surfacesHeading: "Lavora con file e strumenti",
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
  runtimeLink: "Esplora strumenti e integrazioni",
  installBandHeading: "Installa su macOS o Linux",
  copy: "Copia",
  copied: "Copiato ✓",
  binaries: "Binari",
  chinaMirrors: "Mirror in Cina",
  installGuideLink: "Leggi la guida d'installazione",
  communityHeading: "Contribuisci a Codewhale",
  communityBody:
    "Segnala un bug, migliora la documentazione o contribuisci con codice su GitHub. Puoi anche creare plugin e condividere flussi di lavoro con altri utenti.",
  communityLinksAria: "Link della community",
  contribute: "Contribuisci su GitHub",
};
