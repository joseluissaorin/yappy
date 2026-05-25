//! M4B audiobook encoder.
//!
//! Takes raw f32 PCM samples + chapter list, produces an .m4b file (MPEG-4
//! container with AAC-LC audio) containing Apple-compatible chapter metadata
//! via the Nero-style `chpl` atom.
//!
//! ### Why fdk-aac + the `mp4` crate
//!
//! `fdk-aac` vendors the Fraunhofer AAC encoder C source and builds via
//! cc+cmake — no system dependency on the user's machine, builds the same on
//! macOS / Windows / Linux. AAC quality at audiobook bitrates (~80 kbps mono)
//! is excellent for speech.
//!
//! The `mp4` crate gives us a working MP4 muxer for the audio track but does
//! not support custom user-data atoms like `chpl`. After it finishes writing
//! we re-open the file, locate the `moov` box, and inject a `udta/chpl` atom
//! containing the chapter list. Because `moov` lives at the END of the file
//! when we use the streaming writer, mdat offsets in `stco` are unaffected
//! and we only need to adjust the `moov` and `udta` size headers.
//!
//! ### Compatibility
//!
//! `chpl` is the Nero-defined "chapter list" atom. Apple Books, Plex,
//! Audiobookshelf, and VLC all read it. The simpler alternative (writing a
//! second QuickTime "chapter" media track) is more interoperable for some
//! niche players but considerably more code; chpl covers the practical 99%.

use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use fdk_aac::enc::{
    AudioObjectType as FdkAot, BitRate, ChannelMode, Encoder, EncoderParams, Transport,
};

/// Each chapter: human-readable title + start time (seconds, f64).
#[derive(Debug, Clone)]
pub struct Chapter {
    pub title: String,
    pub start_secs: f64,
}

/// Lightweight metadata extracted from an existing .m4b file. Used by the
/// Library UI to show duration, chapter count, etc. without decoding the
/// audio. Returns None if the file isn't readable or doesn't look like an
/// MP4 (no `ftyp` box at the start).
#[derive(Debug, Clone, serde::Serialize)]
pub struct M4bInfo {
    pub duration_secs: f64,
    pub chapter_count: usize,
    pub first_chapter_title: Option<String>,
}

pub fn read_m4b_info(path: &Path) -> Option<M4bInfo> {
    let file = std::fs::File::open(path).ok()?;
    let meta = file.metadata().ok()?;
    let size = meta.len();
    let reader = std::io::BufReader::new(file);
    let mp4 = mp4::Mp4Reader::read_header(reader, size).ok()?;
    let duration_secs = mp4.duration().as_secs_f64();
    // Read the chpl atom by re-scanning the raw bytes — the `mp4` crate
    // doesn't expose user-data atoms, but we wrote the chpl ourselves so we
    // know the layout. Failure here is fine: just report 0 chapters.
    let (chapter_count, first_chapter_title) = read_chpl_from_file(path)
        .unwrap_or((0, None));
    Some(M4bInfo {
        duration_secs,
        chapter_count,
        first_chapter_title,
    })
}

fn read_chpl_from_file(path: &Path) -> Option<(usize, Option<String>)> {
    let chapters = read_chpl_chapters(path).unwrap_or_default();
    if chapters.is_empty() {
        return Some((0, None));
    }
    let first = chapters.first().map(|c| c.title.clone());
    Some((chapters.len(), first))
}

