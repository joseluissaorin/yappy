//! LA IMPRENTA DE PDF, hecha en Rust para que salga igual en iOS, Android y
//! escritorio. El papel lo pone krilla (el mismo escritor de PDF que usa
//! Typst: letras incrustadas en subconjunto, texto seleccionable, marcadores
//! y metadatos); la maqueta es nuestra y pequeña: forma de letra con
//! rustybuzz, cortes de línea UAX #14 (incluido el CJK), bidi UAX #9 para el
//! árabe y el hebreo, y letra de respaldo carácter a carácter.
//!
//! Página A5 de bolsillo, papel crema, tinta de la casa (nunca negra), el
//! título con su desregistro coral sobre ultramar, el loro de marca de agua
//! en la primera página y, al pie de cada una, «Leído con Yappy ·
//! yappy.joseluissaorin.com» con su loro y el folio.

use std::collections::BTreeSet;
use std::ops::Range;
use std::sync::Arc;

use krilla::action::{Action, LinkAction};
use krilla::annotation::{Annotation, LinkAnnotation, Target};
use krilla::destination::XyzDestination;
use krilla::geom::{PathBuilder, Point, Rect};
use krilla::metadata::{DateTime, Metadata};
use krilla::outline::{Outline, OutlineNode};
use krilla::page::PageSettings;
use krilla::paint::{LineCap, Stroke, StrokeDash};
use krilla::surface::Surface;
use krilla::text::{GlyphId, KrillaGlyph};
use krilla::Document;
use unicode_bidi::BidiInfo;
use unicode_linebreak::{linebreaks, BreakOpportunity};

use super::documento::{hoy_iso, Clase, Documento, Marca, URL_WEB, WEB};
use super::fuentes::{self, Fuente, I_BALOO, I_LITERATA, I_LITERATA_NEGRA, I_MONO};
use super::loro::{self, relleno, CORAL, PAPEL, TINTA, ULTRAMAR};

// ── El pliego ─────────────────────────────────────────────────────────────
const ANCHO: f32 = 419.53; // A5
const ALTO: f32 = 595.28;
const MARGEN_X: f32 = 46.0;
const MARGEN_ARRIBA: f32 = 50.0;
const MARGEN_ABAJO: f32 = 60.0;
const CAJA: f32 = ANCHO - 2.0 * MARGEN_X;
const LIMITE: f32 = ALTO - MARGEN_ABAJO;

type Rgb = (u8, u8, u8);
const TINTA_SUAVE: Rgb = (0x82, 0x75, 0x5a);
const TINTA_CITA: Rgb = (0x55, 0x4a, 0x38);
const BORDE: Rgb = (0xe6, 0xdc, 0xc6);
const SUPERFICIE: Rgb = (0xff, 0xfd, 0xf7);

// ── Las letras, listas para dar forma y para escribir ────────────────────
struct Cara<'a> {
    forma: rustybuzz::Face<'a>,
    escritura: krilla::text::Font,
}

impl Cara<'_> {
    fn cubre(&self, c: char) -> bool {
        self.forma.glyph_index(c).is_some()
    }
}

#[derive(Clone)]
struct Estilo {
    cadena: Vec<usize>,
    tam: f32,
    alto_linea: f32,
    color: Rgb,
    justificar: bool,
    /// Espaciado entre letras, en em (el susurro va espaciado).
    espaciado: f32,
}

// ── La maqueta de un párrafo ─────────────────────────────────────────────
struct Item {
    rango: Range<usize>,
    cara: usize,
    glifos: Vec<KrillaGlyph>,
}

struct Trozo {
    cara: usize,
    x: f32,
    glifos: Vec<KrillaGlyph>,
}

struct Linea {
    texto: Arc<String>,
    trozos: Vec<Trozo>,
    ancho: f32,
    /// Desplazamiento por alineación (las líneas de derecha a izquierda se
    /// apoyan en el margen derecho).
    x0: f32,
}

/// Lo que el PDF no sabe pintar como texto: tabuladores, saltos y
/// separadores de párrafo raros se vuelven espacios.
fn sanear(t: &str) -> String {
    t.chars()
        .map(|c| {
            if c == '\t'
                || c == '\u{2028}'
                || c == '\u{2029}'
                || c == '\u{85}'
                || (c.is_control() && c != '\n')
            {
                ' '
            } else {
                c
            }
        })
        .collect()
}

