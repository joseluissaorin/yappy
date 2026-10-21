//! LA IMPRENTA DE HTML: una página autónoma (un solo fichero, sin red):
//! las letras de la casa y el loro van dentro en base64. Papel crema, dos
//! tintas, el título con su desregistro y, al pie, la pegatina «Leído con
//! Yappy». Se lee bien en el móvil, en el ordenador, de noche y en papel.

use base64::Engine;

use super::documento::{Clase, Documento, Marca, URL_WEB, WEB};

const LORO_SVG: &str = include_str!("../../../../brand/loro.svg");

const BALOO: &[u8] = include_bytes!("../../../static/fonts/Baloo2-Variable.woff2");
const MONO: &[u8] = include_bytes!("../../../static/fonts/SpaceMono-Regular.woff2");

/// Los cortes de Literata (romana, variable) por escritura, con su
/// `unicode-range` (el mismo que usa la app en static/fonts/fonts.css). Solo
/// viajan los que el documento necesita.
struct Corte {
    datos: &'static [u8],
    rango: &'static str,
    hace_falta: fn(char) -> bool,
}

const CORTES: &[Corte] = &[
    Corte {
        datos: include_bytes!("../../../static/fonts/literata-latin.woff2"),
        rango: "U+0000-00FF, U+0131, U+0152-0153, U+02BB-02BC, U+02C6, U+02DA, U+02DC, U+0304, U+0308, U+0329, U+2000-206F, U+20AC, U+2122, U+2191, U+2193, U+2212, U+2215, U+FEFF, U+FFFD",
        hace_falta: |_| true,
    },
    Corte {
        datos: include_bytes!("../../../static/fonts/literata-latin-ext.woff2"),
        rango: "U+0100-02BA, U+02BD-02C5, U+02C7-02CC, U+02CE-02D7, U+02DD-02FF, U+0304, U+0308, U+0329, U+1D00-1DBF, U+1E00-1E9F, U+1EF2-1EFF, U+2020, U+20A0-20AB, U+20AD-20C0, U+2113, U+2C60-2C7F, U+A720-A7FF",
        hace_falta: |c| matches!(c as u32, 0x0100..=0x02FF | 0x1E00..=0x1EFF),
    },
    Corte {
        datos: include_bytes!("../../../static/fonts/literata-cyrillic.woff2"),
        rango: "U+0301, U+0400-045F, U+0490-0491, U+04B0-04B1, U+2116",
        hace_falta: |c| matches!(c as u32, 0x0400..=0x045F | 0x0490..=0x04B1),
    },
    Corte {
        datos: include_bytes!("../../../static/fonts/literata-cyrillic-ext.woff2"),
        rango: "U+0460-052F, U+1C80-1C8A, U+20B4, U+2DE0-2DFF, U+A640-A69F, U+FE2E-FE2F",
        hace_falta: |c| matches!(c as u32, 0x0460..=0x052F),
    },
    Corte {
        datos: include_bytes!("../../../static/fonts/literata-greek.woff2"),
        rango: "U+0370-0377, U+037A-037F, U+0384-038A, U+038C, U+038E-03A1, U+03A3-03FF",
        hace_falta: |c| matches!(c as u32, 0x0370..=0x03FF),
    },
    Corte {
        datos: include_bytes!("../../../static/fonts/literata-greek-ext.woff2"),
        rango: "U+1F00-1FFF",
        hace_falta: |c| matches!(c as u32, 0x1F00..=0x1FFF),
    },
    Corte {
        datos: include_bytes!("../../../static/fonts/literata-vietnamese.woff2"),
        rango: "U+0102-0103, U+0110-0111, U+0128-0129, U+0168-0169, U+01A0-01A1, U+01AF-01B0, U+0300-0301, U+0303-0304, U+0308-0309, U+0323, U+0329, U+1EA0-1EF9, U+20AB",
        hace_falta: |c| matches!(c as u32, 0x01A0..=0x01B0 | 0x1EA0..=0x1EF9),
    },
];

