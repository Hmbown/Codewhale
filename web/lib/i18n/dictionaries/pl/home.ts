import type { HomeDict } from "../types";

/**
 * Polish home dictionary — native copy for the Tidal Folio landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale — Twórz i automatyzuj z wybranymi przez siebie modelami",
  metaDescription:
    "Twórz oprogramowanie, pracuj ze swoimi plikami i automatyzuj codzienne zadania za pomocą agentów z otwartym kodem i wybranych przez siebie hostowanych lub lokalnych modeli AI.",
  heroTitle: "Twórz i automatyzuj z wybranymi przez siebie modelami",
  heroIntro:
    "{brand} daje Ci agentów, którzy mogą tworzyć oprogramowanie, pracować z Twoimi plikami i zamieniać powtarzalne zadania w przepływy pracy, z których można korzystać wielokrotnie. Powiedz, co chcesz osiągnąć, i wybierz hostowane lub lokalne modele odpowiednie do zadania, z możliwością zmiany dostawców w trakcie pracy.",
  getCodewhale: "Pobierz Codewhale",
  heroInstallAria: "Polecenie instalacji",
  exploreProduct: "Poznaj produkt",
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
  gainHeading:
    "Co możesz zrobić z Codewhale",
  gainLede:
    "Zacznij od projektu, pytania albo zadania, które chcesz zautomatyzować, a następnie pracuj z jednym agentem lub przydziel części większej pracy kilku agentom.",
  gain: [
    [
      "Stwórz coś",
      "Opisz, co chcesz stworzyć, i pracuj z agentami, którzy mogą czytać Twój kod, edytować pliki, uruchamiać polecenia i sprawdzać wynik."
    ],
    [
      "Automatyzuj codzienną pracę",
      "Twórz skrypty i przepływy pracy dla powtarzających się zadań, aby móc ponownie uruchamiać je z terminala, gdy tylko będą potrzebne."
    ],
    [
      "Pracuj z różnymi modelami",
      "Korzystaj z hostowanych lub lokalnych modeli dla swoich agentów i dobieraj różne modele oraz role do odpowiednich części zadania."
    ]
  ],
  modelsHeading: "Wybór modeli do każdego zadania",
  modelsBody:
    "Połącz się bezpośrednio z dostawcą modeli hostowanych, użyj bramki, by korzystać z kilku dostawców, lub uruchom model lokalnie, a następnie wybieraj w trakcie pracy model dla każdej sesji.",
  modelsFacts: [
    ["Hostowane", "Twój własny klucz API zapisany przez codewhale auth set --provider <id>"],
    ["Bramka", "Jeden endpoint do wielu modeli, dostawcę nadal wybierasz Ty"],
    ["Lokalne", "vLLM, SGLang, Ollama na localhost — zwykle bez klucza"],
  ],
  modelsLink: "Poznaj modele i dostawców",
  startHeading: "Pierwsze kroki z Codewhale",
  startLede:
    "Po zainstalowaniu Codewhale i podłączeniu modelu możesz opisać pierwsze zadanie w terminalu, a gdy zechcesz rozdzielić pracę między kilku agentów, dodać Fleet.",
  startGuideLink: "Przeczytaj przewodnik na start",
  startVocabularyLink: "Zobacz słownik produktu",
  availabilityHeading: "Gdzie możesz korzystać z Codewhale",
  availabilityLede:
    "Możesz już dziś korzystać z Codewhale w terminalu, a my pracujemy nad aplikacją webową, aplikacją desktopową i komputerami w chmurze.",
  availability: [
    [
      "Terminal",
      "Wydany",
      "Gotowe pliki binarne z wydań GitHub dla systemów Linux, macOS i Windows; npm i Cargo to alternatywy. Android w Termux to wersja podglądowa."
    ],
    [
      "Aplikacja webowa",
      "Podgląd deweloperski",
      "Dostęp do konta i parowanie z przeglądarką w podglądzie deweloperskim."
    ],
    [
      "Aplikacja desktopowa",
      "Kompilacja deweloperska",
      "Aplikacja na macOS jest w przygotowaniu; publiczna wersja do pobrania pojawi się później."
    ],
    [
      "Komputery w chmurze",
      "W przygotowaniu",
      "Komputery w chmurze do wykonywania Twoich zadań."
    ]
  ],
  availabilityNote:
    "Możesz korzystać z terminala bez konta Codewhale, a opłaty za użycie modeli hostowanych nalicza Twój dostawca.",
  accountLink: "Załóż konto",
  surfacesHeading: "Sposoby pracy z Codewhale",
  surfaces: [
    ["TUI", "Interaktywna praca w terminalu"],
    ["codewhale exec", "Skrypty i CI"],
    ["Lokalny klient webowy","Interfejs na localhost; hostowane środowisko pracy w przeglądarce jest w przygotowaniu"],
    ["Runtime API + MCP", "Lokalne integracje"],
    ["Fleet","Kilku agentów przy jednym zadaniu"],
  ],
  runtimeLink: "Poznaj integracje",
  installBandHeading: "Zainstaluj Codewhale w systemie macOS lub Linux",
  copy: "Kopiuj",
  copied: "Skopiowano ✓",
  binaries: "Binarki",
  chinaMirrors: "Mirrory w Chinach",
  installGuideLink: "Przeczytaj przewodnik instalacji",
  communityHeading: "Pomóż ulepszać Codewhale",
  communityBody:
    "Niezależnie od tego, czy udało Ci się znaleźć błąd, masz pomysł na funkcję, czy chcesz przesłać swój pierwszy pull request, chętnie Cię wysłuchamy i wspólnie popracujemy nad dalszym rozwojem.",
  communityLinksAria: "Linki społeczności",
  contribute: "Wyślij pull request",
};
