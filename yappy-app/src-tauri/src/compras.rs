//! LAS COMPRAS: Yappy Parlanchín por RevenueCat. En iOS el SDK vive en
//! Swift (gen/apple/Sources/yappy-app/Compras.swift) y se habla por el
//! puente C de la casa: cada petición lleva un número, Swift responde con
//! JSON por un callback registrado al arrancar, y aquí una tabla de
//! `oneshot` casa respuesta con petición. El estado «pro» se cachea en un
//! atómico que Swift refresca cada vez que RevenueCat cambia de opinión.
//!
//! En escritorio y Android NO hay tienda: `es_pro()` es verdadero, la cuota
//! no muerde y el paywall no existe.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use once_cell::sync::{Lazy, OnceCell};
use tauri::{AppHandle, Emitter};

/// La clave PÚBLICA del SDK (la de la app «Yappy (App Store)» en RevenueCat).
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub const CLAVE_PUBLICA: &str = "appl_tnZmqVSbmvnKkZZqgkveJEJTTGl";
/// La entitlement: tal cual está en RevenueCat (con su tilde).
pub const ENTITLEMENT: &str = "yappy_parlanchín";

static PRO: AtomicBool = AtomicBool::new(false);
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
static SIGUIENTE: AtomicU64 = AtomicU64::new(1);
static PENDIENTES: Lazy<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<String>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static APP: OnceCell<AppHandle> = OnceCell::new();
/// Simulación de tienda (solo en depuración): ofertas de mentira y un «pro»
/// que se enciende a mano, para ensayar el paywall en el simulador.
#[cfg(debug_assertions)]
static SIMULADA: AtomicBool = AtomicBool::new(false);

/// ¿Hay tienda en esta plataforma?
pub fn disponible() -> bool {
    cfg!(target_os = "ios")
}

/// ¿Es parlanchín? Donde no hay tienda, siempre.
pub fn es_pro() -> bool {
    if !disponible() {
        return true;
    }
    PRO.load(Ordering::SeqCst)
}

#[derive(serde::Serialize, Clone)]
pub struct EstadoCompras {
    pub disponible: bool,
    pub pro: bool,
    pub cuota: crate::cuota::InfoCuota,
    pub entitlement: &'static str,
}

fn fijar_pro(app: Option<&AppHandle>, pro: bool) {
    let antes = PRO.swap(pro, Ordering::SeqCst);
    if let Some(app) = app {
        let _ = app.emit("pro_cambio", pro);
    }
    if antes != pro {
        tracing::info!("compras: parlanchín = {pro}");
    }
}

// ─── El puente C (solo iOS) ─────────────────────────────────────────────
#[cfg(target_os = "ios")]
mod ffi {
    use std::os::raw::c_char;
    extern "C" {
        pub fn yappy_compras_configurar(api_key: *const c_char, entitlement: *const c_char);
        pub fn yappy_compras_register_resultado(cb: extern "C" fn(u64, *const c_char));
        pub fn yappy_compras_register_pro(cb: extern "C" fn(bool));
        pub fn yappy_compras_es_pro() -> bool;
        pub fn yappy_compras_ofertas(id: u64);
        pub fn yappy_compras_comprar(id: u64, paquete: *const c_char);
        pub fn yappy_compras_restaurar(id: u64);
        pub fn yappy_compras_cliente(id: u64);
        pub fn yappy_compras_gestionar();
    }
}

extern "C" fn cb_resultado(id: u64, json: *const std::os::raw::c_char) {
    let s = if json.is_null() {
        String::from("{\"error\":\"respuesta vacía\"}")
    } else {
        unsafe { std::ffi::CStr::from_ptr(json) }
            .to_string_lossy()
            .into_owned()
    };
    if let Some(tx) = PENDIENTES.lock().unwrap().remove(&id) {
        let _ = tx.send(s);
    }
}

extern "C" fn cb_pro(pro: bool) {
    // En el ensayo (tienda simulada) el SDK real no manda: su callback
    // tardío pisaba la cuerda de mentira y el paywall volvía a salir.
    #[cfg(debug_assertions)]
    if SIMULADA.load(Ordering::SeqCst) {
        return;
    }
    fijar_pro(APP.get(), pro);
}