/// Parse the full chapter list from an m4b's chpl atom. Each entry is
/// (title, start_secs). Empty if the file isn't an m4b or has no chapters.
pub fn read_chpl_chapters(path: &Path) -> Option<Vec<Chapter>> {
    let bytes = std::fs::read(path).ok()?;
    let chpl_off = bytes.windows(4).position(|w| w == b"chpl")?;
    let payload_start = chpl_off + 4 + 4 + 4; // type + version+flags + reserved
    if payload_start + 1 > bytes.len() {
        return Some(Vec::new());
    }
    let count = bytes[payload_start] as usize;
    let mut cursor = payload_start + 1;
    let mut chapters: Vec<Chapter> = Vec::with_capacity(count);
    for _ in 0..count {
        if cursor + 9 > bytes.len() {
            break;
        }
        let start_100ns = u64::from_be_bytes([
            bytes[cursor], bytes[cursor + 1], bytes[cursor + 2], bytes[cursor + 3],
            bytes[cursor + 4], bytes[cursor + 5], bytes[cursor + 6], bytes[cursor + 7],
        ]);
        cursor += 8;
        let title_len = bytes[cursor] as usize;
        cursor += 1;
        if cursor + title_len > bytes.len() {
            break;
        }
        let title = std::str::from_utf8(&bytes[cursor..cursor + title_len])
            .unwrap_or("(chapter)")
            .to_string();
        cursor += title_len;
        chapters.push(Chapter {
            title,
            start_secs: start_100ns as f64 / 10_000_000.0,
        });
    }
    Some(chapters)
}

/// Top-level metadata that goes into the m4b container.
#[derive(Debug, Clone, Default)]
pub struct M4bMetadata {
    pub title: String,
    pub author: String,
    pub album: String,
}

/// Encode raw mono PCM samples to an `.m4b` audiobook with chapter markers.
///
/// `samples` is f32 in `[-1.0, 1.0]`, mono. `sample_rate` is typically 44100.
/// `chapters` is sorted by start_secs ascending (the first one should usually
/// be at 0.0); if empty, the file has no chapter metadata.
pub fn encode_m4b(
    samples: &[f32],
    sample_rate: u32,
    chapters: &[Chapter],
    metadata: &M4bMetadata,
    out_path: &Path,
) -> Result<()> {
    if samples.is_empty() {
        return Err(anyhow!("no samples to encode"));
    }
    if sample_rate == 0 {
        return Err(anyhow!("sample_rate cannot be 0"));
    }

    tracing::info!(
        "audiobook: encoding {} samples ({}s) → {}",
        samples.len(),
        samples.len() as f64 / sample_rate as f64,
        out_path.display()
    );

    // ── 1) AAC encode ───────────────────────────────────────────────────
    let aac_frames = aac_encode_mono(samples, sample_rate, 80_000)?;
    tracing::info!(
        "audiobook: produced {} AAC frames (~{}s of audio)",
        aac_frames.len(),
        // AAC-LC frames are exactly 1024 PCM samples each
        aac_frames.len() * 1024 / sample_rate as usize
    );

    // ── 2) MP4 mux of the AAC audio ────────────────────────────────────
    mux_aac_to_mp4(&aac_frames, sample_rate, metadata, out_path)?;

    // ── 3) Inject chpl atom for chapters ───────────────────────────────
    if !chapters.is_empty() {
        inject_chpl_atom(out_path, chapters)?;
        tracing::info!("audiobook: injected {} chapters", chapters.len());
    }

    Ok(())
}

// ────────────────────────────────────────────────────────────────────────
// AAC ENCODE
// ────────────────────────────────────────────────────────────────────────

