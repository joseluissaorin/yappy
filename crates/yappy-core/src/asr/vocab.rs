//! SentencePiece `vocab.txt` parser for Parakeet TDT.
//!
//! Each line is `"<token> <id>"`. Tokens use the SentencePiece `▁` marker for a
//! leading space. The blank token (`<blk>`) is the last id; the TDT decoder uses
//! `vocab_size - 1` as the blank. Adapted from `parakeet-rs` (MIT/Apache-2.0).

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Vocabulary {
    id_to_token: Vec<String>,
}

impl Vocabulary {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let file =
            File::open(path).with_context(|| format!("opening vocab.txt at {}", path.display()))?;
        let reader = BufReader::new(file);
        let mut id_to_token: Vec<String> = Vec::new();

        for line in reader.lines() {
            let line = line.context("reading vocab.txt")?;
            // Split on the LAST space so tokens that are themselves a space-ish
            // glyph still parse; format is "<token> <id>".
            let parts: Vec<&str> = line.rsplitn(2, ' ').collect();
            if parts.len() == 2 {
                // rsplitn yields [id, token]
                let token = parts[1].to_string();
                if let Ok(id) = parts[0].parse::<usize>() {
                    if id >= id_to_token.len() {
                        id_to_token.resize(id + 1, String::new());
                    }
                    id_to_token[id] = token;
                }
            }
        }

        Ok(Self { id_to_token })
    }

    pub fn id_to_text(&self, id: usize) -> Option<&str> {
        self.id_to_token.get(id).map(|s| s.as_str())
    }

    pub fn size(&self) -> usize {
        self.id_to_token.len()
    }
}
