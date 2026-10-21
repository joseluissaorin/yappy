//! EXPORTAR: lo que se ha metido en Yappy sale a Markdown, HTML o PDF, tal
//! cual lo tiene la app (títulos, citas, listas, separadores), con título,
//! autor y origen si los hay, la fecha y la marca de agua de la casa.
//!
//! Libre para todos (sin puerta de Parlanchín): la marca de agua es
//! publicidad, y José Luis lo confirmó el 21-10-2026.
//!
//! Salida: en el móvil se escribe en la caché de la app y el frontend abre
//! la hoja de compartir del sistema (UIActivityViewController / ACTION_SEND
//! con FileProvider); en el escritorio, el frontend pide antes el destino
//! con el diálogo de guardar y lo pasa en `destino`.

pub mod documento;
mod fuentes;
pub mod html;
mod loro;
pub mod markdown;
pub mod pdf;

use std::path::{Path, PathBuf};

use serde::Deserialize;
use tauri::{AppHandle, Manager};

use documento::{nombre_fichero, Documento, Marca};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formato {
    Markdown,
    Html,
    Pdf,
}

impl Formato {
    pub fn desde(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "md" | "markdown" => Some(Formato::Markdown),
            "html" | "htm" => Some(Formato::Html),
            "pdf" => Some(Formato::Pdf),
            _ => None,
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            Formato::Markdown => "md",
            Formato::Html => "html",
            Formato::Pdf => "pdf",
        }
    }
}

