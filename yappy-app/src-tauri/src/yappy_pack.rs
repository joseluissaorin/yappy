//! EL FORMATO .yappy: un audiolibro COMPLETO en un solo fichero (un ZIP).
//!
//!   pieza.yappy
//!   ├── yappy.json    el manifiesto (título, voz, idioma, duración, capítulos)
//!   ├── audio.m4b     el audio AAC con sus capítulos
//!   ├── tiempos.json  el KARAOKE: una entrada por frase con su momento y su
//!   │                 rango exacto sobre el texto original
//!   ├── texto.md      el documento fuente
//!   └── titulo.wav    (opcional) el título DICHO, para sonar al instante
//!
//! Con esto, renderizar a distancia es mandar un fichero; compartir un
//! audiolibro es compartir un fichero; e importar es leer un ZIP. El audio
//! va SIN recomprimir (Stored: ya es AAC); los textos, con deflate.

use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

pub const EXTENSION: &str = "yappy";
pub const VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapituloPack {
    pub titulo: String,
    pub inicio_s: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifiestoPack {
    pub version: u32,
    pub titulo: String,
    #[serde(default)]
    pub autor: String,
    pub voz: String,
    pub velocidad: f32,
    #[serde(default)]
    pub idioma: String,
    pub duracion_secs: f32,
    #[serde(default)]
    pub capitulos: Vec<CapituloPack>,
    #[serde(default)]
    pub creado_unix: u64,
    #[serde(default)]
    pub app_version: String,
}

/// Una frase del karaoke: cuándo suena y qué rango del texto original es.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TiempoFrase {
    pub ini_s: f32,
    pub fin_s: f32,
    pub parrafo: usize,
    pub origen_ini: usize,
    pub origen_fin: usize,
    pub texto: String,
}

pub struct ContenidoPack {
    pub manifiesto: ManifiestoPack,
    pub tiempos: Vec<TiempoFrase>,
    pub texto: String,
}

/// Escribe un .yappy completo. `audio_m4b` es la ruta del m4b ya codificado
/// (se copia dentro sin recomprimir); `titulo_wav` es opcional.
pub fn escribir(
    destino: &Path,
    manifiesto: &ManifiestoPack,
    audio_m4b: &Path,
    tiempos: &[TiempoFrase],
    texto: &str,
    titulo_wav: Option<&Path>,
) -> Result<()> {
    let f = fs::File::create(destino).with_context(|| format!("creando {}", destino.display()))?;
    let mut zip = zip::ZipWriter::new(f);
    let texto_opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let crudo_opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("yappy.json", texto_opts)?;
    zip.write_all(serde_json::to_string_pretty(manifiesto)?.as_bytes())?;

    zip.start_file("tiempos.json", texto_opts)?;
    zip.write_all(serde_json::to_string(tiempos)?.as_bytes())?;

    zip.start_file("texto.md", texto_opts)?;
    zip.write_all(texto.as_bytes())?;

    zip.start_file("audio.m4b", crudo_opts)?;
    let mut audio =
        fs::File::open(audio_m4b).with_context(|| format!("leyendo {}", audio_m4b.display()))?;
    std::io::copy(&mut audio, &mut zip)?;

    if let Some(wav) = titulo_wav {
        if wav.exists() {
            zip.start_file("titulo.wav", crudo_opts)?;
            let mut w = fs::File::open(wav)?;
            std::io::copy(&mut w, &mut zip)?;
        }
    }
    zip.finish()?;
    Ok(())
}

/// ¿Este fichero es un .yappy? (por extensión o por sus tripas).
pub fn es_yappy(ruta: &Path) -> bool {
    ruta.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case(EXTENSION))
        .unwrap_or(false)
}

fn abrir(ruta: &Path) -> Result<zip::ZipArchive<fs::File>> {
    let f = fs::File::open(ruta).with_context(|| format!("abriendo {}", ruta.display()))?;
    zip::ZipArchive::new(f).map_err(|e| anyhow!("no parece un .yappy válido: {e}"))
}

fn leer_entrada(zip: &mut zip::ZipArchive<fs::File>, nombre: &str) -> Result<Vec<u8>> {
    let mut e = zip
        .by_name(nombre)
        .map_err(|_| anyhow!("al .yappy le falta {nombre}"))?;
    let mut buf = Vec::with_capacity(e.size() as usize);
    e.read_to_end(&mut buf)?;
    Ok(buf)
}