fn maquetar(
    texto: &str,
    est: &Estilo,
    ancho_max: f32,
    caras: &[Cara<'_>],
    idioma: Option<&rustybuzz::Language>,
) -> Vec<Linea> {
    let texto: Arc<String> = Arc::new(sanear(texto).replace('\n', " "));
    if texto.trim().is_empty() {
        return Vec::new();
    }
    let len = texto.len();
    let bidi = BidiInfo::new(&texto, None);
    let Some(para) = bidi.paragraphs.first() else {
        return Vec::new();
    };
    let base_rtl = para.level.is_rtl();

    // 1. La letra de cada carácter: la primera de la cadena que lo tenga;
    //    los neutros (espacios, signos, marcas combinantes) se quedan con la
    //    del vecino para no partir la palabra en dos letras.
    let primera = |c: char| -> usize {
        est.cadena
            .iter()
            .copied()
            .find(|&k| caras[k].cubre(c))
            .unwrap_or(est.cadena[0])
    };
    let mut cara_de: Vec<(usize, usize)> = Vec::with_capacity(len); // (byte, cara)
    let mut previa: Option<usize> = None;
    for (i, c) in texto.char_indices() {
        let k = if fuentes::sin_glifo(c) {
            previa.unwrap_or(est.cadena[0])
        } else if !c.is_alphanumeric() {
            previa
                .filter(|&p| caras[p].cubre(c))
                .unwrap_or_else(|| primera(c))
        } else {
            primera(c)
        };
        cara_de.push((i, k));
        previa = Some(k);
    }

    // 2. Los items: tramos de igual letra y nivel bidi, cada uno con forma.
    let mut items: Vec<Item> = Vec::new();
    let mut ini = 0usize;
    for n in 0..cara_de.len() {
        let (i, k) = cara_de[n];
        let fin = cara_de.get(n + 1).map(|x| x.0).unwrap_or(len);
        let siguiente_distinto = cara_de
            .get(n + 1)
            .map(|&(j, k2)| k2 != k || bidi.levels[j] != bidi.levels[i])
            .unwrap_or(true);
        if siguiente_distinto {
            let rango = ini..fin;
            let rtl = bidi.levels[ini].is_rtl();
            let glifos = dar_forma(&texto, rango.clone(), &caras[k], rtl, est, idioma);
            items.push(Item {
                rango,
                cara: k,
                glifos,
            });
            ini = fin;
        }
    }

    // 3. Anchos acumulados por byte (para medir cualquier tramo lógico).
    let mut por_byte = vec![0f32; len + 1];
    for it in &items {
        for g in &it.glifos {
            por_byte[g.text_range.start] += g.x_advance * est.tam;
        }
    }
    let mut acum = vec![0f32; len + 1];
    for i in 0..len {
        acum[i + 1] = acum[i] + por_byte[i];
    }
    let medir = |a: usize, b: usize| acum[b] - acum[a];
    let recortar = |a: usize, b: usize| -> usize {
        let t = &texto[a..b];
        a + t.trim_end().len()
    };

    // 4. Cortes de línea: voraz sobre las oportunidades de UAX #14.
    let mut rangos: Vec<Range<usize>> = Vec::new();
    let mut ini = 0usize;
    let mut ultimo_ok: Option<usize> = None;
    for (pos, op) in linebreaks(&texto) {
        loop {
            if pos <= ini {
                break;
            }
            let fin = recortar(ini, pos);
            if medir(ini, fin) <= ancho_max + 0.01 {
                ultimo_ok = Some(pos);
                break;
            }
            if let Some(ok) = ultimo_ok.filter(|&o| o > ini) {
                rangos.push(ini..ok);
                ini = ok;
                ultimo_ok = None;
                continue;
            }
            // Una sola «palabra» más ancha que la caja (una URL larga): se
            // corta por caracteres.
            let mut corte = ini;
            for (j, c) in texto[ini..pos].char_indices() {
                let b = ini + j + c.len_utf8();
                if medir(ini, b) > ancho_max && corte > ini {
                    break;
                }
                corte = b;
            }
            rangos.push(ini..corte);
            ini = corte;
            ultimo_ok = None;
        }
        if op == BreakOpportunity::Mandatory && pos > ini {
            rangos.push(ini..pos);
            ini = pos;
            ultimo_ok = None;
        }
    }
    if ini < len {
        rangos.push(ini..len);
    }

    // 5. Cada línea, en orden visual.
    let total = rangos.len();
    let mut lineas = Vec::with_capacity(total);
    for (n, r) in rangos.into_iter().enumerate() {
        let vis = r.start..recortar(r.start, r.end);
        let mut trozos: Vec<Trozo> = Vec::new();
        if !vis.is_empty() {
            let (niveles, tramos) = bidi.visual_runs(para, vis.clone());
            for tramo in tramos {
                let rtl = niveles[tramo.start].is_rtl();
                let mut suyos: Vec<&Item> = items
                    .iter()
                    .filter(|it| it.rango.start < tramo.end && it.rango.end > tramo.start)
                    .collect();
                if rtl {
                    suyos.reverse();
                }
                for it in suyos {
                    let glifos: Vec<KrillaGlyph> = it
                        .glifos
                        .iter()
                        .filter(|g| {
                            g.text_range.start >= tramo.start && g.text_range.start < tramo.end
                        })
                        .cloned()
                        .collect();
                    if glifos.is_empty() {
                        continue;
                    }
                    match trozos.last_mut() {
                        Some(t) if t.cara == it.cara => t.glifos.extend(glifos),
                        _ => trozos.push(Trozo {
                            cara: it.cara,
                            x: 0.0,
                            glifos,
                        }),
                    }
                }
            }
        }
        let mut ancho: f32 = trozos
            .iter()
            .flat_map(|t| t.glifos.iter())
            .map(|g| g.x_advance * est.tam)
            .sum();
        // Justificar: el hueco que sobra se reparte entre los espacios (salvo
        // en la última línea y si el reparto abriría ríos).
        if est.justificar && !base_rtl && n + 1 < total && ancho < ancho_max {
            let espacios = trozos
                .iter()
                .flat_map(|t| t.glifos.iter())
                .filter(|g| texto[g.text_range.clone()].starts_with(' '))
                .count();
            let sobra = ancho_max - ancho;
            if espacios > 0 && sobra / espacios as f32 <= est.tam * 0.9 {
                let extra = sobra / espacios as f32 / est.tam;
                for t in trozos.iter_mut() {
                    for g in t.glifos.iter_mut() {
                        if texto[g.text_range.clone()].starts_with(' ') {
                            g.x_advance += extra;
                        }
                    }
                }
                ancho = ancho_max;
            }
        }
        let mut x = 0.0;
        for t in trozos.iter_mut() {
            t.x = x;
            x += t.glifos.iter().map(|g| g.x_advance * est.tam).sum::<f32>();
        }
        let x0 = if base_rtl {
            (ancho_max - ancho).max(0.0)
        } else {
            0.0
        };
        lineas.push(Linea {
            texto: texto.clone(),
            trozos,
            ancho,
            x0,
        });
    }
    lineas
}

fn dar_forma(
    texto: &str,
    rango: Range<usize>,
    cara: &Cara<'_>,
    rtl: bool,
    est: &Estilo,
    idioma: Option<&rustybuzz::Language>,
) -> Vec<KrillaGlyph> {
    let mut buf = rustybuzz::UnicodeBuffer::new();
    buf.push_str(&texto[rango.clone()]);
    buf.set_direction(if rtl {
        rustybuzz::Direction::RightToLeft
    } else {
        rustybuzz::Direction::LeftToRight
    });
    if let Some(l) = idioma {
        buf.set_language(l.clone());
    }
    buf.guess_segment_properties();
    let salida = rustybuzz::shape(&cara.forma, &[], buf);
    let upem = cara.forma.units_per_em() as f32;
    let infos = salida.glyph_infos();
    let pos = salida.glyph_positions();
    let mut inicios: Vec<usize> = infos.iter().map(|i| i.cluster as usize).collect();
    inicios.sort_unstable();
    inicios.dedup();
    let fin_de = |s: usize| -> usize {
        match inicios.binary_search(&s) {
            Ok(k) => inicios
                .get(k + 1)
                .copied()
                .unwrap_or(rango.end - rango.start),
            Err(_) => rango.end - rango.start,
        }
    };
    let mut v = Vec::with_capacity(infos.len());
    for (info, p) in infos.iter().zip(pos.iter()) {
        // Lo que ninguna letra del aparato sabe dibujar (un emoji sin su
        // fuente de color, por ejemplo) se omite: mejor un hueco que un tofu.
        if info.glyph_id == 0 {
            continue;
        }
        let s = info.cluster as usize;
        let mut avance = p.x_advance as f32 / upem;
        if est.espaciado != 0.0 && avance > 0.0 {
            avance += est.espaciado;
        }
        v.push(KrillaGlyph::new(
            GlyphId::new(info.glyph_id),
            avance,
            p.x_offset as f32 / upem,
            p.y_offset as f32 / upem,
            p.y_advance as f32 / upem,
            (rango.start + s)..(rango.start + fin_de(s)),
            None,
        ));
    }
    v
}

// ── Las órdenes de dibujo (primera pasada) ────────────────────────────────
enum Op {
    Texto {
        linea: Linea,
        x: f32,
        base: f32,
        tam: f32,
        color: Rgb,
        contorno_desplazado: Option<(f32, Rgb)>,
    },
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: Rgb,
    },
    Costura {
        x1: f32,
        x2: f32,
        y: f32,
        color: Rgb,
        grosor: f32,
    },
    Punto {
        cx: f32,
        cy: f32,
        r: f32,
        color: Rgb,
    },
    Enlace {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        url: String,
    },
    Loro {
        x: f32,
        y: f32,
        lado: f32,
    },
}