/// Imprime el documento en el formato pedido.
pub fn imprimir(doc: &Documento, marca: &Marca, formato: Formato) -> Result<Vec<u8>, String> {
    match formato {
        Formato::Markdown => Ok(markdown::markdown(doc, marca).into_bytes()),
        Formato::Html => Ok(html::html(doc, marca).into_bytes()),
        Formato::Pdf => pdf::pdf(doc, marca),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeticionExportar {
    /// "md" | "html" | "pdf".
    pub formato: String,
    /// El documento en disco (la pieza de la cinta, el fichero abierto).
    pub ruta: Option<String>,
    /// O el documento tal como lo tiene la pantalla (con las ediciones del
    /// editor de escritorio): párrafos y su clase, arrays paralelos.
    pub parrafos: Option<Vec<String>>,
    pub kinds: Option<Vec<String>>,
    pub titulo: Option<String>,
    /// La URL o el nombre de archivo original (`ItemCola::origen`).
    pub origen: Option<String>,
    /// Idioma del texto si la app lo sabe ("auto" o vacío = adivinarlo).
    pub idioma: Option<String>,
    pub marca: Marca,
    /// Ruta elegida en el diálogo de guardar (escritorio). Sin ella, se
    /// escribe en la caché de la app para compartir.
    pub destino: Option<String>,
}

/// La ruta local de lo que llega del frontend (iOS manda a veces
/// `file://` con percent-encoding).
fn ruta_local(s: &str) -> PathBuf {
    if s.starts_with("file://") {
        if let Ok(u) = tauri::Url::parse(s) {
            if let Ok(p) = u.to_file_path() {
                return p;
            }
        }
    }
    PathBuf::from(s)
}

/// Metadatos de un EPUB: título, autor e idioma de su OPF.
fn metadatos_epub(p: &Path) -> (Option<String>, Option<String>, Option<String>) {
    let Ok(doc) = epub::doc::EpubDoc::new(p) else {
        return (None, None, None);
    };
    let campo = |n: &str| {
        doc.mdata(n)
            .map(|m| m.value.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    (campo("title"), campo("creator"), campo("language"))
}

fn montar(app: &AppHandle, pet: &PeticionExportar) -> Result<Documento, String> {
    let ruta = pet
        .ruta
        .as_deref()
        .filter(|r| !r.trim().is_empty())
        .map(ruta_local);
    let (parrafos, kinds) = match (&pet.parrafos, &pet.kinds) {
        (Some(p), k) if !p.is_empty() => (p.clone(), k.clone().unwrap_or_default()),
        _ => {
            let r = ruta.as_ref().ok_or("nada que exportar")?;
            let ricos =
                crate::capture::doc_loader::load_rich_from_file(r).map_err(|e| format!("{e:#}"))?;
            ricos.into_iter().map(|rp| (rp.text, rp.kind)).unzip()
        }
    };

    let es_epub = ruta
        .as_ref()
        .and_then(|r| r.extension())
        .map(|e| e.eq_ignore_ascii_case("epub"))
        .unwrap_or(false);
    let (t_epub, autor_epub, idioma_epub) = match (&ruta, es_epub) {
        (Some(r), true) => metadatos_epub(r),
        _ => (None, None, None),
    };
    let titulo = pet
        .titulo
        .clone()
        .filter(|t| !t.trim().is_empty())
        .or(t_epub)
        .or_else(|| {
            ruta.as_ref()
                .and_then(|r| r.file_stem())
                .map(|s| s.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "Yappy".into());

    let mut doc = Documento::nuevo(&titulo, &parrafos, &kinds);
    if doc.bloques.is_empty() {
        return Err("el documento está vacío".into());
    }
    doc.autor = autor_epub;
    let origen = pet.origen.as_deref().map(str::trim).unwrap_or("");
    if origen.starts_with("http://") || origen.starts_with("https://") {
        doc.url = Some(origen.to_string());
    } else if !origen.is_empty() && !origen.contains(['/', '.', '\\']) {
        // Un cuento de la casa: su origen es el id del cuento.
        if let Some(c) = crate::commands::cuento_empaquetado(app, origen) {
            if !c.autor.is_empty() {
                doc.autor = Some(c.autor.clone());
            }
            doc.idioma = Some(c.idioma.clone());
        }
    }
    let idioma_epub = idioma_epub.map(|l| {
        l.chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect::<String>()
            .to_lowercase()
    });
    let idioma_peticion = pet.idioma.clone().filter(|l| !l.is_empty() && l != "auto");
    doc.idioma = idioma_peticion
        .or(doc.idioma.take())
        .or(idioma_epub)
        .or_else(|| {
            let muestra: String = doc
                .bloques
                .iter()
                .take(40)
                .map(|b| b.texto.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let l = yappy_core::lang_detect::detect_document_lang(&muestra, "");
            (!l.is_empty()).then_some(l)
        });
    Ok(doc)
}

/// La carpeta de salida para compartir: se vacía en cada exportación para
/// no acumular copias (la hoja de compartir ya se ha llevado la anterior).
fn carpeta_para_compartir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    let dir = base.join("exportar");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Exporta un documento y devuelve la ruta del fichero escrito.
#[tauri::command]
pub async fn exportar_documento_cmd(
    app: AppHandle,
    peticion: PeticionExportar,
) -> Result<String, String> {
    let formato = Formato::desde(&peticion.formato)
        .ok_or_else(|| format!("formato desconocido: {}", peticion.formato))?;
    tokio::task::spawn_blocking(move || {
        let t = std::time::Instant::now();
        let doc = montar(&app, &peticion)?;
        let bytes = imprimir(&doc, &peticion.marca, formato)?;
        let destino = match peticion.destino.as_deref().filter(|d| !d.trim().is_empty()) {
            Some(d) => ruta_local(d),
            None => {
                carpeta_para_compartir(&app)?.join(nombre_fichero(&doc.titulo, formato.extension()))
            }
        };
        std::fs::write(&destino, &bytes)
            .map_err(|e| format!("no se pudo escribir {}: {e}", destino.display()))?;
        tracing::info!(
            "[exportar] {} «{}» → {} ({} KB, {} bloques, {:?})",
            formato.extension(),
            doc.titulo,
            destino.display(),
            bytes.len() / 1024,
            doc.bloques.len(),
            t.elapsed()
        );
        Ok(destino.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| format!("exportar: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Las muestras de verdad, con el cargador de la app: el EPUB del
    /// fixture, un cuento en español y otros en ruso y japonés (con árabe,
    /// hindi, coreano y hebreo de propina). Quedan en /tmp/yappy-export.
    ///   cargo test -p yappy-app --lib exportar::tests::muestras -- --ignored --nocapture
    #[test]
    #[ignore]
    fn muestras() {
        let dir = PathBuf::from("/tmp/yappy-export");
        std::fs::create_dir_all(&dir).unwrap();
        let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        let mut casos: Vec<(String, Documento, Marca)> = Vec::new();

        // 1. The Yellow Wallpaper (EPUB del fixture, por doc_loader).
        let epub = raiz.join("tests/fixtures/the-yellow-wallpaper.epub");
        let ricos = crate::capture::doc_loader::load_rich_from_file(&epub).expect("epub");
        let (p, k): (Vec<String>, Vec<String>) =
            ricos.into_iter().map(|r| (r.text, r.kind)).unzip();
        let (t, a, l) = metadatos_epub(&epub);
        let mut d = Documento::nuevo(t.as_deref().unwrap_or("The Yellow Wallpaper"), &p, &k);
        d.autor = a;
        d.idioma = l.or(Some("en".into()));
        casos.push((
            "the-yellow-wallpaper".into(),
            d,
            Marca {
                leido_con: "Read with Yappy".into(),
                exportado: "Exported on October 21, 2026".into(),
            },
        ));

        // 2. Un cuento en español (el de la casa), por el cargador de Markdown.
        let crudo =
            std::fs::read_to_string(raiz.join("../resources/cuentos/a-la-deriva.md")).unwrap();
        let cuerpo = crudo.splitn(3, "---").nth(2).unwrap();
        let ricos = crate::capture::doc_loader::parse_markdown_rhythm(cuerpo);
        let (p, k): (Vec<String>, Vec<String>) =
            ricos.into_iter().map(|r| (r.text, r.kind)).unzip();
        let mut d = Documento::nuevo("A la deriva", &p, &k);
        d.autor = Some("Horacio Quiroga".into());
        d.url = Some("https://es.wikisource.org/wiki/A_la_deriva".into());
        d.idioma = Some("es".into());
        casos.push((
            "a-la-deriva".into(),
            d,
            Marca {
                leido_con: "Leído con Yappy".into(),
                exportado: "Exportado el 21 de octubre de 2026".into(),
            },
        ));

        // 3 y 4. Ruso y japonés, escritos en Markdown y pasados por el mismo cargador.
        for (nombre, titulo, autor, url, idioma, md, marca) in [
            (
                "tolstyj-i-tonkij",
                "Толстый и тонкий",
                "Антон Павлович Чехов",
                "https://ru.wikisource.org/wiki/Толстый_и_тонкий_(Чехов)",
                "ru",
                MUESTRA_RU,
                Marca {
                    leido_con: "Прочитано в Yappy".into(),
                    exportado: "Экспортировано 21 октября 2026 г.".into(),
                },
            ),
            (
                "kumo-no-ito",
                "蜘蛛の糸",
                "芥川龍之介",
                "https://www.aozora.gr.jp/cards/000879/card92.html",
                "ja",
                MUESTRA_JA,
                Marca {
                    leido_con: "Yappyで読みました".into(),
                    exportado: "2026年10月21日にエクスポート".into(),
                },
            ),
        ] {
            let ricos = crate::capture::doc_loader::parse_markdown_rhythm(md);
            let (p, k): (Vec<String>, Vec<String>) =
                ricos.into_iter().map(|r| (r.text, r.kind)).unzip();
            let mut d = Documento::nuevo(titulo, &p, &k);
            d.autor = Some(autor.into());
            d.url = Some(url.into());
            d.idioma = Some(idioma.into());
            casos.push((nombre.into(), d, marca));
        }

        for (nombre, d, m) in casos {
            for f in [Formato::Markdown, Formato::Html, Formato::Pdf] {
                let t = std::time::Instant::now();
                let bytes = imprimir(&d, &m, f).unwrap();
                let ruta = dir.join(format!("{nombre}.{}", f.extension()));
                std::fs::write(&ruta, &bytes).unwrap();
                println!(
                    "{} · {} KB · {:?}",
                    ruta.display(),
                    bytes.len() / 1024,
                    t.elapsed()
                );
            }
        }
    }

    const MUESTRA_RU: &str = "# Толстый и тонкий

На вокзале Николаевской железной дороги встретились два приятеля: один толстый, другой тонкий. Толстый только что пообедал на вокзале, и губы его, подёрнутые маслом, лоснились, как спелые вишни. Пахло от него хересом и флёр-д’оранжем. Тонкий же только что вышел из вагона и был навьючен чемоданами, узлами и картонками. Пахло от него ветчиной и кофейной гущей.

Приятели троекратно облобызались и устремили друг на друга глаза, полные слёз. Оба были приятно ошеломлены.

## Примечание

> Рассказ впервые опубликован в журнале «Осколки» в 1883 году.

- чемоданы
- узлы
- картонки

---

Ελληνικά: Ἐν ἀρχῇ ἦν ὁ λόγος. Tiếng Việt: Tôi yêu tiếng nước tôi.
";

    const MUESTRA_JA: &str = "## 一

ある日の事でございます。御釈迦様は極楽の蓮池のふちを、独りでぶらぶら御歩きになっていらっしゃいました。池の中に咲いている蓮の花は、みんな玉のようにまっ白で、そのまん中にある金色の蕊からは、何とも云えない好い匂が、絶間なくあたりへ溢れて居ります。極楽は丁度朝なのでございましょう。

やがて御釈迦様はその池のふちに御佇みになって、水の面を蔽っている蓮の葉の間から、ふと下の容子を御覧になりました。この極楽の蓮池の下は、丁度地獄の底に当って居りますから、水晶のような水を透き徹して、三途の河や針の山の景色が、丁度覗き眼鏡を見るように、はっきりと見えるのでございます。

> 芥川龍之介「蜘蛛の糸」（1918年）、青空文庫より。

---

한국어: 다람쥐 헌 쳇바퀴에 타고파. العربية: مرحبا بالعالم، هذا نص تجريبي يُقرأ من اليمين إلى اليسار مع رقم 2026 وكلمة Yappy في الوسط. हिन्दी: यह एक परीक्षण वाक्य है। עברית: שלום עולם.

مرحبا بالعالم. هذه فقرة كاملة باللغة العربية لاختبار الاتجاه من اليمين إلى اليسار، والمحاذاة إلى اليمين، وتقطيع الأسطر عندما يكون النص أطول من عرض الصفحة.
";
}
