// i18n mínima de la casa: dos diccionarios (español e inglés) y un t()
// reactivo. El idioma de la interfaz sigue al del sistema; el español es
// primera lengua del proyecto, no una traducción.
import { derived, writable } from "svelte/store";

export type IdiomaUI = "es" | "en";

const DICCIONARIOS: Record<IdiomaUI, Record<string, string>> = {
  es: {
    // navegación
    "nav.escuchar": "Escuchar",
    "nav.biblioteca": "Biblioteca",
    "nav.ajustes": "Ajustes",
    // escuchar
    "escuchar.sigue": "Sigue donde ibas",
    "escuchar.cola": "Recién llegado",
    "escuchar.vacia.titulo": "Nada en la percha",
    "escuchar.vacia.texto":
      "Comparte un artículo, un PDF o un vídeo desde cualquier app con el botón de compartir, y aparecerá aquí listo para escucharse.",
    "escuchar.anadir": "Añadir",
    "escuchar.sin_voces": "Las voces aún no están",
    "escuchar.pegar_enlace": "Pegar un enlace",
    "escuchar.leer_portapapeles": "Leer el portapapeles",
    "escuchar.abrir_archivo": "Abrir un archivo",
    "escuchar.enlace_titulo": "Escuchar un enlace",
    "escuchar.enlace_placeholder": "https://…",
    "escuchar.enlace_boton": "A la cola",
    "estado.pendiente": "en cola",
    "estado.preparando": "preparando el guion…",
    "estado.listo": "listo para escuchar",
    "estado.error": "no pude con esto",
    "cola.reintentar": "Reintentar",
    "cola.borrar": "Borrar",
    "cola.escuchar": "Escuchar",
    // biblioteca
    "biblioteca.buscar": "Buscar en la biblioteca",
    "biblioteca.documentos": "Documentos",
    "biblioteca.audiolibros": "Audiolibros",
    "biblioteca.vacia.titulo": "La estantería, recién estrenada",
    "biblioteca.vacia.texto":
      "Todo lo que compartas o abras se guarda aquí, con tu progreso de escucha.",
    "biblioteca.continuar": "continuar",
    "biblioteca.min": "min",
    // ajustes
    "ajustes.titulo": "Ajustes",
    "ajustes.voz": "Voz",
    "ajustes.voz_defecto": "Voz por defecto",
    "ajustes.velocidad": "Velocidad",
    "ajustes.calidad": "Calidad de la voz",
    "ajustes.calidad.rapida": "Rápida",
    "ajustes.calidad.equilibrada": "Equilibrada",
    "ajustes.calidad.mejor": "La mejor",
    "ajustes.idioma": "Idioma preferido",
    "ajustes.modelos": "Modelos en este dispositivo",
    "ajustes.modelo_voces": "Voces (Supertonic)",
    "ajustes.modelo_oido": "Oído (transcripción)",
    "ajustes.descargar": "Descargar",
    "ajustes.descargado": "instalado",
    "ajustes.diagnostico": "Diagnóstico",
    "ajustes.tema": "Aspecto",
    "ajustes.ordenador": "Tu ordenador",
    "ajustes.ordenador_texto_no":
      "Empareja Yappy del ordenador (Preferencias → phone bridge): escanea su QR con la cámara del iPhone, o pega aquí el enlace.",
    "ajustes.ordenador_texto_si":
      "Puedes mandarle libros enteros: se convierten allí y el audiolibro vuelve solo a tu Biblioteca.",
    "ajustes.vincular": "Vincular",
    "ajustes.desvincular": "Desvincular",
    "ajustes.vinculado": "vinculado",
    "ajustes.tema.papel": "Papel",
    "ajustes.tema.noche": "Noche",
    "ajustes.tema.sistema": "Como el sistema",
    // reproductor
    "player.reanudar": "Reanudar",
    "player.pausar": "Pausar",
    // genéricos
    "comun.cancelar": "Cancelar",
    "comun.hecho": "Hecho",
  },
  en: {
    "nav.escuchar": "Listen",
    "nav.biblioteca": "Library",
    "nav.ajustes": "Settings",
    "escuchar.sigue": "Pick up where you left off",
    "escuchar.cola": "Just arrived",
    "escuchar.vacia.titulo": "Nothing on the perch",
    "escuchar.vacia.texto":
      "Share an article, a PDF or a video from any app with the share button, and it will land here ready to listen.",
    "escuchar.anadir": "Add",
    "escuchar.sin_voces": "The voices aren't here yet",
    "escuchar.pegar_enlace": "Paste a link",
    "escuchar.leer_portapapeles": "Read the clipboard",
    "escuchar.abrir_archivo": "Open a file",
    "escuchar.enlace_titulo": "Listen to a link",
    "escuchar.enlace_placeholder": "https://…",
    "escuchar.enlace_boton": "Queue it",
    "estado.pendiente": "queued",
    "estado.preparando": "preparing the script…",
    "estado.listo": "ready to listen",
    "estado.error": "couldn't handle this",
    "cola.reintentar": "Retry",
    "cola.borrar": "Delete",
    "cola.escuchar": "Listen",
    "biblioteca.buscar": "Search the library",
    "biblioteca.documentos": "Documents",
    "biblioteca.audiolibros": "Audiobooks",
    "biblioteca.vacia.titulo": "A brand-new shelf",
    "biblioteca.vacia.texto":
      "Everything you share or open is kept here, with your listening progress.",
    "biblioteca.continuar": "continue",
    "biblioteca.min": "min",
    "ajustes.titulo": "Settings",
    "ajustes.voz": "Voice",
    "ajustes.voz_defecto": "Default voice",
    "ajustes.velocidad": "Speed",
    "ajustes.calidad": "Voice quality",
    "ajustes.calidad.rapida": "Fast",
    "ajustes.calidad.equilibrada": "Balanced",
    "ajustes.calidad.mejor": "Best",
    "ajustes.idioma": "Preferred language",
    "ajustes.modelos": "Models on this device",
    "ajustes.modelo_voces": "Voices (Supertonic)",
    "ajustes.modelo_oido": "Ear (transcription)",
    "ajustes.descargar": "Download",
    "ajustes.descargado": "installed",
    "ajustes.diagnostico": "Diagnostics",
    "ajustes.tema": "Appearance",
    "ajustes.ordenador": "Your computer",
    "ajustes.ordenador_texto_no":
      "Pair Yappy on your computer (Preferences → phone bridge): scan its QR with the iPhone camera, or paste the link here.",
    "ajustes.ordenador_texto_si":
      "You can send it whole books: they convert over there and the audiobook comes back to your Library on its own.",
    "ajustes.vincular": "Link",
    "ajustes.desvincular": "Unlink",
    "ajustes.vinculado": "linked",
    "ajustes.tema.papel": "Paper",
    "ajustes.tema.noche": "Night",
    "ajustes.tema.sistema": "Match the system",
    "player.reanudar": "Resume",
    "player.pausar": "Pause",
    "comun.cancelar": "Cancel",
    "comun.hecho": "Done",
  },
};

export const idiomaUI = writable<IdiomaUI>("en");

export function fijarIdiomaDesdeLocale(locale: string | null | undefined) {
  idiomaUI.set(locale?.toLowerCase().startsWith("es") ? "es" : "en");
}

/// t("clave") reactivo: $t en los componentes.
export const t = derived(idiomaUI, ($l) => (clave: string): string => {
  return DICCIONARIOS[$l][clave] ?? DICCIONARIOS.en[clave] ?? clave;
});