/// Encode mono f32 PCM → list of raw AAC frames (one per 1024-sample slice).
/// Uses fdk-aac's `Raw` transport so the frames can be wrapped in MP4 directly
/// (no ADTS / LATM headers that MP4 doesn't want).
fn aac_encode_mono(samples: &[f32], sample_rate: u32, bit_rate: u32) -> Result<Vec<Vec<u8>>> {
    let params = EncoderParams {
        bit_rate: BitRate::Cbr(bit_rate),
        sample_rate,
        transport: Transport::Raw,
        channels: ChannelMode::Mono,
        audio_object_type: FdkAot::Mpeg4LowComplexity,
    };
    let enc = Encoder::new(params).map_err(|e| anyhow!("fdk-aac init: {e:?}"))?;

    // fdk-aac wants i16 samples.
    let pcm_i16: Vec<i16> = samples
        .iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
        .collect();

    // AAC-LC frames consume exactly 1024 PCM samples each.
    const FRAME_SAMPLES: usize = 1024;
    // Pad the tail with silence so the last frame is full (encoder needs a full
    // frame of input or it returns 0 bytes).
    let total = pcm_i16.len().div_ceil(FRAME_SAMPLES) * FRAME_SAMPLES;
    let mut padded = pcm_i16;
    padded.resize(total, 0);

    let mut out_frames: Vec<Vec<u8>> = Vec::with_capacity(total / FRAME_SAMPLES);
    let mut scratch = vec![0u8; 8192];

    let mut cursor = 0;
    while cursor < padded.len() {
        let end = (cursor + FRAME_SAMPLES).min(padded.len());
        let info = enc
            .encode(&padded[cursor..end], &mut scratch)
            .map_err(|e| anyhow!("fdk-aac encode at sample {cursor}: {e:?}"))?;
        if info.output_size > 0 {
            out_frames.push(scratch[..info.output_size].to_vec());
        }
        // fdk-aac may consume fewer samples than provided if its internal
        // buffer fills up mid-frame; advance by what it consumed.
        let consumed = (info.input_consumed / 1) as usize; // mono → 1 sample per "input"
        if consumed == 0 {
            // Defensive: avoid infinite loop if encoder stalls
            break;
        }
        cursor += consumed;
    }

    // Drain any buffered samples by feeding empty input until encoder returns 0.
    loop {
        let info = enc
            .encode(&[], &mut scratch)
            .map_err(|e| anyhow!("fdk-aac drain: {e:?}"))?;
        if info.output_size == 0 {
            break;
        }
        out_frames.push(scratch[..info.output_size].to_vec());
    }

    Ok(out_frames)
}

// ────────────────────────────────────────────────────────────────────────
// MP4 MUX
// ────────────────────────────────────────────────────────────────────────

fn mux_aac_to_mp4(
    aac_frames: &[Vec<u8>],
    sample_rate: u32,
    metadata: &M4bMetadata,
    out_path: &Path,
) -> Result<()> {
    use mp4::{
        AacConfig, AudioObjectType, MediaConfig, Mp4Config, Mp4Sample, Mp4Writer, SampleFreqIndex,
        TrackConfig, TrackType,
    };

    let file = std::fs::File::create(out_path)
        .with_context(|| format!("creating {}", out_path.display()))?;
    let writer = std::io::BufWriter::new(file);

    let config = Mp4Config {
        major_brand: "M4B ".parse().unwrap(),
        minor_version: 512,
        // Compatible brands: M4B (audiobook), M4A (audio), mp42 (general MPEG-4),
        // isom (ISO base media) — covers every player we care about.
        compatible_brands: vec![
            "M4B ".parse().unwrap(),
            "M4A ".parse().unwrap(),
            "mp42".parse().unwrap(),
            "isom".parse().unwrap(),
        ],
        timescale: 1000,
    };
    let mut writer =
        Mp4Writer::write_start(writer, &config).map_err(|e| anyhow!("mp4 write_start: {e:?}"))?;

    let track_id = 1u32;
    let track_config = TrackConfig {
        track_type: TrackType::Audio,
        timescale: sample_rate,
        language: "und".to_string(),
        media_conf: MediaConfig::AacConfig(AacConfig {
            bitrate: 80_000,
            profile: AudioObjectType::AacLowComplexity,
            freq_index: aac_freq_index(sample_rate)?,
            chan_conf: mp4::ChannelConfig::Mono,
        }),
    };
    writer
        .add_track(&track_config)
        .map_err(|e| anyhow!("mp4 add_track: {e:?}"))?;

    // Each AAC frame is exactly 1024 PCM samples, so its duration in the audio
    // timescale (=sample_rate) is 1024. Decode-time / composition-time identical.
    let frame_duration: u32 = 1024;
    let mut current_ts: u64 = 0;
    for (i, frame) in aac_frames.iter().enumerate() {
        let sample = Mp4Sample {
            start_time: current_ts,
            duration: frame_duration,
            rendering_offset: 0,
            is_sync: true,
            bytes: frame.clone().into(),
        };
        writer
            .write_sample(track_id, &sample)
            .map_err(|e| anyhow!("mp4 write_sample {i}: {e:?}"))?;
        current_ts += frame_duration as u64;
    }
    writer
        .write_end()
        .map_err(|e| anyhow!("mp4 write_end: {e:?}"))?;

    // Append iTunes-style udta metadata for title/author/album.
    // The mp4 crate doesn't write these directly, so we'll post-process.
    if !metadata.title.is_empty() || !metadata.author.is_empty() || !metadata.album.is_empty() {
        inject_itunes_metadata(out_path, metadata)?;
    }
    Ok(())
}