/// Arranque: registra los callbacks y configura el SDK. Llamar una vez.
pub fn arrancar(app: AppHandle) {
    let _ = APP.set(app);
    #[cfg(target_os = "ios")]
    unsafe {
        use std::ffi::CString;
        ffi::yappy_compras_register_resultado(cb_resultado);
        ffi::yappy_compras_register_pro(cb_pro);
        let k = CString::new(CLAVE_PUBLICA).unwrap();
        let e = CString::new(ENTITLEMENT).unwrap();
        ffi::yappy_compras_configurar(k.as_ptr(), e.as_ptr());
        // La caché del SDK ya sabe si eras parlanchín: sin esperar a la red.
        fijar_pro(None, ffi::yappy_compras_es_pro());
        tracing::info!("compras: RevenueCat configurado (pro={})", es_pro());
    }
    // Ensayo desde fuera (solo depuración): `SIMCTL_CHILD_YAPPY_TIENDA=sim`
    // al lanzar en el simulador enciende la tienda de mentira sin tocar la
    // interfaz; `YAPPY_TIENDA_PRO=1` arranca ya con cuerda.
    #[cfg(debug_assertions)]
    if std::env::var("YAPPY_TIENDA")
        .map(|v| v == "sim")
        .unwrap_or(false)
    {
        SIMULADA.store(true, Ordering::SeqCst);
        let pro = std::env::var("YAPPY_TIENDA_PRO")
            .map(|v| v == "1")
            .unwrap_or(false);
        PRO.store(pro, Ordering::SeqCst);
        tracing::info!("compras: tienda SIMULADA por entorno (pro={pro})");
        // `YAPPY_TIENDA_MOTIVO=percha`: a los cuatro segundos, la percha se
        // declara llena (el evento real del aforo), para ver esa viñeta.
        if std::env::var("YAPPY_TIENDA_MOTIVO")
            .map(|v| v == "percha")
            .unwrap_or(false)
        {
            if let Some(app) = APP.get().cloned() {
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(4));
                    let mut info = crate::cuota::info(&app);
                    info.agotada = true;
                    info.usados = info.limite;
                    let _ = app.emit("cuota_agotada", info);
                });
            }
        }
    }
    #[cfg(not(target_os = "ios"))]
    {
        // Sin tienda: nadie está a prueba.
        let _ = (
            cb_resultado as extern "C" fn(u64, *const std::os::raw::c_char),
            cb_pro as extern "C" fn(bool),
        );
        PRO.store(true, Ordering::SeqCst);
    }
}

/// Lanza una petición al puente y espera su JSON (con techo de tiempo: una
/// hoja de StoreKit puede quedarse abierta minutos; más allá, se rinde).
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
async fn pedir(
    lanzar: impl FnOnce(u64),
    techo: std::time::Duration,
) -> Result<serde_json::Value, String> {
    let id = SIGUIENTE.fetch_add(1, Ordering::SeqCst);
    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    PENDIENTES.lock().unwrap().insert(id, tx);
    lanzar(id);
    match tokio::time::timeout(techo, rx).await {
        Ok(Ok(s)) => serde_json::from_str(&s).map_err(|e| format!("json de la tienda: {e}")),
        Ok(Err(_)) => Err("la tienda no respondió".into()),
        Err(_) => {
            PENDIENTES.lock().unwrap().remove(&id);
            Err("la tienda tardó demasiado".into())
        }
    }
}

#[cfg(debug_assertions)]
fn ofertas_simuladas() -> serde_json::Value {
    serde_json::json!({
        "paquetes": [
            { "id": "$rc_monthly", "tipo": "mensual", "producto": "com.joseluissaorin.yappy.parlanchin.mensual", "precio": "4,00 €", "precio_num": 4.0, "moneda": "EUR", "periodo": "P1M", "intro": null },
            { "id": "$rc_annual", "tipo": "anual", "producto": "com.joseluissaorin.yappy.parlanchin.anual", "precio": "30,00 €", "precio_num": 30.0, "moneda": "EUR", "periodo": "P1Y", "intro": null },
            { "id": "$rc_lifetime", "tipo": "vida", "producto": "com.joseluissaorin.yappy.parlanchin.vida", "precio": "60,00 €", "precio_num": 60.0, "moneda": "EUR", "periodo": null, "intro": null }
        ],
        "simulado": true
    })
}

// ─── Comandos ───────────────────────────────────────────────────────────

/// El identificador anónimo de la tienda (para unir las estadísticas
/// anónimas con la cuerda). None sin tienda.
#[tauri::command]
pub fn compras_usuario_cmd() -> Option<String> {
    #[cfg(target_os = "ios")]
    {
        crate::mobile::compras_usuario()
    }
    #[cfg(not(target_os = "ios"))]
    {
        None
    }
}

#[tauri::command]
pub fn compras_estado_cmd(app: AppHandle) -> EstadoCompras {
    EstadoCompras {
        disponible: disponible(),
        pro: es_pro(),
        cuota: crate::cuota::info(&app),
        entitlement: ENTITLEMENT,
    }
}

