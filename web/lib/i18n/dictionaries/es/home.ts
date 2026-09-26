import type { HomeDict } from "../types";

/**
 * Spanish home dictionary — native copy for the Tidal Folio landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale — Crea y automatiza con los modelos que elijas",
  metaDescription:
    "Crea software, trabaja con tus archivos y automatiza las tareas cotidianas con agentes de código abierto y los modelos de IA alojados o locales que elijas.",
  heroTitle: "Crea y automatiza con los modelos que elijas",
  heroIntro:
    "{brand} te ofrece agentes que pueden crear software, trabajar con tus archivos y convertir las tareas repetitivas en flujos de trabajo reutilizables. Diles qué quieres conseguir y elige los modelos alojados o locales adecuados para el trabajo, con la libertad de cambiar de proveedor sobre la marcha.",
  getCodewhale: "Obtener Codewhale",
  heroInstallAria: "Comando de instalación",
  exploreProduct: "Explorar el producto",
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
  chapterTerminalTitle: "Empieza con algo que quieras crear",
  gainHeading:
    "Qué puedes hacer con Codewhale",
  gainLede:
    "Empieza con un proyecto, una pregunta o una tarea que quieras automatizar, y después trabaja con un agente o reparte las partes de un trabajo más grande entre varios.",
  gain: [
    [
      "Crea algo",
      "Describe qué quieres crear y trabaja con agentes que pueden leer tu código, editar archivos, ejecutar comandos y comprobar el resultado."
    ],
    [
      "Automatiza el trabajo cotidiano",
      "Crea scripts y flujos de trabajo para las tareas que repites, de modo que puedas volver a ejecutarlos desde la terminal siempre que los necesites."
    ],
    [
      "Trabaja con distintos modelos",
      "Usa modelos alojados o locales para tus agentes, con distintos modelos y roles que se encarguen de las partes del trabajo para las que son adecuados."
    ]
  ],
  chapterModels: "Tus modelos",
  modelsHeading: "Opciones de modelos para cada tarea",
  modelsBody:
    "Conéctate directamente a un proveedor de modelos alojados, usa una pasarela para acceder a varios proveedores o ejecuta un modelo en local, y elige qué modelo usa cada sesión mientras trabajas.",
  modelsFacts: [
    ["Alojado", "Tu propia clave de API, guardada con codewhale auth set --provider <id>"],
    ["Gateway", "Un endpoint para muchos modelos; el proveedor lo sigues eligiendo tú"],
    ["Local", "vLLM, SGLang, Ollama en localhost; normalmente sin clave"],
  ],
  modelsLink: "Explorar modelos y proveedores",
  startHeading: "Primeros pasos con Codewhale",
  startLede:
    "Una vez que hayas instalado Codewhale y conectado un modelo, puedes describir tu primera tarea en la terminal y añadir un Fleet cuando quieras repartir el trabajo entre varios agentes.",
  startGuideLink: "Leer la guía de primeros pasos",
  startVocabularyLink: "Ver el vocabulario del producto",
  chapterAvailability: "Dónde funciona",
  availabilityHeading: "Dónde puedes usar Codewhale",
  availabilityLede:
    "Ya puedes usar Codewhale en tu terminal mientras desarrollamos la aplicación web, la aplicación de escritorio y las computadoras en la nube.",
  availability: [
    [
      "Terminal",
      "Publicada",
      "Binarios de las versiones publicadas en GitHub para Linux, macOS y Windows; npm y Cargo son alternativas. Android en Termux está en vista previa."
    ],
    [
      "Aplicación web",
      "Vista previa de desarrollo",
      "Acceso a la cuenta y vinculación con el navegador en la vista previa de desarrollo."
    ],
    [
      "Escritorio",
      "Build de desarrollo",
      "La aplicación para macOS está en desarrollo; la descarga pública llegará más adelante."
    ],
    [
      "Computadoras en la nube",
      "En desarrollo",
      "Computadoras alojadas para ejecutar tus tareas."
    ]
  ],
  availabilityNote:
    "Puedes usar la terminal sin una cuenta de Codewhale, y tu proveedor factura cualquier uso de modelos alojados.",
  accountLink: "Crear una cuenta",
  surfacesHeading: "Formas de trabajar con Codewhale",
  surfaces: [
    ["TUI", "Trabajo interactivo en la terminal"],
    ["codewhale exec", "Scripts y CI"],
    ["Cliente web local","Interfaz en localhost; espacio de trabajo web alojado en desarrollo"],
    ["Runtime API + MCP", "Integraciones locales"],
    ["Fleet","Varios agentes en un mismo trabajo"],
  ],
  runtimeLink: "Explorar integraciones",
  installBandHeading: "Instala Codewhale en macOS o Linux",
  copy: "Copiar",
  copied: "Copiado ✓",
  binaries: "Binarios",
  chinaMirrors: "Espejos en China",
  installGuideLink: "Leer la guía de instalación",
  communityHeading: "Ayuda a mejorar Codewhale",
  communityBody:
    "Tanto si has encontrado un error como si tienes una idea para una función o quieres enviar tu primer pull request, nos gustaría escucharte y trabajar juntos en los próximos pasos.",
  communityLinksAria: "Enlaces de la comunidad",
  contribute: "Enviar un pull request",
};