pub fn html(doc: &Documento, marca: &Marca) -> String {
    let b64 = base64::engine::general_purpose::STANDARD;
    let texto = doc.todo_el_texto(marca);

    let mut fuentes = String::new();
    for corte in CORTES {
        if texto.chars().any(corte.hace_falta) {
            fuentes.push_str(&format!(
                "@font-face{{font-family:\"Literata\";font-style:normal;font-weight:200 900;src:url(data:font/woff2;base64,{}) format(\"woff2\");unicode-range:{};}}\n",
                b64.encode(corte.datos),
                corte.rango
            ));
        }
    }
    fuentes.push_str(&format!(
        "@font-face{{font-family:\"Baloo 2\";font-style:normal;font-weight:400 800;src:url(data:font/woff2;base64,{}) format(\"woff2\");}}\n",
        b64.encode(BALOO)
    ));
    fuentes.push_str(&format!(
        "@font-face{{font-family:\"Space Mono\";font-style:normal;font-weight:400;src:url(data:font/woff2;base64,{}) format(\"woff2\");}}\n",
        b64.encode(MONO)
    ));
    let loro = format!(
        "data:image/svg+xml;base64,{}",
        b64.encode(LORO_SVG.as_bytes())
    );

    let lang = doc
        .idioma
        .as_deref()
        .filter(|l| !l.is_empty())
        .unwrap_or("und");
    let mut h = String::with_capacity(texto.len() * 2 + 400_000);
    h.push_str("<!doctype html>\n");
    h.push_str(&format!(
        "<html lang=\"{}\">\n<head>\n<meta charset=\"utf-8\">\n",
        esc(lang)
    ));
    h.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    h.push_str("<meta name=\"color-scheme\" content=\"light dark\">\n");
    h.push_str(&format!("<title>{}</title>\n", esc(&doc.titulo)));
    if let Some(a) = &doc.autor {
        h.push_str(&format!("<meta name=\"author\" content=\"{}\">\n", esc(a)));
    }
    h.push_str("<meta name=\"generator\" content=\"Yappy\">\n");
    h.push_str(&format!("<link rel=\"icon\" href=\"{loro}\">\n"));
    h.push_str("<style>\n");
    h.push_str(&fuentes);
    h.push_str(CSS);
    h.push_str("</style>\n</head>\n<body>\n<main>\n<header class=\"cabecera\">\n");
    h.push_str(&format!(
        "<p class=\"susurro\">{}</p>\n",
        esc(&marca.exportado)
    ));
    h.push_str(&format!(
        "<h1 class=\"titulo\" dir=\"auto\">{}</h1>\n",
        esc(&doc.titulo)
    ));
    if let Some(a) = doc.autor.as_deref().filter(|a| !a.trim().is_empty()) {
        h.push_str(&format!("<p class=\"autor\" dir=\"auto\">{}</p>\n", esc(a)));
    }
    if let Some(u) = doc.url.as_deref().filter(|u| !u.trim().is_empty()) {
        let href = if u.starts_with("http://") || u.starts_with("https://") {
            u
        } else {
            ""
        };
        if href.is_empty() {
            h.push_str(&format!("<p class=\"fuente\">{}</p>\n", esc(u)));
        } else {
            h.push_str(&format!(
                "<p class=\"fuente\"><a href=\"{}\">{}</a></p>\n",
                esc(href),
                esc(u)
            ));
        }
    }
    h.push_str("</header>\n<hr class=\"costura\">\n<article>\n");

    let mut en_lista = false;
    for b in &doc.bloques {
        if en_lista && b.clase != Clase::Lista {
            h.push_str("</ul>\n");
            en_lista = false;
        }
        match b.clase {
            Clase::Titulo(n) => {
                let nivel = (n + 1).min(6);
                h.push_str(&format!(
                    "<h{nivel} dir=\"auto\">{}</h{nivel}>\n",
                    esc(&b.texto)
                ));
            }
            Clase::Parrafo => h.push_str(&format!("<p dir=\"auto\">{}</p>\n", esc(&b.texto))),
            Clase::Cita => h.push_str(&format!(
                "<blockquote dir=\"auto\"><p>{}</p></blockquote>\n",
                esc(&b.texto)
            )),
            Clase::Lista => {
                if !en_lista {
                    h.push_str("<ul>\n");
                    en_lista = true;
                }
                h.push_str(&format!("<li dir=\"auto\">{}</li>\n", esc(&b.texto)));
            }
            Clase::Separador => h.push_str("<hr>\n"),
            Clase::Codigo => h.push_str(&format!("<pre><code>{}</code></pre>\n", esc(&b.texto))),
            Clase::Verso => h.push_str(&format!(
                "<p class=\"verso\" dir=\"auto\">{}</p>\n",
                esc(&b.texto)
            )),
        }
    }
    if en_lista {
        h.push_str("</ul>\n");
    }
    h.push_str("</article>\n");
    h.push_str(&format!(
        "<footer class=\"marca\">\n<a class=\"pegatina\" href=\"{URL_WEB}\">\n<img src=\"{loro}\" alt=\"\" width=\"44\" height=\"44\">\n<span><span class=\"marca-texto\">{}</span><span class=\"marca-web\">{WEB}</span></span>\n</a>\n</footer>\n",
        esc(&marca.leido_con)
    ));
    h.push_str("</main>\n</body>\n</html>\n");
    h
}

