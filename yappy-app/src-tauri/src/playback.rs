//! Audio playback queue with volume, seek, and an export-friendly samples buffer.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{unbounded, Receiver, Sender};
use rubato::{FftFixedInOut, Resampler};
use serde::{Deserialize, Serialize};


/// Oyentes del snapshot de reproducción.
type Oyentes = Arc<Mutex<Vec<Box<dyn Fn(&PlaybackSnapshot) + Send + Sync>>>>;
/// Oyentes del nivel (RMS 0..1 de la ventana que suena).
type OyentesNivel = Arc<Mutex<Vec<Box<dyn Fn(f32) + Send + Sync>>>>;

#[derive(Debug, Clone)]
pub struct AudioChunk {
    #[allow(dead_code)] // trazabilidad del origen del trozo
    pub index: usize,
    pub paragraph_index: usize,
    pub total: usize,
    pub total_paragraphs: usize,
    pub text: String,
    pub origen_ini: usize,
    pub origen_fin: usize,
    pub samples: Vec<f32>,
    pub source_sample_rate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackSnapshot {
    /// La máquina de verdad: "inactivo" | "preparando" | "sonando" | "pausa".
    /// La interfaz pinta ESTO y nada más; playing/paused quedan por
    /// compatibilidad con el escritorio.
    pub estado: String,
    /// Revisión monótona: crece en cada emisión. La interfaz descarta todo
    /// evento con revisión menor que la última pintada (mata las carreras
    /// donde un evento rezagado resucita un estado muerto).
    pub revision: u64,
    /// Título de la sesión de lectura (el que enseñan la aguja y la
    /// pantalla de bloqueo). Vacío si no hay sesión.
    pub titulo: String,
    /// Ruta del documento de la sesión ("" si es texto suelto). Permite a la
    /// aguja restaurar el documento al tocarla aunque el lector se haya
    /// quedado sin estado (relanzamiento de la app).
    pub doc_path: String,
    /// La cocina visible: cuántos trozos hay ya sintetizados.
    pub chunks_cocinados: usize,
    /// Hasta qué párrafo (índice relativo a la sesión) hay audio cocinado.
    /// El mando decide con esto si un salto puede ser instantáneo (dentro de
    /// lo sintetizado) o necesita resintetizar.
    pub parrafo_max_cocinado: usize,
    pub playing: bool,
    pub paused: bool,
    pub current_text: String,
    pub current_index: usize,
    /// Which PARAGRAPH the currently-playing chunk belongs to (for karaoke
    /// highlight in the document window). Multiple chunks can share a paragraph
    /// when the engine sentence-splits a long paragraph.
    pub current_paragraph_index: usize,
    /// Rango (en caracteres) del texto ORIGINAL del párrafo que corresponde
    /// al trozo que suena ahora mismo. El karaoke subraya esto, no busca
    /// substrings: funciona aunque la verbalización haya cambiado el texto.
    pub current_origen_ini: usize,
    pub current_origen_fin: usize,
    pub total: usize,
    pub total_paragraphs: usize,
    pub elapsed_secs: f32,
    pub duration_secs: f32,
    pub volume: f32,
    pub output_sample_rate: u32,
}

#[derive(Debug)]
enum Command {
    /// La sesión existe pero aún no suena: la síntesis del primer trozo está
    /// en marcha. Fija título/ruta y pone estado=preparando. Con session_id
    /// para que un Preparando rezagado de una sesión muerta no reviva nada.
    Preparando { session_id: u64, titulo: String, doc_path: String },
    /// La síntesis murió antes del primer trozo: si seguimos en preparando
    /// de ESA sesión, volver a inactivo (sin esto el estado se queda
    /// colgado en «preparando» para siempre).
    Fallo { session_id: u64 },
    /// Begin a new session. The session_id is the id this synth task was started with;
    /// the audio thread accepts it iff it matches the controller's `current_session`.
    /// `arranque_pausado`: reposicionar sin sonar (saltar desde el guion en pausa).
    NewSession { session_id: u64, chunks: Vec<AudioChunk>, arranque_pausado: bool },
    Enqueue { session_id: u64, chunk: AudioChunk },
    Pause,
    Resume,
    Stop,
    SetVolume(f32),
    SeekSecs(f32),
    /// Salto por FRASE dentro de lo ya sintetizado: instantáneo, sin
    /// resíntesis. delta=+1/-1. Atrás respeta la convención musical: pasado
    /// 1,2 s dentro de la frase, vuelve al principio de la actual.
    SaltarChunk { delta: i32 },
    /// Salto por PÁRRAFO dentro de lo ya sintetizado.
    SaltarParrafo { delta: i32 },
}

pub struct PlaybackController {
    cmd_tx: Sender<Command>,
    snapshot: Arc<Mutex<PlaybackSnapshot>>,
    /// All samples synthesised in the current session, at the device's output sample rate.
    /// Used by "save as audio".
    session_samples: Arc<Mutex<Vec<f32>>>,
    listeners: Oyentes,
    /// Monotonically increasing session id. Bumped on every Stop and every NewSession start.
    /// Synth tasks capture the id at start; if `current_session` later differs, they abort
    /// and the audio thread drops their Enqueue commands. This is what makes Stop *definitive*
    /// — without it, the synth loop keeps generating chunks that get played after Stop.
    session_id: Arc<AtomicU64>,
    /// Oyentes del NIVEL (RMS 0..1 de la ventana que está sonando, ~20 Hz):
    /// el sistema nervioso de la interfaz viva (pico del loro, tipografía
    /// que respira, latido de la aguja).
    #[allow(dead_code)] // lo consume el arranque móvil (cfg(mobile) en lib.rs)
    nivel_listeners: OyentesNivel,
}

impl PlaybackController {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = unbounded::<Command>();
        let snapshot = Arc::new(Mutex::new(PlaybackSnapshot {
            estado: "inactivo".into(),
            revision: 0,
            titulo: String::new(),
            doc_path: String::new(),
            chunks_cocinados: 0,
            parrafo_max_cocinado: 0,
            playing: false,
            paused: false,
            current_text: String::new(),
            current_index: 0,
            current_paragraph_index: 0,
            current_origen_ini: 0,
            current_origen_fin: 0,
            total: 0,
            total_paragraphs: 0,
            elapsed_secs: 0.0,
            duration_secs: 0.0,
            volume: 1.0,
            output_sample_rate: 44100,
        }));
        let listeners: Oyentes =
            Arc::new(Mutex::new(Vec::new()));
        let session_samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let session_id = Arc::new(AtomicU64::new(0));

