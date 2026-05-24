//! Audio playback queue with volume, seek, and an export-friendly samples buffer.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{unbounded, Receiver, Sender};
use rubato::{FftFixedInOut, Resampler};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub index: usize,
    pub paragraph_index: usize,
    pub total: usize,
    pub total_paragraphs: usize,
    pub text: String,
    pub samples: Vec<f32>,
    pub source_sample_rate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackSnapshot {
    pub playing: bool,
    pub paused: bool,
    pub current_text: String,
    pub current_index: usize,
    /// Which PARAGRAPH the currently-playing chunk belongs to (for karaoke
    /// highlight in the document window). Multiple chunks can share a paragraph
    /// when the engine sentence-splits a long paragraph.
    pub current_paragraph_index: usize,
    pub total: usize,
    pub total_paragraphs: usize,
    pub elapsed_secs: f32,
    pub duration_secs: f32,
    pub volume: f32,
    pub output_sample_rate: u32,
}

#[derive(Debug)]
enum Command {
    /// Begin a new session. The session_id is the id this synth task was started with;
    /// the audio thread accepts it iff it matches the controller's `current_session`.
    NewSession { session_id: u64, chunks: Vec<AudioChunk> },
    Enqueue { session_id: u64, chunk: AudioChunk },
    Pause,
    Resume,
    Stop,
    SetVolume(f32),
    SeekSecs(f32),
}

pub struct PlaybackController {
    cmd_tx: Sender<Command>,
    snapshot: Arc<Mutex<PlaybackSnapshot>>,
    /// All samples synthesised in the current session, at the device's output sample rate.
    /// Used by "save as audio".
    session_samples: Arc<Mutex<Vec<f32>>>,
    listeners: Arc<Mutex<Vec<Box<dyn Fn(&PlaybackSnapshot) + Send + Sync>>>>,
    /// Monotonically increasing session id. Bumped on every Stop and every NewSession start.
    /// Synth tasks capture the id at start; if `current_session` later differs, they abort
    /// and the audio thread drops their Enqueue commands. This is what makes Stop *definitive*
    /// — without it, the synth loop keeps generating chunks that get played after Stop.
    session_id: Arc<AtomicU64>,
}

impl PlaybackController {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = unbounded::<Command>();
        let snapshot = Arc::new(Mutex::new(PlaybackSnapshot {
            playing: false,
            paused: false,
            current_text: String::new(),
            current_index: 0,
            current_paragraph_index: 0,
            total: 0,
            total_paragraphs: 0,
            elapsed_secs: 0.0,
            duration_secs: 0.0,
            volume: 1.0,
            output_sample_rate: 44100,
        }));
        let listeners: Arc<Mutex<Vec<Box<dyn Fn(&PlaybackSnapshot) + Send + Sync>>>> =
            Arc::new(Mutex::new(Vec::new()));
        let session_samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let session_id = Arc::new(AtomicU64::new(0));

