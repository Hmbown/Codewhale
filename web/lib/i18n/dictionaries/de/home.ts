import type { HomeDict } from "../types";

/**
 * German home dictionary — native copy for the whale-road landing page,
 * translated from the English reference: an open-source coding agent for
 * any model, control over approvals, and availability stated per surface
 * as it is today. Product vocabulary stays literal (Plan / Work / Operate,
 * Ask / Auto-Review / Full Access, Codewhale, codewhale exec, Fleet,
 * /receipts).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale: der Open-Source-Coding-Agent für jedes Modell",
  metaDescription:
    "Codewhale ist ein Open-Source-Coding-Agent für dein Terminal. Er liest dein Projekt, bearbeitet Dateien und führt deine Tests mit dem gehosteten oder lokalen Modell deiner Wahl aus.",
  heroTitle: "Der Open-Source-Coding-Agent für jedes Modell",
  heroIntro:
    "{brand} liest dein Projekt, bearbeitet Dateien und führt deine Tests in deinem Terminal aus. Verbinde ein gehostetes oder lokales Modell und lege fest, welche Aktionen deine Freigabe brauchen.",
  getCodewhale: "Codewhale installieren",
  heroInstallAria: "Installationsbefehl",
  exploreProduct: "So funktioniert es",
  shotPreview: "Terminal-Vorschau",
  shotBuild: "Entwicklungsbuild v{version}",
  screenshotAlt:
    "Codewhale v{version}, Entwicklungsbuild: Wal, neue Sitzung, Nachrichteneingabe, Ask-Berechtigungen, Work-Modus und Modellstatus. Darstellung der tatsächlichen Ausgabe eines isolierten Terminals.",
  latestRelease: "Aktuellstes Release {tag}",
  releaseUnavailable: "Release-Status nicht verfügbar",
  currentSource: "Quelle",
  sourceCandidate: "Unveröffentlicht",
  publishedRelease: "veröffentlicht",
  figcaptionSourceCandidate: "unveröffentlicht",
  chapterTerminal: "Dein Terminal",
  chapterTerminalTitle: "Verfolge jede Änderung und jeden Befehl während der Ausführung",
  gainHeading: "Gib die Aufgabe ab und behalte die Kontrolle",
  gainLede:
    "Bitte um ein Ergebnis: einen Fehler beheben, ein Modul erklären oder eine wiederkehrende Aufgabe automatisieren. Beginne mit einem Agenten und füge weitere hinzu, wenn die Aufgabe wächst.",
  gain: [
    [
      "Ändere Code und prüfe ihn",
      "Der Agent untersucht dein Projekt, bearbeitet Dateien und führt deine Tests aus. Verfolge jede Änderung und jedes Befehlsergebnis, während er arbeitet."
    ],
    [
      "Automatisiere wiederkehrende Arbeit",
      "Führe codewhale exec aus Skripten und CI aus. Nutze Fleet, um eine größere Aufgabe auf mehrere Agenten aufzuteilen."
    ],
    [
      "Behalte die Kontrolle",
      "Lege Berechtigungen fest, bevor die Arbeit beginnt, beantworte Freigabeanfragen und stoppe eine Aufgabe jederzeit. Führe /receipts aus, um jede Datei, jeden Befehl und jede Freigabe einer Sitzung aufzulisten."
    ]
  ],
  chapterModels: "Deine Modelle",
  modelsHeading: "Wähle ein Modell für jede Aufgabe",
  modelsBody:
    "Wähle für jede Sitzung einen integrierten Anbieter, einen beliebigen OpenAI-kompatiblen Endpoint oder ein lokales Modell. Deine Modellverbindung bleibt von jedem Codewhale-Konto getrennt.",
  modelsFacts: [
    ["Gehostet", "Dein eigener API-Schlüssel, gespeichert mit codewhale auth set --provider <id>"],
    ["Gateway", "Ein Endpoint für viele Modelle; den Anbieter wählst weiterhin du"],
    ["Lokal", "vLLM, SGLang oder Ollama auf localhost, meist ohne Schlüssel"],
  ],
  modelsLink: "Modelle und Anbieter durchsuchen",
  startHeading: "Installieren, Modell verbinden, Aufgabe ausführen",
  startLede:
    "Führe deine erste Aufgabe in drei Schritten in deinem Projektordner aus. Füge später Fleet hinzu, wenn die Arbeit mehrere Agenten braucht.",
  startGuideLink: "Dem Leitfaden für die ersten Schritte folgen",
  startVocabularyLink: "Produktvokabular ansehen",
  chapterAvailability: "Wo es läuft",
  availabilityHeading: "Nutze Codewhale schon heute in deinem Terminal",
  availabilityLede:
    "Nutze jetzt das Terminal, den lokalen Browser-Client oder die von der Community gepflegte CodeWhale GUI. Die Desktop-App und die neu aufgebaute gehostete Web-App sind in Entwicklung und teilen dasselbe Sitzungsmodell.",
  availability: [
    [
      "Terminal und lokaler Browser",
      "Veröffentlicht",
      "Installiere Codewhale unter Linux, macOS oder Windows und führe dann codewhale aus, oder codewhale web für den lokalen Browser-Client. npm und Cargo funktionieren ebenfalls; Android unter Termux ist eine Vorschau."
    ],
    [
      "CodeWhale GUI (VS Code)",
      "Verfügbar",
      "Ein separates, von der Community gepflegtes Projekt: Chat, Threads und Dateiänderungen in einer VS Code-Seitenleiste über dieselbe Codewhale Runtime. Installiere sie über den VS Code Marketplace.",
      "https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode"
    ],
    [
      "Gehostete Web-App",
      "Entwicklungsvorschau",
      "Wird neu aufgebaut, damit sie zur Desktop-App passt. Heute kannst du dich anmelden und dann in einer laufenden Terminal-Sitzung /rc eingeben, um sie im Web fortzusetzen; die Ausführung gehosteter Aufgaben wird noch qualifiziert."
    ],
    [
      "Desktop-App",
      "Entwicklungsbuild",
      "Die native App, die zum wichtigsten Codewhale-Client wird: Ordner, Unterhaltungen und Modellverbindungen in einem Fenster. Einen öffentlichen Download gibt es noch nicht."
    ],
    [
      "Cloud-Computer",
      "In Entwicklung",
      "Gehostete Computer, die deine Aufgaben ausführen."
    ]
  ],
  availabilityNote:
    "Terminal, lokaler Browser und GUI brauchen kein Codewhale-Konto. Gehostetes Web und Desktop nutzen ein Konto, das deine Modellverbindung nicht ersetzt; dein Anbieter rechnet die Nutzung über deinen eigenen Schlüssel ab.",
  accountLink: "Konto erstellen",
  surfacesHeading: "Erweitere, worauf der Agent zugreifen kann",
  surfaces: [
    ["Dateien und Befehle", "Lies das Projekt, bearbeite Dateien, führe Tests aus und prüfe die Ausgabe innerhalb der Berechtigungen, die du festlegst."],
    ["Plugins und MCP", "Verbinde weitere Tools und Dienste. Jedes Plugin bleibt deaktiviert, bis du es prüfst und aktivierst."],
    ["Computer Use · Vorschau", "Ein Plugin, mit dem der Agent andere Apps sehen und bedienen kann. Du aktivierst es und erteilst die Systemberechtigungen, die es anfordert."],
    ["Gespeicherte Sitzungen", "Halte Unterhaltung und Tool-Ergebnisse zusammen und setze die Arbeit fort, statt neu anzufangen. Der lokale Browser öffnet dieselbe Sitzung auf deinem Computer."],
    ["Fleet", "Weise Teile einer Aufgabe Agenten mit unterschiedlichen Modellen und Rollen zu und verfolge dann ihren Fortschritt."],
  ],
  runtimeLink: "Alle Integrationen ansehen",
  installBandHeading: "Installation unter macOS oder Linux",
  copy: "Kopieren",
  copied: "Kopiert ✓",
  binaries: "Binärdateien",
  chinaMirrors: "China-Mirrors",
  installGuideLink: "Installationsleitfaden lesen",
  communityHeading: "Entwickle Codewhale mit uns",
  communityBody:
    "Melde einen Fehler, schlage eine Funktion vor oder sende deinen ersten Pull Request auf GitHub. Kleine, getestete Korrekturen sind willkommen.",
  communityLinksAria: "Community-Links",
  contribute: "Pull Request senden",
};
