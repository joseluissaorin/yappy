//! LA RESEÑA: pedir una valoración con el aviso NATIVO de la tienda
//! (iOS: `AppStore.requestReview(in:)` en Compras.swift; Android: la Play
//! In-App Review API en Puente.kt). Nunca un diálogo propio con estrellas.
//!
//! Cuándo, y solo entonces:
//!   - tras un momento de satisfacción real: el loro termina de leer un
//!     documento entero (y se le ha escuchado al menos un minuto), o el
//!     oyente pausa después de acumular ≥ 20 min escuchados en ≥ 2 días
//!     distintos;
//!   - nunca durante el paseo (`paseo.hecho` falso), nunca al abrir, nunca
//!     a la sombra del muro (lo comprueba la interfaz antes de pedirla);
//!   - como mucho una vez por versión y con ≥ 30 días entre peticiones
//!     (Apple, además, limita a tres avisos al año);
//!   - nunca en el simulador ni con la tienda de mentira (capturas y
//!     Maestro), salvo `YAPPY_RESENA=1` para ensayarla a propósito.
//!
//! El motor escucha los snapshots de reproducción (funciona con la
//! pantalla bloqueada), cuenta el tiempo escuchado y, cuando llega el
//! momento, avisa a la interfaz con `resena_momento`. La interfaz decide si
//! el instante es bueno (visible, sin muro) y llama a `resena_pedir_cmd`,
//! que vuelve a comprobar las reglas, apunta la petición y la lanza.

// En escritorio no hay tienda: el oído y la libreta solo viven en móvil.
#![cfg_attr(not(mobile), allow(dead_code))]

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
#[cfg(mobile)]
use tauri::{Emitter, Manager};

use crate::playback::PlaybackSnapshot;
use crate::state::AppState;

/// Lo que hay que acumular para el segundo camino: 20 minutos…
pub const MIN_SEGUNDOS: f64 = 20.0 * 60.0;
/// …repartidos en al menos dos días distintos.
pub const MIN_DIAS: usize = 2;
/// Entre dos peticiones, como poco, 30 días.
pub const ESPERA_SEGS: i64 = 30 * 24 * 3600;
/// Un «documento entero» tiene que haberse escuchado al menos un minuto
/// en esa sesión (un titular de dos frases no es un momento de nada).
pub const MIN_SESION_DOC: f64 = 60.0;
/// Un hueco entre snapshots mayor que esto no cuenta entero (la app se
/// durmió, el reloj saltó): se cuenta solo hasta aquí.
const TECHO_TRAMO: f64 = 30.0;
/// Cada cuánto se guarda lo escuchado mientras suena.
const GUARDAR_CADA: f64 = 60.0;
/// Días distintos que se recuerdan (basta con saber que hay dos).
const MAX_DIAS: usize = 8;

/// La libreta de la reseña, dentro de los ajustes (`settings.resena`).
/// Es del MOTOR: `set_settings` la conserva aunque la interfaz mande una
/// copia vieja de los ajustes.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Resena {
    /// Versión de la app en la que se pidió la última vez ("" = nunca).
    pub version: String,
    /// Cuándo (segundos Unix) se pidió la última vez (0 = nunca).
    pub pedida_en: i64,
    /// Cuántas veces se ha pedido en total.
    pub veces: u32,
    /// Segundos escuchados en total (fuera del paseo).
    pub segundos: f64,
    /// Días (desde la época Unix, UTC) con algo escuchado.
    pub dias: Vec<i64>,
}

/// Por qué toca pedirla.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Momento {
    /// Terminó de escuchar un documento entero.
    Documento,
    /// Pausó tras ≥ 20 min escuchados en ≥ 2 días.
    Tiempo,
}

impl Resena {
    /// Suma lo escuchado y apunta el día.
    pub fn sumar(&mut self, segundos: f64, dia: i64) {
        if segundos <= 0.0 {
            return;
        }
        self.segundos += segundos;
        if !self.dias.contains(&dia) {
            self.dias.push(dia);
            self.dias.sort_unstable();
            let sobra = self.dias.len().saturating_sub(MAX_DIAS);
            self.dias.drain(..sobra);
        }
    }

    /// ¿Se puede pedir AHORA por este motivo? La regla entera, sin efectos.
    pub fn toca(&self, momento: Momento, version: &str, ahora: i64, paseo_hecho: bool) -> bool {
        if !paseo_hecho {
            return false;
        }
        if !self.version.is_empty() && self.version == version {
            return false;
        }
        if self.pedida_en > 0 && ahora - self.pedida_en < ESPERA_SEGS {
            return false;
        }
        match momento {
            Momento::Documento => true,
            Momento::Tiempo => self.segundos >= MIN_SEGUNDOS && self.dias.len() >= MIN_DIAS,
        }
    }

    /// Apunta que se pidió.
    pub fn apuntar(&mut self, version: &str, ahora: i64) {
        self.version = version.to_string();
        self.pedida_en = ahora;
        self.veces += 1;
    }
}

