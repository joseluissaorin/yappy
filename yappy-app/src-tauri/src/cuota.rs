//! LA PERCHA DEL LORO GRATIS: caben tres documentos a la vez. No es un
//! límite cronológico sino de aforo: quita una pieza de la cinta y cabe
//! otra. La puerta está en el ALTA de la cola (compartir, pegar, abrir);
//! leer lo que ya está en la percha no tiene límite.
//!
//! Solo muerde donde hay tienda (iOS): en escritorio y Android
//! `compras::es_pro()` es verdadero y la percha no tiene fondo.

use tauri::{AppHandle, Emitter, Runtime};

/// Los documentos que caben a la vez en la percha del loro de prueba.
pub const DOCUMENTOS_GRATIS: usize = 3;

#[derive(serde::Serialize, Clone)]
pub struct InfoCuota {
    pub usados: usize,
    pub limite: usize,
    pub agotada: bool,
}

/// Cuántas piezas hay en la cinta y si la percha está llena (para el loro
/// de prueba; con cuerda nunca lo está).
pub fn info<R: Runtime>(app: &AppHandle<R>) -> InfoCuota {
    // Las piezas de la casa (los cuentos del paseo) no ocupan hueco.
    let usados = crate::cola::listar(app)
        .map(|v| v.iter().filter(|i| !i.de_la_casa).count())
        .unwrap_or(0);
    let libre = !crate::compras::disponible() || crate::compras::es_pro();
    InfoCuota {
        usados,
        limite: DOCUMENTOS_GRATIS,
        agotada: !libre && usados >= DOCUMENTOS_GRATIS,
    }
}

/// La puerta del alta: si la percha está llena y no hay cuerda, avisa al
/// caparazón (abre el paywall) y devuelve el error «parlanchin».
pub fn aforo<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<()> {
    let i = info(app);
    if i.agotada {
        tracing::info!("percha llena ({} de {}): pide cuerda", i.usados, i.limite);
        let _ = app.emit("cuota_agotada", i);
        return Err(anyhow::anyhow!("parlanchin"));
    }
    Ok(())
}