#[tauri::command]
pub async fn compras_ofertas_cmd() -> Result<serde_json::Value, String> {
    #[cfg(debug_assertions)]
    if SIMULADA.load(Ordering::SeqCst) {
        return Ok(ofertas_simuladas());
    }
    #[cfg(target_os = "ios")]
    {
        pedir(
            |id| unsafe { ffi::yappy_compras_ofertas(id) },
            std::time::Duration::from_secs(30),
        )
        .await
    }
    #[cfg(not(target_os = "ios"))]
    {
        Err("sin tienda en esta plataforma".into())
    }
}

#[tauri::command]
pub async fn compras_comprar_cmd(paquete: String) -> Result<serde_json::Value, String> {
    #[cfg(debug_assertions)]
    if SIMULADA.load(Ordering::SeqCst) {
        // El ensayo: dos segundos de «hoja de la tienda» y compra hecha.
        tokio::time::sleep(std::time::Duration::from_millis(1800)).await;
        fijar_pro(APP.get(), true);
        return Ok(serde_json::json!({ "ok": true, "pro": true, "paquete": paquete }));
    }
    #[cfg(target_os = "ios")]
    {
        let c = std::ffi::CString::new(paquete).map_err(|e| e.to_string())?;
        let r = pedir(
            |id| unsafe { ffi::yappy_compras_comprar(id, c.as_ptr()) },
            std::time::Duration::from_secs(600),
        )
        .await?;
        if let Some(pro) = r.get("pro").and_then(|v| v.as_bool()) {
            fijar_pro(APP.get(), pro);
        }
        Ok(r)
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = paquete;
        Err("sin tienda en esta plataforma".into())
    }
}

#[tauri::command]
pub async fn compras_restaurar_cmd() -> Result<serde_json::Value, String> {
    #[cfg(debug_assertions)]
    if SIMULADA.load(Ordering::SeqCst) {
        tokio::time::sleep(std::time::Duration::from_millis(900)).await;
        return Ok(serde_json::json!({ "ok": true, "pro": es_pro() }));
    }
    #[cfg(target_os = "ios")]
    {
        let r = pedir(
            |id| unsafe { ffi::yappy_compras_restaurar(id) },
            std::time::Duration::from_secs(120),
        )
        .await?;
        if let Some(pro) = r.get("pro").and_then(|v| v.as_bool()) {
            fijar_pro(APP.get(), pro);
        }
        Ok(r)
    }
    #[cfg(not(target_os = "ios"))]
    {
        Err("sin tienda en esta plataforma".into())
    }
}

#[tauri::command]
pub async fn compras_cliente_cmd() -> Result<serde_json::Value, String> {
    #[cfg(debug_assertions)]
    if SIMULADA.load(Ordering::SeqCst) {
        return Ok(
            serde_json::json!({ "pro": es_pro(), "producto": "com.joseluissaorin.yappy.parlanchin.anual", "desde": "2026-09-09T10:00:00Z", "expira": "2027-09-09T10:00:00Z", "renovara": true, "gestionar": null, "simulado": true }),
        );
    }
    #[cfg(target_os = "ios")]
    {
        let r = pedir(
            |id| unsafe { ffi::yappy_compras_cliente(id) },
            std::time::Duration::from_secs(30),
        )
        .await?;
        if let Some(pro) = r.get("pro").and_then(|v| v.as_bool()) {
            fijar_pro(APP.get(), pro);
        }
        Ok(r)
    }
    #[cfg(not(target_os = "ios"))]
    {
        Ok(serde_json::json!({ "pro": true, "sin_tienda": true }))
    }
}

/// Abre la hoja del sistema para gestionar la suscripción.
#[tauri::command]
pub fn compras_gestionar_cmd() {
    #[cfg(target_os = "ios")]
    unsafe {
        ffi::yappy_compras_gestionar();
    }
}

/// SOLO DEPURACIÓN: enciende la tienda de mentira (ofertas fijas, compra
/// que siempre sale bien) y fija «pro» a mano. Para ensayar el paywall en
/// el simulador, donde StoreKit no tiene cuenta de sandbox.
#[tauri::command]
pub fn compras_simular_cmd(activa: bool, pro: bool) -> Result<(), String> {
    #[cfg(debug_assertions)]
    {
        SIMULADA.store(activa, Ordering::SeqCst);
        fijar_pro(APP.get(), pro);
        Ok(())
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = (activa, pro);
        Err("solo en depuración".into())
    }
}
