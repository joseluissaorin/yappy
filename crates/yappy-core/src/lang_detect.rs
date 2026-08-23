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
    let code = whatlang_to_supertonic(info.lang());
    let detectado = match code {
        Some(c) => c,
        // Idioma sin equivalente en Supertonic: si la detección ni siquiera
        // es fiable, mejor el idioma base; si es fiable de verdad (un texto
        // realmente en catalán, hebreo…), el modo agnóstico «na».
        None if info.is_reliable() => return "na".to_string(),
        None => return default_lang.to_string(),
    };
    if detectado == default_lang {
        return default_lang.to_string();
    }
    // Para CONTRADECIR el idioma base hace falta una detección FIABLE.
    // whatlang marca «no fiable» los párrafos cortos o con muchos números
    // y nombres propios, y verbalizar en el idioma equivocado es mucho
    // peor que quedarse con el del documento. (Su campo de confianza es
    // casi inservible: da 0,09 a un español evidente; no se usa.)
    if !info.is_reliable() {
        return default_lang.to_string();
    }
    detectado.to_string()
}

/// Idioma del documento entero. Con una muestra larga, el primer candidato
/// de whatlang acierta aunque se declare «no fiable» (le bajan la nota los
/// números y los nombres), así que aquí SÍ se acepta el top-1.
pub fn detect_document_lang(text: &str, fallback: &str) -> String {
    let muestra: String = text.chars().take(4000).collect();
    let stripped = muestra.trim();
    if stripped.chars().count() < 120 {
        return detect_lang(stripped, fallback);
    }
    match whatlang::detect(stripped) {
        Some(info) => whatlang_to_supertonic(info.lang())
            .unwrap_or(fallback)
            .to_string(),
        None => fallback.to_string(),
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