        // iOS: cpal/CoreAudio only actually outputs when an AVAudioSession is
        // active in the .playback category. Activate it BEFORE the audio thread
        // builds the output stream — otherwise TTS "plays" silently (the
        // playback clock never advances). Mirrors what the render keepalive does.
        #[cfg(target_os = "ios")]
        crate::mobile::audio_session_activate();

        let nivel_listeners: OyentesNivel =
            Arc::new(Mutex::new(Vec::new()));
        let snap_for_thread = snapshot.clone();
        let listeners_for_thread = listeners.clone();
        let session_for_thread = session_samples.clone();
        let session_id_for_thread = session_id.clone();
        let nivel_for_thread = nivel_listeners.clone();
        std::thread::Builder::new()
            .name("yappy-audio".into())
            .spawn(move || {
                if let Err(e) = run_audio_thread(
                    cmd_rx,
                    snap_for_thread,
                    listeners_for_thread,
                    session_for_thread,
                    session_id_for_thread,
                    nivel_for_thread,
                ) {
                    tracing::error!("audio thread exited: {e:?}");
                }
            })
            .expect("spawn audio thread");

        Self {
            cmd_tx,
            snapshot,
            session_samples,
            listeners,
            session_id,
            nivel_listeners,
        }
    }

    /// Claim a fresh session id, atomically. The caller (the synth orchestrator) holds onto
    /// this for every Enqueue it sends; the audio thread drops Enqueue from stale sessions.
    pub fn begin_session(&self) -> u64 {
        // Bump first so any in-flight synth task notices the change.
        self.session_id.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Read the live session id. Synth tasks call this every chunk to know whether to
    /// keep producing or to abort early (e.g. user pressed Stop).
    pub fn current_session(&self) -> u64 {
        self.session_id.load(Ordering::SeqCst)
    }

    /// Declara la sesión en «preparando» (título + ruta) antes de que exista
    /// el primer trozo. La cocina se ve desde el primer milisegundo.
    pub fn preparando(&self, session_id: u64, titulo: &str, doc_path: &str) {
        let _ = self.cmd_tx.send(Command::Preparando {
            session_id,
            titulo: titulo.to_string(),
            doc_path: doc_path.to_string(),
        });
    }
    /// La síntesis murió antes del primer trozo de esta sesión.
    pub fn fallo(&self, session_id: u64) {
        let _ = self.cmd_tx.send(Command::Fallo { session_id });
    }
    pub fn new_session(&self, session_id: u64, chunks: Vec<AudioChunk>, arranque_pausado: bool) {
        let _ = self
            .cmd_tx
            .send(Command::NewSession { session_id, chunks, arranque_pausado });
    }
    pub fn enqueue(&self, session_id: u64, chunk: AudioChunk) {
        let _ = self.cmd_tx.send(Command::Enqueue { session_id, chunk });
    }
    pub fn saltar_chunk(&self, delta: i32) {
        let _ = self.cmd_tx.send(Command::SaltarChunk { delta });
    }
    pub fn saltar_parrafo(&self, delta: i32) {
        let _ = self.cmd_tx.send(Command::SaltarParrafo { delta });
    }
    pub fn pause(&self) {
        let _ = self.cmd_tx.send(Command::Pause);
    }
    pub fn resume(&self) {
        let _ = self.cmd_tx.send(Command::Resume);
    }
    pub fn stop(&self) {
        // Invalidate any in-flight synth task FIRST so the very next chunk it produces is
        // recognised as stale and dropped — even if the Stop command hasn't reached the
        // audio thread yet.
        self.session_id.fetch_add(1, Ordering::SeqCst);
        let _ = self.cmd_tx.send(Command::Stop);
    }
    pub fn set_volume(&self, v: f32) {
        let _ = self.cmd_tx.send(Command::SetVolume(v));
    }
    pub fn seek(&self, delta_secs: f32) {
        let _ = self.cmd_tx.send(Command::SeekSecs(delta_secs));
    }
    pub fn snapshot(&self) -> PlaybackSnapshot {
        self.snapshot.lock().unwrap().clone()
    }
    /// All synth output for the current session, at the device's output sample rate.
    pub fn session_audio(&self) -> (Vec<f32>, u32) {
        (
            self.session_samples.lock().unwrap().clone(),
            self.snapshot.lock().unwrap().output_sample_rate,
        )
    }
    pub fn subscribe<F: Fn(&PlaybackSnapshot) + Send + Sync + 'static>(&self, f: F) {
        self.listeners.lock().unwrap().push(Box::new(f));
    }
    #[allow(dead_code)] // lo consume el arranque móvil (cfg(mobile) en lib.rs)
    pub fn subscribe_nivel<F: Fn(f32) + Send + Sync + 'static>(&self, f: F) {
        self.nivel_listeners.lock().unwrap().push(Box::new(f));
    }
}

