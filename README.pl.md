<!-- source: README.md sha256:bbb8bfb61e57 -->
# Codewhale

Codewhale to agent o otwartym kodzie źródłowym, który czyta Twój projekt, edytuje pliki, wykonuje polecenia i sprawdza swoją pracę przy użyciu wybranego przez Ciebie modelu hostowanego lub lokalnego. Zacznij od jednego zadania w terminalu. Przy większej pracy powierz jej części agentom korzystającym z różnych modeli i pełniącym różne role.

![Codewhale działający w terminalu](web/public/codewhale-tui-d7a9a1c.png)

*Podgląd terminala z rozwojowej kompilacji v0.10.0.*

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja-JP.md) · [Tiếng Việt](README.vi.md) · [Bahasa Indonesia](README.id.md) · [한국어](README.ko-KR.md) · [Español](README.es-419.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md) · [Українська](README.uk.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [繁體中文](README.zh-TW.md) · [हिन्दी](README.hi.md) · [Türkçe](README.tr.md) · [Italiano](README.it.md) · [العربية](README.ar.md) · [Català](README.ca.md)

[![CI](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml/badge.svg)](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/codewhale-cli?label=crates.io)](https://crates.io/crates/codewhale-cli)
[![npm](https://img.shields.io/npm/v/codewhale?label=npm)](https://www.npmjs.com/package/codewhale)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.gg/37gfS3ksug)

## Instalacja

Przy nowej instalacji na macOS lub Linuksie użyj oficjalnego wydania z GitHuba:

```bash
curl -fsSL https://codewhale.net/install.sh | sh
"$HOME/.local/bin/codewhale"
```

Instalator wybiera najnowsze opublikowane wydanie. [Dziennik zmian](CHANGELOG.md) opisuje również nieopublikowanego jeszcze kandydata do kolejnego wydania; te zmiany trafią do opublikowanych plików do pobrania dopiero po udostępnieniu wydania.

Na Windows pobierz odpowiedni instalator lub archiwum z [GitHub Releases](https://github.com/Hmbown/CodeWhale/releases/latest). Istniejącą instalację bezpośrednią zaktualizujesz poleceniem `codewhale update`; `codewhale update --check` służy tylko do sprawdzenia. Aktualizator pokazuje ścieżkę pliku wykonywalnego i zachowuje nowsze kompilacje. npm i Cargo to opcje dodatkowe. Migrację z menedżera pakietów i konfigurację PATH opisuje [instrukcja instalacji](docs/INSTALL.md).

Pierwsze uruchomienie otwiera od razu edytor wiadomości; nie prowadzi przez żadną konfigurację. Odpowiedzi modelu wymagają połączenia z modelem hostowanym lub lokalnym: dopóki żaden nie jest połączony, ekran startowy pokazuje "no model connected". Uruchom `/provider` (lub naciśnij F3), aby dodać klucz usługi hostowanej albo wybrać lokalne środowisko uruchomieniowe. Jeśli Ollama działa już z modelem czatu, Codewhale sam się na niego przełącza. Codewhale obsługuje również npm i Cargo jako dodatkowe sposoby instalacji, a także Docker, Nix, Scoop, Android/Termux oraz opcjonalny serwer lustrzany CNB. Dla istniejących instalacji zarządzanych przez menedżera pakietów dostępne są instrukcje migracji. Zobacz [pomoc dotyczącą instalacji i PATH](docs/INSTALL.md).

Uzupełnianie klawiszem Tab można włączyć jednym poleceniem dla każdej powłoki — `codewhale completion bash|zsh|fish|powershell|elvish`. Zobacz [uzupełnianie powłoki](docs/INSTALL.md#8-shell-completions).

## Użycie

Otwórz terminal w folderze projektu i uruchom `codewhale`. Wybierz dostawcę poleceniem `/provider`, a model poleceniem `/model`. Następnie opisz konkretne zadanie:

```text
Fix the failing tests and explain what changed.
```

Możesz też uruchomić zadanie bez otwierania TUI:

```bash
codewhale exec "fix the failing tests and explain what changed"
```

Codewhale może czytać Twoje repozytorium, edytować pliki, wykonywać polecenia, sprawdzać wyniki i kontynuować pracę nad celem. Użyj `/mode plan`, aby analizować projekt bez zmian w plikach i wykonywania poleceń powłoki, a `/mode work`, gdy chcesz wprowadzać zmiany. Naciśnij `Shift+Tab`, aby wybrać Ask, Auto-Review lub Full Access; [przewodnik po trybach i uprawnieniach](docs/MODES.md) wyjaśnia, na co pozwala każdy z nich.

## Terminal, aplikacje i Computer Use

Terminal i klienci graficzni łączą się z Codewhale Runtime, który uruchamia agenta i jego narzędzia:

- **Terminal:** `codewhale` otwiera interaktywny interfejs; `codewhale exec` uruchamia zadanie ze skryptu lub zadania CI.
- **Lokalna przeglądarka:** `codewhale web` otwiera dołączonego [lokalnego klienta webowego](docs/WEB.md) dla tego samego środowiska wykonawczego.
- **Aplikacja desktopowa Codewhale (GPUI):** natywna aplikacja desktopowa, rozwijana w osobnym repozytorium, jest kierunkiem klienta produktu dla zalogowanych użytkowników. Hostowana aplikacja webowa na app.codewhale.net zostanie przebudowana na jej wzór; strona marketingowa, logowanie, rozliczenia oraz strony prawne i pobierania pozostają w sieci. Informacje o dostępności znajdują się na [stronie produktu](https://codewhale.net/en/product).

**Computer Use dodaje narzędzia do obserwowania innych aplikacji i interakcji z nimi.** Wtyczka jest dołączona do obecnego kodu źródłowego. Przed użyciem sprawdź, o jaki dostęp prosi, i włącz ją; nadal obowiązują uprawnienia systemu operacyjnego i wymagania platformy. Zobacz dołączony [przewodnik po Computer Use](crates/tui/plugins/computer-use/README.md) oraz [konfigurację wtyczek](docs/PLUGINS.md).

Utrzymywane przez społeczność rozszerzenie CodeWhale dla VS Code łączy się z lokalnym Runtime z panelu bocznego. Zainstaluj je z [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode); kod źródłowy znajdziesz na [GitHub](https://github.com/HengQuWorld/CodeWhale-VSCode).

## Dlaczego Codewhale

- **Wybieraj modele.** Połącz się z hostowanymi dostawcami lub lokalnymi modelami przez Ollama, vLLM albo SGLang. Polecenie `/provider` służy do zmiany dostawcy, a `/model` do wyboru modelu.
- **Zachowaj kontrolę.** Sprawdzaj proponowane działania i wynikające z nich zmiany w plikach. Ustawienia zatwierdzania określają, kiedy potrzebna jest weryfikacja; Full Access nadal przestrzega nieprzekraczalnych ograniczeń zasad. `/undo` i `/restore` pomagają przywrócić przestrzeń roboczą po zmianach.
- **Utrzymuj porządek w długich zadaniach.** Zapisuj sesje, ustawiaj trwały `/goal`, sprawdzaj przepływy pracy przed uruchomieniem i koordynuj agentów bez umieszczania ich wewnętrznych instrukcji w zapisie Twojej rozmowy.
- **Rozszerzaj agenta, którego już masz.** Podłączaj serwery MCP i umiejętności, konfiguruj hooki oraz przechowuj role agentów jako czytelne pliki w projekcie lub ustawieniach osobistych.

Uruchom `/help` w TUI, aby zobaczyć polecenia i skróty klawiaturowe.

## Bezpieczeństwo

Codewhale działa na Twoim komputerze z dostępem, który mu przyznasz. Tryby zatwierdzania i reguły repozytorium ograniczają działania agenta; opcjonalny sandbox systemu operacyjnego zapewnia mocniejszą granicę wykonywania tam, gdzie jest obsługiwany. Nieznane ceny modeli pozostają oznaczone jako nieznane, zamiast być przedstawiane jako bezpłatne.

Przeczytaj o [kolejności autoryzacji](docs/AUTHORIZATION_ORDER.md), aby poznać dokładną hierarchię zasad, oraz o [konfiguracji](docs/CONFIGURATION.md), aby poznać ustawienia lokalne.

## Dokumentacja

- [Dostawcy i modele lokalne](docs/PROVIDERS.md)
- [Zespoły agentów](docs/FLEET.md)
- [MCP](docs/MCP.md), [hooki](docs/HOOKS.md) i [konfiguracja](docs/CONFIGURATION.md)
- [Lokalny klient webowy](docs/WEB.md)
- [Cała dokumentacja](docs/README.md)
- [Struktura repozytorium i przewodnik dla współtwórców](CONTRIBUTING.md#project-structure)

## Dołącz do społeczności

**Zgłoszenia błędów, pomysły na funkcje i pull requesty są mile widziane**, niezależnie od tego, czy używasz Codewhale od miesięcy, czy próbujesz go po raz pierwszy. Jeśli brakuje dostawcy, przepływ pracy jest niewygodny albo interfejs terminala przeszkadza Ci w pracy, [otwórz issue](https://github.com/Hmbown/CodeWhale/issues/new/choose) lub [wyślij pull request](CONTRIBUTING.md), abyśmy mogli wspólnie go ulepszyć. Pierwsze wkłady są mile widziane, a autorzy zachowują uznanie za pracę przyjętą do projektu.

Dołącz do [Discorda](https://discord.gg/37gfS3ksug) albo dodaj Huntera na WeChat (`hunterbown`) i poproś o dołączenie do grupy Whale Brothers.

## Historia projektu

Codewhale rozpoczął się jako `deepseek-tui` i nadal zachowuje zgodność z jego konfiguracją oraz sesjami. Obecnie jest niezależny od dostawców, utrzymywany samodzielnie i nie jest powiązany z żadnym dostawcą modeli.

Dziękujemy wszystkim współtwórcom oraz społecznościom open source, które pomogły projektowi się rozwijać. Zobacz [rejestr współtwórców](docs/CONTRIBUTORS.md).

## Licencja

[MIT](LICENSE). Części zaadaptowane z innych projektów open source są wymienione w [informacjach o komponentach zewnętrznych](docs/THIRD_PARTY_NOTICES.md).
