import type { HomeDict } from "../types";

/** Spanish home copy: useful work with chosen models and connected tools. */

export const home: HomeDict = {
  metaTitle: "Codewhale: construye con tus modelos y herramientas",
  metaDescription:
    "Crea aplicaciones, automatiza flujos de trabajo y usa herramientas conectadas con Codewhale. Código abierto, con tus propias API de modelos o inferencia local y autoalojada.",
  heroTitle: "Un agente de código abierto para tu ordenador.",
  heroIntro:
    "Crea aplicaciones, automatiza flujos de trabajo y trabaja con Slack, Gmail y otras herramientas conectadas. {brand} usa las API de modelos que ya utilizas o inferencia local y autoalojada.",
  getCodewhale: "Instalar Codewhale",
  heroInstallAria: "Comando de instalación",
  exploreProduct: "Explora Codewhale",
  shotPreview: "Vista previa de la terminal",
  screenshotAlt:
    "Codewhale v{version}: captura del terminal con la conversación, el campo de mensaje y los controles de sesión.",
  latestRelease: "Último lanzamiento {tag}",
  releaseUnavailable: "Estado del lanzamiento no disponible",
  currentSource: "Fuente",
  sourceCandidate: "Sin publicar",
  publishedRelease: "publicado",
  gainHeading: "Qué puedes hacer",
  gainLede:
    "Empieza por el resultado que quieres. Codewhale trabaja con archivos, comandos y herramientas conectadas; tú eliges los accesos y las aprobaciones.",
  gain: [
    [
      "Crea aplicaciones y herramientas",
      "Convierte una idea en una aplicación que funcione, un script útil o una función de un proyecto existente. Deja que el agente escriba, ejecute y pruebe contigo."
    ],
    [
      "Automatiza el trabajo repetitivo",
      "Convierte una tarea recurrente en un flujo de trabajo para el terminal, scripts o CI. Usa un Fleet de agentes cuando el trabajo pueda hacerse en paralelo."
    ],
    [
      "Conecta las herramientas que usas",
      "Conecta herramientas como Gmail y Slack mediante plugins, servidores MCP o API. Trabaja con esos servicios junto con tus archivos y comandos."
    ]
  ],
  chapterModels: "Tus modelos",
  modelsHeading: "Sigue usando tus modelos.",
  modelsBody:
    "Conecta las API de modelos que ya pagas, usa una pasarela compatible o ejecuta inferencia en tu propio hardware. Elige un modelo para cada sesión y para cada agente de un Fleet.",
  modelsFacts: [
    [
      "Tus cuentas de API",
      "Conecta proveedores como OpenAI, Anthropic, Google o DeepSeek con tus propias claves."
    ],
    [
      "Tu pasarela",
      "Usa un endpoint compatible con OpenAI y elige los modelos que ofrece."
    ],
    [
      "Tu inferencia",
      "Ejecuta modelos locales o autoalojados con Ollama, vLLM o SGLang."
    ]
  ],
  modelsLink: "Ver modelos y proveedores",
  startHeading: "Trae una tarea. Empieza.",
  startLede:
    "Instala Codewhale, conecta un modelo y dale algo que merezca la pena hacer. Empieza con un agente; añade herramientas o un equipo cuando lo necesites.",
  startGuideLink: "Seguir la guía de primeros pasos",
  startVocabularyLink: "Ver el vocabulario del producto",
  chapterAvailability: "Dónde funciona",
  availabilityHeading: "Empieza en el terminal.",
  availabilityLede:
    "El terminal y el cliente de navegador local ya están disponibles. La aplicación nativa de escritorio y la aplicación web alojada reconstruida están en desarrollo.",
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
    "La terminal, el navegador local y la GUI no necesitan una cuenta de Codewhale. La web alojada y la aplicación de escritorio usan una cuenta. Si usas tu propia clave de proveedor, ese proveedor te factura el uso.",
  accountLink: "Crear una cuenta",
  surfacesHeading: "Una tarea. Tus archivos, aplicaciones y agentes.",
  surfaces: [
    [
      "Archivos y terminal",
      "Crea archivos, ejecuta comandos, examina datos y prueba lo que construyes. Tú defines la carpeta de trabajo y los permisos."
    ],
    [
      "Plugins y aplicaciones conectadas",
      "Añade habilidades y herramientas mediante plugins y MCP. Revisa y habilita las conexiones que quieras que use el agente."
    ],
    [
      "Navegador y ordenador · vista previa",
      "Usa herramientas de navegador y el plugin Computer Use para trabajar en aplicaciones y sitios web con el acceso que concedas."
    ],
    [
      "Sesiones que puedes retomar",
      "Mantén juntos la conversación, los resultados de las herramientas y el historial. Retoma la tarea en el terminal o en su cliente de navegador local."
    ],
    [
      "Equipos de agentes",
      "Usa Fleet para repartir un trabajo grande entre agentes con distintos roles y modelos, y seguir su progreso en un solo lugar."
    ]
  ],
  runtimeLink: "Ver todas las integraciones",
  installBandHeading: "Instala en macOS o Linux",
  copy: "Copiar",
  copied: "Copiado ✓",
  binaries: "Binarios",
  chinaMirrors: "Espejos en China",
  installGuideLink: "Leer la guía de instalación",
  communityHeading: "Haz Codewhale tuyo.",
  communityBody:
    "Codewhale es de código abierto. Lee el código, crea un plugin, comparte un flujo de trabajo o ayuda a mejorar la próxima versión.",
  communityLinksAria: "Enlaces de la comunidad",
  contribute: "Contribuye en GitHub",
};
