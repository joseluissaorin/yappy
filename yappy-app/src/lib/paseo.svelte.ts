// EL PASEO DEL LORO: el onboarding como paseo en catorce pasos, con la
// libreta (lo contestado) guardada en los ajustes. El loro DICE cada paso
// por el canal de efectos; la página solo pone el subtítulo. Nada bloquea:
// saltar siempre es posible, y el paseo se reanuda donde se dejó.
//
// Después del primer día, el paseo sigue como AVISOS contextuales del loro
// en la cinta (segunda apertura sin compartir, libro largo, favoritos).
import { get } from "svelte/store";
import { goto } from "$app/navigation";
import { t, idiomaUI, frase as fraseEn, type IdiomaUI } from "$lib/i18n";
import {
  getSettings,
  setPaseo,
  paseoDecir,
  paseoCallar,
  paseoPrecocinar,
  paseoVozDescargar,
  cuentosListar,
  portapapelesTieneEnlace,
  colaListar,
  type Settings,
  type Paseo,
  type Cuento,
} from "$lib/ipc";
import { medir, medirPantalla, arrancarAnalitica } from "$lib/analitica";
import { repro } from "$lib/reproduccion.svelte";
import { compras } from "$lib/compras.svelte";

// COMPARTIR va de lo primero: es el gesto que hace que Yappy exista (el
// enlace al cuento recomendado, Safari, compartir → Yappy, con el loro en
// PiP señalando). Lo demás (qué lees, cuándo, la velocidad) viene después.
export const PASOS = [
  "hola",
  "idiomas",
  "pajaro",
  "compartir",
  "que",
  "cuando",
  "velocidad",
  "cuanto",
  "escucha",
  "gestos",
  "exportar",
  "avisos",
  "favoritos",
  "resumen",
  "percha",
  "cuerda",
] as const;
export type Paso = (typeof PASOS)[number];

export const OPCIONES_QUE = ["articulos", "libros", "apuntes", "notas", "videos", "correos"] as const;
export const OPCIONES_CUANDO = ["cocinando", "paseando", "cama", "estudiando"] as const;
export const OPCIONES_CUANTO = ["poco", "pila", "montana"] as const;
// Los cinco mandos REALES del cartel: la lección tiene que enseñar los
// que la app tiene, no una versión simplificada.
export const GESTOS = ["pausar", "seguir", "frase", "parrafo", "velocidad"] as const;

function libretaVacia(): Paseo {
  return {
    hecho: false,
    paso: 0,
    que: [],
    cuando: "",
    cuanto: "",
    gestos: [],
    compartido: false,
    segundos_escuchados: 0,
    avisos: [],
    aperturas: 0,
    estadisticas: true,
  };
}

export const paseo = $state<{
  cargado: boolean;
  activo: boolean;
  paso: number;
  libreta: Paseo;
  cuentos: Cuento[];
  enlaceCopiado: boolean;
  hablando: boolean;
  /// Piezas propias en la cinta al entrar en el paso de compartir.
  piezasAntes: number;
  /// El aviso contextual pendiente de enseñar en la cinta (o null).
  aviso: string | null;
}>({
  cargado: false,
  activo: false,
  paso: 0,
  libreta: libretaVacia(),
  cuentos: [],
  enlaceCopiado: false,
  hablando: false,
  piezasAntes: 0,
  aviso: null,
});

let hablaTimer: ReturnType<typeof setTimeout> | undefined;

export const pasoActual = (): Paso => PASOS[Math.max(0, Math.min(PASOS.length - 1, paseo.paso))];

/// Guarda SOLO la libreta (un comando propio: guardar los ajustes enteros
/// pisaba la voz o la velocidad elegidas por el camino). `hecho` también
/// cierra el primer arranque en el motor.
export async function guardarPaseo() {
  try {
    await setPaseo($state.snapshot(paseo.libreta) as Paseo);
  } catch {}
}

/// Arranque: lee los ajustes, decide si el paseo está activo y enciende
/// las estadísticas anónimas según el ajuste.
export async function cargarPaseo(): Promise<boolean> {
  let ajustes: Settings;
  try {
    ajustes = await getSettings();
  } catch {
    return false;
  }
  paseo.libreta = { ...libretaVacia(), ...(ajustes.paseo ?? {}) };
  paseo.libreta.aperturas += 1;
  paseo.activo = !ajustes.first_launch_done && !paseo.libreta.hecho;
  paseo.paso = paseo.activo ? Math.min(paseo.libreta.paso, PASOS.length - 1) : 0;
  paseo.cargado = true;
  void arrancarAnalitica(paseo.libreta.estadisticas !== false);
  if (paseo.activo) {
    medir("paseo_reanudado", { paso: pasoActual() });
    bajarVozPaseo(get(idiomaUI));
  }
  calcularAviso();
  await guardarPaseo();
  return paseo.activo;
}

