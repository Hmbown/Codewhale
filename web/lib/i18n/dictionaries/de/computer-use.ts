import type { ComputerUseDict } from "../types";

/**
 * German dictionary for `app/[locale]/computer-use/page.tsx` and the
 * Computer Use section of the install page. Product names (Codewhale,
 * Computer Use), the menu items Pause, Stop and Check for updates, and the
 * macOS setting names stay as the app and macOS show them. Register follows
 * the other German dictionaries: knapp, Anrede per Du.
 */
export const computerUse: ComputerUseDict = {
  metaTitle: "Computer Use für den Mac · Codewhale",
  metaDescription: "Codewhale Computer Use für den Mac herunterladen und einrichten. App-Steuerung im Hintergrund, Berechtigungen einrichten und Pause und Stop in deiner Hand.",
  title: "Computer Use",
  lead: "Lass Codewhale in deinen Apps arbeiten, während du weiterarbeitest. Der Mac-Helfer bringt Berechtigungen, App-Steuerung im Hintergrund und eine Möglichkeit, Eingaben zu pausieren oder zu stoppen, in deine Menüleiste.",
  publisher: "Von Codewhale",
  download: "Für den Mac herunterladen",
  downloadZip: "ZIP-Archiv (wird vom Updater in der App verwendet)",
  requirements: "macOS 13.5 oder neuer · Apple silicon und Intel",
  included: "Ein einziger App-Download. Keine separate Node-Installation, kein Compiler nötig.",
  pendingTitle: "Mac-Download in Vorbereitung",
  pendingBody: "Der öffentliche Installer erscheint hier, sobald die Beglaubigung durch Apple und die Release-Prüfungen abgeschlossen sind.",
  unavailableTitle: "Verfügbarkeit des Downloads konnte nicht geprüft werden",
  unavailableBody: "Lade die Seite neu, um es erneut zu versuchen, oder sieh dir unten die veröffentlichten Releases an.",
  releases: "Veröffentlichte Releases",
  receipt: "Prüfdetails zum Download",
  setup: "Deinen Mac einrichten",
  steps: [
    { title: "App installieren", body: "Öffne das Disk-Image und ziehe Codewhale Computer Use in den Ordner „Programme“. Öffne die App aus „Programme“ und wähle dann Computer Use über das Wal-Symbol in deiner Menüleiste." },
    { title: "Berechtigungen prüfen", body: "Über die Einrichtungsschaltflächen öffnest du „Bedienungshilfen“ und „Bildschirmaufnahme“ in den Systemeinstellungen. Welche Berechtigungen du erteilst, entscheidest du." },
    { title: "Hintergrundprüfung ausführen", body: "Der Helfer öffnet ein temporäres Übungsfenster, gibt Text ein und erfasst dieses Fenster. Dabei prüft er, ob sich der Zeiger oder die aktive App während des Durchlaufs verändert hat." },
    { title: "Mit Codewhale verbinden", body: "Prüfe Computer Use im Plugin-Marktplatz von Codewhale, stufe es als vertrauenswürdig ein und aktiviere es. Verwende Plugin 0.3.1 oder neuer, damit lokale Aktionen über die Pause- und Stop-Steuerung des Helfers laufen." },
  ],
  controlsTitle: "Weiterarbeiten. Kontrolle behalten.",
  controlsBody: "Unterstützte Aktionen laufen in der ausgewählten App im Hintergrund. Apps und Gesten, die Steuerung im Vordergrund brauchen, erfordern deine Freigabe. Das Menü zeigt Ziel und Eingabemodus; Pause setzt die Eingaben des Helfers aus, Stop beendet seine laufenden Sitzungen.",
  updateTitle: "Updates, wann du willst",
  updateBody: "Wähle in der App Check for updates. Vor der Installation eines Updates prüft die App den Download, die Codewhale-Signatur und die Beglaubigung durch Apple und behält die vorherige App zur Wiederherstellung.",
  help: "Einrichtung und Fehlerbehebung",
  notes: "Versionshinweise",
  demo: "Hintergrundprüfung ansehen",
  source: "Quellcode und weitere Plattformen",
  platforms: "Dieser Download ist für den Mac. Windows und Linux nutzen derzeit das Quellcode-Plugin und die Einrichtung auf dem Host.",
};
