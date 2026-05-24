//! Paragraph-based chunking.
//!
//! Supertonic 3 handles up to ~300 chars per call comfortably (120 for ko/ja).
//! For long-form reading we split by blank lines first, then by sentences when
//! a paragraph is too long. The returned chunks preserve order and are short
//! enough to synthesize quickly so the player can start audio after the first one.

use regex::Regex;

use once_cell::sync::Lazy;

static PARA_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\n\s*\n").unwrap());
static SENT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?P<p>[.!?…])(?:\s+|$)").unwrap());

const ABBREV: &[&str] = &[
    "Dr.", "Mr.", "Mrs.", "Ms.", "Prof.", "Sr.", "Jr.", "St.", "Ave.", "Rd.", "Blvd.", "Dept.",
    "Inc.", "Ltd.", "Co.", "Corp.", "etc.", "vs.", "i.e.", "e.g.", "Ph.D.", "No.", "vol.", "Vol.",
];

#[derive(Debug, Clone)]
pub struct Chunk {
    pub text: String,
    /// Character offset of the chunk's first byte in the original (post-normalize) text.
    pub start: usize,
    pub end: usize,
}

pub fn chunk_for_language(text: &str, lang: &str) -> Vec<Chunk> {
    let max = max_chars_for_lang(lang);
    chunk_paragraphs(text, max)
}

pub fn max_chars_for_lang(lang: &str) -> usize {
    match lang {
        "ko" | "ja" => 120,
        _ => 300,
    }
}

pub fn chunk_paragraphs(text: &str, max_chars: usize) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut cursor = 0usize;
    for raw_para in PARA_RE.split(text) {
        let start_in_text = find_offset(text, cursor, raw_para);
        let para = raw_para.trim();
        cursor = start_in_text + raw_para.len();
        if para.is_empty() {
            continue;
        }
        let para_start = start_in_text + raw_para.find(para.chars().next().unwrap_or(' ')).unwrap_or(0);
        if para.chars().count() <= max_chars {
            chunks.push(Chunk {
                text: para.to_string(),
                start: para_start,
                end: para_start + para.len(),
            });
            continue;
        }
        // Split the paragraph into sentences and re-merge up to max_chars.
        let sentences = split_sentences(para);
        let mut current = String::new();
        let mut current_start = para_start;
        let mut running_offset = para_start;
        for s in sentences {
            let s_len_chars = s.chars().count();
            if s_len_chars > max_chars {
                if !current.is_empty() {
                    chunks.push(Chunk {
                        text: current.trim().to_string(),
                        start: current_start,
                        end: current_start + current.len(),
                    });
                    current.clear();
                }
                // Split by comma, then by space.
                let sub = split_too_long(&s, max_chars);
                let mut sub_start = running_offset;
                for piece in sub {
                    let piece_len = piece.len();
                    chunks.push(Chunk {
                        text: piece.trim().to_string(),
                        start: sub_start,
                        end: sub_start + piece_len,
                    });
                    sub_start += piece_len;
                }
                running_offset += s.len();
                current_start = running_offset;
                continue;
            }
            if (current.chars().count() + s_len_chars + 1) > max_chars && !current.is_empty() {
                chunks.push(Chunk {
                    text: current.trim().to_string(),
                    start: current_start,
                    end: current_start + current.len(),
                });
                current.clear();
                current_start = running_offset;
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(&s);
            running_offset += s.len();
        }
        if !current.is_empty() {
            chunks.push(Chunk {
                text: current.trim().to_string(),
                start: current_start,
                end: current_start + current.len(),
            });
        }
    }
    if chunks.is_empty() && !text.trim().is_empty() {
        chunks.push(Chunk {
            text: text.trim().to_string(),
            start: 0,
            end: text.len(),
        });
    }
    chunks
}

fn find_offset(haystack: &str, start: usize, needle: &str) -> usize {
    if needle.is_empty() {
        return start;
    }
    haystack[start..]
        .find(needle)
        .map(|i| start + i)
        .unwrap_or(start)
}

fn split_sentences(text: &str) -> Vec<String> {
    let matches: Vec<_> = SENT_RE.find_iter(text).collect();
    if matches.is_empty() {
        return vec![text.to_string()];
    }
    let mut sentences = Vec::new();
    let mut last_end = 0;
    for m in matches {
        let before = &text[last_end..m.start() + 1]; // include the punctuation char
        let before_trim = before.trim_start();
        let mut is_abbrev = false;
        for a in ABBREV {
            if before_trim.trim_end().ends_with(a) {
                is_abbrev = true;
                break;
            }
        }
        if !is_abbrev {
            sentences.push(text[last_end..m.end()].to_string());
            last_end = m.end();
        }
    }
    if last_end < text.len() {
        sentences.push(text[last_end..].to_string());
    }
    sentences
}

fn split_too_long(s: &str, max_chars: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let parts: Vec<&str> = s.split(',').collect();
    let mut current = String::new();
    for part in parts {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        if p.chars().count() > max_chars {
            if !current.is_empty() {
                out.push(current.trim().to_string());
                current.clear();
            }
            // last resort: split on spaces
            let mut wchunk = String::new();
            for w in p.split_whitespace() {
                if wchunk.chars().count() + w.chars().count() + 1 > max_chars && !wchunk.is_empty()
                {
                    out.push(wchunk.trim().to_string());
                    wchunk.clear();
                }
                if !wchunk.is_empty() {
                    wchunk.push(' ');
                }
                wchunk.push_str(w);
            }
            if !wchunk.is_empty() {
                out.push(wchunk.trim().to_string());
            }
            continue;
        }
        if current.chars().count() + p.chars().count() + 2 > max_chars && !current.is_empty() {
            out.push(current.trim().to_string());
            current.clear();
        }
        if !current.is_empty() {
            current.push_str(", ");
        }
        current.push_str(p);
    }
    if !current.is_empty() {
        out.push(current.trim().to_string());
    }
    if out.is_empty() {
        out.push(s.trim().to_string());
    }
    out
}
