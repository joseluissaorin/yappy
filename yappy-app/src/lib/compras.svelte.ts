// LA CUERDA DEL LORO: el único estado de compras que se pinta. Sabe si hay
// tienda, si eres parlanchín, cuántas piezas hay en la percha (el loro de
// prueba lleva tres a la vez), y guarda las ofertas para que el paywall
// abra al instante. Escucha al motor (pro_cambio, cuota_agotada,
// cola_actualizada) y abre el paywall con su MOTIVO, que decide la viñeta.
import {
  comprasEstado,
  comprasOfertas,
  comprasComprar,
  comprasRestaurar,
  comprasCliente,
  comprasSimular,
  onProCambio,
  onCuotaAgotada,
  onColaActualizada,
  logToBackend,
  type Paquete,
  type InfoCuota,
  type Cliente,
} from "$lib/ipc";
import { medir } from "$lib/analitica";

export type MotivoPaywall = "percha" | "imprenta" | "puente" | "trastienda" | "paseo";

export const compras = $state<{
  disponible: boolean;
  pro: boolean;
  cuota: InfoCuota;
  paquetes: Paquete[];
  cliente: Cliente | null;
  cargandoOfertas: boolean;
  errorOfertas: string | null;
  abierto: boolean;
  motivo: MotivoPaywall;
}>({
  disponible: false,
  pro: true,
  cuota: { usados: 0, limite: 3, agotada: false },
  paquetes: [],
  cliente: null,
  cargandoOfertas: false,
  errorOfertas: null,
  abierto: false,
  motivo: "trastienda",
});

let arrancado = false;

/// Pide la verdad al motor (estado + cuota). Barato: se puede llamar a menudo.
export async function refrescarCompras() {
  try {
    const e = await comprasEstado();
    compras.disponible = e.disponible;
    compras.pro = e.pro;
    compras.cuota = e.cuota;
  } catch {
    /* el motor aún no está */
  }
}

/// Las ofertas, una vez (y de nuevo si fallaron).
export async function cargarOfertas(fuerza = false) {
  if (!compras.disponible) return;
  if (compras.paquetes.length && !fuerza) return;
  compras.cargandoOfertas = true;
  compras.errorOfertas = null;
  try {
    const r = await comprasOfertas();
    compras.paquetes = r.paquetes ?? [];
    if (!compras.paquetes.length) compras.errorOfertas = "vacio";
  } catch (e) {
    compras.errorOfertas = String(e);
    logToBackend("warn", "compras", `ofertas: ${e}`);
  } finally {
    compras.cargandoOfertas = false;
  }
}

export async function cargarCliente() {
  if (!compras.disponible) return;
  try {
    compras.cliente = await comprasCliente();
    if (typeof compras.cliente?.pro === "boolean") compras.pro = compras.cliente.pro;
  } catch {
    compras.cliente = null;
  }
}

export function abrirPaywall(motivo: MotivoPaywall = "trastienda") {
  if (!compras.disponible) return;
  compras.motivo = motivo;
  compras.abierto = true;
  void cargarOfertas();
}

export function cerrarPaywall() {
  compras.abierto = false;
}

/// ¿Puede hacerse una cosa de parlanchines? Si no, abre el paywall y devuelve falso.
export function exigeParlanchin(motivo: MotivoPaywall): boolean {
  if (!compras.disponible || compras.pro) return true;
  abrirPaywall(motivo);
  return false;
}

/// El error que devuelve el motor cuando algo es de parlanchines.
export function esErrorParlanchin(e: unknown): boolean {
  return String(e).includes("parlanchin");
}

export async function comprar(paqueteId: string) {
  return comprasComprar(paqueteId);
}

export async function restaurar() {
  return comprasRestaurar();
}

/// Arranca el espejo una vez por vida del webview.
export async function arrancarCompras() {
  if (arrancado) {
    await refrescarCompras();
    return;
  }
  arrancado = true;
  // Ensayo en el simulador: ?tienda=sim[&pro=1] enciende la tienda de
  // mentira del motor (solo existe en compilaciones de depuración).
  try {
    const q = new URLSearchParams(window.location.search);
    if (q.get("tienda") === "sim") await comprasSimular(true, q.get("pro") === "1").catch(() => {});
  } catch {}
  await refrescarCompras();
  if (!compras.disponible) return;
  await onProCambio((pro) => {
    compras.pro = pro;
    if (pro) void cargarCliente();
  });
  await onCuotaAgotada((c) => {
    compras.cuota = c;
    medir("percha_llena");
    abrirPaywall("percha");
  });
  // La percha cambia con la cinta: cada alta o baja refresca el aforo.
  await onColaActualizada(() => {
    void refrescarCompras();
  });
  if (typeof document !== "undefined") {
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "visible") void refrescarCompras();
    });
  }
  // Las ofertas se piden en silencio al arrancar: el paywall nace con precios.
  void cargarOfertas();
  if (compras.pro) void cargarCliente();
}
