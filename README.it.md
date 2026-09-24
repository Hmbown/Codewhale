<!-- source: README.md sha256:bbb8bfb61e57 -->
# Codewhale

Codewhale è un agente open source che legge il tuo progetto, modifica file, esegue comandi e verifica il proprio lavoro usando un modello ospitato o locale a tua scelta. Parti da un’attività nel terminale. Per un lavoro più grande, assegna parti del lavoro ad agenti con modelli e ruoli diversi.

![Codewhale in esecuzione in un terminale](web/public/codewhale-tui-d7a9a1c.png)

*Anteprima del terminale da una build di sviluppo della v0.10.0.*

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja-JP.md) · [Tiếng Việt](README.vi.md) · [Bahasa Indonesia](README.id.md) · [한국어](README.ko-KR.md) · [Español](README.es-419.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md) · [Українська](README.uk.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [繁體中文](README.zh-TW.md) · [हिन्दी](README.hi.md) · [Türkçe](README.tr.md) · [Polski](README.pl.md) · [العربية](README.ar.md) · [Català](README.ca.md)

[![CI](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml/badge.svg)](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/codewhale-cli?label=crates.io)](https://crates.io/crates/codewhale-cli)
[![npm](https://img.shields.io/npm/v/codewhale?label=npm)](https://www.npmjs.com/package/codewhale)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.gg/37gfS3ksug)

## Installazione

Per una nuova installazione su macOS o Linux, usa la versione ufficiale di GitHub:

```bash
curl -fsSL https://codewhale.net/install.sh | sh
"$HOME/.local/bin/codewhale"
```

L’installer seleziona l’ultima versione pubblicata. Il [registro delle modifiche](CHANGELOG.md) descrive anche la versione candidata, ancora non pubblicata, della prossima versione; queste modifiche saranno incluse nei download pubblicati solo quando la versione sarà disponibile.

Su Windows, scarica l’installer o l’archivio adatto da [GitHub Releases](https://github.com/Hmbown/CodeWhale/releases/latest). Per aggiornare un’installazione diretta esistente, esegui `codewhale update`, oppure `codewhale update --check` per la sola verifica. L’aggiornamento mostra il percorso dell’eseguibile e conserva le build più recenti. npm e Cargo sono opzioni secondarie; consulta la [guida all’installazione](docs/INSTALL.md) per migrare da un gestore di pacchetti e configurare PATH.

Il primo avvio apre direttamente il compositore; non ti guida attraverso una configurazione. Le risposte del modello richiedono un modello ospitato o locale collegato: finché non ce n’è uno, la schermata iniziale mostra "no model connected". Esegui `/provider` (o premi F3) per aggiungere una chiave ospitata o scegliere un runtime locale. Se Ollama è già in esecuzione con un modello di chat, Codewhale passa a quello da solo. Codewhale supporta anche npm e Cargo come opzioni secondarie di distribuzione, oltre a Docker, Nix, Scoop, Android/Termux e un mirror CNB facoltativo. Le installazioni esistenti gestite da un gestore di pacchetti ricevono istruzioni per la migrazione. Consulta la [guida all’installazione e a PATH](docs/INSTALL.md).

Il completamento con Tab si attiva con un solo comando per ogni shell — `codewhale completion bash|zsh|fish|powershell|elvish`. Consulta il [completamento della shell](docs/INSTALL.md#8-shell-completions).

## Utilizzo

Apri un terminale nella cartella del tuo progetto ed esegui `codewhale`. Scegli il provider con `/provider` e il modello con `/model`. Poi descrivi un’attività concreta:

```text
Fix the failing tests and explain what changed.
```

Oppure esegui un’attività senza aprire la TUI:

```bash
codewhale exec "fix the failing tests and explain what changed"
```

Codewhale può leggere il tuo repository, modificare file, eseguire comandi, controllare i risultati e continuare a lavorare verso un obiettivo. Usa `/mode plan` per esplorare senza modificare file né eseguire comandi shell, e `/mode work` quando vuoi che apporti modifiche. Premi `Shift+Tab` per scegliere Ask, Auto-Review o Full Access; la [guida a modalità e permessi](docs/MODES.md) spiega cosa consente ogni opzione.

## Terminale, app e Computer Use

Il terminale e i client grafici si collegano al Runtime di Codewhale, che esegue l’agente e i suoi strumenti:

- **Terminale:** `codewhale` apre l’interfaccia interattiva; `codewhale exec` esegue un’attività da uno script o da un job di CI.
- **Browser locale:** `codewhale web` apre il [client web locale](docs/WEB.md) incluso, che usa lo stesso runtime.
- **App desktop Codewhale (GPUI):** un'app desktop nativa, sviluppata in un repository separato, è la direzione del client di prodotto con accesso. L'app web ospitata su app.codewhale.net verrà ricostruita a sua immagine; il sito marketing, l'accesso, la fatturazione e le pagine legali e di download restano sul web. La disponibilità è indicata nella [pagina del prodotto](https://codewhale.net/en/product).

**Computer Use aggiunge strumenti per osservare altre applicazioni e interagire con esse.** Il plugin è incluso nel codice sorgente attuale. Controlla l’accesso richiesto e abilitalo prima dell’uso; i permessi del sistema operativo e i requisiti della piattaforma continuano ad applicarsi. Consulta la [guida a Computer Use](crates/tui/plugins/computer-use/README.md) inclusa e la [configurazione dei plugin](docs/PLUGINS.md).

Per VS Code, l’estensione CodeWhale mantenuta dalla comunità si collega al Runtime locale da una barra laterale. Installala dal [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode); il codice sorgente è su [GitHub](https://github.com/HengQuWorld/CodeWhale-VSCode).

## Perché Codewhale

- **Scegli i tuoi modelli.** Collega provider gestiti oppure modelli locali tramite Ollama, vLLM o SGLang. Usa `/provider` per cambiare provider e `/model` per scegliere un modello.
- **Mantieni il controllo.** Controlla le azioni proposte e le modifiche ai file che ne derivano. Le impostazioni di approvazione determinano quando è necessaria una revisione; Full Access continua a rispettare i limiti vincolanti delle regole. `/undo` e `/restore` aiutano a recuperare le modifiche dell’area di lavoro.
- **Mantieni organizzati i lavori lunghi.** Salva le sessioni, imposta un `/goal` duraturo, rivedi i workflow prima dell’esecuzione e coordina gli agenti senza trasformare le loro istruzioni interne in parte della tua conversazione.
- **Estendi l’agente che hai già.** Collega server MCP e skill, configura gli hook e conserva i ruoli degli agenti come file leggibili nel progetto o nelle impostazioni personali.

Esegui `/help` nella TUI per vedere i comandi e le scorciatoie da tastiera.

## Sicurezza

Codewhale viene eseguito sul tuo computer con l’accesso che gli concedi. Le modalità di approvazione e le regole del repository limitano ciò che l’agente può fare; il sandboxing facoltativo del sistema operativo aggiunge un confine di esecuzione più solido dove supportato. I prezzi sconosciuti dei modelli restano indicati come sconosciuti anziché essere segnalati come gratuiti.

Leggi l’[ordine di autorizzazione](docs/AUTHORIZATION_ORDER.md) per conoscere l’esatta gerarchia delle regole e la [configurazione](docs/CONFIGURATION.md) per le impostazioni locali.

## Documentazione

- [Provider e modelli locali](docs/PROVIDERS.md)
- [Team di agenti](docs/FLEET.md)
- [MCP](docs/MCP.md), [hook](docs/HOOKS.md) e [configurazione](docs/CONFIGURATION.md)
- [Client web locale](docs/WEB.md)
- [Tutta la documentazione](docs/README.md)
- [Struttura del repository e guida ai contributi](CONTRIBUTING.md#project-structure)

## Unisciti alla comunità

**Segnalazioni di bug, idee per nuove funzionalità e pull request sono benvenute**, sia che usi Codewhale da mesi sia che lo provi per la prima volta. Se manca un provider, un workflow risulta scomodo o l’interfaccia del terminale ti ostacola, [apri una issue](https://github.com/Hmbown/CodeWhale/issues/new/choose) oppure [invia una pull request](CONTRIBUTING.md) per migliorarlo insieme. I primi contributi sono benvenuti e chi contribuisce mantiene il riconoscimento per il lavoro integrato.

Unisciti a [Discord](https://discord.gg/37gfS3ksug), oppure aggiungi Hunter su WeChat (`hunterbown`) e chiedi di entrare nel gruppo Whale Brothers.

## Storia del progetto

Codewhale è nato come `deepseek-tui` e conserva ancora la compatibilità con la sua configurazione e le sue sessioni. Ora è indipendente dai provider, viene mantenuto in modo autonomo e non è affiliato ad alcun provider di modelli.

Grazie a ogni persona che ha contribuito e alle comunità open source che hanno aiutato il progetto a crescere. Consulta il [registro dei contributori](docs/CONTRIBUTORS.md).

## Licenza

[MIT](LICENSE). Le parti adattate da altri progetti open source sono indicate nelle [note sui componenti di terze parti](docs/THIRD_PARTY_NOTICES.md).