/// Lee manifiesto, tiempos y texto (todo lo LIGERO) de un .yappy.
pub fn leer(ruta: &Path) -> Result<ContenidoPack> {
    let mut zip = abrir(ruta)?;
    let manifiesto: ManifiestoPack = serde_json::from_slice(&leer_entrada(&mut zip, "yappy.json")?)
        .map_err(|e| anyhow!("manifiesto ilegible: {e}"))?;
    if manifiesto.version > VERSION {
        return Err(anyhow!(
            "este .yappy es de una versión más nueva de Yappy (v{})",
            manifiesto.version
        ));
    }
    let tiempos: Vec<TiempoFrase> = leer_entrada(&mut zip, "tiempos.json")
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let texto = leer_entrada(&mut zip, "texto.md")
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default();
    Ok(ContenidoPack {
        manifiesto,
        tiempos,
        texto,
    })
}

/// Solo el manifiesto (para listar la biblioteca sin pagar el resto).
pub fn leer_manifiesto(ruta: &Path) -> Result<ManifiestoPack> {
    let mut zip = abrir(ruta)?;
    serde_json::from_slice(&leer_entrada(&mut zip, "yappy.json")?)
        .map_err(|e| anyhow!("manifiesto ilegible: {e}"))
}

/// EXTRAE el audio (y el título dicho, si viene) a la caché dada, con un
/// nombre estable derivado del propio .yappy. Devuelve la ruta del m4b.
/// Los reproductores nativos quieren un FICHERO, no bytes.
pub fn extraer_audio(ruta: &Path, cache_dir: &Path) -> Result<std::path::PathBuf> {
    fs::create_dir_all(cache_dir)?;
    let mut h: u64 = 0;
    let meta = fs::metadata(ruta)?;
    for b in ruta.to_string_lossy().bytes() {
        h = h.wrapping_mul(131).wrapping_add(b as u64);
    }
    h = h.wrapping_mul(131).wrapping_add(meta.len());
    let destino = cache_dir.join(format!("{h:016x}.m4b"));
    if destino.exists() {
        return Ok(destino);
    }
    let mut zip = abrir(ruta)?;
    let bytes = leer_entrada(&mut zip, "audio.m4b")?;
    // Escritura atómica: nada de m4b a medias en la caché.
    let tmp = destino.with_extension("m4b.tmp");
    fs::write(&tmp, &bytes)?;
    fs::rename(&tmp, &destino)?;
    Ok(destino)
}

