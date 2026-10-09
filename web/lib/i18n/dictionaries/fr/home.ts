import type { HomeDict } from "../types";

/** French home copy: useful work with chosen models and connected tools. */

export const home: HomeDict = {
  metaTitle: "Codewhale : créez avec vos modèles et vos outils",
  metaDescription:
    "Créez des applications, automatisez vos tâches et utilisez vos outils connectés avec Codewhale. Un logiciel libre, avec vos API de modèles ou votre inférence locale et auto-hébergée.",
  heroTitle: "Créez des applications et automatisez votre travail.",
  heroIntro:
    "{brand} est un agent open source qui écrit du code, exécute des commandes et utilise les outils que vous connectez. Utilisez vos API de modèles habituelles, ou exécutez des modèles en local et sur vos propres serveurs.",
  getCodewhale: "Installer Codewhale",
  heroInstallAria: "Commande d'installation",
  exploreProduct: "Découvrir Codewhale",
  shotPreview: "Aperçu du terminal",
  screenshotAlt:
    "Codewhale v{version} : capture du terminal avec la conversation, le champ de message et les commandes de session.",
  latestRelease: "Dernière version {tag}",
  releaseUnavailable: "État des versions indisponible",
  currentSource: "Source",
  sourceCandidate: "Non publiée",
  publishedRelease: "publiée",
  gainHeading: "Ce que vous pouvez faire",
  gainLede:
    "Décrivez ce que vous voulez créer ou automatiser. Codewhale peut modifier des fichiers, exécuter des commandes et vérifier le résultat, avec des accès que vous contrôlez.",
  gain: [
    [
      "Créez des applications et des outils",
      "Créez une application, ajoutez une fonctionnalité ou écrivez un script. Codewhale peut travailler sur les fichiers du projet, exécuter le code et tester ce qu’il crée."
    ],
    [
      "Automatisez les tâches répétitives",
      "Exécutez des workflows depuis votre terminal, vos scripts ou votre CI. Pour les tâches plus importantes, déléguez une partie du travail à une Fleet d’agents utilisant différents modèles."
    ],
    [
      "Connectez vos outils habituels",
      "Ajoutez des outils via des plugins et des serveurs MCP, ou utilisez des API depuis vos propres scripts. Chaque service nécessite sa propre configuration et authentification."
    ]
  ],
  chapterModels: "Vos modèles",
  modelsHeading: "Utilisez les modèles de votre choix",
  modelsBody:
    "Connectez vos comptes fournisseurs, un point de terminaison compatible avec OpenAI, ou des modèles locaux et auto-hébergés. Choisissez un modèle pour la session et pour chaque agent d’une Fleet.",
  modelsFacts: [
    [
      "Vos comptes API",
      "Connectez OpenAI, Anthropic, Google ou DeepSeek avec vos propres clés."
    ],
    [
      "Votre passerelle",
      "Utilisez un endpoint compatible avec OpenAI et choisissez les modèles qu’il propose."
    ],
    [
      "Votre inférence",
      "Exécutez des modèles locaux ou auto-hébergés avec Ollama, vLLM ou SGLang."
    ]
  ],
  modelsLink: "Parcourir les modèles et les fournisseurs",
  startHeading: "Premiers pas",
  startLede:
    "Installez Codewhale, connectez un modèle et ouvrez un dossier de projet. Ajoutez des plugins et d’autres agents selon vos besoins.",
  startGuideLink: "Suivre le guide de démarrage",
  startVocabularyLink: "Voir le vocabulaire du produit",
  chapterAvailability: "Où l’utiliser",
  availabilityHeading: "Ce qui est disponible et en développement",
  availabilityLede:
    "Le terminal et le client de navigateur local sont disponibles. L’application native de bureau et la nouvelle application web hébergée sont en développement.",
  availability: [
    [
      "Terminal et navigateur local",
      "Publié",
      "Installez-le sur Linux, macOS ou Windows, puis lancez codewhale, ou codewhale web pour le client de navigateur local. npm et Cargo fonctionnent aussi ; Android sous Termux est disponible en aperçu."
    ],
    [
      "CodeWhale GUI (VS Code)",
      "Disponible",
      "Un projet distinct, maintenu par la communauté : chat, fils et modifications de fichiers dans une barre latérale VS Code, sur le même Codewhale Runtime. Installez-la depuis le VS Code Marketplace.",
      "https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode"
    ],
    [
      "Application web hébergée",
      "Aperçu de développement",
      "En cours de reconstruction pour correspondre à l’application de bureau. Aujourd’hui, vous pouvez vous connecter, puis saisir /rc dans une session de terminal en cours pour la poursuivre sur le web ; l’exécution de tâches hébergées est encore en cours de qualification."
    ],
    [
      "Bureau",
      "Build de développement",
      "L’application native qui devient le client principal de Codewhale : dossiers, conversations et connexions de modèles dans une seule fenêtre. Aucun téléchargement public n’est encore disponible."
    ],
    [
      "Ordinateurs cloud",
      "En développement",
      "Des ordinateurs hébergés qui exécutent vos tâches."
    ]
  ],
  availabilityNote:
    "Le terminal, le navigateur local et la GUI ne nécessitent aucun compte Codewhale. Le web hébergé et l’application de bureau utilisent un compte. Si vous utilisez votre propre clé fournisseur, celui-ci vous facture cet usage.",
  accountLink: "Créer un compte",
  surfacesHeading: "Travaillez avec vos fichiers et vos outils",
  surfaces: [
    [
      "Fichiers et terminal",
      "Créez des fichiers, exécutez des commandes, analysez des données et testez vos réalisations. Vous définissez le dossier de travail et les permissions."
    ],
    [
      "Plugins et applications connectées",
      "Ajoutez des compétences et des outils via les plugins et MCP. Vérifiez et activez les connexions que l’agent peut utiliser."
    ],
    [
      "Navigateur et ordinateur · aperçu",
      "Utilisez les outils de navigation et le plugin Computer Use dans les applications et les sites, avec les accès que vous accordez."
    ],
    [
      "Des sessions à reprendre",
      "Conservez ensemble la conversation, les résultats des outils et l’historique. Reprenez la tâche dans le terminal ou son client de navigateur local."
    ],
    [
      "Équipes d’agents",
      "Répartissez un travail important entre des agents aux rôles et modèles différents avec Fleet, et suivez leur progression au même endroit."
    ]
  ],
  runtimeLink: "Découvrir les outils et intégrations",
  installBandHeading: "Installer sur macOS ou Linux",
  copy: "Copier",
  copied: "Copié ✓",
  binaries: "Binaires",
  chinaMirrors: "Miroirs en Chine",
  installGuideLink: "Lire le guide d’installation",
  communityHeading: "Contribuez à Codewhale",
  communityBody:
    "Signalez un bug, améliorez la documentation ou contribuez au code sur GitHub. Vous pouvez aussi créer des plugins et partager des workflows avec d’autres utilisateurs.",
  communityLinksAria: "Liens de la communauté",
  contribute: "Contribuer sur GitHub",
};
