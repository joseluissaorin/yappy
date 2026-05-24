//! Named voice registry. Maps friendly names (Alex, James, …) to Supertonic 3
//! voice-style JSON files (M1.json, F1.json, …).

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub enum Gender {
    Male,
    Female,
}

#[derive(Debug, Clone, Serialize)]
pub struct Voice {
    /// Friendly UI name (Alex, James, …).
    pub name: &'static str,
    /// Internal Supertonic id (M1, F3, …) — also the filename stem in voice_styles/.
    pub id: &'static str,
    pub gender: Gender,
    /// One-line style description.
    pub description: &'static str,
    /// Suggested use cases.
    pub tags: &'static [&'static str],
}

/// All 10 open-weight voices Supertonic 3 ships, mapped to friendly names that
/// match Supertone's HF demo Space ordering.
pub const VOICES: &[Voice] = &[
    Voice {
        name: "Alex",
        id: "M1",
        gender: Gender::Male,
        description: "Lively, upbeat male voice with confident energy and a clear tone.",
        tags: &["energetic", "confident", "clear"],
    },
    Voice {
        name: "James",
        id: "M2",
        gender: Gender::Male,
        description: "Deep, robust male voice; calm, composed, and serious.",
        tags: &["deep", "serious", "composed"],
    },
    Voice {
        name: "Robert",
        id: "M3",
        gender: Gender::Male,
        description: "Polished, authoritative male voice; trustworthy and presentational.",
        tags: &["authoritative", "trustworthy", "polished"],
    },
    Voice {
        name: "Sam",
        id: "M4",
        gender: Gender::Male,
        description: "Soft, neutral-toned male voice; gentle and approachable.",
        tags: &["gentle", "friendly", "youthful"],
    },
    Voice {
        name: "Daniel",
        id: "M5",
        gender: Gender::Male,
        description: "Warm, soft-spoken male voice; calm storyteller.",
        tags: &["warm", "soothing", "storytelling"],
    },
    Voice {
        name: "Sarah",
        id: "F1",
        gender: Gender::Female,
        description: "Calm female voice with a slightly low tone; steady and composed.",
        tags: &["calm", "steady", "composed"],
    },
    Voice {
        name: "Lily",
        id: "F2",
        gender: Gender::Female,
        description: "Bright, cheerful female voice; lively, playful, and youthful.",
        tags: &["bright", "playful", "spirited"],
    },
    Voice {
        name: "Jessica",
        id: "F3",
        gender: Gender::Female,
        description: "Clear, professional announcer-style female voice; broadcast-ready.",
        tags: &["professional", "articulate", "broadcast"],
    },
    Voice {
        name: "Olivia",
        id: "F4",
        gender: Gender::Female,
        description: "Crisp, confident female voice; distinct and expressive.",
        tags: &["confident", "expressive", "distinct"],
    },
    Voice {
        name: "Emily",
        id: "F5",
        gender: Gender::Female,
        description: "Kind, gentle female voice; soft-spoken and soothing.",
        tags: &["gentle", "soothing", "empathetic"],
    },
];

pub fn by_name(name: &str) -> Option<&'static Voice> {
    VOICES.iter().find(|v| v.name.eq_ignore_ascii_case(name))
}

pub fn by_id(id: &str) -> Option<&'static Voice> {
    VOICES.iter().find(|v| v.id.eq_ignore_ascii_case(id))
}

pub fn default_voice() -> &'static Voice {
    &VOICES[7] // Jessica — professional broadcast-style, a safe default for "read me this".
}