/// El título dicho (titulo.wav), si el .yappy lo trae.
pub fn extraer_titulo_wav(ruta: &Path, destino: &Path) -> Result<bool> {
    let mut zip = abrir(ruta)?;
    match leer_entrada(&mut zip, "titulo.wav") {
        Ok(bytes) => {
            if let Some(padre) = destino.parent() {
                fs::create_dir_all(padre)?;
            }
            fs::write(destino, &bytes)?;
            Ok(true)
        }
        Err(_) => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "motor real: sintetiza, empaqueta un .yappy y lo relee"]
    fn el_circuito_entero_con_el_motor_real() {
        let root = dirs::data_dir()
            .unwrap()
            .join("com.yappy.app/models/supertonic-3");
        assert!(root.exists(), "no está el modelo en {root:?}");
        let engine =
            yappy_core::TtsEngine::new(yappy_core::engine::engine_config(&root)).expect("motor");
        let opts = yappy_core::engine::SynthesisOptions {
            voice: "Daniel".into(),
            speed: 1.05,
            default_lang: "es".into(),
            total_steps: 4,
            seed: None,
            detectar_idioma: true,
            pausa_entre_parrafos_s: 0.0,
        };
        let texto = "Hola, esto es una prueba del formato de la casa. La segunda frase confirma los tiempos.";
        let mut samples: Vec<f32> = Vec::new();
        let mut sr = 44_100u32;
        let mut tiempos: Vec<TiempoFrase> = Vec::new();
        engine
            .synthesize_streaming(texto, &opts, |chunk| {
                sr = chunk.sample_rate as u32;
                let ini = samples.len();
                samples.extend_from_slice(&chunk.samples);
                if !chunk.es_pausa && !chunk.text.trim().is_empty() {
                    tiempos.push(TiempoFrase {
                        ini_s: ini as f32 / sr as f32,
                        fin_s: samples.len() as f32 / sr as f32,
                        parrafo: chunk.paragraph_index,
                        origen_ini: chunk.origen_ini,
                        origen_fin: chunk.origen_fin,
                        texto: chunk.text.clone(),
                    });
                }
                Ok(())
            })
            .expect("síntesis");
        assert!(
            tiempos.len() >= 2,
            "esperaba 2 frases, hay {}",
            tiempos.len()
        );
        // Los tiempos son monótonos y cubren el audio.
        for v in tiempos.windows(2) {
            assert!(v[1].ini_s >= v[0].ini_s);
        }
        let dur = samples.len() as f32 / sr as f32;
        assert!(tiempos.last().unwrap().fin_s <= dur + 0.01);

        let dir = std::env::temp_dir().join("yappy-circuito");
        let _ = fs::create_dir_all(&dir);
        let m4b = dir.join("circuito.m4b");
        crate::audiobook::encode_m4b(
            &samples,
            sr,
            &[crate::audiobook::Chapter {
                title: "Prueba".into(),
                start_secs: 0.0,
            }],
            &crate::audiobook::M4bMetadata {
                title: "La prueba".into(),
                author: "Yappy".into(),
                album: "La prueba".into(),
            },
            &m4b,
        )
        .expect("m4b");
        let destino = dir.join("circuito.yappy");
        let manifiesto = ManifiestoPack {
            version: VERSION,
            titulo: "La prueba del circuito".into(),
            autor: "Yappy".into(),
            voz: "Daniel".into(),
            velocidad: 1.05,
            idioma: "es".into(),
            duracion_secs: dur,
            capitulos: vec![CapituloPack {
                titulo: "Prueba".into(),
                inicio_s: 0.0,
            }],
            creado_unix: 0,
            app_version: "test".into(),
        };
        escribir(&destino, &manifiesto, &m4b, &tiempos, texto, None).expect("pack");

        let leido = leer(&destino).expect("releer");
        assert_eq!(leido.manifiesto.titulo, "La prueba del circuito");
        assert_eq!(leido.tiempos.len(), tiempos.len());
        assert!(leido.texto.contains("segunda frase"));
        let cache = dir.join("cache");
        let audio = extraer_audio(&destino, &cache).expect("audio");
        let info = crate::audiobook::read_m4b_info(&audio).expect("info del m4b extraído");
        assert!(info.duration_secs > 1.0);
        println!(
            "CIRCUITO OK: {:.1}s de audio, {} frases, m4b de {} bytes",
            dur,
            leido.tiempos.len(),
            fs::metadata(&audio).unwrap().len()
        );
        for t in &leido.tiempos {
            println!(
                "  {:.2}-{:.2}s  p{} [{}..{}] «{}»",
                t.ini_s, t.fin_s, t.parrafo, t.origen_ini, t.origen_fin, t.texto
            );
        }
    }

    #[test]
    fn un_yappy_va_y_vuelve_entero() {
        let dir = std::env::temp_dir().join("yappy-pack-prueba");
        let _ = fs::create_dir_all(&dir);
        let m4b = dir.join("audio-falso.m4b");
        fs::write(&m4b, b"no es un m4b de verdad pero pesa").unwrap();
        let destino = dir.join("pieza.yappy");
        let manifiesto = ManifiestoPack {
            version: VERSION,
            titulo: "La última pregunta".into(),
            autor: "Isaac Asimov".into(),
            voz: "Alex".into(),
            velocidad: 1.05,
            idioma: "es".into(),
            duracion_secs: 123.5,
            capitulos: vec![CapituloPack {
                titulo: "Uno".into(),
                inicio_s: 0.0,
            }],
            creado_unix: 0,
            app_version: "0.2.0".into(),
        };
        let tiempos = vec![TiempoFrase {
            ini_s: 0.0,
            fin_s: 2.5,
            parrafo: 0,
            origen_ini: 0,
            origen_fin: 21,
            texto: "La última pregunta se".into(),
        }];
        escribir(
            &destino,
            &manifiesto,
            &m4b,
            &tiempos,
            "# La última pregunta\n\nHola.",
            None,
        )
        .unwrap();

        assert!(es_yappy(&destino));
        let leido = leer(&destino).unwrap();
        assert_eq!(leido.manifiesto.titulo, "La última pregunta");
        assert_eq!(leido.tiempos.len(), 1);
        assert!(leido.texto.contains("Hola."));
        let cache = dir.join("cache");
        let audio = extraer_audio(&destino, &cache).unwrap();
        assert_eq!(
            fs::read(audio).unwrap(),
            b"no es un m4b de verdad pero pesa"
        );
        let mani = leer_manifiesto(&destino).unwrap();
        assert_eq!(mani.capitulos.len(), 1);
    }
}
