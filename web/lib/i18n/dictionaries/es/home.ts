import type { HomeDict } from "../types";

/**
 * Spanish home dictionary — native copy for the whale-road landing page.
 * Translates the English reference in en/home.ts: an open-source coding
 * agent for any model, control you can check, and availability stated per
 * surface as it is today. Product vocabulary stays literal (Plan / Work /
 * Operate, Ask / Auto-Review / Full Access, Codewhale, codewhale exec,
 * Fleet, MCP, Runtime, /receipts).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale: el agente de programación de código abierto para cualquier modelo",
  metaDescription:
    "Codewhale es un agente de programación de código abierto para tu terminal. Lee tu proyecto, edita archivos y ejecuta tus pruebas con el modelo alojado o local que elijas.",
  heroTitle: "El agente de programación de código abierto para cualquier modelo",
  heroIntro:
    "{brand} lee tu proyecto, edita archivos y ejecuta tus pruebas desde tu terminal. Conecta un modelo alojado o local, y elige qué acciones necesitan tu aprobación.",
  getCodewhale: "Instalar Codewhale",
  heroInstallAria: "Comando de instalación",
  exploreProduct: "Ver cómo funciona",
  shotPreview: "Vista previa de la terminal",
  shotBuild: "build de desarrollo v{version}",
  screenshotAlt:
    "Codewhale v{version}, versión de desarrollo: ballena, nueva sesión, campo de mensaje, permisos Ask, modo Work y estado del modelo. Representación de la salida real de un terminal aislado.",
  latestRelease: "Último lanzamiento {tag}",
  releaseUnavailable: "Estado del lanzamiento no disponible",
  currentSource: "Fuente",
  sourceCandidate: "Sin publicar",
  publishedRelease: "publicado",
  figcaptionSourceCandidate: "sin publicar",
  chapterTerminal: "Tu terminal",
  chapterTerminalTitle: "Sigue cada edición y cada comando mientras se ejecutan",
  gainHeading: "Delega la tarea y mantén el control",
  gainLede:
    "Pide un resultado: corregir un error, explicar un módulo o automatizar una tarea que repites. Empieza con un agente y agrega más agentes cuando el trabajo crezca.",
  gain: [
    [
      "Cambia el código y compruébalo",
      "El agente inspecciona tu proyecto, edita archivos y ejecuta tus pruebas. Sigue cada edición y cada resultado de comando mientras trabaja."
    ],
    [
      "Automatiza el trabajo repetido",
      "Ejecuta codewhale exec desde scripts y CI. Usa un Fleet para dividir un trabajo más grande entre varios agentes."
    ],
    [
      "Mantén el control",
      "Define los permisos antes de empezar, responde a las solicitudes de aprobación y detén una tarea en cualquier momento. Ejecuta /receipts para listar cada archivo, comando y aprobación de una sesión."
    ]
  ],
  chapterModels: "Tus modelos",
  modelsHeading: "Elige un modelo para cada tarea",
  modelsBody:
    "Elige para cada sesión un proveedor integrado, cualquier endpoint compatible con OpenAI o un modelo local. Tu conexión de modelo se mantiene separada de cualquier cuenta de Codewhale.",
  modelsFacts: [
    ["Alojado", "Tu propia clave de API, guardada con codewhale auth set --provider <id>"],
    ["Gateway", "Un endpoint para muchos modelos; tú sigues eligiendo el proveedor"],
    ["Local", "vLLM, SGLang u Ollama en localhost, normalmente sin clave"],
  ],
  modelsLink: "Ver modelos y proveedores",
  startHeading: "Instala, conecta un modelo y ejecuta una tarea",
  startLede:
    "Ejecuta tu primera tarea en tres pasos desde la carpeta de tu proyecto. Agrega un Fleet más adelante si el trabajo necesita varios agentes.",
  startGuideLink: "Seguir la guía de primeros pasos",
  startVocabularyLink: "Ver el vocabulario del producto",
  chapterAvailability: "Dónde funciona",
  availabilityHeading: "Úsalo hoy en tu terminal",
  availabilityLede:
    "Ya puedes usar la terminal, el cliente de navegador local o la interfaz CodeWhale GUI de la comunidad. La aplicación de escritorio y la aplicación web alojada reconstruida están en desarrollo y comparten el mismo modelo de sesión.",
  availability: [
    [
      "Terminal y navegador local",
      "Publicado",
      "Instala en Linux, macOS o Windows y luego ejecuta codewhale, o codewhale web para el cliente de navegador local. npm y Cargo también funcionan; Android en Termux está en vista previa."
    ],
    [
      "CodeWhale GUI (VS Code)",
      "Disponible",
      "Un proyecto aparte, mantenido por la comunidad: chat, hilos y cambios de archivos en una barra lateral de VS Code sobre el mismo Codewhale Runtime. Instálalo desde el VS Code Marketplace.",
      "https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode"
    ],
    [
      "Aplicación web alojada",
      "Vista previa de desarrollo",
      "Se está reconstruyendo para igualar la aplicación de escritorio. Hoy puedes iniciar sesión y luego escribir /rc en una sesión de terminal en ejecución para continuarla en la web; la ejecución de tareas alojadas aún se está validando."
    ],
    [
      "Escritorio",
      "Build de desarrollo",
      "La aplicación nativa que se está convirtiendo en el cliente principal de Codewhale: carpetas, conversaciones y conexiones de modelos en una sola ventana. Aún no hay descarga pública."
    ],
    [
      "Computadoras en la nube",
      "En desarrollo",
      "Computadoras alojadas que ejecutan tus tareas."
    ]
  ],
  availabilityNote:
    "La terminal, el navegador local y la GUI no necesitan una cuenta de Codewhale. La web alojada y el escritorio usan una cuenta, que no reemplaza tu conexión de modelo; tu proveedor factura el uso con tu propia clave.",
  accountLink: "Crear una cuenta",
  surfacesHeading: "Amplía el alcance del agente",
  surfaces: [
    ["Archivos y comandos", "Lee el proyecto, edita archivos, ejecuta pruebas y revisa la salida dentro de los permisos que definas."],
    ["Plugins y MCP", "Conecta más herramientas y servicios. Cada plugin permanece desactivado hasta que lo revises y lo actives."],
    ["Computer Use · vista previa", "Un plugin que permite al agente ver y operar otras aplicaciones. Tú lo activas y concedes los permisos del sistema que solicita."],
    ["Sesiones guardadas", "Mantén juntos la conversación y los resultados de las herramientas, y reanuda el trabajo en lugar de empezar de cero. El navegador local abre la misma sesión en tu computadora."],
    ["Fleet", "Asigna partes de una tarea a agentes con distintos modelos y roles, y luego sigue su progreso."],
  ],
  runtimeLink: "Ver todas las integraciones",
  installBandHeading: "Instala en macOS o Linux",
  copy: "Copiar",
  copied: "Copiado ✓",
  binaries: "Binarios",
  chinaMirrors: "Espejos en China",
  installGuideLink: "Leer la guía de instalación",
  communityHeading: "Construye Codewhale con nosotros",
  communityBody:
    "Reporta un error, propón una función o envía tu primer pull request en GitHub. Las correcciones pequeñas y probadas son bienvenidas.",
  communityLinksAria: "Enlaces de la comunidad",
  contribute: "Enviar un pull request",
};
