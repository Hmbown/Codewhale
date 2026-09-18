import type { ComputerUseDict } from "../types";

/**
 * Spanish dictionary for `app/[locale]/computer-use/page.tsx` and the
 * Computer Use section of the install page. Neutral Spanish (Latin America
 * and Spain), informal `tú`, matching the register of the other es
 * dictionaries. Product names (Codewhale, Computer Use), the menu items
 * Pause, Stop and Check for updates, and the macOS setting names stay as
 * the app shows them.
 */
export const computerUse: ComputerUseDict = {
  metaTitle: "Computer Use para Mac · Codewhale",
  metaDescription: "Descarga y configura Codewhale Computer Use para Mac. Control de aplicaciones en segundo plano, configuración de permisos y controles Pause y Stop en tus manos.",
  title: "Computer Use",
  lead: "Deja que Codewhale trabaje en tus aplicaciones mientras tú sigues con lo tuyo. El asistente para Mac reúne en la barra de menús los permisos, el control de aplicaciones en segundo plano y una forma de pausar o detener la entrada.",
  publisher: "Por Codewhale",
  download: "Descargar para Mac",
  downloadZip: "Archivo ZIP (lo usa el actualizador integrado en la aplicación)",
  requirements: "macOS 13.5 o posterior · Apple silicon e Intel",
  included: "Una sola descarga. No necesitas instalar Node ni un compilador por separado.",
  pendingTitle: "Descarga para Mac en preparación",
  pendingBody: "El instalador público aparecerá aquí cuando se completen la notarización de Apple y las comprobaciones de lanzamiento.",
  unavailableTitle: "No se pudo comprobar la disponibilidad de la descarga",
  unavailableBody: "Recarga esta página para volver a intentarlo o consulta los lanzamientos publicados más abajo.",
  releases: "Lanzamientos publicados",
  receipt: "Datos de verificación de la descarga",
  setup: "Configura tu Mac",
  steps: [
    { title: "Instala la aplicación", body: "Abre la imagen de disco y arrastra Codewhale Computer Use a «Aplicaciones». Ábrela desde «Aplicaciones» y luego elige Computer Use en el icono de la ballena de la barra de menús." },
    { title: "Revisa los permisos", body: "Usa los botones de configuración para abrir «Accesibilidad» y «Grabación de pantalla» en Ajustes del Sistema. Tú decides qué permisos conceder." },
    { title: "Ejecuta la comprobación en segundo plano", body: "El asistente abre una ventana de práctica desechable, escribe texto y captura esa ventana. Comprueba si el puntero o la aplicación activa cambiaron durante la ejecución." },
    { title: "Conéctalo con Codewhale", body: "Revisa, marca como de confianza y activa Computer Use en el mercado de plugins de Codewhale. Usa el plugin 0.3.1 o posterior para que las acciones locales pasen por los controles Pause y Stop del asistente." },
  ],
  controlsTitle: "Sigue trabajando. Mantén el control.",
  controlsBody: "Las acciones compatibles se ejecutan en segundo plano sobre la aplicación seleccionada. Las aplicaciones y los gestos que requieren control en primer plano necesitan tu autorización. El menú muestra el destino y el modo de entrada; Pause (pausar) suspende la entrada del asistente y Stop (detener) finaliza sus sesiones existentes.",
  updateTitle: "Actualizaciones cuando tú quieras",
  updateBody: "Elige Check for updates (buscar actualizaciones) en la aplicación. Antes de instalar una actualización, comprueba la descarga, la firma de Codewhale y la notarización de Apple, y conserva la versión anterior por si necesitas recuperarla.",
  help: "Configuración y solución de problemas",
  notes: "Notas de la versión",
  demo: "Ver la comprobación en segundo plano",
  source: "Código fuente y otras plataformas",
  platforms: "Esta descarga es para Mac. En Windows y Linux, por ahora, se usa el plugin desde el código fuente y la configuración del lado del host.",
  installTitle: "Computer Use para Mac",
  installLead: "Añade el control de aplicaciones en segundo plano con el asistente de Computer Use. Configura los permisos de tu Mac, ejecuta una comprobación en segundo plano y pausa o detén la entrada del asistente desde la barra de menús.",
  installLink: "Descarga y configuración de Computer Use",
};
