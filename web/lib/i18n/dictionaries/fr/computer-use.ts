import type { ComputerUseDict } from "../types";

/**
 * French dictionary for `app/[locale]/computer-use/page.tsx` and the
 * Computer Use section of the install page. Product names, the menu items
 * Pause, Stop and Check for updates, and the macOS setting names stay as
 * the app shows them. Vouvoiement, typographic apostrophe (’) and French
 * guillemets, in line with the other French dictionaries.
 */
export const computerUse: ComputerUseDict = {
  metaTitle: "Computer Use pour Mac · Codewhale",
  metaDescription: "Téléchargez et configurez Codewhale Computer Use pour Mac : contrôle des apps en arrière-plan, configuration des autorisations et commandes Pause et Stop à portée de main.",
  title: "Computer Use",
  lead: "Laissez Codewhale agir dans vos apps pendant que vous continuez à travailler. L’assistant Mac regroupe dans votre barre des menus les autorisations, le contrôle des apps en arrière-plan et de quoi mettre en pause ou arrêter la saisie.",
  publisher: "Par Codewhale",
  download: "Télécharger pour Mac",
  downloadZip: "Archive ZIP (utilisée par la mise à jour intégrée)",
  requirements: "macOS 13.5 ou ultérieur · Apple silicon et Intel",
  included: "Une seule app à télécharger. Aucune installation séparée de Node ni de compilateur.",
  pendingTitle: "Téléchargement Mac en préparation",
  pendingBody: "Le programme d’installation public apparaîtra ici une fois la notarisation Apple et les vérifications de publication terminées.",
  unavailableTitle: "Impossible de vérifier la disponibilité du téléchargement",
  unavailableBody: "Actualisez cette page pour réessayer, ou consultez les versions publiées ci-dessous.",
  releases: "Versions publiées",
  receipt: "Détails de vérification du téléchargement",
  setup: "Configurer votre Mac",
  steps: [
    { title: "Installer l’app", body: "Ouvrez l’image disque et faites glisser Codewhale Computer Use dans « Applications ». Lancez-la depuis « Applications », puis choisissez Computer Use dans l’icône de baleine de votre barre des menus." },
    { title: "Vérifier les autorisations", body: "Les boutons de configuration ouvrent « Accessibilité » et « Enregistrement de l’écran » dans Réglages Système. C’est vous qui décidez des autorisations à accorder." },
    { title: "Lancer la vérification en arrière-plan", body: "L’assistant ouvre une fenêtre d’essai temporaire, y saisit du texte et la capture. Il vérifie si le pointeur ou l’app active a changé pendant l’opération." },
    { title: "Connecter à Codewhale", body: "Examinez, approuvez et activez Computer Use dans le marketplace de plugins de Codewhale. Utilisez la version 0.3.1 ou ultérieure du plugin pour que les actions locales passent par les commandes Pause et Stop de l’assistant." },
  ],
  controlsTitle: "Continuez à travailler. Gardez le contrôle.",
  controlsBody: "Les actions prises en charge s’exécutent en arrière-plan dans l’app sélectionnée. Les apps et les gestes qui exigent le premier plan demandent votre autorisation. Le menu affiche la cible et le mode de saisie ; Pause suspend la saisie de l’assistant et Stop met fin à ses sessions en cours.",
  updateTitle: "Des mises à jour quand vous le décidez",
  updateBody: "Choisissez Check for updates (rechercher les mises à jour) dans l’app. Avant d’installer une mise à jour, l’assistant vérifie le téléchargement, la signature Codewhale et la notarisation Apple, et conserve la version précédente pour pouvoir la restaurer.",
  help: "Configuration et dépannage",
  notes: "Notes de version",
  demo: "Voir la vérification en arrière-plan",
  source: "Code source et autres plateformes",
  platforms: "Ce téléchargement est destiné au Mac. Sous Windows et Linux, on utilise pour l’instant le plugin depuis les sources et la configuration côté hôte.",
  installTitle: "Computer Use pour Mac",
  installLead: "Ajoutez le contrôle des apps en arrière-plan avec l’assistant Computer Use. Configurez les autorisations du Mac, lancez une vérification en arrière-plan, puis mettez en pause ou arrêtez la saisie de l’assistant depuis votre barre des menus.",
  installLink: "Téléchargement et configuration de Computer Use",
};
