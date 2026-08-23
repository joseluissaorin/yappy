//! Decode arbitrary audio files to **mono 16 kHz f32** for the Parakeet ASR
//! engine. Compiled on every platform (iOS included) so the main app's
//! transcription works in the Simulator and on device.
//!
//! `symphonia` (pure Rust) handles mp3 / aac (m4a) / flac / vorbis / wav on all
//! platforms. Ogg-Opus (WhatsApp Android voice notes) has no symphonia decoder,
//! so on **desktop** we demux with the `ogg` crate and decode with `opus`
//! (libopus, C) — that path is desktop-only to keep the iOS build free of the C
//! libopus dependency (iOS WhatsApp shares m4a/aac, which symphonia handles).
//! Everything is downmixed to mono and resampled to 16 kHz via
//! `playback::resample_mono`.

use std::path::Path;

use anyhow::{anyhow, Context, Result};

use crate::playback::resample_mono;

/// Container/codecs we recognize by extension. Anything else is attempted via
/// symphonia probing regardless.
pub const SUPPORTED_AUDIO_EXTS: &[&str] = &[
    "opus", "ogg", "oga", "m4a", "mp4", "aac", "mp3", "wav", "wave", "flac", "caf", "amr",
];

pub fn is_audio_ext(ext: &str) -> bool {
    let e = ext.trim_start_matches('.').to_ascii_lowercase();
    SUPPORTED_AUDIO_EXTS.contains(&e.as_str())
}

/// Decode `path` to mono 16 kHz f32 samples.
pub fn decode_to_mono16k(path: &Path) -> Result<Vec<f32>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    // Ogg-Opus path (desktop only — libopus C dep). On iOS/Android, opus files
    // fall through to symphonia (which won't decode opus) and surface an error;
    // the common iOS case (m4a/aac) is handled fine.
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        if ext == "opus" {
            return decode_ogg_opus(path).context("decoding Ogg-Opus");
        }
        if ext == "ogg" || ext == "oga" {
            if let Ok(samples) = decode_ogg_opus(path) {
                return Ok(samples);
            }
        }
    }
    decode_symphonia(path).context("decoding audio via symphonia")
}

/// Generic symphonia decode → mono 16 kHz.
fn decode_symphonia(path: &Path) -> Result<Vec<f32>> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file = std::fs::File::open(path)
        .with_context(|| format!("opening audio {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .context("probing audio format")?;
    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow!("no decodable audio track"))?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("creating decoder")?;

    let mut src_rate: u32 = 0;
    let mut channels: usize = 1;
    let mut interleaved: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break
            }
            Err(symphonia::core::errors::Error::ResetRequired) => break,
            Err(e) => return Err(anyhow!("reading packet: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                src_rate = spec.rate;
                channels = spec.channels.count().max(1);
                let mut sbuf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                sbuf.copy_interleaved_ref(decoded);
                interleaved.extend_from_slice(sbuf.samples());
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => return Err(anyhow!("decode error: {e}")),
        }
    }

    if interleaved.is_empty() || src_rate == 0 {
        return Err(anyhow!("decoded no audio samples"));
    }
    let mono = downmix(&interleaved, channels);
    Ok(resample_mono(&mono, src_rate, 16_000)?)
}

/// Decode an Ogg-Opus file → mono 16 kHz. libopus always decodes at 48 kHz.
/// Desktop only (libopus is a C dependency we keep off the iOS build).
#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn decode_ogg_opus(path: &Path) -> Result<Vec<f32>> {
    use ogg::PacketReader;
    use opus::{Channels, Decoder};

    let file = std::fs::File::open(path)
        .with_context(|| format!("opening {}", path.display()))?;
    let mut reader = PacketReader::new(file);

    let mut decoder: Option<Decoder> = None;
    let mut channels = 1usize;
    let mut pre_skip = 0usize;
    let mut skipped = 0usize;
    let mut pcm48: Vec<f32> = Vec::new();
    // 120 ms max frame @ 48 kHz stereo.
    let mut frame_buf = vec![0.0f32; 5760 * 2];

    while let Some(packet) = reader.read_packet()? {
        let data = &packet.data;
        if data.starts_with(b"OpusHead") {
            // OpusHead layout: magic(8) version(1) channels(1) preskip(2 LE)
            // input_rate(4) ...
            channels = *data.get(9).unwrap_or(&1) as usize;
            pre_skip = u16::from_le_bytes([data[10], data[11]]) as usize;
            let ch = if channels >= 2 { Channels::Stereo } else { Channels::Mono };
            decoder = Some(Decoder::new(48_000, ch).map_err(|e| anyhow!("opus init: {e}"))?);
            continue;
        }
        if data.starts_with(b"OpusTags") {
            continue;
        }
        let Some(dec) = decoder.as_mut() else { continue };
        let decoded = dec
            .decode_float(data, &mut frame_buf, false)
            .map_err(|e| anyhow!("opus decode: {e}"))?;
        // `decoded` = samples per channel.
        let n = decoded * channels.max(1);
        // Downmix interleaved → mono on the fly, applying pre-skip.
        let mut i = 0;
        while i < n {
            if skipped < pre_skip {
                skipped += 1;
                i += channels.max(1);
                continue;
            }
            let mut acc = 0.0f32;
            for c in 0..channels.max(1) {
                acc += frame_buf[i + c];
            }
            pcm48.push(acc / channels.max(1) as f32);
            i += channels.max(1);
        }
    }

    if pcm48.is_empty() {
        return Err(anyhow!("no opus audio decoded"));
    }
    Ok(resample_mono(&pcm48, 48_000, 16_000)?)
}

fn downmix(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks(channels)
        .map(|c| c.iter().sum::<f32>() / c.len() as f32)
        .collect()
}
