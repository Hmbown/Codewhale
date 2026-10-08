import type { HomeDict } from "../types";

/**
 * French home dictionary — native copy for the whale-road landing page,
 * translated from the English reference: an open-source coding agent for
 * any model, control over approvals, and availability stated per surface
 * as it is today. Product vocabulary stays literal (Plan / Work / Operate,
 * Ask / Auto-Review / Full Access, Codewhale, codewhale exec, Fleet,
 * /receipts).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale : l’agent de code open source pour tous les modèles",
  metaDescription:
    "Codewhale est un agent de code open source pour votre terminal. Il lit votre projet, modifie des fichiers et exécute vos tests avec le modèle hébergé ou local de votre choix.",
  heroTitle: "L’agent de code open source pour tous les modèles",
  heroIntro:
    "{brand} lit votre projet, modifie des fichiers et exécute vos tests depuis votre terminal. Connectez un modèle hébergé ou local, et choisissez les actions qui demandent votre approbation.",
  getCodewhale: "Installer Codewhale",
  heroInstallAria: "Commande d'installation",
  exploreProduct: "Voir le fonctionnement",
  shotPreview: "Aperçu du terminal",
  shotBuild: "build de développement v{version}",
  screenshotAlt:
    "Version de développement Codewhale v{version} : baleine, nouvelle session, saisie du message, permissions Ask, mode Work et état du modèle. Rendu de la sortie réelle d’un terminal isolé.",
  latestRelease: "Dernière version {tag}",
  releaseUnavailable: "État des versions indisponible",
  currentSource: "Source",
  sourceCandidate: "Non publiée",
  publishedRelease: "publiée",
  figcaptionSourceCandidate: "non publiée",
  chapterTerminal: "Votre terminal",
  chapterTerminalTitle: "Suivez chaque modification et chaque commande pendant leur exécution",
  gainHeading: "Déléguez la tâche et gardez le contrôle",
  gainLede:
    "Demandez un résultat : corriger un bug, expliquer un module ou automatiser une tâche répétée. Commencez avec un agent, puis ajoutez-en d’autres quand le travail augmente.",
  gain: [
    [
      "Modifiez le code et vérifiez-le",
      "L’agent inspecte votre projet, modifie des fichiers et exécute vos tests. Suivez chaque modification et chaque résultat de commande pendant son travail."
    ],
    [
      "Automatisez le travail répétitif",
      "Exécutez codewhale exec depuis des scripts et la CI. Utilisez un Fleet pour répartir un travail plus important entre plusieurs agents."
    ],
    [
      "Gardez le contrôle",
      "Définissez les permissions avant le début du travail, répondez aux demandes d’approbation et arrêtez une tâche à tout moment. Exécutez /receipts pour lister chaque fichier, commande et approbation d’une session."
    ]
  ],
  chapterModels: "Vos modèles",
  modelsHeading: "Choisissez un modèle pour chaque tâche",
  modelsBody:
    "Pour chaque session, choisissez un fournisseur intégré, n’importe quel endpoint compatible OpenAI ou un modèle local. Votre connexion au modèle reste séparée de tout compte Codewhale.",
  modelsFacts: [
    ["Hébergé", "Votre propre clé d’API, enregistrée avec codewhale auth set --provider <id>"],
    ["Passerelle", "Un seul endpoint pour de nombreux modèles ; vous choisissez toujours le fournisseur"],
    ["Local", "vLLM, SGLang ou Ollama sur localhost, généralement sans clé"],
  ],
  modelsLink: "Parcourir les modèles et les fournisseurs",
  startHeading: "Installer, connecter un modèle, lancer une tâche",
  startLede:
    "Lancez votre première tâche en trois étapes depuis le dossier de votre projet. Ajoutez un Fleet plus tard si le travail nécessite plusieurs agents.",
  startGuideLink: "Suivre le guide de démarrage",
  startVocabularyLink: "Voir le vocabulaire du produit",
  chapterAvailability: "Où l’utiliser",
  availabilityHeading: "Utilisez Codewhale dans votre terminal dès aujourd’hui",
  availabilityLede:
    "Utilisez dès maintenant le terminal, le client de navigateur local ou l’interface CodeWhale GUI de la communauté. L’application de bureau et l’application web hébergée reconstruite sont en développement et partagent le même modèle de session.",
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
    "Le terminal, le navigateur local et la GUI ne nécessitent aucun compte Codewhale. Le web hébergé et l’application de bureau utilisent un compte, qui ne remplace pas votre connexion au modèle ; votre fournisseur facture l’utilisation sur votre propre clé.",
  accountLink: "Créer un compte",
  surfacesHeading: "Étendez ce à quoi l’agent a accès",
  surfaces: [
    ["Fichiers et commandes", "Lisez le projet, modifiez des fichiers, exécutez des tests et examinez la sortie dans les limites des permissions que vous définissez."],
    ["Plugins et MCP", "Connectez d’autres outils et services. Chaque plugin reste désactivé jusqu’à ce que vous l’examiniez et l’activiez."],
    ["Computer Use · aperçu", "Un plugin qui permet à l’agent de voir et d’utiliser d’autres applications. Vous l’activez et accordez les permissions système qu’il demande."],
    ["Sessions enregistrées", "Gardez ensemble la conversation et les résultats des outils, et reprenez votre travail au lieu de recommencer. Le navigateur local ouvre la même session sur votre ordinateur."],
    ["Fleet", "Attribuez des parties d’une tâche à des agents dotés de modèles et de rôles différents, puis suivez leur progression."],
  ],
  runtimeLink: "Voir toutes les intégrations",
  installBandHeading: "Installer sur macOS ou Linux",
  copy: "Copier",
  copied: "Copié ✓",
  binaries: "Binaires",
  chinaMirrors: "Miroirs en Chine",
  installGuideLink: "Lire le guide d’installation",
  communityHeading: "Construisez Codewhale avec nous",
  communityBody:
    "Signalez un bug, proposez une fonctionnalité ou envoyez votre première pull request sur GitHub. Les petits correctifs testés sont les bienvenus.",
  communityLinksAria: "Liens de la communauté",
  contribute: "Envoyer une pull request",
};
