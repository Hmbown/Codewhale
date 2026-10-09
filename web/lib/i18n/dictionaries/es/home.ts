import type { HomeDict } from "../types";

/** Spanish home copy: useful work with chosen models and connected tools. */

export const home: HomeDict = {
  metaTitle: "Codewhale: construye con tus modelos y herramientas",
  metaDescription:
    "Crea aplicaciones, automatiza flujos de trabajo y usa herramientas conectadas con Codewhale. Código abierto, con tus propias API de modelos o inferencia local y autoalojada.",
  heroTitle: "Crea aplicaciones y automatiza tu trabajo.",
  heroIntro:
    "{brand} es un agente de código abierto que escribe código, ejecuta comandos y trabaja con las herramientas que conectas. Usa las API de modelos que ya utilizas o ejecuta modelos de forma local y en tus propios servidores.",
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
    "Describe lo que quieres crear o automatizar. Codewhale puede editar archivos, ejecutar comandos y comprobar el resultado, con un acceso que tú controlas.",
  gain: [
    [
      "Crea aplicaciones y herramientas",
      "Crea una aplicación, añade una función o escribe un script. Codewhale puede trabajar con los archivos del proyecto, ejecutar el código y probar lo que crea."
    ],
    [
      "Automatiza el trabajo repetitivo",
      "Ejecuta flujos de trabajo desde tu terminal, scripts o CI. Para tareas más grandes, delega partes del trabajo a una Fleet de agentes con distintos modelos."
    ],
    [
      "Conecta las herramientas que usas",
      "Añade herramientas mediante plugins y servidores MCP, o usa API desde tus propios scripts. Cada servicio necesita su propia configuración y autenticación."
    ]
  ],
  chapterModels: "Tus modelos",
  modelsHeading: "Usa los modelos que elijas",
  modelsBody:
    "Conecta tus cuentas de proveedores, un endpoint compatible con OpenAI o modelos locales y autoalojados. Elige un modelo para la sesión y para cada agente de una Fleet.",
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
  startHeading: "Primeros pasos",
  startLede:
    "Instala Codewhale, conecta un modelo y abre una carpeta de proyecto. Puedes añadir plugins y más agentes según los necesites.",
  startGuideLink: "Seguir la guía de primeros pasos",
  startVocabularyLink: "Ver el vocabulario del producto",
  chapterAvailability: "Dónde funciona",
  availabilityHeading: "Disponible ahora y en desarrollo",
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
  surfacesHeading: "Trabaja con archivos y herramientas",
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
  runtimeLink: "Explora herramientas e integraciones",
  installBandHeading: "Instala en macOS o Linux",
  copy: "Copiar",
  copied: "Copiado ✓",
  binaries: "Binarios",
  chinaMirrors: "Espejos en China",
  installGuideLink: "Leer la guía de instalación",
  communityHeading: "Contribuye a Codewhale",
  communityBody:
    "Informa de un error, mejora la documentación o contribuye con código en GitHub. También puedes crear plugins y compartir flujos de trabajo con otros usuarios.",
  communityLinksAria: "Enlaces de la comunidad",
  contribute: "Contribuye en GitHub",
};