/// Lo que importa de un snapshot para el oído de la reseña.
#[derive(Debug, Clone, PartialEq)]
pub struct Instante {
    pub estado: String,
    /// Suena el último párrafo de la sesión.
    pub en_ultimo: bool,
    /// La síntesis ya cocinó todos los trozos.
    pub cocina_terminada: bool,
}

impl Instante {
    pub fn de(s: &PlaybackSnapshot) -> Self {
        Self {
            estado: s.estado.clone(),
            en_ultimo: s.total_paragraphs > 0
                && s.current_paragraph_index + 1 >= s.total_paragraphs,
            cocina_terminada: s.total > 0 && s.chunks_cocinados >= s.total,
        }
    }
}

/// EL OÍDO: sigue los snapshots y cuenta lo escuchado. Puro (el reloj
/// entra de fuera) para poder probarlo.
#[derive(Debug, Default)]
pub struct Oido {
    previo: Option<(Instante, f64)>,
    /// Escuchado en la sesión en curso (se pone a cero al volver a inactivo).
    sesion: f64,
    /// Escuchado y aún no sumado a la libreta.
    pendiente: f64,
}

/// Lo que el oído saca de cada snapshot.
#[derive(Debug, Default, PartialEq)]
pub struct Lectura {
    /// Segundos para sumar a la libreta ahora (0 si toca esperar).
    pub sumar: f64,
    /// Si el oyente acaba de dejar de escuchar, por qué podría tocar.
    pub momento: Option<Momento>,
}

impl Oido {
    /// `t` = segundos monótonos del reloj de quien llama.
    pub fn oir(&mut self, ahora: Instante, t: f64) -> Lectura {
        let mut out = Lectura::default();
        if let Some((antes, t0)) = self.previo.take() {
            if antes.estado == "sonando" {
                let tramo = (t - t0).clamp(0.0, TECHO_TRAMO);
                self.sesion += tramo;
                self.pendiente += tramo;
            }
            let deja = antes.estado == "sonando"
                && ahora.estado != "sonando"
                && ahora.estado != "preparando";
            if deja {
                out.sumar = std::mem::take(&mut self.pendiente);
                let fin_natural =
                    ahora.estado == "inactivo" && antes.en_ultimo && antes.cocina_terminada;
                out.momento = Some(if fin_natural && self.sesion >= MIN_SESION_DOC {
                    Momento::Documento
                } else {
                    Momento::Tiempo
                });
            } else if self.pendiente >= GUARDAR_CADA {
                out.sumar = std::mem::take(&mut self.pendiente);
            }
        }
        if ahora.estado == "inactivo" {
            self.sesion = 0.0;
        }
        self.previo = Some((ahora, t));
        out
    }
}

/// ¿Está apagada la reseña en este proceso? (Simulador, capturas, Maestro.)
pub fn apagada() -> bool {
    match std::env::var("YAPPY_RESENA").as_deref() {
        Ok("1") => return false,
        Ok("0") => return true,
        _ => {}
    }
    if std::env::var("YAPPY_TIENDA").is_ok() {
        return true;
    }
    cfg!(all(target_os = "ios", target_abi = "sim"))
}

fn ahora_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Engancha el oído al motor de reproducción (una vez, al arrancar).
#[cfg(mobile)]
pub fn arrancar(app: AppHandle, state: &Arc<AppState>) {
    let oido = std::sync::Mutex::new(Oido::default());
    let origen = std::time::Instant::now();
    state.playback.subscribe(move |snap| {
        let t = origen.elapsed().as_secs_f64();
        let lectura = oido.lock().unwrap().oir(Instante::de(snap), t);
        if lectura.sumar <= 0.0 && lectura.momento.is_none() {
            return;
        }
        let app = app.clone();
        // Fuera del hilo del motor: aquí se toca disco.
        std::thread::spawn(move || anotar(&app, lectura));
    });
}

#[cfg(mobile)]
fn anotar(app: &AppHandle, lectura: Lectura) {
    let Some(state) = app.try_state::<Arc<AppState>>() else {
        return;
    };
    let state = state.inner().clone();
    let paseo_hecho = state.settings.lock().unwrap().paseo.hecho;
    // Lo escuchado durante el paseo no cuenta: el paseo no es uso.
    if paseo_hecho && lectura.sumar > 0.0 {
        let dia = ahora_unix().div_euclid(86_400);
        let _ = crate::settings::update(app, &state, |s| s.resena.sumar(lectura.sumar, dia));
    }
    let Some(momento) = lectura.momento else {
        return;
    };
    if apagada() {
        return;
    }
    let toca = {
        let s = state.settings.lock().unwrap();
        s.resena
            .toca(momento, &version(app), ahora_unix(), s.paseo.hecho)
    };
    if toca {
        let _ = app.emit("resena_momento", momento);
    }
}