/// Lo que el loro dice en un paso (con la voz elegida). Calla si algo suena.
/// Sin `idioma`, la frase va en el idioma de la interfaz; con él, en el
/// diccionario de ESA lengua (los sellos de idioma).
export async function decirPaso(clave: string, vars: Record<string, string | number> = {}, velocidad?: number, idioma?: string) {
  const lang = idioma ?? get(idiomaUI);
  let texto = idioma ? fraseEn(idioma as IdiomaUI, clave) : get(t)(clave);
  for (const [k, v] of Object.entries(vars)) texto = texto.replaceAll(`{${k}}`, String(v));
  if (!texto || texto === clave) return;
  await decirTexto(clave, texto, lang, velocidad);
}

/// El turno de la voz: lo que llega tarde de un turno viejo no anima al loro.
let turnoVoz = 0;
/// El último callar en vuelo: un decir espera a que llegue antes de sonar
/// (si no, el stop de un paso viejo podría cortar la voz del nuevo).
let callando: Promise<unknown> = Promise.resolve();

/// Dice `texto` (identificado por `clave` para su audio pregrabado).
export async function decirTexto(clave: string, texto: string, idioma?: string, velocidad?: number) {
  if (repro.snap && repro.snap.estado !== "inactivo") return;
  clearTimeout(hablaTimer);
  const turno = ++turnoVoz;
  paseo.hablando = true;
  try {
    await callando;
    if (turno !== turnoVoz) return;
    const dur = await paseoDecir(clave, texto, idioma, velocidad);
    if (turno !== turnoVoz) return;
    hablaTimer = setTimeout(() => (paseo.hablando = false), Math.max(600, dur * 1000));
  } catch {
    if (turno === turnoVoz) paseo.hablando = false;
  }
}

/// Corta la voz EN SECO: al cambiar de paso, al saltar y al salir.
export function callar() {
  clearTimeout(hablaTimer);
  turnoVoz++;
  paseo.hablando = false;
  callando = paseoCallar().catch(() => {});
}

/// Todo lo que el loro dice en el paseo (lo mismo que graba
/// tooling/paseo/voz.mjs), en el orden en que se dice.
function clavesDichas(desde = 0): string[] {
  const claves: string[] = [];
  for (const p of PASOS.slice(desde)) {
    if (p !== "gestos" && p !== "cuerda") claves.push(`paseo.dice.${p}`);
    if (p === "idiomas") claves.push("paseo.hola_corto");
    if (p === "que") claves.push(...OPCIONES_QUE.map((o) => `paseo.que.comentario.${o}`));
    if (p === "cuando") claves.push(...OPCIONES_CUANDO.map((o) => `paseo.cuando.comentario.${o}`));
    if (p === "velocidad") claves.push("paseo.velocidad.demo");
    if (p === "cuanto") claves.push(...OPCIONES_CUANTO.map((o) => `paseo.cuanto.dice.${o}`));
    if (p === "avisos") claves.push("paseo.avisos.gracias");
  }
  return claves;
}

/// Lo que no va en el paquete (en iOS, todo menos español e inglés; en
/// Android, todo) baja del espejo al arrancar en ese idioma o al elegirlo;
/// mientras, el loro sintetiza en vivo. Rust decide qué falta.
export function bajarVozPaseo(lang: string, claves: string[] = clavesDichas()) {
  const frases = claves
    .map((clave) => ({ clave, texto: fraseEn(lang as IdiomaUI, clave), idioma: lang }))
    .filter((f) => f.texto && f.texto !== f.clave && !f.texto.includes("{"));
  void paseoVozDescargar(frases).catch(() => {});
}

/// El «hola» de los 31 sellos, para que tocar un idioma suene al instante.
export function bajarHolas(idiomas: readonly string[]) {
  for (const l of idiomas) bajarVozPaseo(l, ["paseo.hola_corto"]);
}

/// Con otro pájaro elegido, el motor prepara en segundo plano lo que queda
/// del paseo con SU voz (en el orden en que se dirá). Con la voz del loro no
/// hace nada: ya viene grabado.
export function precocinarPaseo(voz: string) {
  const tr = get(t);
  const lang = get(idiomaUI);
  const desde = Math.max(0, PASOS.indexOf("pajaro") + 1, paseo.paso + 1);
  const frases = clavesDichas(desde)
    .map((clave) => ({ clave, texto: tr(clave), idioma: lang }))
    .filter((f) => f.texto && f.texto !== f.clave && !f.texto.includes("{"));
  void paseoPrecocinar(voz, frases).catch(() => {});
}

export async function irAlPaso(n: number) {
  // Nunca dos pasos hablando a la vez: el anterior se corta en seco.
  callar();
  paseo.paso = Math.max(0, Math.min(PASOS.length - 1, n));
  paseo.libreta.paso = paseo.paso;
  medirPantalla(`paseo_${pasoActual()}`);
  await guardarPaseo();
}

export async function siguiente() {
  callar();
  if (paseo.paso >= PASOS.length - 1) {
    await terminarPaseo("fin");
    return;
  }
  await irAlPaso(paseo.paso + 1);
}

export async function saltarPaso() {
  medir("paseo_saltado", { paso: pasoActual() });
  await siguiente();
}