struct Pliego<'a> {
    caras: &'a [Cara<'a>],
    idioma: Option<rustybuzz::Language>,
    paginas: Vec<Vec<Op>>,
    y: f32,
    marcadores: Vec<(u8, String, usize, f32)>,
}

impl<'a> Pliego<'a> {
    fn pagina(&mut self) -> &mut Vec<Op> {
        self.paginas.last_mut().expect("siempre hay una página")
    }
    fn nueva_pagina(&mut self) {
        self.paginas.push(Vec::new());
        self.y = MARGEN_ARRIBA;
    }
    fn queda(&self) -> f32 {
        LIMITE - self.y
    }
    fn lineas(&self, texto: &str, est: &Estilo, ancho: f32) -> Vec<Linea> {
        maquetar(texto, est, ancho, self.caras, self.idioma.as_ref())
    }
    /// Coloca líneas ya maquetadas, saltando de página cuando no caben.
    /// Devuelve, por línea, (página, y de arriba) para las decoraciones.
    fn colocar(
        &mut self,
        lineas: Vec<Linea>,
        est: &Estilo,
        x: f32,
        contorno: Option<(f32, Rgb)>,
    ) -> Vec<(usize, f32)> {
        let mut sitios = Vec::with_capacity(lineas.len());
        let n = lineas.len();
        for (i, l) in lineas.into_iter().enumerate() {
            // Sin huérfanas: no se deja una primera línea sola al pie.
            let necesita = if i == 0 && n > 1 {
                est.alto_linea * 2.0
            } else {
                est.alto_linea
            };
            if self.queda() < necesita && self.y > MARGEN_ARRIBA + 1.0 {
                self.nueva_pagina();
            }
            let base = self.y + (est.alto_linea - est.tam) / 2.0 + est.tam * 0.8;
            sitios.push((self.paginas.len() - 1, self.y));
            let tam = est.tam;
            let color = est.color;
            self.pagina().push(Op::Texto {
                linea: l,
                x,
                base,
                tam,
                color,
                contorno_desplazado: contorno,
            });
            self.y += est.alto_linea;
        }
        sitios
    }
}

