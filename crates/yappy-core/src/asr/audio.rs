//! Log-mel spectrogram front-end for Parakeet TDT.
//!
//! Mirrors NeMo's `FilterbankFeatures` (the `ParakeetFeatureExtractor`): pre-emphasis
//! → STFT (Hann, power spectrum) → Slaney mel filterbank → log (additive guard) →
//! per-feature mean/var normalization. Adapted from `parakeet-rs` (MIT/Apache-2.0).
//!
//! Input must already be **mono 16 kHz `f32`** — resampling/downmix happens in the
//! Tauri layer (`yappy-app`) before we get here.

use std::f32::consts::PI;

use anyhow::{anyhow, Result};
use ndarray::Array2;

/// The Parakeet TDT v3 mel front-end parameters (NeMo `nemo128` preprocessor).
#[derive(Debug, Clone)]
pub struct MelConfig {
    pub feature_size: usize, // n_mels
    pub hop_length: usize,
    pub n_fft: usize,
    pub win_length: usize,
    pub preemphasis: f32,
    pub sampling_rate: usize,
}

impl Default for MelConfig {
    fn default() -> Self {
        // 128-mel config used by parakeet-tdt-0.6b-v3 (NeMo nemo128.onnx).
        Self {
            feature_size: 128,
            hop_length: 160,
            n_fft: 512,
            win_length: 400,
            preemphasis: 0.97,
            sampling_rate: 16_000,
        }
    }
}

fn apply_preemphasis(audio: &[f32], coef: f32) -> Vec<f32> {
    if audio.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(audio.len());
    out.push(audio[0]);
    for i in 1..audio.len() {
        out.push(audio[i] - coef * audio[i - 1]);
    }
    out
}

fn hann_window(window_length: usize) -> Vec<f32> {
    (0..window_length)
        .map(|i| 0.5 - 0.5 * ((2.0 * PI * i as f32) / (window_length as f32 - 1.0)).cos())
        .collect()
}

/// Short-time Fourier transform → power spectrum, `(freq_bins, num_frames)`.
///
/// A correct FFT (realfft over RustFFT) is required: a naive DFT yields wrong
/// frequency bins and the model emits all-blank tokens.
fn stft(audio: &[f32], n_fft: usize, hop_length: usize, win_length: usize) -> Result<Array2<f32>> {
    use realfft::RealFftPlanner;

    let pad_amount = n_fft / 2;
    let mut padded = vec![0.0f32; pad_amount];
    padded.extend_from_slice(audio);
    padded.resize(padded.len() + pad_amount, 0.0);

    let window = hann_window(win_length);
    if padded.len() < n_fft {
        return Err(anyhow!("audio too short for STFT ({} samples)", audio.len()));
    }
    let num_frames = (padded.len() - n_fft) / hop_length + 1;
    let freq_bins = n_fft / 2 + 1;
    let mut spectrogram = Array2::<f32>::zeros((freq_bins, num_frames));

    let mut planner = RealFftPlanner::<f32>::new();
    let r2c = planner.plan_fft_forward(n_fft);
    let mut input = vec![0.0f32; n_fft];
    let mut output = r2c.make_output_vec();
    let mut scratch = r2c.make_scratch_vec();

    for frame_idx in 0..num_frames {
        let start = frame_idx * hop_length;
        input.fill(0.0);
        let take = win_length.min(padded.len().saturating_sub(start));
        for i in 0..take {
            input[i] = padded[start + i] * window[i];
        }
        r2c.process_with_scratch(&mut input, &mut output, &mut scratch)
            .map_err(|e| anyhow!("FFT failed: {e}"))?;
        for k in 0..freq_bins {
            spectrogram[[k, frame_idx]] = output[k].norm_sqr();
        }
    }

    Ok(spectrogram)
}