        let snap_for_thread = snapshot.clone();
        let listeners_for_thread = listeners.clone();
        let session_for_thread = session_samples.clone();
        let session_id_for_thread = session_id.clone();
        std::thread::Builder::new()
            .name("yappy-audio".into())
            .spawn(move || {
                if let Err(e) =
                    run_audio_thread(cmd_rx, snap_for_thread, listeners_for_thread, session_for_thread, session_id_for_thread)
                {
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

    pub fn new_session(&self, session_id: u64, chunks: Vec<AudioChunk>) {
        let _ = self.cmd_tx.send(Command::NewSession { session_id, chunks });
    }
    pub fn enqueue(&self, session_id: u64, chunk: AudioChunk) {
        let _ = self.cmd_tx.send(Command::Enqueue { session_id, chunk });
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
}

fn run_audio_thread(
    cmd_rx: Receiver<Command>,
    snapshot: Arc<Mutex<PlaybackSnapshot>>,
    listeners: Arc<Mutex<Vec<Box<dyn Fn(&PlaybackSnapshot) + Send + Sync>>>>,
    session_samples: Arc<Mutex<Vec<f32>>>,
    live_session_id: Arc<AtomicU64>,
) -> Result<()> {
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
    let mut current_paragraph_index: usize = 0;
    let mut total_paragraphs: usize = 0;

    let emit = |snapshot: &Arc<Mutex<PlaybackSnapshot>>,
                listeners: &Arc<Mutex<Vec<Box<dyn Fn(&PlaybackSnapshot) + Send + Sync>>>>| {
        let snap = snapshot.lock().unwrap().clone();
        for f in listeners.lock().unwrap().iter() {
            f(&snap);
        }
    };

    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                Command::NewSession { session_id, chunks } => {
                    // Drop sessions that were already invalidated by a Stop that raced ahead.
                    if session_id != live_session_id.load(Ordering::SeqCst) {
                        continue;
                    }
                    *buffer.lock().unwrap() = Vec::new();
                    *session_samples.lock().unwrap() = Vec::new();
                    session_total = chunks.first().map(|c| c.total).unwrap_or(0);
                    total_paragraphs = chunks.first().map(|c| c.total_paragraphs).unwrap_or(0);
                    current_index = 0;
                    current_paragraph_index = 0;
                    *played_samples.lock().unwrap() = 0;
                    session_duration_samples = 0;
                    chunk_boundaries.clear();
                    chunk_texts.clear();
                    chunk_paragraph_idx.clear();
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
                        chunk_texts.push(chunk.text.clone());
                        current_text = chunk.text;
                    }
                    // For NewSession, the first chunk IS what plays first.
                    current_index = 0;
                    current_paragraph_index = chunk_paragraph_idx.first().copied().unwrap_or(0);
                    *paused.lock().unwrap() = false;
                    paused_state = false;
                    {
                        let mut s = snapshot.lock().unwrap();
                        s.playing = true;
                        s.paused = false;
                        s.current_text = chunk_texts.first().cloned().unwrap_or_default();
                        s.current_index = 0;
                        s.current_paragraph_index = current_paragraph_index;
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
                    chunk_texts.push(chunk.text.clone());
                    session_total = chunk.total.max(session_total);
                    total_paragraphs = chunk.total_paragraphs.max(total_paragraphs);
                    // Do NOT bump current_index here — synth completion isn't playback.
                    // The tick loop below will bump it based on played_samples.
                    {
                        let mut s = snapshot.lock().unwrap();
                        s.total = session_total;
                        s.total_paragraphs = total_paragraphs;
                        s.duration_secs = session_duration_samples as f32 / out_sr as f32;
                    }
                    emit(&snapshot, &listeners);
                }
                Command::Pause => {
                    *paused.lock().unwrap() = true;
                    paused_state = true;
                    snapshot.lock().unwrap().paused = true;
                    emit(&snapshot, &listeners);
                }
                Command::Resume => {
                    *paused.lock().unwrap() = false;
                    paused_state = false;
                    {
                        let mut s = snapshot.lock().unwrap();
                        s.paused = false;
                        s.playing = true;
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
                    current_index = 0;
                    current_paragraph_index = 0;
                    current_text.clear();
                    chunk_boundaries.clear();
                    chunk_texts.clear();
                    chunk_paragraph_idx.clear();
                    *played_samples.lock().unwrap() = 0;
                    *session_samples.lock().unwrap() = Vec::new();
                    {
                        let mut s = snapshot.lock().unwrap();
                        s.playing = false;
                        s.paused = false;
                        s.current_text.clear();
                        s.current_index = 0;
                        s.current_paragraph_index = 0;
                        s.total = 0;
                        s.total_paragraphs = 0;
                        s.elapsed_secs = 0.0;
                        s.duration_secs = 0.0;
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
            }
            let ended = s.playing && buf_empty && !paused_state;
            if ended { s.playing = false; }
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

pub fn resample_mono(input: &[f32], sr_in: u32, sr_out: u32) -> Result<Vec<f32>> {
    if sr_in == sr_out {
        return Ok(input.to_vec());
    }
    let chunk = 1024usize;
    let mut resampler = FftFixedInOut::<f32>::new(sr_in as usize, sr_out as usize, chunk, 1)?;
    let mut out: Vec<f32> = Vec::with_capacity(
        ((input.len() as f64) * (sr_out as f64) / (sr_in as f64)).ceil() as usize,
    );
    let mut pos = 0usize;
    while pos < input.len() {
        let end = (pos + chunk).min(input.len());
        let mut frame = vec![0.0f32; chunk];
        let slice = &input[pos..end];
        frame[..slice.len()].copy_from_slice(slice);
        let waves_in: [&[f32]; 1] = [&frame];
        let waves_out = resampler.process(&waves_in, None)?;
        out.extend_from_slice(&waves_out[0]);
        pos = end;
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