/// La interfaz confirma que el instante es bueno: se vuelven a comprobar
/// las reglas, se apunta y se lanza el aviso nativo. Devuelve si se pidió
/// (la interfaz mide `review_requested` solo entonces).
#[tauri::command]
pub fn resena_pedir_cmd(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    motivo: Momento,
) -> bool {
    if apagada() || !cfg!(mobile) {
        return false;
    }
    let ver = version(&app);
    let ahora = ahora_unix();
    let ok = {
        let s = state.settings.lock().unwrap();
        s.resena.toca(motivo, &ver, ahora, s.paseo.hecho)
    };
    if !ok {
        return false;
    }
    if crate::settings::update(&app, state.inner(), |s| s.resena.apuntar(&ver, ahora)).is_err() {
        return false;
    }
    tracing::info!("reseña: se pide el aviso nativo ({motivo:?}, versión {ver})");
    #[cfg(mobile)]
    crate::mobile::pedir_resena();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn i(estado: &str, en_ultimo: bool, cocina: bool) -> Instante {
        Instante {
            estado: estado.into(),
            en_ultimo,
            cocina_terminada: cocina,
        }
    }

    #[test]
    fn nunca_durante_el_paseo() {
        let r = Resena::default();
        assert!(!r.toca(Momento::Documento, "1.0.2", 1_000_000, false));
        assert!(r.toca(Momento::Documento, "1.0.2", 1_000_000, true));
    }

    #[test]
    fn una_vez_por_version_y_treinta_dias() {
        let mut r = Resena::default();
        let t0 = 2_000_000_000;
        r.apuntar("1.0.2", t0);
        assert!(!r.toca(Momento::Documento, "1.0.2", t0 + ESPERA_SEGS * 3, true));
        assert!(!r.toca(Momento::Documento, "1.0.3", t0 + ESPERA_SEGS - 1, true));
        assert!(r.toca(Momento::Documento, "1.0.3", t0 + ESPERA_SEGS, true));
        assert_eq!(r.veces, 1);
    }

    #[test]
    fn el_tiempo_pide_veinte_minutos_en_dos_dias() {
        let mut r = Resena::default();
        r.sumar(25.0 * 60.0, 100);
        assert!(
            !r.toca(Momento::Tiempo, "1.0.2", 0, true),
            "un solo día no basta"
        );
        r.sumar(1.0, 100);
        assert_eq!(r.dias, vec![100]);
        r.sumar(10.0, 101);
        assert!(r.toca(Momento::Tiempo, "1.0.2", 0, true));
        let mut poco = Resena::default();
        poco.sumar(5.0 * 60.0, 1);
        poco.sumar(5.0 * 60.0, 2);
        assert!(
            !poco.toca(Momento::Tiempo, "1.0.2", 0, true),
            "diez minutos no bastan"
        );
    }

    #[test]
    fn los_dias_no_crecen_sin_fin() {
        let mut r = Resena::default();
        for d in 0..50 {
            r.sumar(1.0, d);
        }
        assert_eq!(r.dias.len(), MAX_DIAS);
        assert_eq!(*r.dias.last().unwrap(), 49);
    }

    #[test]
    fn el_oido_cuenta_y_reconoce_el_fin_natural() {
        let mut o = Oido::default();
        assert_eq!(
            o.oir(i("preparando", false, false), 0.0),
            Lectura::default()
        );
        o.oir(i("sonando", false, false), 1.0);
        // Snapshots cada pocos segundos durante 90 s.
        let mut sumado = 0.0;
        for k in 1..=30 {
            sumado += o
                .oir(i("sonando", k == 30, true), 1.0 + k as f64 * 3.0)
                .sumar;
        }
        let fin = o.oir(i("inactivo", false, false), 94.0);
        sumado += fin.sumar;
        assert_eq!(fin.momento, Some(Momento::Documento));
        assert!((sumado - 93.0).abs() < 1e-6, "sumado {sumado}");
    }

    #[test]
    fn pausar_no_es_terminar() {
        let mut o = Oido::default();
        o.oir(i("sonando", true, true), 0.0);
        let l = o.oir(i("pausa", true, true), 120.0);
        assert_eq!(l.momento, Some(Momento::Tiempo));
        // El tramo largo se recorta al techo (la app pudo dormirse).
        assert!((l.sumar - TECHO_TRAMO).abs() < 1e-6);
    }

    #[test]
    fn un_documento_cortito_no_es_un_momento_de_documento() {
        let mut o = Oido::default();
        o.oir(i("sonando", true, true), 0.0);
        let l = o.oir(i("inactivo", false, false), 10.0);
        assert_eq!(l.momento, Some(Momento::Tiempo));
    }

    #[test]
    fn mientras_suena_guarda_cada_minuto() {
        let mut o = Oido::default();
        o.oir(i("sonando", false, false), 0.0);
        let mut guardados = 0;
        for k in 1..=40 {
            if o.oir(i("sonando", false, false), k as f64 * 5.0).sumar > 0.0 {
                guardados += 1;
            }
        }
        assert_eq!(guardados, 3, "200 s sonando = tres guardados de un minuto");
    }
}