/// MPEG-4 AAC sampling-frequency index → mp4 crate's enum representation.
fn aac_freq_index(sample_rate: u32) -> Result<mp4::SampleFreqIndex> {
    use mp4::SampleFreqIndex::*;
    Ok(match sample_rate {
        96000 => Freq96000,
        88200 => Freq88200,
        64000 => Freq64000,
        48000 => Freq48000,
        44100 => Freq44100,
        32000 => Freq32000,
        24000 => Freq24000,
        22050 => Freq22050,
        16000 => Freq16000,
        12000 => Freq12000,
        11025 => Freq11025,
        8000 => Freq8000,
        7350 => Freq7350,
        other => return Err(anyhow!("unsupported sample rate for AAC: {other}")),
    })
}

// ────────────────────────────────────────────────────────────────────────
// CHAPTER ATOM (chpl) INJECTION
// ────────────────────────────────────────────────────────────────────────
//
// Nero's `chpl` atom layout (matches mp4v2 / ffmpeg's mov_read_chpl):
//
//   size:u32       (BE, includes header)
//   type:u32       ("chpl")
//   version:u8     (=1)
//   flags:u24      (=0)
//   reserved:u32   (=0)
//   count:u8       (chapter count, max 255)
//   for each chapter:
//     start_100ns:u64    (start time in 100-nanosecond units, BE)
//     title_len:u8       (UTF-8 bytes that follow)
//     title_bytes:[u8]   (no null terminator)
//
// The atom must live inside the `udta` (user-data) box, which itself lives
// inside `moov`. We add a new udta if one doesn't exist; otherwise extend it.

fn build_chpl_atom(chapters: &[Chapter]) -> Vec<u8> {
    let mut body: Vec<u8> = Vec::new();
    body.write_u8(1).unwrap(); // version
    body.write_u24::<BigEndian>(0).unwrap(); // flags
    body.write_u32::<BigEndian>(0).unwrap(); // reserved
    // count, capped to 255 because the count field is a single byte.
    let cap = chapters.len().min(255) as u8;
    body.write_u8(cap).unwrap();
    for c in chapters.iter().take(255) {
        let title = c.title.trim();
        let title_bytes = title.as_bytes();
        // Atom uses a u8 length prefix, so 255 max bytes.
        let truncated = if title_bytes.len() > 255 {
            &title_bytes[..255]
        } else {
            title_bytes
        };
        let start_100ns = (c.start_secs * 10_000_000.0).max(0.0) as u64;
        body.write_u64::<BigEndian>(start_100ns).unwrap();
        body.write_u8(truncated.len() as u8).unwrap();
        body.extend_from_slice(truncated);
    }
    wrap_atom(b"chpl", &body)
}

fn wrap_atom(four_cc: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + body.len());
    out.write_u32::<BigEndian>((8 + body.len()) as u32).unwrap();
    out.extend_from_slice(four_cc);
    out.extend_from_slice(body);
    out
}

