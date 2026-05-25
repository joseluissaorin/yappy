//! End-to-end smoke test: encode a known sine wave to .m4b with 3 chapters,
//! re-open the file, verify chpl atom is present + chapters readable.

use std::fs;

use yappy_app_lib::audiobook::{encode_m4b, Chapter, M4bMetadata};

#[test]
fn writes_valid_m4b_with_chapters() {
    let sr: u32 = 44100;
    let dur_secs = 6.0;
    let n = (sr as f64 * dur_secs) as usize;
    let samples: Vec<f32> = (0..n)
        .map(|i| (i as f32 * 2.0 * std::f32::consts::PI * 440.0 / sr as f32).sin() * 0.2)
        .collect();

    let chapters = vec![
        Chapter { title: "Intro".into(), start_secs: 0.0 },
        Chapter { title: "Middle thing".into(), start_secs: 2.0 },
        Chapter { title: "Outro".into(), start_secs: 4.0 },
    ];
    let meta = M4bMetadata {
        title: "Yappy m4b smoke".into(),
        author: "test".into(),
        album: "test".into(),
    };

    let out = std::env::temp_dir().join("yappy_m4b_smoke.m4b");
    let _ = fs::remove_file(&out);
    encode_m4b(&samples, sr, &chapters, &meta, &out).expect("encode_m4b");

    let bytes = fs::read(&out).expect("read out");
    assert!(bytes.len() > 1000, "file suspiciously small: {} bytes", bytes.len());
    // Must contain ftyp/M4B major brand
    assert!(window_contains(&bytes, b"ftyp"), "missing ftyp box");
    assert!(window_contains(&bytes, b"M4B "), "missing M4B brand");
    // Must contain chpl somewhere (we just appended it)
    assert!(window_contains(&bytes, b"chpl"), "missing chpl atom");
    // Chapter titles must be present in raw bytes
    assert!(window_contains(&bytes, b"Intro"), "missing Intro title");
    assert!(window_contains(&bytes, b"Middle thing"), "missing middle title");
    assert!(window_contains(&bytes, b"Outro"), "missing Outro title");
    // mdat box must exist (audio data)
    assert!(window_contains(&bytes, b"mdat"), "missing mdat box");
    println!("OK: {} bytes, chpl present, 3 chapters", bytes.len());
}

fn window_contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}
