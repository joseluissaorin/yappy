// LOS HECHOS. Fuente única de verdad de la web: ningún componente inventa
// una cifra, un precio ni un enlace. Si algo cambia en el producto, cambia
// aquí y cambia en los 31 idiomas a la vez.

export const ENLACES = {
  appStore: "https://apps.apple.com/app/id6773110015",
  appStoreId: "6773110015",
  googlePlay: null as string | null, // aún no publicada: no inventar enlace
  github: "https://github.com/joseluissaorin/yappy",
  releases: "https://github.com/joseluissaorin/yappy/releases/latest",
  novedades: "https://github.com/joseluissaorin/yappy/releases",
  issues: "https://github.com/joseluissaorin/yappy/issues",
  licencia: "https://github.com/joseluissaorin/yappy/blob/main/LICENSE",
  modelo: "https://huggingface.co/Supertone/supertonic-3",
  oido: "https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3",
  iroh: "https://iroh.computer",
  revenuecat: "https://www.revenuecat.com/privacy",
  appleLegal: "https://www.apple.com/legal/privacy/",
  eula: "https://www.apple.com/legal/internet-services/itunes/dev/stdeula/",
  autor: "https://joseluissaorin.com",
  correo: "mailto:jl@joseluissaorin.com",
} as const;

export const MOTOR = {
  modelo: "Supertonic 3",
  fabricante: "Supertone",
  parametros: "99 M",
  licenciaModelo: "OpenRAIL-M",
  pesoEscritorio: "398 MB",
  pesoMovil: "200 MB",
  voces: 10,
  idiomas: 31,
  oido: "Parakeet TDT 0.6B v3",
  licenciaOido: "CC-BY-4.0",
} as const;

// Las diez voces, con su tinta. Fuente: crates/yappy-core/src/voices.rs y
// yappy-app/src/lib/voces.ts (TINTAS_VOZ).
export const VOCES = [
  { nombre: "Alex", sexo: "m", tinta: "#e0502a" },
  { nombre: "James", sexo: "m", tinta: "#2f4bc4" },
  { nombre: "Robert", sexo: "m", tinta: "#e8b41a" },
  { nombre: "Sam", sexo: "m", tinta: "#2e7d5b" },
  { nombre: "Daniel", sexo: "m", tinta: "#8a4fbe" },
  { nombre: "Sarah", sexo: "f", tinta: "#c43e6a" },
  { nombre: "Lily", sexo: "f", tinta: "#1f8a9c" },
  { nombre: "Jessica", sexo: "f", tinta: "#b8651f" },
  { nombre: "Olivia", sexo: "f", tinta: "#5b6d2e" },
  { nombre: "Emily", sexo: "f", tinta: "#7a4a32" },
] as const;

// Los 31 idiomas que habla el loro. El código es el de la interfaz de la app
// (y el de esta web). «endonimo» es cómo se llama la lengua en sí misma.
export const IDIOMAS = [
  { codigo: "es", endonimo: "Español", bcp: "es" },
  { codigo: "en", endonimo: "English", bcp: "en" },
  { codigo: "fr", endonimo: "Français", bcp: "fr" },
  { codigo: "de", endonimo: "Deutsch", bcp: "de" },
  { codigo: "it", endonimo: "Italiano", bcp: "it" },
  { codigo: "pt", endonimo: "Português", bcp: "pt" },
  { codigo: "nl", endonimo: "Nederlands", bcp: "nl" },
  { codigo: "pl", endonimo: "Polski", bcp: "pl" },
  { codigo: "ro", endonimo: "Română", bcp: "ro" },
  { codigo: "sv", endonimo: "Svenska", bcp: "sv" },
  { codigo: "da", endonimo: "Dansk", bcp: "da" },
  { codigo: "fi", endonimo: "Suomi", bcp: "fi" },
  { codigo: "et", endonimo: "Eesti", bcp: "et" },
  { codigo: "lt", endonimo: "Lietuvių", bcp: "lt" },
  { codigo: "lv", endonimo: "Latviešu", bcp: "lv" },
  { codigo: "hr", endonimo: "Hrvatski", bcp: "hr" },
  { codigo: "sl", endonimo: "Slovenščina", bcp: "sl" },
  { codigo: "sk", endonimo: "Slovenčina", bcp: "sk" },
  { codigo: "cs", endonimo: "Čeština", bcp: "cs" },
  { codigo: "hu", endonimo: "Magyar", bcp: "hu" },
  { codigo: "el", endonimo: "Ελληνικά", bcp: "el" },
  { codigo: "bg", endonimo: "Български", bcp: "bg" },
  { codigo: "uk", endonimo: "Українська", bcp: "uk" },
  { codigo: "ru", endonimo: "Русский", bcp: "ru" },
  { codigo: "tr", endonimo: "Türkçe", bcp: "tr" },
  { codigo: "ar", endonimo: "العربية", bcp: "ar", rtl: true },
  { codigo: "hi", endonimo: "हिन्दी", bcp: "hi" },
  { codigo: "id", endonimo: "Bahasa Indonesia", bcp: "id" },
  { codigo: "vi", endonimo: "Tiếng Việt", bcp: "vi" },
  { codigo: "ko", endonimo: "한국어", bcp: "ko" },
  { codigo: "ja", endonimo: "日本語", bcp: "ja" },
] as const;

export const RTL = IDIOMAS.filter((i) => "rtl" in i).map((i) => i.codigo);

// La cuerda. Precios base de España; Apple los equaliza a 175 territorios,
// así que la web dice el euro y avisa de que cada tienda pone el suyo.
// NO HAY PRUEBA GRATUITA: decisión de José Luis, no reintroducirla.
export const CUERDA = {
  mensual: "3,99 €",
  anual: "29,99 €",
  vida: "59,99 €",
  perchaGratis: 3,
} as const;

// Lo que cobran los otros, para la tabla de comparación. Cifras públicas de
// septiembre de 2026; llevan fecha porque envejecen.
export const RIVALES = {
  fecha: "2026-09",
  speechify: { precio: "29 $/mes · 11,60 $/mes al año", gratis: "10 voces básicas, hasta 1,5×" },
  naturalreader: { precio: "13,90 $/mes · 79 $/año", gratis: "voces básicas sin límite; 5 min al día de las buenas" },
  elevenreader: { precio: "11 $/mes · 99 $/año", gratis: "10 h de audio al mes" },
} as const;

export const ATAJOS = [
  { que: "leer", mac: "⌥⌘R", pc: "Ctrl+Alt+R" },
  { que: "portapapeles", mac: "⌥⌘V", pc: "Ctrl+Alt+V" },
  { que: "pausar", mac: "⌥⌘Espacio", pc: "Ctrl+Alt+Espacio" },
] as const;

export const FECHAS = {
  privacidad: "2026-09-09",
  lanzamiento: "2026-09-16",
} as const;
