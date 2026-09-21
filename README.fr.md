<!-- source: README.md sha256:29c349b6f2b4 -->
# Codewhale

Codewhale est un agent open source qui lit votre projet, modifie des fichiers, exécute des commandes et vérifie son travail avec un modèle hébergé ou local de votre choix. Commencez par une tâche dans votre terminal. Pour un travail plus important, confiez-en des parties à des agents utilisant différents modèles et rôles.

![Codewhale en cours d’exécution dans un terminal](web/public/codewhale-tui-d7a9a1c.png)

*Aperçu du terminal dans une version de développement de la v0.10.0.*

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja-JP.md) · [Tiếng Việt](README.vi.md) · [Bahasa Indonesia](README.id.md) · [한국어](README.ko-KR.md) · [Español](README.es-419.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md) · [Українська](README.uk.md) · [Deutsch](README.de.md) · [繁體中文](README.zh-TW.md) · [हिन्दी](README.hi.md) · [Türkçe](README.tr.md) · [Italiano](README.it.md) · [Polski](README.pl.md) · [العربية](README.ar.md) · [Català](README.ca.md)

[![CI](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml/badge.svg)](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/codewhale-cli?label=crates.io)](https://crates.io/crates/codewhale-cli)
[![npm](https://img.shields.io/npm/v/codewhale?label=npm)](https://www.npmjs.com/package/codewhale)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.gg/37gfS3ksug)

## Installation

Pour une nouvelle installation sur macOS ou Linux, utilisez la version officielle de GitHub :

```bash
curl -fsSL https://codewhale.net/install.sh | sh
"$HOME/.local/bin/codewhale"
```

L’installeur sélectionne la dernière version publiée. Le [journal des modifications](CHANGELOG.md) décrit aussi la version candidate, encore non publiée, de la prochaine version ; ces modifications ne sont incluses dans les téléchargements publiés qu’une fois la version disponible.

Sur Windows, téléchargez l’installeur ou l’archive adaptés depuis [GitHub Releases](https://github.com/Hmbown/CodeWhale/releases/latest). Pour une installation directe existante, lancez `codewhale update`, ou `codewhale update --check` pour vérifier sans installer. L’outil affiche le chemin de l’exécutable et conserve les versions de développement plus récentes. npm et Cargo sont des options secondaires ; consultez le [guide d’installation](docs/INSTALL.md) pour migrer depuis un gestionnaire de paquets et configurer PATH.

Au premier lancement, Codewhale vous aide à connecter un fournisseur ou à configurer Codewhale hors ligne. Les réponses nécessitent un modèle hébergé ou local connecté. Codewhale prend aussi en charge npm et Cargo comme options de distribution secondaires, ainsi que Docker, Nix, Scoop, Android/Termux et un miroir CNB facultatif. Les installations existantes gérées par un gestionnaire de paquets reçoivent des instructions de migration. Consultez l’[aide à l’installation et à la configuration du PATH](docs/INSTALL.md).

L’autocomplétion avec Tab s’active avec une commande par shell — `codewhale completion bash|zsh|fish|powershell|elvish`. Consultez [l’autocomplétion du shell](docs/INSTALL.md#8-shell-completions).

## Utilisation

Ouvrez un terminal dans le dossier de votre projet et lancez `codewhale`. Choisissez votre fournisseur avec `/provider` et votre modèle avec `/model`. Décrivez ensuite une tâche concrète :

```text
Fix the failing tests and explain what changed.
```

Vous pouvez aussi exécuter une tâche sans ouvrir la TUI :

```bash
codewhale exec "fix the failing tests and explain what changed"
```

Codewhale peut lire votre dépôt, modifier des fichiers, exécuter des commandes, inspecter les résultats et continuer à travailler vers un objectif. Utilisez `/mode plan` pour explorer sans modifier de fichiers ni exécuter de commandes shell, et `/mode work` lorsque vous souhaitez qu’il effectue des modifications. Appuyez sur `Shift+Tab` pour choisir Ask, Auto-Review ou Full Access ; le [guide des modes et des permissions](docs/MODES.md) explique ce que chaque option autorise.

## Terminal, applications et Computer Use

Le terminal et les clients graphiques se connectent au Runtime Codewhale, qui exécute l’agent et ses outils :

- **Terminal :** `codewhale` ouvre l’interface interactive ; `codewhale exec` exécute une tâche depuis un script ou une tâche de CI.
- **Navigateur local :** `codewhale web` ouvre le [client web local](docs/WEB.md) fourni, qui utilise le même runtime.
- **Application de bureau Codewhale (GPUI) :** l'application de bureau native GPUI est l'orientation du client produit (décision du 2026-09-14 ; la carte des phases est dans docs/TRANSITION.md du dépôt privé codehwhale-gpui). L'application web hébergée sur app.codewhale.net est retirée par étapes ; le site marketing, la connexion, la facturation, les pages légales et de téléchargement restent sur le web de façon permanente. La disponibilité est indiquée sur la [page du produit](https://codewhale.net/en/product).

**Computer Use ajoute des outils pour observer d’autres applications et interagir avec elles.** Le plugin est inclus dans le code source actuel. Examinez les accès demandés et activez-le avant de l’utiliser ; les permissions du système d’exploitation et les exigences de la plateforme s’appliquent toujours. Consultez le [guide Computer Use](crates/tui/plugins/computer-use/README.md) inclus et la [configuration des plugins](docs/PLUGINS.md).

Pour VS Code, l’extension CodeWhale maintenue par la communauté se connecte au Runtime local depuis une barre latérale. Installez-la depuis le [marketplace VS Code](https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode) ; le code source est sur [GitHub](https://github.com/HengQuWorld/CodeWhale-VSCode).

## Pourquoi Codewhale

- **Choisissez vos modèles.** Connectez des fournisseurs hébergés ou des modèles locaux via Ollama, vLLM ou SGLang. Utilisez `/provider` pour changer de fournisseur et `/model` pour choisir un modèle.
- **Gardez le contrôle.** Examinez les actions proposées et les modifications de fichiers qui en résultent. Les paramètres d’approbation déterminent quand un examen est nécessaire ; Full Access respecte toujours les limites impératives des politiques. `/undo` et `/restore` aident à récupérer les modifications de l’espace de travail.
- **Organisez les travaux de longue durée.** Enregistrez les sessions, définissez un `/goal` durable, examinez les workflows avant leur exécution et coordonnez des agents sans faire apparaître leurs instructions internes dans votre conversation.
- **Étendez l’agent que vous possédez déjà.** Connectez des serveurs MCP et des compétences, configurez des hooks et conservez les rôles d’agent sous forme de fichiers lisibles dans votre projet ou vos paramètres personnels.

Exécutez `/help` dans la TUI pour afficher les commandes et les raccourcis clavier.

## Sécurité

Codewhale s’exécute sur votre machine avec les accès que vous lui accordez. Les modes d’approbation et les règles du dépôt limitent les actions de l’agent ; un bac à sable facultatif du système d’exploitation renforce la limite d’exécution lorsqu’il est pris en charge. Le prix d’un modèle inconnu reste indiqué comme tel au lieu d’être présenté comme gratuit.

Consultez l’[ordre d’autorisation](docs/AUTHORIZATION_ORDER.md) pour connaître la hiérarchie exacte des politiques et la [configuration](docs/CONFIGURATION.md) pour les paramètres locaux.

## Documentation

- [Fournisseurs et modèles locaux](docs/PROVIDERS.md)
- [Équipes d’agents](docs/FLEET.md)
- [MCP](docs/MCP.md), [hooks](docs/HOOKS.md) et [configuration](docs/CONFIGURATION.md)
- [Client web local](docs/WEB.md)
- [Toute la documentation](docs)
- [Organisation du dépôt et guide de contribution](CONTRIBUTING.md#project-structure)

## Rejoindre la communauté

**Les signalements de bugs, les idées de fonctionnalités et les pull requests sont les bienvenus**, que vous utilisiez Codewhale depuis des mois ou que vous l’essayiez pour la première fois. S’il manque un fournisseur, si un workflow est peu pratique ou si l’interface du terminal vous gêne, [ouvrez une issue](https://github.com/Hmbown/CodeWhale/issues/new/choose) ou [envoyez une pull request](CONTRIBUTING.md) pour que nous puissions l’améliorer ensemble. Les premières contributions sont les bienvenues, et les personnes qui contribuent restent créditées pour le travail intégré.

Rejoignez le [Discord](https://discord.gg/37gfS3ksug), ou ajoutez Hunter sur WeChat (`hunterbown`) et demandez à rejoindre le groupe Whale Brothers.

## Historique du projet

Codewhale a commencé sous le nom de `deepseek-tui` et conserve la compatibilité avec sa configuration et ses sessions. Il est désormais indépendant de tout fournisseur, maintenu de manière autonome et n’est affilié à aucun fournisseur de modèles.

Merci à toutes les personnes qui contribuent et aux communautés open source qui ont aidé le projet à grandir. Consultez le [registre des contributeurs](docs/CONTRIBUTORS.md).

## Licence

[MIT](LICENSE). Les parties adaptées d’autres projets open source sont répertoriées dans les [mentions relatives aux logiciels tiers](docs/THIRD_PARTY_NOTICES.md).
