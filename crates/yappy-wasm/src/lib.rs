//! El guionizador para el navegador: los mismos ficheros de yappy-core,
//! incluidos por ruta, con una única función exportada. La demo de la web
//! no simula nada: ejecuta este código.

#[path = "../../yappy-core/src/lang_detect.rs"]
pub mod lang_detect;
#[path = "../../yappy-core/src/chunker.rs"]
pub mod chunker;
#[path = "../../yappy-core/src/guion/mod.rs"]
pub mod guion;

use wasm_bindgen::prelude::*;

/// Devuelve el Guion como JSON: piezas con clase, ritmo y spans
/// (rango original ↔ texto hablado, con su clase).
#[wasm_bindgen]
pub fn guionizar(texto: &str, idioma: &str) -> String {
    let g = guion::construir_desde_texto(texto, idioma);
    serde_json::to_string(&g).unwrap_or_else(|_| "{\"piezas\":[]}".to_string())
}