// ── Slaney mel scale (librosa parity) ────────────────────────────────────
const F_SP: f64 = 200.0 / 3.0;
const MIN_LOG_HZ: f64 = 1000.0;
const MIN_LOG_MEL: f64 = MIN_LOG_HZ / F_SP;
const LOG_STEP: f64 = 0.068_751_777_420_949_12;

fn hz_to_mel_slaney(hz: f64) -> f64 {
    if hz < MIN_LOG_HZ {
        hz / F_SP
    } else {
        MIN_LOG_MEL + (hz / MIN_LOG_HZ).ln() / LOG_STEP
    }
}

fn mel_to_hz_slaney(mel: f64) -> f64 {
    if mel < MIN_LOG_MEL {
        mel * F_SP
    } else {
        MIN_LOG_HZ * ((mel - MIN_LOG_MEL) * LOG_STEP).exp()
    }
}

fn create_mel_filterbank(n_fft: usize, n_mels: usize, sample_rate: usize) -> Array2<f32> {
    let freq_bins = n_fft / 2 + 1;
    let mut filterbank = Array2::<f32>::zeros((n_mels, freq_bins));

    let fmax = sample_rate as f64 / 2.0;
    let mel_min = hz_to_mel_slaney(0.0);
    let mel_max = hz_to_mel_slaney(fmax);

    let mel_points: Vec<f64> = (0..=n_mels + 1)
        .map(|i| mel_to_hz_slaney(mel_min + (mel_max - mel_min) * i as f64 / (n_mels + 1) as f64))
        .collect();
    let fft_freqs: Vec<f64> = (0..freq_bins)
        .map(|i| i as f64 * sample_rate as f64 / n_fft as f64)
        .collect();
    let fdiff: Vec<f64> = mel_points.windows(2).map(|w| w[1] - w[0]).collect();

    for i in 0..n_mels {
        for (k, &freq) in fft_freqs.iter().enumerate() {
            let lower = (freq - mel_points[i]) / fdiff[i];
            let upper = (mel_points[i + 2] - freq) / fdiff[i + 1];
            filterbank[[i, k]] = 0.0f64.max(lower.min(upper)) as f32;
        }
    }
    // Slaney normalization.
    for i in 0..n_mels {
        let enorm = 2.0 / (mel_points[i + 2] - mel_points[i]);
        for k in 0..freq_bins {
            filterbank[[i, k]] *= enorm as f32;
        }
    }
    filterbank
}

/// Extract `(time_steps, feature_size)` log-mel features from mono 16 kHz audio.
pub fn extract_features(audio: &[f32], config: &MelConfig) -> Result<Array2<f32>> {
    let audio = apply_preemphasis(audio, config.preemphasis);
    let spectrogram = stft(&audio, config.n_fft, config.hop_length, config.win_length)?;

    let mel_filterbank = create_mel_filterbank(config.n_fft, config.feature_size, config.sampling_rate);
    let mel_spectrogram = mel_filterbank.dot(&spectrogram);

    // NeMo: log_zero_guard_type="add", value=2^-24.
    let log_zero_guard: f32 = 2.0f32.powi(-24);
    let mel_spectrogram = mel_spectrogram.mapv(|x| (x + log_zero_guard).ln());

    // (n_mels, frames) → (frames, n_mels)
    let mut mel = mel_spectrogram.t().to_owned();
    let num_frames = mel.shape()[0];
    let num_features = mel.shape()[1];
    if num_frames <= 1 {
        return Ok(mel);
    }

    // Per-feature normalization (mean 0, std 1) with Bessel's correction.
    for feat_idx in 0..num_features {
        let mut column = mel.column_mut(feat_idx);
        let mean: f32 = column.iter().sum::<f32>() / num_frames as f32;
        let variance: f32 =
            column.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / (num_frames as f32 - 1.0);
        let std = variance.sqrt() + 1e-5;
        for val in column.iter_mut() {
            *val = (*val - mean) / std;
        }
    }

    Ok(mel)
}
