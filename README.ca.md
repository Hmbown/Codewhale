<!-- source: README.md sha256:bbb8bfb61e57 -->
# Codewhale

Codewhale és un agent de codi obert que llegeix el teu projecte, edita fitxers, executa ordres i comprova la seva feina amb un model allotjat o local que tu tries. Comença amb una tasca al terminal. Per a una feina més gran, assigna parts de la feina a agents amb models i rols diferents.

![Codewhale executant-se en un terminal](web/public/codewhale-tui-d7a9a1c.png)

*Previsualització del terminal d’una compilació de desenvolupament de la v0.10.0.*

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja-JP.md) · [Tiếng Việt](README.vi.md) · [Bahasa Indonesia](README.id.md) · [한국어](README.ko-KR.md) · [Español](README.es-419.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md) · [Українська](README.uk.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [繁體中文](README.zh-TW.md) · [हिन्दी](README.hi.md) · [Türkçe](README.tr.md) · [Italiano](README.it.md) · [Polski](README.pl.md) · [العربية](README.ar.md)

[![CI](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml/badge.svg)](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/codewhale-cli?label=crates.io)](https://crates.io/crates/codewhale-cli)
[![npm](https://img.shields.io/npm/v/codewhale?label=npm)](https://www.npmjs.com/package/codewhale)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.gg/37gfS3ksug)

## Instal·lació

Per a una instal·lació nova a macOS o Linux, fes servir la versió oficial de GitHub:

```bash
curl -fsSL https://codewhale.net/install.sh | sh
"$HOME/.local/bin/codewhale"
```

L’instal·lador selecciona l’última versió publicada. El [registre de canvis](CHANGELOG.md) també descriu la versió candidata, encara no publicada, de la pròxima versió; aquests canvis no s’inclouen en les descàrregues publicades fins que la versió està disponible.

A Windows, descarrega l’instal·lador o l’arxiu corresponent de [GitHub Releases](https://github.com/Hmbown/CodeWhale/releases/latest). Per actualitzar una instal·lació directa existent, executa `codewhale update`, o `codewhale update --check` només per comprovar-la. L’actualitzador mostra el camí de l’executable i conserva les compilacions més noves. npm i Cargo són opcions secundàries; consulta la [guia d’instal·lació](docs/INSTALL.md) per migrar una instal·lació gestionada per paquets i configurar PATH.

La primera execució obre directament el compositor; no et guia per cap configuració. Les respostes del model requereixen un model allotjat o local connectat: fins que n’hi hagi un, la pantalla d’inici indica "no model connected". Executa `/provider` (o prem F3) per afegir una clau allotjada o triar un entorn local. Si Ollama ja s’està executant amb un model de xat, Codewhale hi canvia automàticament. Codewhale també admet npm i Cargo com a opcions secundàries de distribució, a més de Docker, Nix, Scoop, Android/Termux i un mirall CNB opcional. Les instal·lacions existents gestionades per paquets reben instruccions de migració. Consulta l’[ajuda d’instal·lació i PATH](docs/INSTALL.md).

L’autocompleció amb Tab s’activa amb una sola ordre per shell — `codewhale completion bash|zsh|fish|powershell|elvish`. Consulta [l’autocompleció del shell](docs/INSTALL.md#8-shell-completions).

## Ús

Obre un terminal a la carpeta del teu projecte i executa `codewhale`. Tria el proveïdor amb `/provider` i el model amb `/model`. Després, descriu una tasca concreta:

```text
Fix the failing tests and explain what changed.
```

També pots executar una tasca sense obrir la TUI:

```bash
codewhale exec "fix the failing tests and explain what changed"
```

Codewhale pot llegir el teu repositori, editar fitxers, executar ordres, inspeccionar els resultats i continuar treballant cap a un objectiu. Fes servir `/mode plan` per explorar sense modificar fitxers ni executar ordres del shell, i `/mode work` quan vulguis que faci canvis. Prem `Shift+Tab` per triar Ask, Auto-Review o Full Access; la [guia de modes i permisos](docs/MODES.md) explica què permet cada opció.

## Terminal, aplicacions i Computer Use

El terminal i els clients gràfics es connecten al Runtime de Codewhale, que executa l’agent i les seves eines:

- **Terminal:** `codewhale` obre la interfície interactiva; `codewhale exec` executa una tasca des d’un script o d’una feina de CI.
- **Navegador local:** `codewhale web` obre el [client web local](docs/WEB.md) inclòs, que fa servir el mateix runtime.
- **Aplicació d'escriptori Codewhale (GPUI):** una aplicació d'escriptori nativa, desenvolupada en un repositori separat, és la direcció del client de producte amb sessió iniciada. L'aplicació web allotjada a app.codewhale.net es reconstruirà a imatge seva; el lloc de màrqueting, l'inici de sessió, la facturació i les pàgines legals i de descàrrega es queden al web. La seva disponibilitat s'indica a la [pàgina del producte](https://codewhale.net/en/product).

**Computer Use afegeix eines per observar altres aplicacions i interactuar-hi.** El connector està inclòs en el codi font actual. Revisa l’accés que demana i activa’l abans de fer-lo servir; els permisos del sistema operatiu i els requisits de la plataforma continuen sent necessaris. Consulta la [guia de Computer Use](crates/tui/plugins/computer-use/README.md) inclosa i la [configuració de connectors](docs/PLUGINS.md).

Per al VS Code, l’extensió CodeWhale mantinguda per la comunitat es connecta al Runtime local des d’una barra lateral. Instal·la-la des del [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode); el codi font és a [GitHub](https://github.com/HengQuWorld/CodeWhale-VSCode).

## Per què Codewhale

- **Tria els teus models.** Connecta proveïdors allotjats o models locals mitjançant Ollama, vLLM o SGLang. Fes servir `/provider` per canviar de proveïdor i `/model` per triar un model.
- **Mantén el control.** Revisa les accions proposades i els canvis que produeixen als fitxers. La configuració d’aprovacions determina quan cal una revisió; Full Access continua respectant els límits obligatoris de les polítiques. `/undo` i `/restore` ajuden a recuperar canvis de l’espai de treball.
- **Mantén organitzades les feines llargues.** Desa sessions, defineix un `/goal` durador, revisa els fluxos de treball abans que s’executin i coordina agents sense convertir les seves instruccions internes en part de la teva conversa.
- **Amplia l’agent que ja tens.** Connecta servidors MCP i habilitats, configura hooks i conserva els rols d’agent com a fitxers llegibles al projecte o a la configuració personal.

Executa `/help` a la TUI per veure les ordres i les dreceres de teclat.

## Seguretat

Codewhale s’executa a la teva màquina amb l’accés que li concedeixes. Els modes d’aprovació i les regles del repositori limiten què pot fer l’agent; l’aïllament opcional del sistema operatiu afegeix un límit d’execució més sòlid allà on és compatible. Els preus desconeguts dels models continuen indicant-se com a desconeguts en lloc de presentar-se com a gratuïts.

Llegeix l’[ordre d’autorització](docs/AUTHORIZATION_ORDER.md) per conèixer la jerarquia exacta de polítiques i la [configuració](docs/CONFIGURATION.md) per als ajustos locals.

## Documentació

- [Proveïdors i models locals](docs/PROVIDERS.md)
- [Equips d’agents](docs/FLEET.md)
- [MCP](docs/MCP.md), [hooks](docs/HOOKS.md) i [configuració](docs/CONFIGURATION.md)
- [Client web local](docs/WEB.md)
- [Tota la documentació](docs/README.md)
- [Estructura del repositori i guia de contribució](CONTRIBUTING.md#project-structure)

## Uneix-te a la comunitat

**Els informes d’errors, les idees de funcionalitats i les pull requests són benvinguts**, tant si fa mesos que fas servir Codewhale com si el proves per primera vegada. Si falta un proveïdor, un flux de treball és incòmode o la interfície del terminal et dificulta la feina, [obre una incidència](https://github.com/Hmbown/CodeWhale/issues/new/choose) o [envia una pull request](CONTRIBUTING.md) perquè el puguem millorar plegats. Les primeres contribucions són benvingudes i qui hi contribueix conserva el reconeixement per la feina incorporada.

Uneix-te al [Discord](https://discord.gg/37gfS3ksug), o afegeix Hunter a WeChat (`hunterbown`) i demana entrar al grup Whale Brothers.

## Història del projecte

Codewhale va començar com a `deepseek-tui` i encara manté la compatibilitat amb la seva configuració i les seves sessions. Ara és neutral pel que fa als proveïdors, es manté de manera independent i no està afiliat a cap proveïdor de models.

Gràcies a totes les persones que hi han contribuït i a les comunitats de codi obert que han ajudat el projecte a créixer. Consulta el [registre de col·laboradors](docs/CONTRIBUTORS.md).

## Llicència

[MIT](LICENSE). Les parts adaptades d’altres projectes de codi obert consten als [avisos de tercers](docs/THIRD_PARTY_NOTICES.md).
