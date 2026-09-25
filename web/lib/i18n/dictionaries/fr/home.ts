import type { HomeDict } from "../types";

/**
 * French home dictionary — native copy for the Tidal Folio landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale — Créez et automatisez avec les modèles de votre choix",
  metaDescription:
    "Créez des logiciels, travaillez sur vos fichiers et automatisez les tâches du quotidien avec des agents open source et les modèles d’IA hébergés ou locaux de votre choix.",
  heroTitle: "Créez et automatisez avec les modèles de votre choix",
  heroIntro:
    "{brand} vous donne des agents capables de créer des logiciels, de travailler sur vos fichiers et de transformer les tâches répétitives en workflows réutilisables. Dites-leur ce que vous voulez accomplir et choisissez les modèles hébergés ou locaux adaptés au travail, avec la liberté de changer de fournisseur en cours de route.",
  getCodewhale: "Obtenir Codewhale",
  heroInstallAria: "Commande d'installation",
  exploreProduct: "Découvrir le produit",
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
  gainHeading:
    "Ce que vous pouvez faire avec Codewhale",
  gainLede:
    "Commencez par un projet, une question ou une tâche à automatiser, puis travaillez avec un agent ou répartissez un travail plus important entre plusieurs agents.",
  gain: [
    [
      "Créez quelque chose",
      "Décrivez ce que vous voulez créer et travaillez avec des agents capables de lire votre code, de modifier des fichiers, d’exécuter des commandes et de vérifier le résultat."
    ],
    [
      "Automatisez le travail du quotidien",
      "Créez des scripts et des workflows pour les tâches récurrentes, afin de pouvoir les relancer depuis le terminal chaque fois que vous en avez besoin."
    ],
    [
      "Travaillez avec différents modèles",
      "Utilisez des modèles hébergés ou locaux pour vos agents, avec différents modèles et rôles pour les parties du travail auxquelles ils sont adaptés."
    ]
  ],
  modelsHeading: "Un choix de modèles pour chaque tâche",
  modelsBody:
    "Connectez-vous directement à un fournisseur de modèles hébergés, passez par une passerelle pour accéder à plusieurs fournisseurs ou exécutez un modèle en local, puis choisissez le modèle utilisé par chaque session au fil de votre travail.",
  modelsFacts: [
    ["Hébergé", "Votre propre clé d’API, enregistrée avec codewhale auth set --provider <id>"],
    ["Passerelle", "Un seul endpoint pour de nombreux modèles, le fournisseur reste votre choix"],
    ["Local", "vLLM, SGLang, Ollama sur localhost — généralement sans clé"],
  ],
  modelsLink: "Explorer les modèles et les fournisseurs",
  startHeading: "Premiers pas avec Codewhale",
  startLede:
    "Une fois Codewhale installé et un modèle connecté, vous pouvez décrire votre première tâche dans le terminal et ajouter un Fleet lorsque vous souhaitez répartir le travail entre plusieurs agents.",
  startGuideLink: "Lire le guide de démarrage",
  startVocabularyLink: "Voir le vocabulaire du produit",
  availabilityHeading: "Où vous pouvez utiliser Codewhale",
  availabilityLede:
    "Vous pouvez utiliser Codewhale dans votre terminal dès aujourd’hui, pendant que nous développons l’application web, l’application de bureau et les ordinateurs cloud.",
  availability: [
    [
      "Terminal",
      "Publié",
      "Binaires des versions publiées sur GitHub pour Linux, macOS et Windows ; npm et Cargo sont des alternatives. Android sous Termux est disponible en aperçu."
    ],
    [
      "Application web",
      "Aperçu de développement",
      "Accès au compte et association avec le navigateur dans l’aperçu de développement."
    ],
    [
      "Bureau",
      "Build de développement",
      "L’application macOS est en développement ; un téléchargement public sera proposé ultérieurement."
    ],
    [
      "Ordinateurs cloud",
      "En développement",
      "Des ordinateurs hébergés pour exécuter vos tâches."
    ]
  ],
  availabilityNote:
    "Vous pouvez utiliser le terminal sans compte Codewhale, et toute utilisation de modèles hébergés est facturée par votre fournisseur.",
  accountLink: "Créer un compte",
  surfacesHeading: "Différentes façons de travailler avec Codewhale",
  surfaces: [
    ["TUI", "Travail interactif dans le terminal"],
    ["codewhale exec", "Scripts et CI"],
    ["Client web local","Interface sur localhost ; espace de travail web hébergé en développement"],
    ["Runtime API + MCP", "Intégrations locales"],
    ["Fleet","Plusieurs agents sur une même tâche"],
  ],
  runtimeLink: "Explorer les intégrations",
  installBandHeading: "Installez Codewhale sur macOS ou Linux",
  copy: "Copier",
  copied: "Copié ✓",
  binaries: "Binaires",
  chinaMirrors: "Miroirs en Chine",
  installGuideLink: "Lire le guide d’installation",
  communityHeading: "Aidez à améliorer Codewhale",
  communityBody:
    "Que vous ayez trouvé un bug, que vous ayez une idée de fonctionnalité ou que vous souhaitiez envoyer votre première pull request, nous aimerions échanger avec vous et travailler ensemble sur la suite.",
  communityLinksAria: "Liens de la communauté",
  contribute: "Envoyer une pull request",
};
