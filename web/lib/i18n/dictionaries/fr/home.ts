import type { HomeDict } from "../types";

/** French home copy: useful work with chosen models and connected tools. */

export const home: HomeDict = {
  metaTitle: "Codewhale : créez avec vos modèles et vos outils",
  metaDescription:
    "Créez des applications, automatisez vos tâches et utilisez vos outils connectés avec Codewhale. Un logiciel libre, avec vos API de modèles ou votre inférence locale et auto-hébergée.",
  heroTitle: "Donnez forme à vos idées.",
  heroIntro:
    "Créez une application, automatisez une tâche ou tirez quelque chose d’utile de vos recherches. {brand} utilise vos API de modèles habituelles, votre propre inférence et les outils que vous connectez.",
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
  gainHeading: "Créez quelque chose d’utile.",
  gainLede:
    "Partez du résultat souhaité. Codewhale travaille avec les fichiers, les commandes et les outils connectés ; vous choisissez les accès et les approbations.",
  gain: [
    [
      "Créez des applications et des outils",
      "Transformez une idée en application fonctionnelle, en script utile ou en fonctionnalité pour un projet existant. L’agent écrit, exécute et teste avec vous."
    ],
    [
      "Automatisez les tâches répétitives",
      "Transformez une tâche récurrente en workflow pour votre terminal, vos scripts ou votre CI. Faites appel à Fleet lorsque plusieurs agents peuvent travailler en parallèle."
    ],
    [
      "Connectez vos outils habituels",
      "Connectez des outils comme Gmail et Slack via des plugins, des serveurs MCP ou des API. Utilisez ces services avec vos fichiers et vos commandes."
    ]
  ],
  exampleTasks: [
    "Crée une application de prise de rendez-vous.",
    "Transforme un CSV de ventes en un rapport hebdomadaire que je peux régénérer.",
    "Transforme les messages de ma messagerie connectée en liste de tâches.",
  ],
  // A static example report built from local sample orders.
  reportTitle: "Rapport de ventes hebdomadaire",
  reportSampleLabel: "Exemple de rapport · données fictives",
  reportDescription: "Demandez à Codewhale de regrouper les commandes par semaine et de conserver le processus pour le prochain CSV.",
  reportSourceLabel: "Données sources :",
  reportColumns: ["Début de semaine","Commandes","Ventes (USD)"],
  reportTotalLabel: "Total",
  reportTrend: "Évolution des ventes, de la première semaine à la dernière : {change}.",
  reportDownloadLabel: "Télécharger le rapport CSV",
  chapterModels: "Vos modèles",
  modelsHeading: "Gardez les modèles de votre choix.",
  modelsBody:
    "Connectez vos API de modèles habituelles, utilisez une passerelle compatible ou exécutez l’inférence sur votre matériel. Choisissez un modèle par session et pour chaque agent d’un Fleet.",
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
  startHeading: "Apportez une tâche. Lancez-vous.",
  startLede:
    "Installez Codewhale, connectez un modèle et confiez-lui une tâche utile. Commencez avec un agent ; ajoutez des outils ou une équipe selon vos besoins.",
  startGuideLink: "Suivre le guide de démarrage",
  startVocabularyLink: "Voir le vocabulaire du produit",
  chapterAvailability: "Où l’utiliser",
  availabilityHeading: "Commencez dans le terminal.",
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
  surfacesHeading: "Une tâche. Vos fichiers, applications et agents.",
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
  runtimeLink: "Voir toutes les intégrations",
  installBandHeading: "Installer sur macOS ou Linux",
  copy: "Copier",
  copied: "Copié ✓",
  binaries: "Binaires",
  chinaMirrors: "Miroirs en Chine",
  installGuideLink: "Lire le guide d’installation",
  communityHeading: "Faites de Codewhale votre outil.",
  communityBody:
    "Codewhale est un logiciel libre. Lisez le code, créez un plugin, partagez un workflow ou contribuez à améliorer la prochaine version.",
  communityLinksAria: "Liens de la communauté",
  contribute: "Contribuer sur GitHub",
};
