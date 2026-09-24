<!-- source: README.md sha256:bbb8bfb61e57 -->
# Codewhale

Codewhale ist ein Open-Source-Agent, der dein Projekt liest, Dateien bearbeitet, Befehle ausführt und seine Arbeit mit einem gehosteten oder lokalen Modell deiner Wahl prüft. Starte mit einer Aufgabe im Terminal. Teile eine größere Aufgabe auf Agenten mit verschiedenen Modellen und Rollen auf.

![Codewhale in einem Terminal](web/public/codewhale-tui-d7a9a1c.png)

*Terminalvorschau aus einem Entwicklungsbuild von v0.10.0.*

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja-JP.md) · [Tiếng Việt](README.vi.md) · [Bahasa Indonesia](README.id.md) · [한국어](README.ko-KR.md) · [Español](README.es-419.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md) · [Українська](README.uk.md) · [Français](README.fr.md) · [繁體中文](README.zh-TW.md) · [हिन्दी](README.hi.md) · [Türkçe](README.tr.md) · [Italiano](README.it.md) · [Polski](README.pl.md) · [العربية](README.ar.md) · [Català](README.ca.md)

[![CI](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml/badge.svg)](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/codewhale-cli?label=crates.io)](https://crates.io/crates/codewhale-cli)
[![npm](https://img.shields.io/npm/v/codewhale?label=npm)](https://www.npmjs.com/package/codewhale)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.gg/37gfS3ksug)

## Installation

Für eine neue Installation unter macOS oder Linux verwende die offizielle GitHub-Version:

```bash
curl -fsSL https://codewhale.net/install.sh | sh
"$HOME/.local/bin/codewhale"
```

Das Installationsprogramm wählt die neueste veröffentlichte Version aus. Das [Änderungsprotokoll](CHANGELOG.md) beschreibt auch den noch unveröffentlichten Kandidaten für die nächste Version; diese Änderungen sind erst in den veröffentlichten Downloads enthalten, wenn die Version verfügbar ist.

Unter Windows lade das passende Installationsprogramm oder Archiv von [GitHub Releases](https://github.com/Hmbown/CodeWhale/releases/latest) herunter. Bestehende direkte Installationen aktualisierst du mit `codewhale update`; `codewhale update --check` prüft nur. Der Updater zeigt den Pfad der ausführbaren Datei und behält neuere Builds bei. npm und Cargo sind nachrangige Paketoptionen. Hinweise zur Migration aus einer Paketverwaltung und zu PATH stehen in der [Installationsanleitung](docs/INSTALL.md).

Der erste Start öffnet direkt den Editor für Nachrichten; es gibt keinen Einrichtungsassistenten. Antworten erfordern ein verbundenes gehostetes oder lokales Modell: Solange keines verbunden ist, zeigt der Startbildschirm "no model connected". Führe `/provider` aus (oder drücke F3), um einen Schlüssel für einen gehosteten Anbieter hinzuzufügen oder eine lokale Laufzeit zu wählen. Läuft Ollama bereits mit einem Chat-Modell, wechselt Codewhale von selbst dorthin. Codewhale unterstützt außerdem npm und Cargo als nachrangige Paketoptionen sowie Docker, Nix, Scoop, Android/Termux und einen optionalen CNB-Spiegel. Bestehende Installationen über Paketverwaltungen erhalten Migrationshinweise. Siehe die [Hilfe zu Installation und PATH](docs/INSTALL.md).

Die Tab-Vervollständigung lässt sich für jede Shell mit einem einzigen Befehl aktivieren — `codewhale completion bash|zsh|fish|powershell|elvish`. Siehe [Shell-Vervollständigung](docs/INSTALL.md#8-shell-completions).

## Verwendung

Öffne ein Terminal im Ordner deines Projekts und starte `codewhale`. Wähle deinen Anbieter mit `/provider` und dein Modell mit `/model`. Beschreibe dann eine konkrete Aufgabe:

```text
Fix the failing tests and explain what changed.
```

Du kannst eine Aufgabe auch ausführen, ohne die TUI zu öffnen:

```bash
codewhale exec "fix the failing tests and explain what changed"
```

Codewhale kann dein Repository lesen, Dateien bearbeiten, Befehle ausführen, Ergebnisse prüfen und auf ein Ziel hinarbeiten. Nutze `/mode plan`, um ohne Dateiänderungen oder Shell-Ausführung zu erkunden, und `/mode work`, wenn der Agent Änderungen vornehmen soll. Drücke `Shift+Tab`, um Ask, Auto-Review oder Full Access auszuwählen; die [Anleitung zu Modi und Berechtigungen](docs/MODES.md) erklärt, was jeweils erlaubt ist.

## Terminal, Apps und Computer Use

Das Terminal und die grafischen Clients verbinden sich mit der Codewhale Runtime, die den Agenten und seine Werkzeuge ausführt:

- **Terminal:** `codewhale` öffnet die interaktive Oberfläche; `codewhale exec` führt eine Aufgabe aus einem Skript oder CI-Job aus.
- **Lokaler Browser:** `codewhale web` öffnet den mitgelieferten [lokalen Webclient](docs/WEB.md) für dieselbe Runtime.
- **Codewhale-Desktop-App (GPUI):** Eine native Desktop-App, die in einem separaten Repository entwickelt wird, ist die Richtung für den angemeldeten Produkt-Client. Die gehostete Web-App unter app.codewhale.net wird nach ihrem Vorbild neu gebaut; Marketing-Website, Anmeldung, Abrechnung, Rechts- und Download-Seiten bleiben im Web. Die Verfügbarkeit ist auf der [Produktseite](https://codewhale.net/en/product) angegeben.

**Computer Use ergänzt Werkzeuge zum Beobachten anderer Anwendungen und zur Interaktion mit ihnen.** Das Plugin ist im aktuellen Quellcode enthalten. Prüfe die angeforderten Zugriffsrechte und aktiviere es vor der Verwendung; Betriebssystemberechtigungen und Plattformanforderungen gelten weiterhin. Siehe die mitgelieferte [Anleitung zu Computer Use](crates/tui/plugins/computer-use/README.md) und die [Plugin-Einrichtung](docs/PLUGINS.md).

Für VS Code verbindet sich die von der Community gepflegte CodeWhale-Erweiterung über eine Seitenleiste mit der lokalen Runtime. Installiere sie aus dem [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode); der Quellcode liegt auf [GitHub](https://github.com/HengQuWorld/CodeWhale-VSCode).

## Warum Codewhale

- **Wähle deine Modelle.** Verbinde gehostete Anbieter oder lokale Modelle über Ollama, vLLM oder SGLang. Mit `/provider` wechselst du den Anbieter, mit `/model` wählst du ein Modell.
- **Behalte die Kontrolle.** Prüfe vorgeschlagene Aktionen und die daraus entstehenden Dateiänderungen. Die Genehmigungseinstellungen bestimmen, wann eine Prüfung nötig ist; Full Access beachtet weiterhin die verbindlichen Grenzen der Richtlinien. `/undo` und `/restore` helfen bei der Wiederherstellung von Änderungen im Arbeitsbereich.
- **Halte lange Arbeiten übersichtlich.** Speichere Sitzungen, setze ein dauerhaftes `/goal`, prüfe Workflows vor der Ausführung und koordiniere Agenten, ohne dass ihre internen Anweisungen in deinem Gesprächsverlauf erscheinen.
- **Erweitere deinen vorhandenen Agenten.** Verbinde MCP-Server und Skills, konfiguriere Hooks und verwalte Agentenrollen als lesbare Dateien in deinem Projekt oder in deinen persönlichen Einstellungen.

Führe `/help` in der TUI aus, um Befehle und Tastenkürzel anzuzeigen.

## Sicherheit

Codewhale läuft auf deinem Rechner mit den von dir gewährten Zugriffsrechten. Genehmigungsmodi und Repository-Regeln begrenzen, was der Agent tun darf; optionales OS-Sandboxing schafft auf unterstützten Systemen eine stärkere Ausführungsgrenze. Unbekannte Modellpreise bleiben als unbekannt gekennzeichnet, statt als kostenlos gemeldet zu werden.

Lies die [Autorisierungsreihenfolge](docs/AUTHORIZATION_ORDER.md) für die genaue Richtlinienhierarchie und die [Konfiguration](docs/CONFIGURATION.md) für lokale Einstellungen.

## Dokumentation

- [Anbieter und lokale Modelle](docs/PROVIDERS.md)
- [Agententeams](docs/FLEET.md)
- [MCP](docs/MCP.md), [Hooks](docs/HOOKS.md) und [Konfiguration](docs/CONFIGURATION.md)
- [Lokaler Webclient](docs/WEB.md)
- [Gesamte Dokumentation](docs/README.md)
- [Aufbau des Repositorys und Anleitung zum Mitwirken](CONTRIBUTING.md#project-structure)

## Der Community beitreten

**Fehlerberichte, Funktionsideen und Pull Requests sind willkommen**, egal ob du Codewhale seit Monaten nutzt oder zum ersten Mal ausprobierst. Wenn ein Anbieter fehlt, ein Workflow umständlich ist oder dir die Terminaloberfläche im Weg steht, [eröffne ein Issue](https://github.com/Hmbown/CodeWhale/issues/new/choose) oder [sende einen Pull Request](CONTRIBUTING.md), damit wir es gemeinsam verbessern können. Erste Beiträge sind willkommen, und Mitwirkende behalten die Anerkennung für ihre übernommenen Arbeiten.

Tritt unserem [Discord](https://discord.gg/37gfS3ksug) bei oder füge Hunter auf WeChat (`hunterbown`) hinzu und bitte um Aufnahme in die Whale-Brothers-Gruppe.

## Projektgeschichte

Codewhale begann als `deepseek-tui` und bewahrt weiterhin die Kompatibilität mit dessen Konfiguration und Sitzungen. Heute ist es anbieterneutral, wird unabhängig gepflegt und ist mit keinem Modellanbieter verbunden.

Vielen Dank an alle Mitwirkenden und die Open-Source-Communitys, die das Projekt beim Wachsen unterstützt haben. Siehe [Liste der Mitwirkenden](docs/CONTRIBUTORS.md).

## Lizenz

[MIT](LICENSE). Aus anderen Open-Source-Projekten übernommene Teile sind in den [Hinweisen zu Drittanbieterkomponenten](docs/THIRD_PARTY_NOTICES.md) aufgeführt.