/// Cierra el paseo del primer día (terminado o abandonado) y vuelve a la cinta.
export async function terminarPaseo(como: "fin" | "salir") {
  callar();
  paseo.libreta.hecho = true;
  paseo.activo = false;
  medir("paseo_terminado", { como, paso: pasoActual() });
  await guardarPaseo();
  goto("/escuchar", { replaceState: true }).catch(() => {});
}

/// Respuestas de la libreta.
export async function responder(campo: "que" | "cuando" | "cuanto", valor: string) {
  if (campo === "que") {
    const i = paseo.libreta.que.indexOf(valor);
    if (i >= 0) paseo.libreta.que.splice(i, 1);
    else paseo.libreta.que.push(valor);
  } else {
    paseo.libreta[campo] = valor;
  }
  medir("paseo_respuesta", { paso: campo, valor });
  await guardarPaseo();
}

/// Un gesto aprendido en el cartel (pausar, seguir, saltar).
export async function marcarGesto(g: (typeof GESTOS)[number]) {
  if (!paseo.activo || pasoActual() !== "gestos") return;
  if (!(GESTOS as readonly string[]).includes(g)) return;
  if (paseo.libreta.gestos.includes(g)) return;
  paseo.libreta.gestos.push(g);
  medir("paseo_gesto", { gesto: g });
  await guardarPaseo();
}

export const gestosCompletos = () => GESTOS.every((g) => paseo.libreta.gestos.includes(g));

/// Segundos escuchados durante el paseo (para el resumen). Se guarda cada
/// pocos segundos: el resumen llega después de navegar y volver.
let ultimoGuardadoEscucha = 0;
export function anotarEscucha(segundos: number) {
  if (!paseo.activo) return;
  if (segundos <= paseo.libreta.segundos_escuchados) return;
  paseo.libreta.segundos_escuchados = segundos;
  if (segundos - ultimoGuardadoEscucha > 5) {
    ultimoGuardadoEscucha = segundos;
    void guardarPaseo();
  }
}

/// Los cuentos empaquetados del idioma del usuario (o del inglés).
export async function cargarCuentos() {
  try {
    const todos = await cuentosListar();
    const lang = get(idiomaUI);
    const mios = todos.filter((c) => c.idioma === lang);
    paseo.cuentos = mios.length ? mios : todos.filter((c) => c.idioma === "en");
    if (!paseo.cuentos.length) paseo.cuentos = todos;
  } catch {
    paseo.cuentos = [];
  }
  try {
    paseo.enlaceCopiado = await portapapelesTieneEnlace();
  } catch {
    paseo.enlaceCopiado = false;
  }
}

/// Cuenta las piezas PROPIAS de la cinta (las de la casa no cuentan).
export async function piezasPropias(): Promise<number> {
  try {
    const items = await colaListar();
    return items.filter((i) => !i.de_la_casa).length;
  } catch {
    return 0;
  }
}

export async function registrarCompartido() {
  if (paseo.libreta.compartido) return;
  paseo.libreta.compartido = true;
  medir("paseo_compartido");
  await guardarPaseo();
}

// ─── Los avisos contextuales (el paseo que sigue) ─────────────────────────
// Cada uno se enseña UNA vez, en la cinta, y se cierra con un toque.
function calcularAviso() {
  const l = paseo.libreta;
  if (paseo.activo || !l.hecho) {
    paseo.aviso = null;
    return;
  }
  const dado = (id: string) => l.avisos.includes(id);
  if (!l.compartido && l.aperturas >= 2 && !dado("compartir")) {
    paseo.aviso = "compartir";
    return;
  }
  if (l.aperturas >= 3 && !dado("favoritos")) {
    paseo.aviso = "favoritos";
    return;
  }
  paseo.aviso = null;
}

/// Un libro largo escuchado media hora: la imprenta lo deja entero. Lo
/// llama la cinta cuando ve una pieza larga con progreso.
export function avisoImprenta() {
  const l = paseo.libreta;
  if (paseo.activo || !l.hecho || l.avisos.includes("imprenta")) return;
  if (compras.pro) return;
  paseo.aviso = "imprenta";
}

export async function cerrarAviso(id: string) {
  if (!paseo.libreta.avisos.includes(id)) paseo.libreta.avisos.push(id);
  paseo.aviso = null;
  medir("aviso_cerrado", { aviso: id });
  await guardarPaseo();
}

/// Volver a hacer el paseo desde la trastienda.
export async function repetirPaseo() {
  paseo.libreta.paso = 0;
  paseo.libreta.hecho = false;
  paseo.activo = true;
  paseo.paso = 0;
  medir("paseo_repetido");
  // `hecho: false` no reabre el primer arranque en el motor, así que el
  // paseo repetido vive solo en esta sesión: basta, y no se pierde nada.
  await guardarPaseo();
  goto("/paseo").catch(() => {});
}

/// Apagar o encender las estadísticas anónimas.
export async function fijarEstadisticasPaseo(encendidas: boolean) {
  paseo.libreta.estadisticas = encendidas;
  const { fijarEstadisticas } = await import("$lib/analitica");
  fijarEstadisticas(encendidas);
  await guardarPaseo();
}
