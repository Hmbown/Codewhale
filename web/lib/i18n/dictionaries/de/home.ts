import type { HomeDict } from "../types";

/** German home copy: useful work with chosen models and connected tools. */

export const home: HomeDict = {
  metaTitle: "Codewhale: Bauen mit deinen Modellen und Werkzeugen",
  metaDescription:
    "Baue Apps, automatisiere Abläufe und arbeite mit verbundenen Werkzeugen. Codewhale ist Open Source und nutzt deine Modell-APIs oder lokale und selbst gehostete Inferenz.",
  heroTitle: "Baue, was du dir vorstellst.",
  heroIntro:
    "Entwickle eine App, automatisiere einen Ablauf oder mache aus gesammelten Recherchen etwas Nützliches. {brand} arbeitet mit deinen bisherigen Modell-APIs, deiner eigenen Inferenz und den Werkzeugen, die du verbindest.",
  getCodewhale: "Codewhale installieren",
  heroInstallAria: "Installationsbefehl",
  exploreProduct: "Codewhale entdecken",
  shotPreview: "Terminal-Vorschau",
  screenshotAlt:
    "Codewhale v{version}: eine Terminalaufnahme mit Gespräch, Nachrichteneingabe und Sitzungssteuerung.",
  latestRelease: "Aktuellstes Release {tag}",
  releaseUnavailable: "Release-Status nicht verfügbar",
  currentSource: "Quelle",
  sourceCandidate: "Unveröffentlicht",
  publishedRelease: "veröffentlicht",
  gainHeading: "Schaffe etwas Nützliches.",
  gainLede:
    "Beginne mit dem gewünschten Ergebnis. Codewhale arbeitet mit Dateien, Befehlen und verbundenen Werkzeugen; Zugriff und Freigaben bestimmst du.",
  gain: [
    [
      "Apps und Werkzeuge bauen",
      "Mache aus einer Idee eine funktionierende App, ein nützliches Skript oder eine neue Projektfunktion. Lass den Agenten mit dir schreiben, ausführen und testen."
    ],
    [
      "Wiederkehrende Arbeit automatisieren",
      "Mache aus einer wiederkehrenden Aufgabe einen Ablauf für Terminal, Skripte oder CI. Nutze Fleet, wenn mehrere Agenten parallel arbeiten können."
    ],
    [
      "Deine Werkzeuge verbinden",
      "Verbinde Werkzeuge wie Gmail und Slack über Plugins, MCP-Server oder APIs. Nutze diese Dienste zusammen mit deinen Dateien und Befehlen."
    ]
  ],
  exampleTasks: [
    "Baue eine App zur Terminbuchung.",
    "Erstelle aus einer Verkaufs-CSV einen Wochenbericht, den ich erneut erzeugen kann.",
    "Mach aus den Nachrichten meines verbundenen E-Mail-Kontos eine Aufgabenliste.",
  ],
  // A static example report built from local sample orders.
  reportTitle: "Wöchentlicher Verkaufsbericht",
  reportSampleLabel: "Beispielbericht · Beispieldaten",
  reportDescription: "Lass Codewhale Bestellungen nach Wochen zusammenfassen und den Ablauf für die nächste CSV-Datei speichern.",
  reportSourceLabel: "Ausgangsdaten:",
  reportColumns: ["Wochenbeginn","Bestellungen","Umsatz (USD)"],
  reportTotalLabel: "Gesamt",
  reportTrend: "Umsatzänderung von der ersten bis zur letzten Woche: {change}.",
  reportDownloadLabel: "Bericht als CSV herunterladen",
  chapterModels: "Deine Modelle",
  modelsHeading: "Nutze deine Modelle weiter.",
  modelsBody:
    "Verbinde deine bisherigen Modell-APIs, nutze ein kompatibles Gateway oder betreibe Inferenz auf eigener Hardware. Wähle Modelle pro Sitzung und für einzelne Agenten in Fleet.",
  modelsFacts: [
    [
      "Deine API-Konten",
      "Verbinde etwa OpenAI, Anthropic, Google oder DeepSeek mit deinen eigenen Schlüsseln."
    ],
    [
      "Dein Gateway",
      "Nutze einen OpenAI-kompatiblen Endpoint und wähle dessen Modelle."
    ],
    [
      "Deine Inferenz",
      "Betreibe lokale oder selbst gehostete Modelle mit Ollama, vLLM oder SGLang."
    ]
  ],
  modelsLink: "Modelle und Anbieter durchsuchen",
  startHeading: "Bring eine Aufgabe mit. Leg los.",
  startLede:
    "Installiere Codewhale, verbinde ein Modell und gib ihm eine sinnvolle Aufgabe. Beginne mit einem Agenten und ergänze bei Bedarf Werkzeuge oder ein Team.",
  startGuideLink: "Dem Leitfaden für die ersten Schritte folgen",
  startVocabularyLink: "Produktvokabular ansehen",
  chapterAvailability: "Wo es läuft",
  availabilityHeading: "Starte im Terminal.",
  availabilityLede:
    "Terminal und lokaler Browser-Client sind verfügbar. Eine native Desktop-App und eine neu aufgebaute gehostete Web-App sind in Entwicklung.",
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
    "Terminal, lokaler Browser und GUI brauchen kein Codewhale-Konto. Gehostetes Web und Desktop nutzen ein Konto. Wenn du deinen eigenen Anbieterschlüssel verwendest, rechnet der Anbieter diese Nutzung ab.",
  accountLink: "Konto erstellen",
  surfacesHeading: "Eine Aufgabe. Deine Dateien, Apps und Agenten.",
  surfaces: [
    [
      "Dateien und Terminal",
      "Erstelle Dateien, führe Befehle aus, untersuche Daten und teste deine Ergebnisse. Arbeitsordner und Berechtigungen bestimmst du."
    ],
    [
      "Plugins und verbundene Apps",
      "Ergänze Skills und Werkzeuge über Plugins und MCP. Prüfe und aktiviere die Verbindungen, die dein Agent nutzen darf."
    ],
    [
      "Browser und Computer · Vorschau",
      "Nutze Browser-Werkzeuge und das Computer-Use-Plugin für Apps und Websites mit dem Zugriff, den du erlaubst."
    ],
    [
      "Sitzungen zum Fortsetzen",
      "Behalte Gespräch, Werkzeugergebnisse und Arbeitsverlauf zusammen. Setze die Aufgabe im Terminal oder lokalen Browser-Client fort."
    ],
    [
      "Agententeams",
      "Verteile größere Aufgaben mit Fleet auf Agenten mit verschiedenen Rollen und Modellen. Verfolge ihren Fortschritt an einem Ort."
    ]
  ],
  runtimeLink: "Alle Integrationen ansehen",
  installBandHeading: "Installation unter macOS oder Linux",
  copy: "Kopieren",
  copied: "Kopiert ✓",
  binaries: "Binärdateien",
  chinaMirrors: "China-Mirrors",
  installGuideLink: "Installationsleitfaden lesen",
  communityHeading: "Mach Codewhale zu deinem Werkzeug.",
  communityBody:
    "Codewhale ist Open Source. Lies den Code, baue ein Plugin, teile einen Ablauf oder hilf, die nächste Version zu verbessern.",
  communityLinksAria: "Community-Links",
  contribute: "Auf GitHub mitwirken",
};
