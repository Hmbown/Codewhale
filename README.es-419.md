<!-- source: README.md sha256:bbb8bfb61e57 -->
# Codewhale

Codewhale es un agente de código abierto que lee tu proyecto, edita archivos, ejecuta comandos y comprueba su trabajo con un modelo alojado o local que tú eliges. Empieza con una tarea en la terminal. Para un trabajo más grande, asigna partes del trabajo a agentes con distintos modelos y roles.

![Codewhale ejecutándose en una terminal](web/public/codewhale-tui-d7a9a1c.png)

*Vista previa de la terminal de una compilación de desarrollo de v0.10.0.*

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja-JP.md) · [Tiếng Việt](README.vi.md) · [Bahasa Indonesia](README.id.md) · [한국어](README.ko-KR.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md) · [Українська](README.uk.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [繁體中文](README.zh-TW.md) · [हिन्दी](README.hi.md) · [Türkçe](README.tr.md) · [Italiano](README.it.md) · [Polski](README.pl.md) · [العربية](README.ar.md) · [Català](README.ca.md)

[![CI](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml/badge.svg)](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/codewhale-cli?label=crates.io)](https://crates.io/crates/codewhale-cli)
[![npm](https://img.shields.io/npm/v/codewhale?label=npm)](https://www.npmjs.com/package/codewhale)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.gg/37gfS3ksug)

## Instalación

Para una instalación nueva en macOS o Linux, usa la versión oficial de GitHub:

```bash
curl -fsSL https://codewhale.net/install.sh | sh
"$HOME/.local/bin/codewhale"
```

El instalador selecciona la última versión publicada. El [registro de cambios](CHANGELOG.md) también describe la versión candidata aún no publicada de la próxima versión; esos cambios no se incluyen en las descargas publicadas hasta que la versión esté disponible.

En Windows, descarga el instalador o archivo correspondiente de [GitHub Releases](https://github.com/Hmbown/CodeWhale/releases/latest). Para actualizar una instalación directa existente, ejecuta `codewhale update`, o `codewhale update --check` para consultar sin instalar. El actualizador muestra la ruta del ejecutable y conserva las compilaciones más recientes. npm y Cargo son opciones secundarias; consulta la [guía de instalación](docs/INSTALL.md) para migrar desde un gestor de paquetes y configurar PATH.

La primera ejecución abre directamente el compositor; no te guía por ninguna configuración. Las respuestas del modelo requieren un modelo alojado o local conectado: mientras no haya uno, la pantalla de inicio indica "no model connected". Ejecuta `/provider` (o presiona F3) para agregar una clave alojada o elegir un entorno local. Si Ollama ya se está ejecutando con un modelo de chat, Codewhale cambia a él por sí solo. Codewhale también admite npm y Cargo como opciones secundarias de distribución, además de Docker, Nix, Scoop, Android/Termux y un espejo opcional de CNB. Las instalaciones existentes gestionadas por paquetes reciben instrucciones de migración. Consulta la [ayuda de instalación y PATH](docs/INSTALL.md).

El completado con Tab se configura con un comando por shell — `codewhale completion bash|zsh|fish|powershell|elvish`. Consulta el [completado de shell](docs/INSTALL.md#8-shell-completions).

## Uso

Abre una terminal en la carpeta de tu proyecto y ejecuta `codewhale`. Elige tu proveedor con `/provider` y tu modelo con `/model`. Después, describe una tarea concreta:

```text
Fix the failing tests and explain what changed.
```

También puedes ejecutar una tarea sin abrir la TUI:

```bash
codewhale exec "fix the failing tests and explain what changed"
```

Codewhale puede leer tu repositorio, editar archivos, ejecutar comandos, revisar los resultados y seguir trabajando para alcanzar un objetivo. Usa `/mode plan` para explorar sin modificar archivos ni ejecutar comandos de shell, y `/mode work` cuando quieras que haga cambios. Presiona `Shift+Tab` para elegir Ask, Auto-Review o Full Access; la [guía de modos y permisos](docs/MODES.md) explica qué permite cada opción.

## Terminal, aplicaciones y Computer Use

La terminal y los clientes gráficos se conectan al Runtime de Codewhale, que ejecuta el agente y sus herramientas:

- **Terminal:** `codewhale` abre la interfaz interactiva; `codewhale exec` ejecuta una tarea desde un script o un trabajo de CI.
- **Navegador local:** `codewhale web` abre el [cliente web local](docs/WEB.md) incluido, que usa el mismo runtime.
- **Aplicación de escritorio Codewhale (GPUI):** una aplicación de escritorio nativa, desarrollada en un repositorio separado, es la dirección del cliente de producto con sesión iniciada. La aplicación web alojada en app.codewhale.net se reconstruirá a su imagen; el sitio de marketing, el inicio de sesión, la facturación y las páginas legales y de descarga permanecen en la web. Su disponibilidad se indica en la [página del producto](https://codewhale.net/en/product).

**Computer Use agrega herramientas para observar otras aplicaciones e interactuar con ellas.** El plugin está incluido en el código fuente actual. Revisa el acceso que solicita y habilítalo antes de usarlo; los permisos del sistema operativo y los requisitos de la plataforma siguen siendo necesarios. Consulta la [guía de Computer Use](crates/tui/plugins/computer-use/README.md) incluida y la [configuración de plugins](docs/PLUGINS.md).

Para VS Code, la extensión CodeWhale mantenida por la comunidad se conecta al Runtime local desde una barra lateral. Instálala desde el [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode); el código fuente está en [GitHub](https://github.com/HengQuWorld/CodeWhale-VSCode).

## Por qué Codewhale

- **Elige tus modelos.** Conecta proveedores alojados o modelos locales mediante Ollama, vLLM o SGLang. Usa `/provider` para cambiar de proveedor y `/model` para elegir un modelo.
- **Mantén el control.** Revisa las acciones propuestas y los cambios que producen en los archivos. La configuración de aprobaciones determina cuándo se necesita una revisión; Full Access sigue respetando los límites obligatorios de las políticas. `/undo` y `/restore` ayudan a recuperar cambios del espacio de trabajo.
- **Mantén organizado el trabajo de larga duración.** Guarda sesiones, establece un `/goal` duradero, revisa los flujos de trabajo antes de ejecutarlos y coordina agentes sin convertir sus instrucciones internas en parte de tu conversación.
- **Amplía el agente que ya tienes.** Conecta servidores MCP y habilidades, configura hooks y conserva los roles de los agentes como archivos legibles en tu proyecto o configuración personal.

Ejecuta `/help` en la TUI para ver los comandos y atajos de teclado.

## Seguridad

Codewhale se ejecuta en tu equipo con el acceso que le otorgues. Los modos de aprobación y las reglas del repositorio limitan lo que el agente puede hacer; el aislamiento opcional del sistema operativo añade un límite de ejecución más sólido cuando es compatible. Los precios desconocidos de los modelos permanecen como desconocidos en lugar de mostrarse como gratuitos.

Lee el [orden de autorización](docs/AUTHORIZATION_ORDER.md) para conocer la jerarquía exacta de políticas y la [configuración](docs/CONFIGURATION.md) para los ajustes locales.

## Documentación

- [Proveedores y modelos locales](docs/PROVIDERS.md)
- [Equipos de agentes](docs/FLEET.md)
- [MCP](docs/MCP.md), [hooks](docs/HOOKS.md) y [configuración](docs/CONFIGURATION.md)
- [Cliente web local](docs/WEB.md)
- [Toda la documentación](docs/README.md)
- [Estructura del repositorio y guía de contribución](CONTRIBUTING.md#project-structure)

## Únete a la comunidad

**Recibimos con gusto reportes de errores, ideas de funciones y pull requests**, tanto si llevas meses usando Codewhale como si lo pruebas por primera vez. Si falta un proveedor, un flujo de trabajo resulta incómodo o la interfaz de terminal te estorba, [abre un issue](https://github.com/Hmbown/CodeWhale/issues/new/choose) o [envía un pull request](CONTRIBUTING.md) para que podamos mejorarlo juntos. Las primeras contribuciones son bienvenidas y quienes contribuyen conservan el crédito por el trabajo que se incorpora.

Únete a [Discord](https://discord.gg/37gfS3ksug), o agrega a Hunter en WeChat (`hunterbown`) y pide entrar al grupo Whale Brothers.

## Historia del proyecto

Codewhale comenzó como `deepseek-tui` y aún conserva la compatibilidad con su configuración y sus sesiones. Ahora es neutral respecto de los proveedores, se mantiene de forma independiente y no está afiliado a ningún proveedor de modelos.

Gracias a cada colaborador y a las comunidades de código abierto que ayudaron a crecer al proyecto. Consulta el [registro de colaboradores](docs/CONTRIBUTORS.md).

## Licencia

[MIT](LICENSE). Las partes adaptadas de otros proyectos de código abierto se registran en los [avisos de terceros](docs/THIRD_PARTY_NOTICES.md).
