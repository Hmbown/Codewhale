import type { HomeDict } from "../types";

/** Polish home copy: useful work with chosen models and connected tools. */

export const home: HomeDict = {
  metaTitle: "Codewhale: twórz z własnymi modelami i narzędziami",
  metaDescription:
    "Twórz aplikacje, automatyzuj pracę i korzystaj z połączonych narzędzi w Codewhale. Otwarty kod, własne API modeli oraz lokalne lub samodzielnie hostowane wnioskowanie.",
  heroTitle: "Twórz aplikacje i automatyzuj swoją pracę.",
  heroIntro:
    "{brand} to agent open source, który pisze kod, uruchamia polecenia i pracuje z narzędziami, które podłączysz. Korzystaj z używanych już API modeli lub uruchamiaj modele lokalnie i na własnych serwerach.",
  getCodewhale: "Zainstaluj Codewhale",
  heroInstallAria: "Polecenie instalacji",
  exploreProduct: "Poznaj Codewhale",
  shotPreview: "Podgląd terminala",
  screenshotAlt:
    "Codewhale v{version}: zrzut terminala z rozmową, polem wiadomości i kontrolkami sesji.",
  latestRelease: "Najnowsze wydanie {tag}",
  releaseUnavailable: "Status wydania niedostępny",
  currentSource: "Źródło",
  sourceCandidate: "Niewydane",
  publishedRelease: "wydane",
  gainHeading: "Co możesz zrobić",
  gainLede:
    "Opisz, co chcesz stworzyć lub zautomatyzować. Codewhale może edytować pliki, uruchamiać polecenia i sprawdzać wyniki, a Ty kontrolujesz jego dostęp.",
  gain: [
    [
      "Twórz aplikacje i narzędzia",
      "Stwórz aplikację, dodaj funkcję lub napisz skrypt. Codewhale może pracować z plikami projektu, uruchamiać kod i testować to, co tworzy."
    ],
    [
      "Automatyzuj powtarzalną pracę",
      "Uruchamiaj przepływy pracy z terminala, skryptów lub CI. Przy większych zadaniach deleguj części pracy zespołowi Fleet złożonemu z agentów korzystających z różnych modeli."
    ],
    [
      "Połącz swoje narzędzia",
      "Dodawaj narzędzia przez wtyczki i serwery MCP lub korzystaj z API we własnych skryptach. Każda usługa wymaga osobnej konfiguracji i uwierzytelnienia."
    ]
  ],
  chapterModels: "Twoje modele",
  modelsHeading: "Używaj wybranych przez siebie modeli",
  modelsBody:
    "Połącz konta dostawców, endpoint zgodny z OpenAI albo modele lokalne i utrzymywane na własnej infrastrukturze. Wybierz model dla sesji i dla każdego agenta w zespole Fleet.",
  modelsFacts: [
    [
      "Twoje konta API",
      "Połącz OpenAI, Anthropic, Google lub DeepSeek za pomocą własnych kluczy."
    ],
    [
      "Twoja brama",
      "Użyj punktu końcowego zgodnego z OpenAI i wybierz oferowane przez niego modele."
    ],
    [
      "Twoje wnioskowanie",
      "Uruchamiaj modele lokalne lub samodzielnie hostowane przez Ollama, vLLM lub SGLang."
    ]
  ],
  modelsLink: "Przeglądaj modele i dostawców",
  startHeading: "Jak zacząć",
  startLede:
    "Zainstaluj Codewhale, połącz model i otwórz folder projektu. W razie potrzeby możesz dodać wtyczki i kolejnych agentów.",
  startGuideLink: "Skorzystaj z przewodnika na start",
  startVocabularyLink: "Zobacz słownik produktu",
  chapterAvailability: "Gdzie działa",
  availabilityHeading: "Dostępne teraz i w fazie rozwoju",
  availabilityLede:
    "Terminal i lokalny klient przeglądarkowy są dostępne. Natywna aplikacja desktopowa i przebudowana aplikacja webowa są w trakcie rozwoju.",
  availability: [
    [
      "Terminal i lokalna przeglądarka",
      "Wydany",
      "Zainstaluj w systemie Linux, macOS lub Windows, a następnie uruchom codewhale albo codewhale web, aby otworzyć lokalnego klienta w przeglądarce. Działają też npm i Cargo; Android w Termux to wersja podglądowa."
    ],
    [
      "CodeWhale GUI (VS Code)",
      "Dostępny",
      "Osobny projekt utrzymywany przez społeczność: czat, wątki i zmiany plików w panelu bocznym VS Code na tym samym Codewhale Runtime. Zainstaluj z VS Code Marketplace.",
      "https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode"
    ],
    [
      "Hostowana aplikacja webowa",
      "Podgląd deweloperski",
      "Budowana od nowa według wzoru aplikacji desktopowej. Dziś możesz się zalogować i wpisać /rc w działającej sesji terminala, aby kontynuować ją w przeglądarce; wykonywanie zadań w chmurze jest nadal weryfikowane."
    ],
    [
      "Aplikacja desktopowa",
      "Kompilacja deweloperska",
      "Natywna aplikacja, która staje się głównym klientem Codewhale: foldery, rozmowy i połączenia z modelami w jednym oknie. Publiczna wersja do pobrania nie jest jeszcze dostępna."
    ],
    [
      "Komputery w chmurze",
      "W przygotowaniu",
      "Komputery w chmurze, które wykonują Twoje zadania."
    ]
  ],
  availabilityNote:
    "Terminal, lokalna przeglądarka i GUI nie wymagają konta Codewhale. Hostowana wersja webowa i aplikacja desktopowa korzystają z konta. Jeśli używasz własnego klucza dostawcy, opłaty za to użycie nalicza ten dostawca.",
  accountLink: "Załóż konto",
  surfacesHeading: "Pracuj z plikami i narzędziami",
  surfaces: [
    [
      "Pliki i terminal",
      "Twórz pliki, uruchamiaj polecenia, analizuj dane i testuj rezultaty. Ty określasz katalog roboczy i uprawnienia."
    ],
    [
      "Wtyczki i połączone aplikacje",
      "Dodawaj umiejętności i narzędzia przez wtyczki i MCP. Sprawdzaj i włączaj połączenia, z których ma korzystać agent."
    ],
    [
      "Przeglądarka i komputer · podgląd",
      "Używaj narzędzi przeglądarkowych i wtyczki Computer Use w aplikacjach i witrynach w granicach przyznanego dostępu."
    ],
    [
      "Sesje, do których wrócisz",
      "Zachowuj razem rozmowę, wyniki narzędzi i historię pracy. Wznawiaj zadanie w terminalu lub lokalnym kliencie przeglądarkowym."
    ],
    [
      "Zespoły agentów",
      "Podziel duże zadanie w Fleet między agentów z różnymi rolami i modelami. Śledź ich postępy w jednym miejscu."
    ]
  ],
  runtimeLink: "Poznaj narzędzia i integracje",
  installBandHeading: "Zainstaluj w systemie macOS lub Linux",
  copy: "Kopiuj",
  copied: "Skopiowano ✓",
  binaries: "Binarki",
  chinaMirrors: "Mirrory w Chinach",
  installGuideLink: "Przeczytaj przewodnik instalacji",
  communityHeading: "Współtwórz Codewhale",
  communityBody:
    "Zgłoś błąd, popraw dokumentację lub dodaj kod na GitHubie. Możesz też tworzyć wtyczki i dzielić się przepływami pracy z innymi użytkownikami.",
  communityLinksAria: "Linki społeczności",
  contribute: "Współtwórz na GitHubie",
};