fn esc(t: &str) -> String {
    let mut s = String::with_capacity(t.len() + 8);
    for c in t.chars() {
        match c {
            '&' => s.push_str("&amp;"),
            '<' => s.push_str("&lt;"),
            '>' => s.push_str("&gt;"),
            '"' => s.push_str("&quot;"),
            c => s.push(c),
        }
    }
    s
}

/// Los tokens de app.css (papel, tintas, sombra dura sin desenfoque) y las
/// tres voces: grito (Baloo 2), susurro (Space Mono) y lectura (Literata).
const CSS: &str = r#"
:root{--papel:#f7f2e7;--superficie:#fffdf7;--borde:#e6dcc6;--tinta:#2b2418;--tinta-suave:#82755a;--voz:#e0502a;--ultramar:#2f4bc4;--sombra:#ded7c2;
--lectura:"Literata",Georgia,"Hiragino Mincho ProN","Yu Mincho","Noto Serif CJK JP","Songti SC","Noto Serif CJK SC","AppleMyungjo","Noto Serif CJK KR","Noto Naskh Arabic","Geeza Pro","Kohinoor Devanagari","Noto Serif Devanagari",serif;
--grito:"Baloo 2","Literata","Hiragino Sans","PingFang SC","Apple SD Gothic Neo","Noto Sans CJK JP",system-ui,sans-serif;
--susurro:"Space Mono",ui-monospace,SFMono-Regular,Menlo,monospace}
@media (prefers-color-scheme:dark){:root{--papel:#191b22;--superficie:#232631;--borde:#3a3d4a;--tinta:#f2edda;--tinta-suave:#a9a48c;--voz:#ff7a4d;--ultramar:#93a5ff;--sombra:#0f1116}}
*{box-sizing:border-box}
html{background:var(--papel);-webkit-text-size-adjust:100%}
body{margin:0;background:var(--papel);color:var(--tinta);font-family:var(--lectura);font-size:19px;line-height:1.68;font-optical-sizing:auto;text-rendering:optimizeLegibility;hyphens:auto;-webkit-hyphens:auto;overflow-wrap:break-word}
main{max-width:38rem;margin:0 auto;padding:3.4rem 1.35rem 2.6rem}
.susurro{margin:0 0 .9rem;font-family:var(--susurro);font-size:.7rem;letter-spacing:.16em;text-transform:uppercase;color:var(--tinta-suave)}
.titulo{margin:0;font-family:var(--grito);font-weight:800;font-size:clamp(2.1rem,7vw,3rem);line-height:1.06;letter-spacing:-.01em;color:var(--voz);text-shadow:.045em .05em 0 var(--ultramar);hyphens:manual}
.autor{margin:.9rem 0 0;font-size:1.12rem;color:var(--tinta-suave)}
.fuente{margin:.35rem 0 0;font-family:var(--susurro);font-size:.74rem;line-height:1.5;word-break:break-all}
.fuente a{color:var(--ultramar)}
.costura{border:0;border-top:2.5px dashed var(--voz);opacity:.6;margin:2.1rem 0 2.2rem}
article h2,article h3,article h4,article h5,article h6{font-family:var(--grito);font-weight:800;color:var(--ultramar);line-height:1.15;margin:2.1em 0 .55em;hyphens:manual}
article h2{font-size:1.75rem}article h3{font-size:1.4rem}article h4{font-size:1.18rem}article h5,article h6{font-size:1.05rem}
article p{margin:0 0 1.05em}
article p+p{text-indent:0}
blockquote{margin:1.5em 0;padding:.15em 0 .15em 1.15em;border-left:3px solid var(--voz);color:var(--tinta)}
blockquote p{margin:0;opacity:.88}
[dir=rtl] blockquote,blockquote[dir=rtl]{border-left:0;border-right:3px solid var(--voz);padding:.15em 1.15em .15em 0}
ul{margin:0 0 1.1em;padding-left:1.35em}
li{margin:.3em 0}
li::marker{color:var(--voz)}
article hr{border:0;border-top:2.5px dashed var(--voz);width:5.5rem;margin:2.4em auto;opacity:.7}
pre{font-family:var(--susurro);font-size:.8rem;line-height:1.55;background:var(--superficie);border:1px solid var(--borde);border-radius:14px;padding:1em 1.1em;overflow:auto;box-shadow:2.5px 3px 0 var(--sombra);white-space:pre-wrap}
.verso{white-space:pre-line}
a{color:var(--ultramar)}
.marca{margin:4rem 0 1rem;display:flex;justify-content:center}
.pegatina{display:inline-flex;align-items:center;gap:.75rem;padding:.5rem 1.15rem .5rem .55rem;background:var(--superficie);border:1.5px solid var(--tinta);border-radius:28px 16px 24px 14px/18px 26px 16px 24px;box-shadow:3px 3.5px 0 var(--sombra);transform:rotate(-2.2deg);color:var(--tinta);text-decoration:none;transition:transform .16s cubic-bezier(.3,1.4,.6,1)}
.pegatina:hover{transform:rotate(1.2deg) scale(1.03)}
.pegatina img{display:block;width:44px;height:44px}
.pegatina>span{display:flex;flex-direction:column;line-height:1.2}
.marca-texto{font-family:var(--grito);font-weight:800;font-size:1.05rem}
.marca-web{font-family:var(--susurro);font-size:.68rem;letter-spacing:.06em;color:var(--tinta-suave)}
@media (max-width:480px){body{font-size:18px}main{padding-top:2.4rem}}
@media print{html,body{background:#fff}main{max-width:none;padding:0}.pegatina{box-shadow:none;transform:none}a{text-decoration:none}article h2,article h3{break-after:avoid}}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autonomo_y_con_marca() {
        let d = Documento::nuevo(
            "Cuento <raro> & cía",
            &["uno".into(), "dos".into(), "fin".into(), "Привет".into()],
            &[
                "list".into(),
                "list".into(),
                "paragraph".into(),
                "paragraph".into(),
            ],
        );
        let m = Marca {
            leido_con: "Leído con Yappy".into(),
            exportado: "Exportado hoy".into(),
        };
        let h = html(&d, &m);
        assert!(h.contains("<title>Cuento &lt;raro&gt; &amp; cía</title>"));
        assert!(h.contains("<ul>\n<li dir=\"auto\">uno</li>\n<li dir=\"auto\">dos</li>\n</ul>\n<p dir=\"auto\">fin</p>"));
        assert!(h.contains("Leído con Yappy"));
        assert!(h.contains("yappy.joseluissaorin.com"));
        assert!(h.contains("data:image/svg+xml;base64,"));
        // Cirílico presente: viaja su corte; griego no.
        assert!(h.contains("U+0400-045F"));
        assert!(!h.contains("U+1F00-1FFF"));
        // Nada fuera: ni una URL que no sea la del origen o la de la casa.
        assert!(!h.contains("src=\"http"));
    }
}
