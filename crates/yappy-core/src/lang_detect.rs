//! Per-paragraph language detection. Falls back to the user-preferred language
//! when detection confidence is low or the text is too short. Supertonic's
//! `lang="na"` (language-agnostic) is used as the last resort.

use whatlang::Lang;

/// Try to detect a Supertonic-compatible BCP-47-ish 2-letter code for `text`.
/// Returns `default_lang` when the text is short or confidence is low.
pub fn detect_lang(text: &str, default_lang: &str) -> String {
    let stripped = text.trim();
    if stripped.chars().count() < 4 {
        return default_lang.to_string();
    }
    let info = match whatlang::detect(stripped) {
        Some(i) => i,
        None => return default_lang.to_string(),
    };
    // Accept lower-confidence detections for short text — Spanish/Italian/French often
    // get flagged "not reliable" by whatlang on short inputs even when correct.
    let len = stripped.chars().count();
    if !info.is_reliable() && len > 12 && info.confidence() < 0.50 {
        return default_lang.to_string();
    }
    let code = whatlang_to_supertonic(info.lang());
    if let Some(c) = code {
        c.to_string()
    } else {
        // Supertonic supports "na" as language-agnostic.
        "na".to_string()
    }
}

fn whatlang_to_supertonic(l: Lang) -> Option<&'static str> {
    // Supertonic 3 supported codes:
    // en ko ja ar bg cs da de el es et fi fr hi hr hu id it lt lv nl pl pt ro ru sk sl sv tr uk vi
    Some(match l {
        Lang::Eng => "en",
        Lang::Kor => "ko",
        Lang::Jpn => "ja",
        Lang::Ara => "ar",
        Lang::Bul => "bg",
        Lang::Ces => "cs",
        Lang::Dan => "da",
        Lang::Deu => "de",
        Lang::Ell => "el",
        Lang::Spa => "es",
        Lang::Est => "et",
        Lang::Fin => "fi",
        Lang::Fra => "fr",
        Lang::Hin => "hi",
        Lang::Hrv => "hr",
        Lang::Hun => "hu",
        Lang::Ind => "id",
        Lang::Ita => "it",
        Lang::Lit => "lt",
        Lang::Lav => "lv",
        Lang::Nld => "nl",
        Lang::Pol => "pl",
        Lang::Por => "pt",
        Lang::Ron => "ro",
        Lang::Rus => "ru",
        Lang::Slk => "sk",
        Lang::Slv => "sl",
        Lang::Swe => "sv",
        Lang::Tur => "tr",
        Lang::Ukr => "uk",
        Lang::Vie => "vi",
        _ => return None,
    })
}
