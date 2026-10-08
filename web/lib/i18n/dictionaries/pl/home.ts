import type { HomeDict } from "../types";

/**
 * Polish home dictionary — native copy for the whale-road landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale: agent programistyczny z otwartym kodem dla dowolnego modelu",
  metaDescription:
    "Codewhale to agent programistyczny z otwartym kodem do pracy w terminalu. Czyta Twój projekt, edytuje pliki i uruchamia Twoje testy z wybranym przez Ciebie modelem hostowanym lub lokalnym.",
  heroTitle: "Agent programistyczny z otwartym kodem dla dowolnego modelu",
  heroIntro:
    "{brand} czyta Twój projekt, edytuje pliki i uruchamia Twoje testy w terminalu. Podłącz model hostowany lub lokalny i wybierz, które działania wymagają Twojej zgody.",
  getCodewhale: "Zainstaluj Codewhale",
  heroInstallAria: "Polecenie instalacji",
  exploreProduct: "Zobacz, jak to działa",
  shotPreview: "Podgląd terminala",
  shotBuild: "kompilacja deweloperska v{version}",
  screenshotAlt:
    "Codewhale v{version}, kompilacja deweloperska: wieloryb, nowa sesja, pole wiadomości, uprawnienia Ask, tryb Work i stan modelu. Obraz rzeczywistego wyjścia odizolowanego terminala.",
  latestRelease: "Najnowsze wydanie {tag}",
  releaseUnavailable: "Status wydania niedostępny",
  currentSource: "Źródło",
  sourceCandidate: "Niewydane",
  publishedRelease: "wydane",
  figcaptionSourceCandidate: "niewydane",
  chapterTerminal: "Twój terminal",
  chapterTerminalTitle: "Śledź każdą zmianę i każde polecenie podczas wykonywania",
  gainHeading: "Zleć zadanie i zachowaj kontrolę",
  gainLede:
    "Poproś o wynik: naprawę błędu, wyjaśnienie modułu albo automatyzację powtarzanego zadania. Zacznij od jednego agenta i dodaj kolejnych, gdy praca się rozrośnie.",
  gain: [
    [
      "Zmieniaj kod i sprawdzaj go",
      "Agent analizuje Twój projekt, edytuje pliki i uruchamia Twoje testy. Śledź każdą zmianę i każdy wynik polecenia w trakcie jego pracy."
    ],
    [
      "Automatyzuj powtarzalną pracę",
      "Uruchamiaj codewhale exec ze skryptów i CI. Użyj Fleet, aby podzielić większe zadanie między kilku agentów."
    ],
    [
      "Zachowaj kontrolę",
      "Ustaw uprawnienia przed rozpoczęciem pracy, odpowiadaj na prośby o zgodę i zatrzymaj zadanie w dowolnym momencie. Uruchom /receipts, aby wyświetlić każdy plik, polecenie i zgodę w sesji."
    ]
  ],
  chapterModels: "Twoje modele",
  modelsHeading: "Wybierz model do każdego zadania",
  modelsBody:
    "Dla każdej sesji wybierz wbudowanego dostawcę, dowolny endpoint zgodny z OpenAI lub model lokalny. Połączenie z modelem pozostaje oddzielone od konta Codewhale.",
  modelsFacts: [
    ["Hostowane", "Twój własny klucz API zapisany przez codewhale auth set --provider <id>"],
    ["Bramka", "Jeden endpoint do wielu modeli; dostawcę nadal wybierasz Ty"],
    ["Lokalne", "vLLM, SGLang lub Ollama na localhost, zwykle bez klucza"],
  ],
  modelsLink: "Przeglądaj modele i dostawców",
  startHeading: "Zainstaluj, podłącz model i uruchom zadanie",
  startLede:
    "Uruchom pierwsze zadanie w trzech krokach z folderu projektu. Dodaj Fleet później, jeśli praca wymaga kilku agentów.",
  startGuideLink: "Skorzystaj z przewodnika na start",
  startVocabularyLink: "Zobacz słownik produktu",
  chapterAvailability: "Gdzie działa",
  availabilityHeading: "Korzystaj z Codewhale w terminalu już dziś",
  availabilityLede:
    "Już teraz możesz korzystać z terminala, lokalnego klienta w przeglądarce lub utrzymywanego przez społeczność interfejsu CodeWhale GUI. Aplikacja desktopowa i budowana od nowa hostowana aplikacja webowa są w przygotowaniu i mają ten sam model sesji.",
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
    "Terminal, lokalna przeglądarka i GUI nie wymagają konta Codewhale. Hostowana wersja webowa i aplikacja desktopowa korzystają z konta, które nie zastępuje połączenia z modelem; za użycie Twojego klucza opłaty nalicza Twój dostawca.",
  accountLink: "Załóż konto",
  surfacesHeading: "Rozszerz to, do czego agent ma dostęp",
  surfaces: [
    ["Pliki i polecenia", "Czytaj projekt, edytuj pliki, uruchamiaj testy i sprawdzaj wyniki w granicach ustawionych uprawnień."],
    ["Wtyczki i MCP", "Podłącz kolejne narzędzia i usługi. Każda wtyczka pozostaje wyłączona, dopóki jej nie sprawdzisz i nie włączysz."],
    ["Computer Use · podgląd", "Wtyczka, która pozwala agentowi widzieć inne aplikacje i nimi sterować. To Ty ją włączasz i przyznajesz uprawnienia systemowe, o które prosi."],
    ["Zapisane sesje", "Przechowuj rozmowę i wyniki narzędzi razem i wznawiaj pracę zamiast zaczynać od nowa. Lokalna przeglądarka otwiera tę samą sesję na Twoim komputerze."],
    ["Fleet", "Przydzielaj części zadania agentom z różnymi modelami i rolami, a następnie śledź ich postępy."],
  ],
  runtimeLink: "Zobacz wszystkie integracje",
  installBandHeading: "Zainstaluj w systemie macOS lub Linux",
  copy: "Kopiuj",
  copied: "Skopiowano ✓",
  binaries: "Binarki",
  chinaMirrors: "Mirrory w Chinach",
  installGuideLink: "Przeczytaj przewodnik instalacji",
  communityHeading: "Rozwijaj Codewhale razem z nami",
  communityBody:
    "Zgłoś błąd, zaproponuj funkcję lub wyślij swój pierwszy pull request na GitHubie. Małe, przetestowane poprawki są mile widziane.",
  communityLinksAria: "Linki społeczności",
  contribute: "Wyślij pull request",
};
