// Las ESTADÍSTICAS ANÓNIMAS de la casa: un worker propio en Cloudflare con
// una base D1, sin terceros. La ficha de ingesta solo puede ESCRIBIR eventos
// (leerlos exige otra clave que no viaja en la app), así que vive aquí sin
// peligro. Ningún evento lleva nombre, correo ni texto del usuario: solo el
// paso del paseo, el plan tocado, o que algo se compartió.
export const ANALITICA = {
  endpoint: "https://analytics-proxy.jlsf2005.workers.dev",
  ficha: "axp_live_QJm7ibcPzAz3rU8fPi0AOFTgwGlwzZUD",
} as const;