/// Inject (or extend) a `udta` containing `chpl` inside the `moov` box.
///
/// Algorithm:
/// 1. Read the whole file into memory.
/// 2. Walk top-level boxes to find `moov`.
/// 3. Inside `moov`, look for an existing `udta`.
/// 4. Build the chpl atom; either wrap it in a new udta or append it to the
///    existing udta body.
/// 5. Patch the moov size header to include the added bytes.
/// 6. Write back. Because `moov` lives AFTER `mdat` in the file the streaming
///    writer produced, `stco`/`co64` offsets that point into `mdat` remain
///    correct — we only mutate bytes after them.
fn inject_chpl_atom(path: &Path, chapters: &[Chapter]) -> Result<()> {
    let chpl = build_chpl_atom(chapters);

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .with_context(|| format!("opening {}", path.display()))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .with_context(|| format!("reading {}", path.display()))?;

    let moov_offset = find_top_level_box(&buf, b"moov")
        .ok_or_else(|| anyhow!("moov box not found in {}", path.display()))?;
    let moov_size = read_box_size(&buf, moov_offset);

    // Look for existing udta inside moov.
    let body_start = moov_offset + 8;
    let body_end = moov_offset + moov_size as usize;
    let udta_offset = find_child_box(&buf, body_start, body_end, b"udta");

    let mut new_buf: Vec<u8> = Vec::with_capacity(buf.len() + chpl.len() + 8);

    match udta_offset {
        Some(off) => {
            // Append chpl to the existing udta. Adjust udta size and moov size.
            let udta_size = read_box_size(&buf, off) as usize;
            let udta_end = off + udta_size;
            // Everything up to end of udta body
            new_buf.extend_from_slice(&buf[..udta_end]);
            // Append chpl
            new_buf.extend_from_slice(&chpl);
            // Rest of moov + everything after
            new_buf.extend_from_slice(&buf[udta_end..]);

            // Patch udta size header
            let new_udta_size = (udta_size + chpl.len()) as u32;
            write_box_size(&mut new_buf, off, new_udta_size);
            // Patch moov size header
            let new_moov_size = (moov_size + chpl.len() as u64) as u32;
            write_box_size(&mut new_buf, moov_offset, new_moov_size);
        }
        None => {
            // Create a fresh udta containing chpl. Place it at the END of moov.
            let new_udta = wrap_atom(b"udta", &chpl);
            new_buf.extend_from_slice(&buf[..body_end]);
            new_buf.extend_from_slice(&new_udta);
            new_buf.extend_from_slice(&buf[body_end..]);
            // Patch moov size to include the new udta.
            let new_moov_size = (moov_size + new_udta.len() as u64) as u32;
            write_box_size(&mut new_buf, moov_offset, new_moov_size);
        }
    }

    // Truncate + rewrite.
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&new_buf)?;
    file.set_len(new_buf.len() as u64)?;
    Ok(())
}

/// iTunes-style metadata via the `udta/meta/ilst` tree. Smaller, optional —
/// adds title/author/album so the file shows up nicely in Apple Books.
fn inject_itunes_metadata(path: &Path, metadata: &M4bMetadata) -> Result<()> {
    // For now this is a no-op stub — chpl handles the user-visible chapter
    // metadata. Title/author/album are nice-to-have and can be added in a
    // follow-up. (Apple Books happily plays an m4b without these and shows
    // the filename as the title.)
    let _ = (path, metadata);
    Ok(())
}

// ────────────────────────────────────────────────────────────────────────
// MP4 box helpers
// ────────────────────────────────────────────────────────────────────────

fn read_box_size(buf: &[u8], offset: usize) -> u64 {
    let mut cursor = &buf[offset..offset + 4];
    let sz = cursor.read_u32::<BigEndian>().unwrap() as u64;
    if sz == 1 {
        // Extended 64-bit size lives at offset+8 (after 4-byte size + 4-byte type)
        let mut c2 = &buf[offset + 8..offset + 16];
        c2.read_u64::<BigEndian>().unwrap()
    } else {
        sz
    }
}

fn write_box_size(buf: &mut [u8], offset: usize, size: u32) {
    let bytes = size.to_be_bytes();
    buf[offset..offset + 4].copy_from_slice(&bytes);
}

fn find_top_level_box(buf: &[u8], four_cc: &[u8; 4]) -> Option<usize> {
    let mut cursor = 0;
    while cursor + 8 <= buf.len() {
        let size = read_box_size(buf, cursor) as usize;
        if size < 8 || cursor + size > buf.len() {
            return None;
        }
        if &buf[cursor + 4..cursor + 8] == four_cc {
            return Some(cursor);
        }
        cursor += size;
    }
    None
}

fn find_child_box(buf: &[u8], body_start: usize, body_end: usize, four_cc: &[u8; 4]) -> Option<usize> {
    let mut cursor = body_start;
    while cursor + 8 <= body_end {
        let size = read_box_size(buf, cursor) as usize;
        if size < 8 || cursor + size > body_end {
            return None;
        }
        if &buf[cursor + 4..cursor + 8] == four_cc {
            return Some(cursor);
        }
        cursor += size;
    }
    None
}