fn run_audio_thread(
    cmd_rx: Receiver<Command>,
    snapshot: Arc<Mutex<PlaybackSnapshot>>,
    listeners: Oyentes,
    session_samples: Arc<Mutex<Vec<f32>>>,
    live_session_id: Arc<AtomicU64>,
    nivel_listeners: OyentesNivel,
) -> Result<()> {
    // Android: cpal (AAudio) necesita el contexto NDK que Tauri inicializa
    // en su arranque; este hilo puede llegar antes. Esperar a que exista en
    // vez de reventar (el pánico de ndk-context mataba el hilo de audio).
    #[cfg(target_os = "android")]
    {
        let mut intentos = 0;
        while std::panic::catch_unwind(|| {
            let _ = ndk_context::android_context();
        })
        .is_err()
        {
            intentos += 1;
            if intentos > 100 {
                return Err(anyhow!("contexto Android nunca llegó"));
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow!("no default audio output"))?;
    let supported = device.default_output_config()?;
    let out_sr = supported.sample_rate().0;
    let channels = supported.channels() as usize;
    let sample_format = supported.sample_format();
    {
        let mut s = snapshot.lock().unwrap();
        s.output_sample_rate = out_sr;
    }

    // Shared state for the audio callback.
    let buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let buffer_cb = buffer.clone();
    let paused = Arc::new(Mutex::new(false));
    let paused_cb = paused.clone();
    let played_samples = Arc::new(Mutex::new(0u64));
    let played_cb = played_samples.clone();
    let volume = Arc::new(Mutex::new(1.0f32));
    let volume_cb = volume.clone();

    let mut config = supported.config();
    config.buffer_size = cpal::BufferSize::Default;

    let err_fn = |e| tracing::error!("cpal stream error: {e}");

    let stream = match sample_format {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                fill::<f32>(
                    data,
                    channels,
                    &buffer_cb,
                    &paused_cb,
                    &played_cb,
                    &volume_cb,
                    |v| v,
                );
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config,
            move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                fill::<i16>(
                    data,
                    channels,
                    &buffer_cb,
                    &paused_cb,
                    &played_cb,
                    &volume_cb,
                    |v| (v.clamp(-1.0, 1.0) * 32767.0) as i16,
                );
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::U16 => device.build_output_stream(
            &config,
            move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                fill::<u16>(
                    data,
                    channels,
                    &buffer_cb,
                    &paused_cb,
                    &played_cb,
                    &volume_cb,
                    |v| ((v.clamp(-1.0, 1.0) * 32767.0) + 32768.0) as u16,
                );
            },
            err_fn,
            None,
        )?,
        _ => return Err(anyhow!("unsupported sample format {:?}", sample_format)),
    };
    stream.play()?;

    let mut session_total = 0usize;
    let mut current_index = 0usize;
    let mut current_text = String::new();
    let mut session_duration_samples: u64 = 0;
    let mut paused_state = false;

    // ─── KARAOKE SYNC: track which chunk is *playing*, not which is synth'd ─
    // Each chunk's audio is appended sequentially. `chunk_boundaries[i]` is the
    // cumulative end-sample-offset for the i-th chunk in the current session.
    // `chunk_texts[i]` is the text for that chunk. On every audio tick we look
    // at played_samples and figure out which chunk it falls within — that's the
    // chunk currently being heard. Without this, current_index reflects synth
    // completion (which runs 5-10× ahead of playback) and karaoke jumps wildly.
    let mut chunk_boundaries: Vec<u64> = Vec::new();
    let mut chunk_texts: Vec<String> = Vec::new();
    let mut chunk_paragraph_idx: Vec<usize> = Vec::new();
    let mut chunk_origen: Vec<(usize, usize)> = Vec::new();
    let mut current_paragraph_index: usize = 0;
    let mut total_paragraphs: usize = 0;
    let mut nivel_anterior: f32 = 0.0;

    let emit = |snapshot: &Arc<Mutex<PlaybackSnapshot>>,
                listeners: &Oyentes| {
        // La revisión crece en CADA emisión: la interfaz descarta lo viejo.
        let snap = {
            let mut s = snapshot.lock().unwrap();
            s.revision += 1;
            s.clone()
        };
        for f in listeners.lock().unwrap().iter() {
            f(&snap);
        }
    };

    // Reposiciona la reproducción al principio del chunk `destino` (índices
    // de la sesión actual). No toca paused: saltar en pausa deja en pausa.
    let reposicionar = |destino: usize,
                        chunk_boundaries: &[u64],
                        chunk_texts: &[String],
                        chunk_paragraph_idx: &[usize],
                        chunk_origen: &[(usize, usize)],
                        session_samples: &Arc<Mutex<Vec<f32>>>,
                        buffer: &Arc<Mutex<Vec<f32>>>,
                        played_samples: &Arc<Mutex<u64>>,
                        snapshot: &Arc<Mutex<PlaybackSnapshot>>,
                        out_sr: u32|
     -> (usize, String, usize) {
        let inicio = if destino == 0 { 0 } else { chunk_boundaries[destino - 1] } as usize;
        let sesion = session_samples.lock().unwrap();
        let inicio = inicio.min(sesion.len());
        *played_samples.lock().unwrap() = inicio as u64;
        {
            let mut buf = buffer.lock().unwrap();
            buf.clear();
            buf.extend_from_slice(&sesion[inicio..]);
        }
        drop(sesion);
        let texto = chunk_texts.get(destino).cloned().unwrap_or_default();
        let parrafo = chunk_paragraph_idx.get(destino).copied().unwrap_or(0);
        let (oi, of) = chunk_origen.get(destino).copied().unwrap_or((0, 0));
        {
            let mut s = snapshot.lock().unwrap();
            s.current_index = destino;
            s.current_text = texto.clone();
            s.current_paragraph_index = parrafo;
            s.current_origen_ini = oi;
            s.current_origen_fin = of;
            s.elapsed_secs = inicio as f32 / out_sr as f32;
        }
        (destino, texto, parrafo)
    };

    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                Command::Preparando { session_id, titulo, doc_path } => {
                    if session_id != live_session_id.load(Ordering::SeqCst) {
                        continue;
                    }
                    {
                        let mut s = snapshot.lock().unwrap();
                        s.estado = "preparando".into();
                        s.titulo = titulo;
                        s.doc_path = doc_path;
                        s.chunks_cocinados = 0;
                        s.parrafo_max_cocinado = 0;
                    }
                    emit(&snapshot, &listeners);
                }
                Command::Fallo { session_id } => {
                    if session_id != live_session_id.load(Ordering::SeqCst) {
                        continue;
                    }
                    let era_preparando = snapshot.lock().unwrap().estado == "preparando";
                    if era_preparando {
                        let mut s = snapshot.lock().unwrap();
                        s.estado = "inactivo".into();
                        s.titulo.clear();
                        s.doc_path.clear();
                        drop(s);
                        emit(&snapshot, &listeners);
                    }
                }
                Command::NewSession { session_id, chunks, arranque_pausado } => {
                    // Drop sessions that were already invalidated by a Stop that raced ahead.
                    if session_id != live_session_id.load(Ordering::SeqCst) {
                        continue;
                    }
                    // iOS: (re)activate the audio session right as playback begins.
                    // The Now-Playing bridge can deactivate it while idle, and
                    // cpal's RemoteIO output unit produces nothing if the session
                    // isn't active — so re-assert it here, not just at startup.
                    #[cfg(target_os = "ios")]
                    crate::mobile::audio_session_activate();
                    *buffer.lock().unwrap() = Vec::new();
                    *session_samples.lock().unwrap() = Vec::new();
                    session_total = chunks.first().map(|c| c.total).unwrap_or(0);
                    total_paragraphs = chunks.first().map(|c| c.total_paragraphs).unwrap_or(0);
                    *played_samples.lock().unwrap() = 0;
                    session_duration_samples = 0;
                    chunk_boundaries.clear();
                    chunk_texts.clear();
                    chunk_paragraph_idx.clear();
                    chunk_origen.clear();
                    for chunk in chunks {
                        let resampled = if chunk.source_sample_rate != out_sr {
                            resample_mono(&chunk.samples, chunk.source_sample_rate, out_sr)?
                        } else {
                            chunk.samples.clone()
                        };
                        session_duration_samples += resampled.len() as u64;
                        buffer.lock().unwrap().extend(resampled.iter().copied());
                        session_samples.lock().unwrap().extend(resampled);
                        chunk_boundaries.push(session_duration_samples);
                        chunk_paragraph_idx.push(chunk.paragraph_index);
                        chunk_origen.push((chunk.origen_ini, chunk.origen_fin));
                        chunk_texts.push(chunk.text.clone());
                        current_text = chunk.text;
                    }
                    // For NewSession, the first chunk IS what plays first.
                    current_index = 0;
                    current_paragraph_index = chunk_paragraph_idx.first().copied().unwrap_or(0);
                    *paused.lock().unwrap() = arranque_pausado;
                    paused_state = arranque_pausado;
                    {
                        let mut s = snapshot.lock().unwrap();
                        s.estado = if arranque_pausado { "pausa" } else { "sonando" }.into();
                        s.chunks_cocinados = chunk_boundaries.len();
                        s.parrafo_max_cocinado =
                            chunk_paragraph_idx.iter().copied().max().unwrap_or(0);
                        s.playing = true;
                        s.paused = arranque_pausado;
                        s.current_text = chunk_texts.first().cloned().unwrap_or_default();
                        s.current_index = 0;
                        s.current_paragraph_index = current_paragraph_index;
                        let (oi, of) = chunk_origen.first().copied().unwrap_or((0, 0));
                        s.current_origen_ini = oi;
                        s.current_origen_fin = of;
                        s.total = session_total;
                        s.total_paragraphs = total_paragraphs;
                        s.elapsed_secs = 0.0;
                        s.duration_secs = session_duration_samples as f32 / out_sr as f32;
                    }
                    emit(&snapshot, &listeners);
                }
                Command::Enqueue { session_id, chunk } => {
                    if session_id != live_session_id.load(Ordering::SeqCst) {
                        // Stale chunk produced by a synth task that hadn't yet noticed Stop.
                        // Drop it silently — no audio, no session updates.
                        continue;
                    }
                    let resampled = if chunk.source_sample_rate != out_sr {
                        resample_mono(&chunk.samples, chunk.source_sample_rate, out_sr)?
                    } else {
                        chunk.samples.clone()
                    };
                    session_duration_samples += resampled.len() as u64;
                    buffer.lock().unwrap().extend(resampled.iter().copied());
                    session_samples.lock().unwrap().extend(resampled);
                    chunk_boundaries.push(session_duration_samples);
                    chunk_paragraph_idx.push(chunk.paragraph_index);
                    chunk_origen.push((chunk.origen_ini, chunk.origen_fin));
                    chunk_texts.push(chunk.text.clone());
                    session_total = chunk.total.max(session_total);
                    total_paragraphs = chunk.total_paragraphs.max(total_paragraphs);
                    // Do NOT bump current_index here — synth completion isn't playback.
                    // The tick loop below will bump it based on played_samples.
                    {
                        let mut s = snapshot.lock().unwrap();
                        s.total = session_total;
                        s.total_paragraphs = total_paragraphs;
                        s.chunks_cocinados = chunk_boundaries.len();
                        s.parrafo_max_cocinado = s
                            .parrafo_max_cocinado
                            .max(chunk_paragraph_idx.last().copied().unwrap_or(0));
                        s.duration_secs = session_duration_samples as f32 / out_sr as f32;
                    }
                    emit(&snapshot, &listeners);
                }
                Command::Pause => {
                    *paused.lock().unwrap() = true;
                    paused_state = true;
                    {
                        let mut s = snapshot.lock().unwrap();
                        s.paused = true;
                        if s.estado == "sonando" {
                            s.estado = "pausa".into();
                        }
                    }
                    emit(&snapshot, &listeners);
                }
                Command::Resume => {
                    *paused.lock().unwrap() = false;
                    paused_state = false;
                    {
                        let mut s = snapshot.lock().unwrap();
                        s.paused = false;
                        s.playing = true;
                        if s.estado == "pausa" {
                            s.estado = "sonando".into();
                        }
                    }
                    emit(&snapshot, &listeners);
                }
                Command::Stop => {
                    *buffer.lock().unwrap() = Vec::new();
                    *paused.lock().unwrap() = false;
                    paused_state = false;
                    session_duration_samples = 0;
                    session_total = 0;
                    total_paragraphs = 0;
                    current_text.clear();
                    chunk_boundaries.clear();
                    chunk_texts.clear();
                    chunk_paragraph_idx.clear();
                    chunk_origen.clear();
                    *played_samples.lock().unwrap() = 0;
                    *session_samples.lock().unwrap() = Vec::new();
                    {
                        let mut s = snapshot.lock().unwrap();
                        s.estado = "inactivo".into();
                        s.titulo.clear();
                        s.doc_path.clear();
                        s.chunks_cocinados = 0;
                        s.parrafo_max_cocinado = 0;
                        s.playing = false;
                        s.paused = false;
                        s.current_text.clear();
                        s.current_index = 0;
                        s.current_paragraph_index = 0;
                        s.current_origen_ini = 0;
                        s.current_origen_fin = 0;
                        s.total = 0;
                        s.total_paragraphs = 0;
                        s.elapsed_secs = 0.0;
                        s.duration_secs = 0.0;
                    }
                    emit(&snapshot, &listeners);
                }
                Command::SaltarChunk { delta } => {
                    if chunk_boundaries.is_empty() {
                        continue;
                    }
                    let played = *played_samples.lock().unwrap();
                    let actual = chunk_boundaries
                        .iter()
                        .position(|&end| end > played)
                        .unwrap_or_else(|| chunk_boundaries.len().saturating_sub(1));
                    let inicio_actual =
                        if actual == 0 { 0 } else { chunk_boundaries[actual - 1] };
                    let segs_en_frase =
                        played.saturating_sub(inicio_actual) as f32 / out_sr as f32;
                    let saltable: Vec<bool> = chunk_texts
                        .iter()
                        .map(|t| !t.trim().is_empty())
                        .collect();
                    let destino = destino_salto_frase(actual, segs_en_frase, delta, &saltable);
                    let (nuevo, _, parrafo) = reposicionar(
                        destino,
                        &chunk_boundaries,
                        &chunk_texts,
                        &chunk_paragraph_idx,
                        &chunk_origen,
                        &session_samples,
                        &buffer,
                        &played_samples,
                        &snapshot,
                        out_sr,
                    );
                    current_index = nuevo;
                    current_paragraph_index = parrafo;
                    if let Some(t) = chunk_texts.get(nuevo) {
                        current_text = t.clone();
                    }
                    emit(&snapshot, &listeners);
                }
                Command::SaltarParrafo { delta } => {
                    if chunk_boundaries.is_empty() {
                        continue;
                    }
                    let played = *played_samples.lock().unwrap();
                    let actual = chunk_boundaries
                        .iter()
                        .position(|&end| end > played)
                        .unwrap_or_else(|| chunk_boundaries.len().saturating_sub(1));
                    let saltable: Vec<bool> = chunk_texts
                        .iter()
                        .map(|t| !t.trim().is_empty())
                        .collect();
                    let destino =
                        destino_salto_parrafo(actual, delta, &chunk_paragraph_idx, &saltable);
                    let (nuevo, _, parrafo) = reposicionar(
                        destino,
                        &chunk_boundaries,
                        &chunk_texts,
                        &chunk_paragraph_idx,
                        &chunk_origen,
                        &session_samples,
                        &buffer,
                        &played_samples,
                        &snapshot,
                        out_sr,
                    );
                    current_index = nuevo;
                    current_paragraph_index = parrafo;
                    if let Some(t) = chunk_texts.get(nuevo) {
                        current_text = t.clone();
                    }
                    emit(&snapshot, &listeners);
                }
                Command::SetVolume(v) => {
                    let v = v.clamp(0.0, 2.0);
                    *volume.lock().unwrap() = v;
                    snapshot.lock().unwrap().volume = v;
                    emit(&snapshot, &listeners);
                }
                Command::SeekSecs(delta) => {
                    // delta < 0 means rewind. Implemented by re-staging samples from session.
                    let session = session_samples.lock().unwrap().clone();
                    let played = *played_samples.lock().unwrap() as i64;
                    let delta_samples = (delta * out_sr as f32) as i64;
                    let target = (played + delta_samples)
                        .clamp(0, session.len() as i64) as usize;
                    *played_samples.lock().unwrap() = target as u64;
                    let mut buf = buffer.lock().unwrap();
                    buf.clear();
                    buf.extend_from_slice(&session[target..]);
                    drop(buf);
                    snapshot.lock().unwrap().elapsed_secs = target as f32 / out_sr as f32;
                    emit(&snapshot, &listeners);
                }
            }
        }

        // El NIVEL: RMS de la ventana que acaba de sonar, escalado a 0..1 y
        // emitido a ~20 Hz solo cuando cambia lo suficiente. Es la señal que
        // anima el pico del loro, el peso de la letra y la aguja.
        {
            let played = *played_samples.lock().unwrap() as usize;
            let nivel = if paused_state {
                0.0
            } else {
                let sesion = session_samples.lock().unwrap();
                if played == 0 || sesion.is_empty() {
                    0.0
                } else {
                    let fin = played.min(sesion.len());
                    let ini = fin.saturating_sub(2048);
                    let v = &sesion[ini..fin];
                    if v.is_empty() {
                        0.0
                    } else {
                        let rms = (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
                        (rms * 5.5).min(1.0)
                    }
                }
            };
            if (nivel - nivel_anterior).abs() > 0.02 || (nivel == 0.0 && nivel_anterior != 0.0) {
                nivel_anterior = nivel;
                for f in nivel_listeners.lock().unwrap().iter() {
                    f(nivel);
                }
            }
        }

        // Tick: update elapsed AND re-derive which chunk is currently being heard.
        {
            let played = *played_samples.lock().unwrap();
            let elapsed = played as f32 / out_sr as f32;
            let buf_empty = buffer.lock().unwrap().is_empty();

            // Find the chunk whose end-boundary is the first one greater than played.
            // That's the chunk currently being heard.
            let playing_idx = chunk_boundaries
                .iter()
                .position(|&end| end > played)
                .unwrap_or_else(|| chunk_boundaries.len().saturating_sub(1));
            let chunk_changed = playing_idx != current_index;
            if chunk_changed {
                current_index = playing_idx;
                if let Some(t) = chunk_texts.get(playing_idx) {
                    if !t.is_empty() {
                        current_text = t.clone();
                    }
                }
                if let Some(&p) = chunk_paragraph_idx.get(playing_idx) {
                    current_paragraph_index = p;
                }
            }

            let mut s = snapshot.lock().unwrap();
            s.elapsed_secs = elapsed;
            if chunk_changed {
                s.current_index = current_index;
                s.current_paragraph_index = current_paragraph_index;
                s.current_text = current_text.clone();
                let (oi, of) = chunk_origen.get(current_index).copied().unwrap_or((0, 0));
                s.current_origen_ini = oi;
                s.current_origen_fin = of;
            }
            let ended = s.playing && buf_empty && !paused_state;
            if ended {
                s.playing = false;
                s.estado = "inactivo".into();
                s.titulo.clear();
                s.doc_path.clear();
            }
            if chunk_changed || ended {
                drop(s);
                emit(&snapshot, &listeners);
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn fill<S: Copy + Default>(
    data: &mut [S],
    channels: usize,
    buffer: &Arc<Mutex<Vec<f32>>>,
    paused: &Arc<Mutex<bool>>,
    played: &Arc<Mutex<u64>>,
    volume: &Arc<Mutex<f32>>,
    encode: impl Fn(f32) -> S,
) {
    let mut buf = buffer.lock().unwrap();
    let p = *paused.lock().unwrap();
    let vol = *volume.lock().unwrap();
    if p || buf.is_empty() {
        for s in data.iter_mut() {
            *s = encode(0.0);
        }
        return;
    }
    let frames = data.len() / channels;
    let take = frames.min(buf.len());
    for i in 0..take {
        let v = buf[i] * vol;
        let enc = encode(v);
        for c in 0..channels {
            data[i * channels + c] = enc;
        }
    }
    for i in take..frames {
        for c in 0..channels {
            data[i * channels + c] = encode(0.0);
        }
    }
    buf.drain(..take);
    *played.lock().unwrap() += take as u64;
}

/// Elige el chunk destino de un salto por FRASE. `saltable[i]` dice si el
/// chunk i tiene texto real (los silencios entre párrafos no cuentan como
/// frases). Convención musical: yendo atrás con más de 1,2 s dentro de la
/// frase actual, el destino es el principio de la frase actual.
pub fn destino_salto_frase(
    actual: usize,
    segs_en_frase: f32,
    delta: i32,
    saltable: &[bool],
) -> usize {
    if saltable.is_empty() {
        return 0;
    }
    let n = saltable.len();
    let actual = actual.min(n - 1);
    if delta < 0 && segs_en_frase > 1.2 {
        return actual;
    }
    let paso: i64 = if delta >= 0 { 1 } else { -1 };
    let mut destino = actual as i64;
    let mut restantes = delta.abs();
    while restantes > 0 {
        let mut siguiente = destino + paso;
        while siguiente >= 0 && (siguiente as usize) < n && !saltable[siguiente as usize] {
            siguiente += paso;
        }
        if siguiente < 0 || siguiente as usize >= n {
            break;
        }
        destino = siguiente;
        restantes -= 1;
    }
    destino.max(0) as usize
}

/// Elige el chunk destino de un salto por PÁRRAFO: el primer chunk con texto
/// real del párrafo destino, con clamp a lo ya sintetizado.
pub fn destino_salto_parrafo(
    actual_chunk: usize,
    delta: i32,
    parrafo_de: &[usize],
    saltable: &[bool],
) -> usize {
    if parrafo_de.is_empty() {
        return 0;
    }
    let n = parrafo_de.len();
    let actual_chunk = actual_chunk.min(n - 1);
    let parrafo_actual = parrafo_de[actual_chunk] as i64;
    let max_parrafo = parrafo_de.iter().copied().max().unwrap_or(0) as i64;
    let destino_parrafo = (parrafo_actual + delta as i64).clamp(0, max_parrafo);
    for (i, &p) in parrafo_de.iter().enumerate().take(n) {
        if p as i64 == destino_parrafo && saltable.get(i).copied().unwrap_or(true) {
            return i;
        }
    }
    parrafo_de
        .iter()
        .position(|&p| p as i64 == destino_parrafo)
        .unwrap_or(actual_chunk)
}

pub fn resample_mono(input: &[f32], sr_in: u32, sr_out: u32) -> Result<Vec<f32>> {
    if sr_in == sr_out {
        return Ok(input.to_vec());
    }
    let mut resampler = FftFixedInOut::<f32>::new(sr_in as usize, sr_out as usize, 1024, 1)?;
    let mut out: Vec<f32> = Vec::with_capacity(
        ((input.len() as f64) * (sr_out as f64) / (sr_in as f64)).ceil() as usize,
    );
    // FftFixedInOut requires EXACTLY `input_frames_next()` input frames per
    // process() call (e.g. 1323 for 44.1k→16k) — feeding a fixed 1024 errors
    // with "Insufficient buffer size". Drive the loop by that size and
    // zero-pad the final partial block.
    let mut pos = 0usize;
    while pos < input.len() {
        let need = resampler.input_frames_next();
        let end = (pos + need).min(input.len());
        let mut frame = vec![0.0f32; need];
        let slice = &input[pos..end];
        frame[..slice.len()].copy_from_slice(slice);
        let waves_out = resampler.process(&[frame.as_slice()], None)?;
        out.extend_from_slice(&waves_out[0]);
        pos += need;
    }
    Ok(out)
}

pub fn write_wav_file<P: AsRef<std::path::Path>>(
    path: P,
    samples: &[f32],
    sample_rate: u32,
) -> Result<()> {
    use hound::{SampleFormat, WavSpec, WavWriter};
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut w = WavWriter::create(path, spec)?;
    for &s in samples {
        let v = s.clamp(-1.0, 1.0);
        w.write_sample((v * 32767.0) as i16)?;
    }
    w.finalize()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn salto_frase_adelante_simple() {
        let s = [true, true, true, true];
        assert_eq!(destino_salto_frase(1, 0.4, 1, &s), 2);
        assert_eq!(destino_salto_frase(1, 5.0, 1, &s), 2);
    }

    #[test]
    fn salto_frase_atras_convencion_musical() {
        let s = [true, true, true, true];
        // Recién empezada la frase: a la anterior.
        assert_eq!(destino_salto_frase(2, 0.6, -1, &s), 1);
        // Pasado 1,2 s: al principio de la actual.
        assert_eq!(destino_salto_frase(2, 2.0, -1, &s), 2);
    }

    #[test]
    fn salto_frase_salta_silencios() {
        // El chunk 2 es un silencio entre parrafos: no cuenta como frase.
        let s = [true, true, false, true];
        assert_eq!(destino_salto_frase(1, 0.2, 1, &s), 3);
        assert_eq!(destino_salto_frase(3, 0.2, -1, &s), 1);
    }

    #[test]
    fn salto_frase_clava_en_los_bordes() {
        let s = [true, true, true];
        assert_eq!(destino_salto_frase(2, 0.1, 1, &s), 2);
        assert_eq!(destino_salto_frase(0, 0.1, -1, &s), 0);
        assert_eq!(destino_salto_frase(0, 0.0, -3, &s), 0);
        assert_eq!(destino_salto_frase(9, 0.0, 1, &s), 2);
        assert_eq!(destino_salto_frase(0, 0.0, 1, &[]), 0);
    }

    #[test]
    fn salto_frase_acumulado() {
        let s = [true, true, true, true, true];
        assert_eq!(destino_salto_frase(0, 0.0, 3, &s), 3);
        assert_eq!(destino_salto_frase(4, 0.3, -2, &s), 2);
    }

    #[test]
    fn salto_parrafo_al_primer_chunk_con_texto() {
        // Parrafos: 0,0,1(silencio),1,1,2
        let p = [0usize, 0, 1, 1, 1, 2];
        let s = [true, true, false, true, true, true];
        assert_eq!(destino_salto_parrafo(0, 1, &p, &s), 3);
        assert_eq!(destino_salto_parrafo(4, 1, &p, &s), 5);
        assert_eq!(destino_salto_parrafo(5, -1, &p, &s), 3);
        assert_eq!(destino_salto_parrafo(5, -2, &p, &s), 0);
    }

    #[test]
    fn salto_parrafo_clamp_a_lo_cocinado() {
        let p = [0usize, 1];
        let s = [true, true];
        // Pedir dos parrafos mas alla de lo cocinado se queda en el ultimo.
        assert_eq!(destino_salto_parrafo(1, 3, &p, &s), 1);
        assert_eq!(destino_salto_parrafo(0, -5, &p, &s), 0);
        assert_eq!(destino_salto_parrafo(0, 1, &[], &[]), 0);
    }
}