fn estilo(cadena: Vec<usize>, tam: f32, interlineado: f32, color: Rgb, justificar: bool) -> Estilo {
    Estilo {
        cadena,
        tam,
        alto_linea: tam * interlineado,
        color,
        justificar,
        espaciado: 0.0,
    }
}

/// Imprime el documento. `fuentes` = las propias + las de respaldo.
pub fn pdf(doc: &Documento, marca: &Marca) -> Result<Vec<u8>, String> {
    let mut juego = fuentes::propias();
    let texto = doc.todo_el_texto(marca).to_uppercase() + &doc.todo_el_texto(marca);
    let mut faltan: BTreeSet<char> = BTreeSet::new();
    {
        let propias: Vec<rustybuzz::Face<'_>> = juego
            .iter()
            .filter_map(|f| rustybuzz::Face::from_slice(f.bytes(), f.indice))
            .collect();
        for c in texto.chars() {
            if c.is_whitespace() || fuentes::sin_glifo(c) {
                continue;
            }
            if !propias.iter().any(|f| f.glyph_index(c).is_some()) {
                faltan.insert(c);
            }
        }
    }
    let respaldo = fuentes::respaldo(&faltan, doc.idioma.as_deref());
    if !respaldo.is_empty() {
        tracing::info!(
            "[exportar] letras prestadas del sistema: {}",
            respaldo
                .iter()
                .map(|f| f.nombre.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let n_propias = juego.len();
    juego.extend(respaldo);
    imprimir(doc, marca, &juego, n_propias)
}

fn imprimir(
    doc: &Documento,
    marca: &Marca,
    juego: &[Fuente],
    n_propias: usize,
) -> Result<Vec<u8>, String> {
    let mut caras: Vec<Cara<'_>> = Vec::with_capacity(juego.len());
    for f in juego {
        let forma = rustybuzz::Face::from_slice(f.bytes(), f.indice)
            .ok_or_else(|| format!("letra ilegible: {}", f.nombre))?;
        let escritura = f
            .krilla()
            .ok_or_else(|| format!("letra sin escritura: {}", f.nombre))?;
        caras.push(Cara { forma, escritura });
    }
    let respaldo: Vec<usize> = (n_propias..juego.len()).collect();
    let con = |mut v: Vec<usize>| {
        v.extend(respaldo.iter().copied());
        v
    };
    let idioma: Option<rustybuzz::Language> = doc.idioma.as_deref().and_then(|l| l.parse().ok());

    // Bandera a la izquierda (sin justificar): sin guionado, la justificación
    // abre ríos; la lectura se agradece más que el bloque perfecto.
    let lectura = estilo(con(vec![I_LITERATA]), 10.6, 1.53, TINTA, false);
    let cita = estilo(con(vec![I_LITERATA]), 10.2, 1.5, TINTA_CITA, false);
    let verso = estilo(con(vec![I_LITERATA]), 10.6, 1.5, TINTA, false);
    let codigo = estilo(con(vec![I_MONO, I_LITERATA]), 8.2, 1.5, TINTA, false);
    let grito = |tam: f32, color: Rgb| {
        estilo(
            con(vec![I_BALOO, I_LITERATA_NEGRA]),
            tam,
            1.16,
            color,
            false,
        )
    };
    let mut susurro = estilo(con(vec![I_MONO, I_LITERATA]), 6.6, 1.45, TINTA_SUAVE, false);
    susurro.espaciado = 0.12;

    let mut p = Pliego {
        caras: &caras,
        idioma,
        paginas: Vec::new(),
        y: MARGEN_ARRIBA,
        marcadores: Vec::new(),
    };
    p.nueva_pagina();

    // ── La cabecera ──
    let fecha = p.lineas(&marca.exportado.to_uppercase(), &susurro, CAJA);
    p.colocar(fecha, &susurro, MARGEN_X, None);
    p.y += 9.0;
    let titulo = grito(24.0, CORAL);
    let lt = p.lineas(&doc.titulo, &titulo, CAJA);
    // El desregistro: la tinta del texto (ultramar) asoma detrás, corrida.
    p.colocar(lt, &titulo, MARGEN_X, Some((0.95, ULTRAMAR)));
    if let Some(a) = doc.autor.as_deref().filter(|a| !a.trim().is_empty()) {
        p.y += 6.0;
        let est = estilo(con(vec![I_LITERATA]), 11.5, 1.4, TINTA_SUAVE, false);
        let l = p.lineas(a, &est, CAJA);
        p.colocar(l, &est, MARGEN_X, None);
    }
    if let Some(u) = doc.url.as_deref().filter(|u| !u.trim().is_empty()) {
        p.y += 3.0;
        let mut est = estilo(con(vec![I_MONO, I_LITERATA]), 7.0, 1.45, ULTRAMAR, false);
        est.espaciado = 0.0;
        let l = p.lineas(u, &est, CAJA);
        let y0 = p.y;
        let sitios = p.colocar(l, &est, MARGEN_X, None);
        if u.starts_with("http://") || u.starts_with("https://") {
            if let Some(&(pag, _)) = sitios.first() {
                if pag == 0 {
                    let alto = p.y - y0;
                    p.paginas[0].push(Op::Enlace {
                        x: MARGEN_X,
                        y: y0,
                        w: CAJA,
                        h: alto,
                        url: u.to_string(),
                    });
                }
            }
        }
    }
    p.y += 13.0;
    let y = p.y;
    p.pagina().push(Op::Costura {
        x1: MARGEN_X,
        x2: ANCHO - MARGEN_X,
        y,
        color: CORAL,
        grosor: 1.3,
    });
    p.y += 18.0;

    // ── El cuerpo ──
    let mut previa: Option<Clase> = None;
    for b in &doc.bloques {
        match b.clase {
            Clase::Titulo(n) => {
                let (tam, antes) = match n {
                    1 => (17.0, 16.0),
                    2 => (14.5, 13.0),
                    3 => (12.6, 11.0),
                    _ => (11.2, 9.0),
                };
                let est = grito(tam, ULTRAMAR);
                let l = p.lineas(&b.texto, &est, CAJA);
                if previa.is_some() {
                    p.y += antes;
                }
                // Un título nunca se queda solo al pie: pide sitio para él y
                // dos líneas de lo que sigue.
                let alto = l.len() as f32 * est.alto_linea + lectura.alto_linea * 2.0;
                if p.queda() < alto && p.y > MARGEN_ARRIBA + 1.0 {
                    p.nueva_pagina();
                }
                p.marcadores
                    .push((n, b.texto.clone(), p.paginas.len() - 1, p.y));
                p.colocar(l, &est, MARGEN_X, None);
                p.y += tam * 0.42;
            }
            Clase::Parrafo => {
                let l = p.lineas(&b.texto, &lectura, CAJA);
                p.colocar(l, &lectura, MARGEN_X, None);
                p.y += 6.0;
            }
            Clase::Verso => {
                for linea in b.texto.lines() {
                    if linea.trim().is_empty() {
                        p.y += verso.alto_linea * 0.6;
                        continue;
                    }
                    // Las líneas largas de un verso sangran al romper.
                    let l = p.lineas(linea, &verso, CAJA - 10.0);
                    let mut primeras = true;
                    for un in l {
                        let x = if primeras { MARGEN_X } else { MARGEN_X + 10.0 };
                        primeras = false;
                        p.colocar(vec![un], &verso, x, None);
                    }
                }
                p.y += 6.0;
            }
            Clase::Cita => {
                p.y += 2.0;
                let l = p.lineas(&b.texto, &cita, CAJA - 18.0);
                let sitios = p.colocar(l, &cita, MARGEN_X + 18.0, None);
                for (pag, y) in sitios {
                    p.paginas[pag].push(Op::Rect {
                        x: MARGEN_X + 3.0,
                        y: y + 1.5,
                        w: 1.8,
                        h: cita.alto_linea,
                        color: CORAL,
                    });
                }
                p.y += 8.0;
            }
            Clase::Lista => {
                let l = p.lineas(&b.texto, &lectura, CAJA - 16.0);
                let mut est = lectura.clone();
                est.justificar = false;
                let sitios = p.colocar(l, &est, MARGEN_X + 16.0, None);
                if let Some(&(pag, y)) = sitios.first() {
                    let cy = y + (lectura.alto_linea - lectura.tam) / 2.0 + lectura.tam * 0.47;
                    p.paginas[pag].push(Op::Punto {
                        cx: MARGEN_X + 6.0,
                        cy,
                        r: 1.9,
                        color: CORAL,
                    });
                }
                p.y += 2.5;
            }
            Clase::Separador => {
                p.y += 10.0;
                if p.queda() < 30.0 {
                    p.nueva_pagina();
                } else {
                    let y = p.y;
                    let mitad = ANCHO / 2.0;
                    p.pagina().push(Op::Costura {
                        x1: mitad - 36.0,
                        x2: mitad + 36.0,
                        y,
                        color: CORAL,
                        grosor: 1.3,
                    });
                    p.y += 18.0;
                }
            }
            Clase::Codigo => {
                p.y += 3.0;
                for linea in b.texto.lines() {
                    let l = p.lineas(
                        if linea.is_empty() { " " } else { linea },
                        &codigo,
                        CAJA - 20.0,
                    );
                    let l = if l.is_empty() {
                        // Línea en blanco dentro del código: se guarda el hueco.
                        p.y += codigo.alto_linea;
                        continue;
                    } else {
                        l
                    };
                    let sitios = p.colocar(l, &codigo, MARGEN_X + 10.0, None);
                    for (pag, y) in sitios {
                        // El fondo va DEBAJO del texto: se inserta al principio.
                        p.paginas[pag].insert(
                            0,
                            Op::Rect {
                                x: MARGEN_X,
                                y,
                                w: CAJA,
                                h: codigo.alto_linea,
                                color: SUPERFICIE,
                            },
                        );
                    }
                }
                p.y += 10.0;
            }
        }
        previa = Some(b.clase);
    }

    // ── El colofón: la página tiene final (el loro entre dos costuras) ──
    p.y += 16.0;
    if p.queda() < 40.0 {
        p.nueva_pagina();
    }
    let (y, mitad) = (p.y + 14.0, ANCHO / 2.0);
    p.pagina().push(Op::Costura {
        x1: mitad - 74.0,
        x2: mitad - 22.0,
        y,
        color: CORAL,
        grosor: 1.1,
    });
    p.pagina().push(Op::Costura {
        x1: mitad + 22.0,
        x2: mitad + 74.0,
        y,
        color: CORAL,
        grosor: 1.1,
    });
    p.pagina().push(Op::Loro {
        x: mitad - 13.0,
        y: y - 14.0,
        lado: 26.0,
    });

    // ── Segunda pasada: al papel, con el pie y el folio ──
    let total = p.paginas.len();
    let mut documento = Document::new();
    let pie_est = susurro.clone();
    let pie = maquetar(
        &format!("{} · {WEB}", marca.leido_con),
        &pie_est,
        CAJA,
        &caras,
        None,
    );
    let paginas = std::mem::take(&mut p.paginas);
    for (n, ops) in paginas.into_iter().enumerate() {
        let ajustes = PageSettings::from_wh(ANCHO, ALTO).ok_or("tamaño de página")?;
        let mut pagina = documento.start_page_with(ajustes);
        {
            let mut s = pagina.surface();
            // El papel (nunca blanco).
            rect(&mut s, 0.0, 0.0, ANCHO, ALTO, PAPEL);
            if n == 0 {
                // La marca de agua: el loro grande, tenue, saliéndose del pliego.
                loro::dibujar(&mut s, ANCHO - 182.0, ALTO - 214.0, 236.0, 0.05);
            }
            for op in &ops {
                match op {
                    Op::Rect { x, y, w, h, color } => rect(&mut s, *x, *y, *w, *h, *color),
                    Op::Costura {
                        x1,
                        x2,
                        y,
                        color,
                        grosor,
                    } => costura(&mut s, *x1, *x2, *y, *color, *grosor),
                    Op::Punto { cx, cy, r, color } => punto(&mut s, *cx, *cy, *r, *color),
                    Op::Texto {
                        linea,
                        x,
                        base,
                        tam,
                        color,
                        contorno_desplazado,
                    } => {
                        if let Some((d, c)) = contorno_desplazado {
                            // La capa de detrás va como trazo (outlined): así
                            // el texto del título no sale duplicado al copiar.
                            escribir(&mut s, &caras, linea, x + d, base + d, *tam, *c, true);
                        }
                        escribir(&mut s, &caras, linea, *x, *base, *tam, *color, false);
                    }
                    Op::Loro { x, y, lado } => loro::dibujar(&mut s, *x, *y, *lado, 1.0),
                    Op::Enlace { .. } => {}
                }
            }
            // El pie: costura fina, el loro chico, la marca y el folio.
            let y_pie = ALTO - 30.0;
            costura(&mut s, MARGEN_X, ANCHO - MARGEN_X, y_pie - 13.0, BORDE, 0.8);
            loro::dibujar(&mut s, MARGEN_X - 1.0, y_pie - 8.6, 11.5, 1.0);
            if let Some(l) = pie.first() {
                escribir(
                    &mut s,
                    &caras,
                    l,
                    MARGEN_X + 15.0,
                    y_pie,
                    pie_est.tam,
                    pie_est.color,
                    false,
                );
            }
            let folio = maquetar(
                &format!("{} / {}", n + 1, total),
                &pie_est,
                CAJA,
                &caras,
                None,
            );
            if let Some(l) = folio.first() {
                escribir(
                    &mut s,
                    &caras,
                    l,
                    ANCHO - MARGEN_X - l.ancho,
                    y_pie,
                    pie_est.tam,
                    pie_est.color,
                    false,
                );
            }
            s.finish();
        }
        // Los enlaces: el del origen y el de la marca, en cada pie.
        for op in &ops {
            if let Op::Enlace { x, y, w, h, url } = op {
                enlace(&mut pagina, *x, *y, *w, *h, url);
            }
        }
        let ancho_pie = 15.0 + pie.first().map(|l| l.ancho).unwrap_or(120.0);
        enlace(
            &mut pagina,
            MARGEN_X - 1.0,
            ALTO - 41.0,
            ancho_pie,
            16.0,
            URL_WEB,
        );
        pagina.finish();
    }

    // Marcadores (el índice del lector de PDF) y metadatos.
    if !p.marcadores.is_empty() {
        let mut indice = Outline::new();
        for nodo in arbol(&p.marcadores) {
            indice.push_child(nodo);
        }
        documento.set_outline(indice);
    }
    let (y, m, d, _) = hoy_iso();
    let mut meta = Metadata::new()
        .title(doc.titulo.clone())
        .creator(format!("Yappy · {WEB}"))
        .producer("Yappy (krilla)".into())
        .creation_date(DateTime::new(y).month(m).day(d));
    if let Some(a) = doc.autor.clone().filter(|a| !a.trim().is_empty()) {
        meta = meta.authors(vec![a]);
    }
    if let Some(l) = doc.idioma.clone().filter(|l| !l.is_empty()) {
        meta = meta.language(l);
    }
    documento.set_metadata(meta);
    documento.finish().map_err(|e| format!("krilla: {e:?}"))
}

fn arbol(entradas: &[(u8, String, usize, f32)]) -> Vec<OutlineNode> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < entradas.len() {
        let (nivel, ref texto, pag, y) = entradas[i];
        let mut j = i + 1;
        while j < entradas.len() && entradas[j].0 > nivel {
            j += 1;
        }
        let mut nodo = OutlineNode::new(
            texto.clone(),
            XyzDestination::new(pag, Point::from_xy(MARGEN_X, y)),
        );
        for hijo in arbol(&entradas[i + 1..j]) {
            nodo.push_child(hijo);
        }
        out.push(nodo);
        i = j;
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn escribir(
    s: &mut Surface<'_>,
    caras: &[Cara<'_>],
    l: &Linea,
    x: f32,
    base: f32,
    tam: f32,
    color: Rgb,
    contorno: bool,
) {
    s.set_stroke(None);
    s.set_fill(Some(relleno(color, 1.0)));
    for t in &l.trozos {
        s.draw_glyphs(
            Point::from_xy(x + l.x0 + t.x, base),
            &t.glifos,
            caras[t.cara].escritura.clone(),
            &l.texto,
            tam,
            contorno,
        );
    }
}

fn rect(s: &mut Surface<'_>, x: f32, y: f32, w: f32, h: f32, color: Rgb) {
    let Some(r) = Rect::from_xywh(x, y, w, h) else {
        return;
    };
    let mut pb = PathBuilder::new();
    pb.push_rect(r);
    if let Some(path) = pb.finish() {
        s.set_stroke(None);
        s.set_fill(Some(relleno(color, 1.0)));
        s.draw_path(&path);
    }
}

fn punto(s: &mut Surface<'_>, cx: f32, cy: f32, r: f32, color: Rgb) {
    const K: f32 = 0.552_284_8;
    let mut pb = PathBuilder::new();
    pb.move_to(cx + r, cy);
    pb.cubic_to(cx + r, cy + r * K, cx + r * K, cy + r, cx, cy + r);
    pb.cubic_to(cx - r * K, cy + r, cx - r, cy + r * K, cx - r, cy);
    pb.cubic_to(cx - r, cy - r * K, cx - r * K, cy - r, cx, cy - r);
    pb.cubic_to(cx + r * K, cy - r, cx + r, cy - r * K, cx + r, cy);
    pb.close();
    if let Some(path) = pb.finish() {
        s.set_stroke(None);
        s.set_fill(Some(relleno(color, 1.0)));
        s.draw_path(&path);
    }
}

/// La costura: puntadas discontinuas, el borde de la casa.
fn costura(s: &mut Surface<'_>, x1: f32, x2: f32, y: f32, color: Rgb, grosor: f32) {
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y);
    pb.line_to(x2, y);
    let Some(path) = pb.finish() else { return };
    s.set_fill(None);
    s.set_stroke(Some(Stroke {
        paint: krilla::color::rgb::Color::new(color.0, color.1, color.2).into(),
        width: grosor,
        line_cap: LineCap::Round,
        dash: Some(StrokeDash {
            array: vec![grosor * 2.6, grosor * 2.4],
            offset: 0.0,
        }),
        ..Default::default()
    }));
    s.draw_path(&path);
    s.set_stroke(None);
}

fn enlace(pagina: &mut krilla::page::Page<'_>, x: f32, y: f32, w: f32, h: f32, url: &str) {
    if let Some(r) = Rect::from_xywh(x, y, w, h) {
        pagina.add_annotation(Annotation::new_link(
            LinkAnnotation::new(
                r,
                Target::Action(Action::Link(LinkAction::new(url.to_string()))),
            ),
            None,
        ));
    }
}
