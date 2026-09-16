/**
 * Las capturas de la app. Solo se generan en español e inglés: los demás
 * idiomas enseñan la inglesa, que es lo honesto (no vamos a fingir una
 * pantalla en letón que no hemos fotografiado) y evita 31 copias del mismo
 * peso en el repositorio.
 */
import type { Idioma } from "../i18n";

const CON_CAPTURAS = new Set(["es", "en"]);

export function capturaDe(lang: Idioma, nombre: string): string {
  const carpeta = CON_CAPTURAS.has(lang) ? lang : "en";
  return `/capturas/${carpeta}/${nombre}.webp`;
}
