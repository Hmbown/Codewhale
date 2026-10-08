import type { HomeDict } from "../types";

/** Polish home copy: useful work with chosen models and connected tools. */

export const home: HomeDict = {
  metaTitle: "Codewhale: twórz z własnymi modelami i narzędziami",
  metaDescription:
    "Twórz aplikacje, automatyzuj pracę i korzystaj z połączonych narzędzi w Codewhale. Otwarty kod, własne API modeli oraz lokalne lub samodzielnie hostowane wnioskowanie.",
  heroTitle: "Agent open source dla Twojego komputera.",
  heroIntro:
    "Twórz aplikacje, automatyzuj procesy i korzystaj ze Slacka, Gmaila oraz innych połączonych narzędzi. {brand} korzysta z używanych przez Ciebie API modeli lub z wnioskowania lokalnego i na własnej infrastrukturze.",
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
    "Zacznij od oczekiwanego wyniku. Codewhale pracuje z plikami, poleceniami i połączonymi narzędziami; Ty wybierasz dostęp i zasady zatwierdzania.",
  gain: [
    [
      "Twórz aplikacje i narzędzia",
      "Zamień pomysł w działającą aplikację, przydatny skrypt lub funkcję istniejącego projektu. Agent pisze, uruchamia i testuje razem z Tobą."
    ],
    [
      "Automatyzuj powtarzalną pracę",
      "Zamień cykliczne zadanie w proces uruchamiany z terminala, skryptów lub CI. Użyj Fleet, gdy kilku agentów może pracować równolegle."
    ],
    [
      "Połącz swoje narzędzia",
      "Połącz narzędzia takie jak Gmail i Slack przez wtyczki, serwery MCP lub API. Korzystaj z tych usług razem z plikami i poleceniami."
    ]
  ],
  chapterModels: "Twoje modele",
  modelsHeading: "Korzystaj dalej ze swoich modeli.",
  modelsBody:
    "Podłącz API modeli, za które już płacisz, użyj zgodnej bramy lub uruchom wnioskowanie na własnym sprzęcie. Wybieraj modele dla sesji i poszczególnych agentów Fleet.",
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
  startHeading: "Przynieś zadanie. Zacznij.",
  startLede:
    "Zainstaluj Codewhale, połącz model i daj mu coś wartego zrobienia. Zacznij od jednego agenta; dodawaj narzędzia lub zespół, gdy ich potrzebujesz.",
  startGuideLink: "Skorzystaj z przewodnika na start",
  startVocabularyLink: "Zobacz słownik produktu",
  chapterAvailability: "Gdzie działa",
  availabilityHeading: "Zacznij od terminala.",
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
  surfacesHeading: "Jedno zadanie. Twoje pliki, aplikacje i agenci.",
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
  runtimeLink: "Zobacz wszystkie integracje",
  installBandHeading: "Zainstaluj w systemie macOS lub Linux",
  copy: "Kopiuj",
  copied: "Skopiowano ✓",
  binaries: "Binarki",
  chinaMirrors: "Mirrory w Chinach",
  installGuideLink: "Przeczytaj przewodnik instalacji",
  communityHeading: "Dopasuj Codewhale do siebie.",
  communityBody:
    "Codewhale ma otwarty kod. Czytaj go, stwórz wtyczkę, udostępnij proces pracy lub pomóż ulepszyć kolejną wersję.",
  communityLinksAria: "Linki społeczności",
  contribute: "Współtwórz na GitHubie",
};
