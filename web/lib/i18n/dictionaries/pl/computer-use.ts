import type { ComputerUseDict } from "../types";

/**
 * Polish dictionary for `app/[locale]/computer-use/page.tsx` and the
 * Computer Use section of the install page. Product names, the menu items
 * Pause, Stop and Check for updates, and the macOS setting names stay as
 * the app shows them.
 */
export const computerUse: ComputerUseDict = {
  metaTitle: "Computer Use na Maca · Codewhale",
  metaDescription: "Pobierz i skonfiguruj Codewhale Computer Use na Maca. Sterowanie aplikacjami w tle, konfiguracja uprawnień oraz przyciski Pause i Stop pod Twoją kontrolą.",
  title: "Computer Use",
  lead: "Pozwól Codewhale działać w Twoich aplikacjach, a sam pracuj dalej. Pomocnik na Maca zbiera w pasku menu uprawnienia, sterowanie aplikacjami w tle oraz możliwość wstrzymania lub zatrzymania wprowadzania danych.",
  publisher: "Od Codewhale",
  download: "Pobierz na Maca",
  downloadZip: "Archiwum ZIP (używane przez aktualizator w aplikacji)",
  requirements: "macOS 13.5 lub nowszy · Apple silicon i Intel",
  included: "Jedna aplikacja do pobrania. Nie trzeba osobno instalować Node ani kompilatora.",
  pendingTitle: "Wersja na Maca w przygotowaniu",
  pendingBody: "Publiczny instalator pojawi się tutaj po zakończeniu notaryzacji Apple i kontroli wydania.",
  unavailableTitle: "Nie udało się sprawdzić dostępności pobierania",
  unavailableBody: "Odśwież stronę, aby spróbować ponownie, albo sprawdź opublikowane wydania poniżej.",
  releases: "Opublikowane wydania",
  receipt: "Dane do weryfikacji pobranego pliku",
  setup: "Skonfiguruj Maca",
  steps: [
    { title: "Zainstaluj aplikację", body: "Otwórz obraz dysku i przeciągnij Codewhale Computer Use do folderu „Programy”. Uruchom aplikację z folderu „Programy”, a następnie wybierz Computer Use z menu pod ikoną wieloryba w pasku menu." },
    { title: "Sprawdź uprawnienia", body: "Przyciski konfiguracji otwierają sekcje „Dostępność” i „Nagrywanie ekranu” w Ustawieniach systemowych. To Ty decydujesz, które uprawnienia przyznać." },
    { title: "Uruchom test w tle", body: "Pomocnik otwiera jednorazowe okno testowe, wpisuje w nim tekst i zapisuje jego zrzut. Sprawdza przy tym, czy w trakcie testu zmienił się wskaźnik lub aktywna aplikacja." },
    { title: "Połącz z Codewhale", body: "Przejrzyj, zatwierdź jako zaufaną i włącz wtyczkę Computer Use w sklepie z wtyczkami Codewhale. Używaj wtyczki w wersji 0.3.1 lub nowszej, aby lokalne działania przechodziły przez przyciski Pause i Stop pomocnika." },
  ],
  controlsTitle: "Pracuj dalej. Zachowaj kontrolę.",
  controlsBody: "Obsługiwane działania są wykonywane w tle w wybranej aplikacji. Aplikacje i gesty wymagające przejęcia pierwszego planu potrzebują Twojej zgody. Menu pokazuje docelową aplikację i tryb wprowadzania danych; Pause (wstrzymaj) zawiesza wprowadzanie danych przez pomocnika, a Stop (zatrzymaj) kończy jego bieżące sesje.",
  updateTitle: "Aktualizacje wtedy, gdy chcesz",
  updateBody: "Wybierz w aplikacji Check for updates (sprawdź aktualizacje). Przed instalacją aktualizacji pomocnik sprawdza pobrany plik, podpis Codewhale i notaryzację Apple, a poprzednią wersję zachowuje na wypadek konieczności przywrócenia.",
  help: "Konfiguracja i rozwiązywanie problemów",
  notes: "Informacje o wydaniu",
  demo: "Zobacz test w tle",
  source: "Kod źródłowy i inne platformy",
  platforms: "Ten plik jest przeznaczony na Maca. Windows i Linux korzystają obecnie z wtyczki źródłowej i konfiguracji po stronie hosta.",
};
