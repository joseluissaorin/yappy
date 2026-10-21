//! PDF con capa de texto → párrafos de lectura, a partir de los CARACTERES y
//! su geometría (pdf_oxide `extract_chars`), igual en todas las plataformas.
//!
//! Por qué no el `to_markdown` de pdf_oxide: pierde los apóstrofos y comillas
//! tipográficas que el PDF dibuja como glifos sueltos («Munro’s» → «Munros»),
//! desordena las ligaduras («deﬁning» → «ﬁ ningde»), mete las cabeceras
//! corrientes en mitad de la prosa («Hiromi Goto 77 of the iconic…») y las
//! notas al pie entre dos mitades de una frase. Con la posición de cada
//! carácter se puede hacer lo que hace un lector humano:
//!
//! 1. **Renglones**: los caracteres se agrupan por línea base; los volados
//!    pequeños (llamadas de nota «¹²») se reconocen por tamaño y altura y no
//!    se leen. Los espacios salen de los huecos reales entre glifos.
//! 2. **Orden de lectura**: los renglones que se solapan en altura forman
//!    filas; las filas consecutivas que dejan libre la misma calle vertical
//!    forman una franja a varias columnas, que se lee columna a columna. Si
//!    las «columnas» son estrechas es una tabla y se lee fila a fila.
//! 3. **Mobiliario**: lo que está en la franja superior o inferior de la
//!    página y se repite (con los números enmascarados) en muchas páginas es
//!    cabecera o pie; los folios caen por forma.
//! 4. **Notas al pie**: bloque de letra menor al fondo de la página, sin
//!    cuerpo debajo, que empieza por número. Se sacan del flujo (ya no
//!    parten frases entre páginas) y se leen al final, tras una pausa.
//! 5. **Párrafos**: se cortan por hueco vertical, sangría, cambio de cuerpo
//!    de letra o línea corta acabada en punto; una frase que sigue en la
//!    columna o página siguiente se cose. Los guiones de corte de línea se
//!    resuelven con el vocabulario del propio documento.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{anyhow, Result};

use super::doc_loader::{rich, RichParagraph};

#[derive(Clone, Debug)]
struct Car {
    c: char,
    x0: f32,
    x1: f32,
    base: f32,
    fs: f32,
    negrita: bool,
    /// Volado (superíndice) respecto de su renglón.
    volado: bool,
}

#[derive(Clone, Debug)]
struct Renglon {
    pagina: usize,
    x0: f32,
    x1: f32,
    base: f32,
    fs: f32,
    texto: String,
    /// Renglón entero en negrita (para diagnóstico; aún no decide nada).
    #[allow(dead_code)]
    negrita: bool,
    /// Límites de la columna (o del texto de la página) en que vive.
    izq: f32,
    der: f32,
    /// Primer renglón de una columna o franja nueva (el orden «salta»).
    salto: bool,
    /// Fila de tabla: párrafo propio.
    tabla: bool,
    /// Empieza por una cifra volada: la marca de una nota al pie.
    abre_volado: bool,
}

/// Lee el PDF y devuelve sus párrafos. `Err` si no hay capa de texto útil.
pub fn leer(path: &Path) -> Result<Vec<RichParagraph>> {
    let doc = pdf_oxide::PdfDocument::open(path).map_err(|e| anyhow!("pdf_oxide open: {e:?}"))?;
    let paginas = doc
        .page_count()
        .map_err(|e| anyhow!("pdf_oxide page_count: {e:?}"))?;
    let mut por_pagina: Vec<Vec<Renglon>> = Vec::with_capacity(paginas);
    let mut alturas: Vec<(f32, f32)> = Vec::with_capacity(paginas);
    for i in 0..paginas {
        let caja = doc
            .get_page_media_box(i)
            .unwrap_or((0.0, 0.0, 612.0, 792.0));
        let (y_min, y_max) = (caja.1.min(caja.3), caja.1.max(caja.3));
        alturas.push((y_min, y_max));
        let chars = match doc.extract_chars(i) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("pdf_maqueta: página {} sin caracteres: {e:?}", i + 1);
                Vec::new()
            }
        };
        let cars = a_cars(&chars);
        let renglones = maquetar_pagina(i, cars);
        if std::env::var_os("YAPPY_PDF_RENGLONES").is_some() {
            for r in &renglones {
                eprintln!(
                    "p{} x={:.0}-{:.0} base={:.1} fs={:.1} [{:.0},{:.0}]{}{} {}",
                    r.pagina,
                    r.x0,
                    r.x1,
                    r.base,
                    r.fs,
                    r.izq,
                    r.der,
                    if r.salto { " SALTO" } else { "" },
                    if r.tabla { " TABLA" } else { "" },
                    r.texto
                );
            }
        }
        por_pagina.push(renglones);
    }
    Ok(ensamblar(por_pagina, &alturas))
}

// ───────────────────────────── caracteres ─────────────────────────────

/// Una franja de la página: índices de sus filas y los tramos horizontales que cubre.
type Franja = (Vec<usize>, Vec<(f32, f32)>);

fn a_cars(chars: &[pdf_oxide::layout::TextChar]) -> Vec<Car> {
    chars
        .iter()
        .filter(|t| {
            let r = t.rotation_degrees.rem_euclid(360.0);
            // El texto girado (lomos, márgenes de arXiv, sellos) no se lee.
            !(5.0..=355.0).contains(&r) && !t.char.is_control()
        })
        .map(|t| {
            let alto = t.bbox.height.abs();
            let mut fs = t.font_size.abs();
            if fs < 0.5 || (alto > 0.5 && (fs < alto * 0.45 || fs > alto * 3.0)) {
                fs = alto.max(1.0);
            }
            let ancho = if t.bbox.width > 0.0 {
                t.bbox.width
            } else {
                t.advance_width.max(0.0)
            };
            Car {
                c: if t.char.is_whitespace() { ' ' } else { t.char },
                x0: t.bbox.x,
                x1: t.bbox.x + ancho,
                base: t.origin_y,
                fs,
                negrita: (t.font_weight as u16) >= 600
                    || t.font_name.to_ascii_lowercase().contains("bold"),
                volado: false,
            }
        })
        .collect()
}

fn mediana(mut v: Vec<f32>) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v[v.len() / 2]
}

fn es_rtl(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF)
}

fn es_cjk(c: char) -> bool {
    matches!(c as u32, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF)
}

// ───────────────────────────── una página ─────────────────────────────

/// Un trozo de renglón: caracteres de la misma línea base sin calle en medio.
struct Trozo {
    cars: Vec<Car>,
    x0: f32,
    x1: f32,
    base: f32,
    fs: f32,
}

fn maquetar_pagina(pagina: usize, mut cars: Vec<Car>) -> Vec<Renglon> {
    if cars.is_empty() {
        return Vec::new();
    }
    // 1. Líneas base finas.
    cars.sort_by(|a, b| {
        b.base
            .partial_cmp(&a.base)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut lineas: Vec<Vec<Car>> = Vec::new();
    for c in cars {
        match lineas.last_mut() {
            Some(l) if (l[0].base - c.base).abs() <= 0.2 * l[0].fs.max(c.fs).max(1.0) => l.push(c),
            _ => lineas.push(vec![c]),
        }
    }
    // 2. Volados: una línea de letra pequeña pegada por encima (o por debajo)
    //    a otra mayor y dentro de su ancho se funde en ella.
    let fs_de = |l: &Vec<Car>| mediana(l.iter().filter(|c| c.c != ' ').map(|c| c.fs).collect());
    let extension = |l: &Vec<Car>| {
        (
            l.iter().map(|c| c.x0).fold(f32::MAX, f32::min),
            l.iter().map(|c| c.x1).fold(f32::MIN, f32::max),
        )
    };
    let mut i = 0;
    while i < lineas.len() {
        let fs_i = fs_de(&lineas[i]);
        let (a0, a1) = extension(&lineas[i]);
        let mut destino = None;
        for j in [i.wrapping_sub(1), i + 1] {
            if j >= lineas.len() || j == i {
                continue;
            }
            let fs_j = fs_de(&lineas[j]);
            let (b0, b1) = extension(&lineas[j]);
            let dist = (lineas[i][0].base - lineas[j][0].base).abs();
            if fs_i < 0.85 * fs_j
                && dist <= 0.6 * fs_j
                && a0 >= b0 - fs_j
                && a1 <= b1 + fs_j
                && lineas[i].iter().filter(|c| c.c != ' ').count() <= 4
            {
                destino = Some(j);
                break;
            }
        }
        if let Some(j) = destino {
            let base_j = lineas[j][0].base;
            let mut movidos = std::mem::take(&mut lineas[i]);
            for c in &mut movidos {
                c.volado = c.base > base_j;
            }
            lineas[j].extend(movidos);
            lineas.remove(i);
            continue;
        }
        i += 1;
    }
    // 3. Trozos: se corta donde hay una calle (hueco mayor que un cuerpo).
    let mut trozos: Vec<Trozo> = Vec::new();
    for mut l in lineas {
        l.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal));
        let fs_l = fs_de(&l).max(1.0);
        // Dentro de la línea, los volados se marcan también por tamaño y altura.
        let base_l = mediana(
            l.iter()
                .filter(|c| c.fs >= 0.9 * fs_l)
                .map(|c| c.base)
                .collect(),
        );
        for c in &mut l {
            if c.fs < 0.85 * fs_l && c.base > base_l + 0.15 * fs_l {
                c.volado = true;
            }
        }
        let mut actual: Vec<Car> = Vec::new();
        let mut fin = f32::MIN;
        for c in l {
            if c.c == ' ' {
                if !actual.is_empty() {
                    actual.push(c);
                }
                continue;
            }
            if !actual.is_empty() && c.x0 - fin > 1.4 * fs_l {
                trozos.push(cerrar_trozo(std::mem::take(&mut actual)));
            }
            fin = fin.max(c.x1);
            if actual.is_empty() {
                fin = c.x1;
            }
            actual.push(c);
        }
        if actual.iter().any(|c| c.c != ' ') {
            trozos.push(cerrar_trozo(actual));
        }
    }
    trozos.retain(|t| t.cars.iter().any(|c| !c.c.is_whitespace()));
    if trozos.is_empty() {
        return Vec::new();
    }
    let fs_pagina = mediana(
        trozos
            .iter()
            .flat_map(|t| t.cars.iter().filter(|c| c.c != ' ').map(|c| c.fs))
            .collect(),
    )
    .max(1.0);

    // Folios en el margen lateral (fuera de la caja de texto): fuera ya,
    // antes de que abran una «columna» fantasma.
    {
        let largos: Vec<&Trozo> = trozos
            .iter()
            .filter(|t| t.cars.iter().filter(|c| c.c != ' ').count() >= 15)
            .collect();
        if largos.len() >= 3 {
            let mut x0s: Vec<f32> = largos.iter().map(|t| t.x0).collect();
            let mut x1s: Vec<f32> = largos.iter().map(|t| t.x1).collect();
            x0s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            x1s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let izq = x0s[x0s.len() / 10];
            let der = x1s[x1s.len() * 9 / 10];
            trozos.retain(|t| {
                let fuera = t.x1 < izq - 2.0 || t.x0 > der + 2.0;
                !(fuera && es_folio(&texto_de(&t.cars, t.fs).0))
            });
        }
    }

    // 4. Capitulares: una letra enorme al principio se pega al renglón que
    //    abre a su derecha.
    let mut k = 0;
    while k < trozos.len() {
        let t = &trozos[k];
        let letras: Vec<char> = t.cars.iter().filter(|c| c.c != ' ').map(|c| c.c).collect();
        let es_capitular =
            letras.len() <= 2 && letras.iter().all(|c| c.is_uppercase()) && t.fs >= 1.8 * fs_pagina;
        if es_capitular {
            let (cx0, cx1, cbase, cfs) = (t.x0, t.x1, t.base, t.fs);
            let objetivo = trozos
                .iter()
                .enumerate()
                .filter(|(j, o)| {
                    *j != k
                        && o.x0 >= cx1 - 0.5 * fs_pagina
                        && o.x0 <= cx1 + 2.5 * fs_pagina
                        && o.base > cbase - 0.3 * fs_pagina
                        && o.base <= cbase + cfs * 1.1
                })
                .max_by(|a, b| {
                    a.1.base
                        .partial_cmp(&b.1.base)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(j, _)| j);
            if let Some(j) = objetivo {
                let mut cap = trozos[k].cars.clone();
                for c in &mut cap {
                    c.fs = trozos[j].fs;
                    c.base = trozos[j].base;
                    c.x1 = c.x0; // sin hueco: la letra se pega a la palabra
                }
                let mut nuevos = cap;
                nuevos.append(&mut trozos[j].cars);
                trozos[j].cars = nuevos;
                trozos[j].x0 = cx0;
                trozos.remove(k);
                continue;
            }
        }
        k += 1;
    }

    // 5. Filas (trozos que se solapan en altura) y franjas (filas que dejan
    //    libre la misma calle).
    trozos.sort_by(|a, b| {
        (b.base + 0.75 * b.fs)
            .partial_cmp(&(a.base + 0.75 * a.fs))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut filas: Vec<Vec<Trozo>> = Vec::new();
    let mut suelo = f32::MAX;
    for t in trozos {
        let techo = t.base + 0.75 * t.fs;
        let pie = t.base - 0.2 * t.fs;
        match filas.last_mut() {
            Some(f) if techo >= suelo => {
                f.push(t);
                suelo = suelo.min(pie);
            }
            _ => {
                filas.push(vec![t]);
                suelo = pie;
            }
        }
    }
    let calle = 1.0 * fs_pagina;
    let cobertura = |ts: &[&Trozo]| -> Vec<(f32, f32)> {
        let mut iv: Vec<(f32, f32)> = ts.iter().map(|t| (t.x0, t.x1)).collect();
        fusionar_intervalos(&mut iv, calle);
        iv
    };
    let ancho_texto = {
        let todos: Vec<&Trozo> = filas.iter().flatten().collect();
        let x0 = todos.iter().map(|t| t.x0).fold(f32::MAX, f32::min);
        let x1 = todos.iter().map(|t| t.x1).fold(f32::MIN, f32::max);
        (x0, x1)
    };

    // Franjas: índices de filas.
    let mut franjas: Vec<Franja> = Vec::new();
    for (fi, f) in filas.iter().enumerate() {
        let cov_f = cobertura(&f.iter().collect::<Vec<_>>());
        if let Some((filas_b, cov_b)) = franjas.last_mut() {
            if cov_b.len() >= 2 {
                let mut junto = cov_b.clone();
                junto.extend(cov_f.iter().copied());
                fusionar_intervalos(&mut junto, calle);
                if junto.len() >= 2 {
                    filas_b.push(fi);
                    *cov_b = junto;
                    continue;
                }
            }
        }
        franjas.push((vec![fi], cov_f));
    }
    // Absorción hacia atrás: filas sueltas de justo antes que caben enteras
    // en una columna de la franja (la columna derecha que empieza más arriba
    // que la izquierda, la última línea de un párrafo de la columna).
    let mut fr2: Vec<Franja> = Vec::new();
    for (filas_b, cov_b) in franjas {
        let mut filas_b = filas_b;
        if cov_b.len() >= 2 && filas_b.len() >= 2 {
            let mut n = 0;
            while n < 3 {
                let cabe = match fr2.last() {
                    Some((pf, pc)) if pf.len() == 1 && pc.len() == 1 => {
                        let (p0, p1) = pc[0];
                        cov_b
                            .iter()
                            .any(|(c0, c1)| p0 >= c0 - calle && p1 <= c1 + calle)
                    }
                    _ => false,
                };
                if !cabe {
                    break;
                }
                let (mut pf, _) = fr2.pop().unwrap();
                pf.extend(filas_b);
                filas_b = pf;
                n += 1;
            }
        }
        fr2.push((filas_b, cov_b));
    }

    // 6. Orden de lectura → renglones.
    let mut filas_opt: Vec<Option<Vec<Trozo>>> = filas.into_iter().map(Some).collect();
    let mut salida: Vec<Renglon> = Vec::new();
    for (filas_b, cov_b) in fr2 {
        let trozos_b: Vec<Trozo> = filas_b
            .iter()
            .flat_map(|&fi| filas_opt[fi].take().unwrap_or_default())
            .collect();
        if cov_b.len() >= 2 {
            let anchura = (ancho_texto.1 - ancho_texto.0).max(1.0);
            // Columnas de texto (se leen una tras otra) o tabla (fila a
            // fila). Columnas: todas anchas, o una columna de prosa de
            // verdad (renglones largos) junto a otras estrechas: la barra
            // lateral de una web impresa, una glosa al margen.
            let largos_por_col: Vec<f32> = cov_b
                .iter()
                .map(|(a, b)| {
                    mediana(
                        trozos_b
                            .iter()
                            .filter(|t| {
                                let c = (t.x0 + t.x1) / 2.0;
                                c >= a - calle && c <= b + calle
                            })
                            .map(|t| t.cars.len() as f32)
                            .collect(),
                    )
                })
                .collect();
            let columnas_de_verdad = cov_b.iter().all(|(a, b)| (b - a) >= 0.2 * anchura)
                || (filas_b.len() >= 4
                    && cov_b
                        .iter()
                        .zip(&largos_por_col)
                        .any(|((a, b), l)| (b - a) >= 0.3 * anchura && *l >= 30.0));
            if columnas_de_verdad {
                let mut cols: Vec<Vec<Trozo>> = (0..cov_b.len()).map(|_| Vec::new()).collect();
                for t in trozos_b {
                    let centro = (t.x0 + t.x1) / 2.0;
                    let idx = cov_b
                        .iter()
                        .position(|(a, b)| centro >= a - calle && centro <= b + calle)
                        .unwrap_or(0);
                    cols[idx].push(t);
                }
                for (ci, mut col) in cols.into_iter().enumerate() {
                    col.sort_by(|a, b| {
                        b.base
                            .partial_cmp(&a.base)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    let (izq, der) = cov_b[ci];
                    let mut primero = true;
                    for t in col {
                        let mut r = a_renglon(pagina, &t, izq, der);
                        r.salto = primero;
                        primero = false;
                        salida.push(r);
                    }
                }
            } else {
                // Tabla: fila a fila, celdas separadas por comas.
                let mut ts = trozos_b;
                ts.sort_by(|a, b| {
                    b.base
                        .partial_cmp(&a.base)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                let mut grupos: Vec<Vec<Trozo>> = Vec::new();
                for t in ts {
                    match grupos.last_mut() {
                        Some(g) if (g[0].base - t.base).abs() <= 0.5 * t.fs.max(g[0].fs) => {
                            g.push(t)
                        }
                        _ => grupos.push(vec![t]),
                    }
                }
                for g in &mut grupos {
                    g.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal));
                }
                // Tabla de verdad: al menos dos filas con varias celdas. Una
                // fila suelta con un hueco grande es un renglón justificado
                // muy holgado («1.1   Lives of Girls»): se lee seguido.
                // Y además las celdas se alinean en vertical: en una tabla,
                // las columnas empiezan a la misma x en varias filas; en un
                // texto justificado muy holgado, los huecos caen al azar.
                let es_tabla = {
                    let multiples: Vec<&Vec<Trozo>> =
                        grupos.iter().filter(|g| g.len() > 1).collect();
                    let alineadas = multiples.iter().enumerate().any(|(i, g)| {
                        multiples.iter().skip(i + 1).any(|h| {
                            g.iter()
                                .skip(1)
                                .any(|a| h.iter().skip(1).any(|b| (a.x0 - b.x0).abs() < 3.0))
                        })
                    });
                    multiples.len() >= 2 && alineadas
                };
                let junta = if es_tabla { ", " } else { " " };
                for g in grupos {
                    let celdas: Vec<String> = g.iter().map(|t| texto_de(&t.cars, t.fs).0).collect();
                    let primero = &g[0];
                    let mut r = a_renglon(pagina, primero, ancho_texto.0, ancho_texto.1);
                    r.x1 = g.iter().map(|t| t.x1).fold(f32::MIN, f32::max);
                    r.texto = celdas
                        .into_iter()
                        .filter(|c| !c.trim().is_empty())
                        .collect::<Vec<_>>()
                        .join(junta);
                    r.tabla = es_tabla && g.len() > 1;
                    salida.push(r);
                }
            }
        } else {
            let mut ts = trozos_b;
            ts.sort_by(|a, b| {
                b.base
                    .partial_cmp(&a.base)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal))
            });
            for t in ts {
                salida.push(a_renglon(pagina, &t, ancho_texto.0, ancho_texto.1));
            }
        }
    }
    salida.retain(|r| !r.texto.trim().is_empty());
    salida
}

fn fusionar_intervalos(iv: &mut Vec<(f32, f32)>, hueco: f32) {
    iv.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut out: Vec<(f32, f32)> = Vec::with_capacity(iv.len());
    for &(a, b) in iv.iter() {
        match out.last_mut() {
            Some(u) if a - u.1 < hueco => u.1 = u.1.max(b),
            _ => out.push((a, b)),
        }
    }
    *iv = out;
}

fn cerrar_trozo(cars: Vec<Car>) -> Trozo {
    let utiles: Vec<&Car> = cars.iter().filter(|c| c.c != ' ').collect();
    let fs = mediana(utiles.iter().filter(|c| !c.volado).map(|c| c.fs).collect())
        .max(mediana(utiles.iter().map(|c| c.fs).collect()))
        .max(1.0);
    let base = mediana(
        utiles
            .iter()
            .filter(|c| !c.volado)
            .map(|c| c.base)
            .collect(),
    );
    let base = if base == 0.0 { utiles[0].base } else { base };
    Trozo {
        x0: utiles.iter().map(|c| c.x0).fold(f32::MAX, f32::min),
        x1: utiles.iter().map(|c| c.x1).fold(f32::MIN, f32::max),
        base,
        fs,
        cars,
    }
}

fn a_renglon(pagina: usize, t: &Trozo, izq: f32, der: f32) -> Renglon {
    let utiles = t.cars.iter().filter(|c| c.c != ' ').count().max(1);
    let negritas = t.cars.iter().filter(|c| c.c != ' ' && c.negrita).count();
    let (texto, abre_volado) = texto_de(&t.cars, t.fs);
    Renglon {
        pagina,
        x0: t.x0,
        x1: t.x1,
        base: t.base,
        fs: t.fs,
        texto,
        abre_volado,
        negrita: negritas * 10 >= utiles * 9,
        izq,
        der,
        salto: false,
        tabla: false,
    }
}

/// El texto de un trozo: espacios por huecos reales (adaptados al
/// espaciado del propio trozo), sin glifos dobles de la negrita simulada y
/// sin las llamadas de nota voladas.
fn texto_de(cars: &[Car], fs: f32) -> (String, bool) {
    let mut cs: Vec<&Car> = cars.iter().collect();
    let rtl = cs.iter().filter(|c| es_rtl(c.c)).count() * 2 > cs.len();
    if rtl {
        cs.reverse();
    }
    // Hueco típico entre letras de una misma palabra.
    let mut huecos: Vec<f32> = Vec::new();
    for w in cs.windows(2) {
        if w[0].c != ' ' && w[1].c != ' ' {
            let h = if rtl {
                w[0].x0 - w[1].x1
            } else {
                w[1].x0 - w[0].x1
            };
            if h < 0.5 * fs {
                huecos.push(h);
            }
        }
    }
    let umbral = (0.15 * fs).max(2.0 * mediana(huecos));
    // Una llamada de nota es un volado de cifras o signos: fuera. Los demás
    // volados (la «a» de «1.ª») se conservan.
    let es_llamada = |c: &Car| c.volado && (c.c.is_ascii_digit() || "*†‡§¶".contains(c.c));
    let mut out = String::with_capacity(cs.len());
    let mut prev: Option<&Car> = None;
    let mut espacio = false;
    let mut abre_volado = false;
    for c in cs {
        if c.c.is_whitespace() {
            espacio = true;
            continue;
        }
        if es_llamada(c) {
            if prev.is_none() {
                abre_volado = true;
            }
            continue;
        }
        if let Some(p) = prev {
            if p.c == c.c && (c.x0 - p.x0).abs() < 0.1 * fs {
                continue; // negrita simulada: el mismo glifo dos veces
            }
            let hueco = if rtl { p.x0 - c.x1 } else { c.x0 - p.x1 };
            if (espacio || hueco > umbral) && !out.ends_with(' ') {
                out.push(' ');
            }
        }
        espacio = false;
        out.push(c.c);
        prev = Some(c);
    }
    (out.trim().to_string(), abre_volado)
}

// ───────────────────────────── el documento ─────────────────────────────

fn clave_mobiliario(t: &str) -> String {
    t.chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| if c.is_ascii_digit() { '#' } else { c })
        .flat_map(char::to_lowercase)
        .collect()
}

fn es_folio(t: &str) -> bool {
    use once_cell::sync::Lazy;
    use regex::Regex;
    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(
            r"(?i)^[\s\-–—·|•\[\(]*(?:p(?:age|ág(?:ina)?|agina|\.)|seite|s\.|pag\.?)?\s*(?:\d{1,4}|[ivxlcdm]{1,7})(?:\s*(?:/|of|de|di|von|sur|\|)\s*\d{1,4})?[\s\-–—·|•\]\)]*$",
        )
        .unwrap()
    });
    !t.trim().is_empty() && RE.is_match(t.trim())
}

fn termina_frase(t: &str) -> bool {
    t.trim_end()
        .chars()
        .last()
        .is_some_and(|c| ".!?…:;»\"”’)]".contains(c))
}

fn empieza_minuscula(t: &str) -> bool {
    t.chars()
        .find(|c| c.is_alphanumeric())
        .is_some_and(|c| c.is_lowercase())
}

/// ¿Empieza por la marca de una nota? «1 Texto», «12. Texto», «* Texto».
fn empieza_marca_nota(t: &str) -> bool {
    let t = t.trim_start();
    let cifras: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    if !cifras.is_empty() && cifras.len() <= 3 {
        let resto = &t[cifras.len()..];
        return resto.is_empty()
            || resto.starts_with([' ', '.', ')', '\u{a0}'])
            || resto.chars().next().is_some_and(|c| c.is_alphabetic());
    }
    t.starts_with(['*', '†', '‡', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'])
}

fn ensamblar(mut paginas: Vec<Vec<Renglon>>, alturas: &[(f32, f32)]) -> Vec<RichParagraph> {
    let n_pag = paginas.len();
    // Cuerpo de letra del texto: la moda ponderada por caracteres.
    let mut votos: HashMap<i32, usize> = HashMap::new();
    for r in paginas.iter().flatten() {
        *votos.entry((r.fs * 2.0).round() as i32).or_default() += r.texto.chars().count();
    }
    let cuerpo = votos
        .into_iter()
        .max_by_key(|(_, v)| *v)
        .map(|(k, _)| k as f32 / 2.0)
        .unwrap_or(10.0)
        .max(1.0);

    // ── Mobiliario: cabeceras y pies repetidos, folios.
    let en_zona = |r: &Renglon| -> bool {
        let (y0, y1) = alturas.get(r.pagina).copied().unwrap_or((0.0, 792.0));
        let alto = (y1 - y0).max(1.0);
        r.base >= y1 - 0.13 * alto || r.base <= y0 + 0.13 * alto
    };
    let mut cuenta: HashMap<String, HashSet<usize>> = HashMap::new();
    for pag in &paginas {
        for r in pag.iter().filter(|r| en_zona(r)) {
            cuenta
                .entry(clave_mobiliario(&r.texto))
                .or_default()
                .insert(r.pagina);
        }
    }
    let minimo = 3usize.max((n_pag as f32 * 0.25).ceil() as usize);
    for pag in &mut paginas {
        pag.retain(|r| {
            if !en_zona(r) || r.texto.chars().count() > 160 {
                return true;
            }
            if es_folio(&r.texto) {
                return false;
            }
            let veces = cuenta
                .get(&clave_mobiliario(&r.texto))
                .map_or(0, |s| s.len());
            !(n_pag >= 3 && veces >= minimo)
        });
    }

    // ── Notas al pie: bloque de letra menor al fondo de la página.
    let mut notas: Vec<String> = Vec::new();
    let mut habia_notas = false;
    for pag in &mut paginas {
        let mut marcar = vec![false; pag.len()];
        // Candidatos: letra menor sin renglón de cuerpo por debajo con el que
        // se solape en horizontal.
        let es_pequeno = |r: &Renglon| r.fs <= 0.9 * cuerpo;
        for (i, r) in pag.iter().enumerate() {
            if !es_pequeno(r) {
                continue;
            }
            let cuerpo_debajo = pag.iter().any(|o| {
                !es_pequeno(o)
                    && o.base < r.base - 0.1
                    && o.x0 < r.x1
                    && o.x1 > r.x0
                    && o.texto.chars().count() > 3
            });
            if !cuerpo_debajo {
                marcar[i] = true;
            }
        }
        // El bloque debe empezar por marca de nota (o seguir una nota de la
        // página anterior).
        let primero = (0..pag.len()).filter(|&i| marcar[i]).max_by(|&a, &b| {
            pag[a]
                .base
                .partial_cmp(&pag[b].base)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let Some(primero) = primero else {
            habia_notas = false;
            continue;
        };
        if !(empieza_marca_nota(&pag[primero].texto) || pag[primero].abre_volado || habia_notas) {
            habia_notas = false;
            continue;
        }
        let total_marcados = marcar.iter().filter(|m| **m).count();
        if total_marcados * 10 > pag.len() * 8 && pag.len() > 5 {
            // Casi toda la página es «pequeña»: no es un bloque de notas.
            habia_notas = false;
            continue;
        }
        let mut idx: Vec<usize> = (0..pag.len()).filter(|&i| marcar[i]).collect();
        idx.sort_by(|&a, &b| {
            pag[b]
                .base
                .partial_cmp(&pag[a].base)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(
                    pag[a]
                        .x0
                        .partial_cmp(&pag[b].x0)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
        });
        for &i in &idx {
            let t = pag[i].texto.trim().to_string();
            if empieza_marca_nota(&t) || pag[i].abre_volado || notas.is_empty() {
                notas.push(t);
            } else {
                let u = notas.last_mut().unwrap();
                coser(u, &t);
            }
        }
        habia_notas = true;
        let mut k = 0;
        pag.retain(|_| {
            let quitar = marcar[k];
            k += 1;
            !quitar
        });
    }

    // ── Interlineado típico (distancia entre líneas base del cuerpo).
    let mut saltos: Vec<f32> = Vec::new();
    for pag in &paginas {
        for w in pag.windows(2) {
            let d = w[0].base - w[1].base;
            if !w[1].salto
                && d > 0.0
                && (w[0].fs - cuerpo).abs() < 0.6
                && (w[1].fs - cuerpo).abs() < 0.6
            {
                saltos.push(d);
            }
        }
    }
    let interlinea = {
        let m = mediana(saltos);
        if m <= 0.0 {
            cuerpo * 1.2
        } else {
            m.clamp(cuerpo * 0.9, cuerpo * 2.2)
        }
    };

    // Tamaños de título (más grandes que el cuerpo), del mayor al menor.
    let mut tamanos: Vec<i32> = paginas
        .iter()
        .flatten()
        .filter(|r| r.fs >= cuerpo * 1.18 && r.texto.chars().count() < 200)
        .map(|r| (r.fs * 2.0).round() as i32)
        .collect();
    tamanos.sort_unstable_by(|a, b| b.cmp(a));
    tamanos.dedup();
    let nivel = |fs: f32| -> Option<usize> {
        if fs < cuerpo * 1.18 {
            return None;
        }
        let k = (fs * 2.0).round() as i32;
        tamanos.iter().position(|t| *t == k).map(|p| (p + 1).min(3))
    };

    // ── Párrafos.
    struct Bloque {
        texto: String,
        clase: String,
        fs: f32,
        cerrado: bool,
    }
    let mut bloques: Vec<Bloque> = Vec::new();
    let mut prev: Option<Renglon> = None;
    for r in paginas.into_iter().flatten() {
        let n_cur = nivel(r.fs);
        let romper = match &prev {
            None => true,
            Some(p) => {
                let mismo_flujo = p.pagina == r.pagina && !r.salto;
                let dy = p.base - r.base;
                let nivel_p = nivel(p.fs);
                let fs_cambia = (r.fs - p.fs).abs() > 0.12 * r.fs.max(p.fs);
                // La frase sigue: el renglón anterior no la cerró y este
                // empieza en minúscula. En las capas de OCR el cuerpo de
                // letra y el interlineado bailan de un renglón a otro; esto
                // pesa más que cualquier medida.
                let sigue = empieza_minuscula(&r.texto)
                    && !p
                        .texto
                        .trim_end()
                        .chars()
                        .last()
                        .is_some_and(|c| ".!?…".contains(c));
                if r.tabla || p.tabla {
                    true
                } else if sigue && !(nivel_p.is_some() && n_cur.is_none()) {
                    false
                } else if n_cur.is_some() || nivel_p.is_some() {
                    // Un título continúa en su siguiente línea si es del mismo
                    // tamaño y está cerca.
                    !(n_cur == nivel_p && mismo_flujo && dy > 0.0 && dy < 1.9 * r.fs && !fs_cambia)
                } else if mismo_flujo && (fs_cambia || dy > 1.45 * interlinea || dy < 0.0) {
                    true
                } else if !mismo_flujo {
                    // Columna o página nueva: se cose si la frase quedó abierta.
                    (!empieza_minuscula(&r.texto) && termina_frase(&p.texto))
                        || (r.x0 > r.izq + 0.8 * cuerpo && termina_frase(&p.texto))
                } else {
                    let sangra = r.x0 > p.x0 + 0.8 * cuerpo && r.x0 < p.x0 + 6.0 * cuerpo;
                    let vuelve = r.x0 < p.x0 - 0.8 * cuerpo;
                    let corta = p.x1 < r.der - 3.0 * cuerpo;
                    let fin = termina_frase(&p.texto);
                    if sangra && (fin || corta || p.x0 <= r.izq + 0.5 * cuerpo) {
                        !empieza_minuscula(&r.texto) || fin
                    } else if vuelve {
                        fin
                    } else {
                        corta && fin
                    }
                }
            }
        };
        let clase = match n_cur {
            Some(n) => format!("heading{n}"),
            None => "paragraph".to_string(),
        };
        if romper || bloques.is_empty() {
            if let Some(u) = bloques.last_mut() {
                u.cerrado = true;
            }
            bloques.push(Bloque {
                texto: r.texto.clone(),
                clase,
                fs: r.fs,
                cerrado: false,
            });
        } else {
            let u = bloques.last_mut().unwrap();
            coser(&mut u.texto, &r.texto);
            u.fs = u.fs.max(r.fs);
        }
        prev = Some(r);
    }

    // ── Guiones de corte: decide el vocabulario del propio documento.
    let mut vocab: HashSet<String> = HashSet::new();
    for b in &bloques {
        for w in b.texto.split(|c: char| !(c.is_alphanumeric() || c == '-')) {
            if !w.contains(CORTE) && !w.is_empty() {
                vocab.insert(w.to_lowercase());
            }
        }
    }
    for n in &notas {
        for w in n.split(|c: char| !(c.is_alphanumeric() || c == '-')) {
            if !w.contains(CORTE) && !w.is_empty() {
                vocab.insert(w.to_lowercase());
            }
        }
    }

    let mut salida: Vec<RichParagraph> = Vec::new();
    for b in bloques {
        let texto = pulir(&resolver_cortes(&b.texto, &vocab));
        if texto.is_empty() {
            continue;
        }
        let clase = if b.clase.starts_with("heading") && texto.chars().count() > 200 {
            "paragraph".to_string()
        } else {
            b.clase
        };
        salida.push(rich(texto, &clase));
    }
    if !notas.is_empty() {
        salida.push(rich(String::new(), "hr"));
        for n in notas {
            let t = pulir(&resolver_cortes(&n, &vocab));
            if !t.is_empty() {
                salida.push(rich(t, "paragraph"));
            }
        }
    }
    salida
}

/// Marca provisional de «palabra partida por guion al final de línea».
const CORTE: char = '\u{1F}';

/// Cose el renglón `sig` al texto acumulado.
fn coser(acum: &mut String, sig: &str) {
    let sig = sig.trim_start();
    let fin = acum.trim_end();
    let ult = fin.chars().last();
    let penult = fin.chars().rev().nth(1);
    let partida = matches!(ult, Some('-' | '‐' | '‑' | '\u{ad}' | '¬'))
        && penult.is_some_and(|c| c.is_alphabetic())
        && sig.chars().next().is_some_and(|c| c.is_lowercase());
    if partida {
        let corte = fin.len() - ult.unwrap().len_utf8();
        acum.truncate(corte);
        acum.push(CORTE);
        acum.push_str(sig);
    } else if ult.is_some_and(es_cjk) && sig.chars().next().is_some_and(es_cjk) {
        acum.truncate(fin.len());
        acum.push_str(sig);
    } else {
        acum.truncate(fin.len());
        if !acum.is_empty() {
            acum.push(' ');
        }
        acum.push_str(sig);
    }
}

/// Prefijos que en inglés (y en compuestos de otras lenguas) llevan guion de
/// verdad: «self-\nreflection» es «self-reflection».
const PREFIJOS_CON_GUION: &[&str] = &[
    "self", "non", "well", "ex", "anti", "quasi", "semi", "cross", "multi", "neo", "pseudo", "ill",
    "half", "all", "mid", "post", "pre", "co", "socio", "afro", "euro", "franco", "hispano",
    "luso", "greco", "judeo", "austro", "anglo",
];

fn resolver_cortes(t: &str, vocab: &HashSet<String>) -> String {
    if !t.contains(CORTE) {
        return t.to_string();
    }
    let mut out = String::with_capacity(t.len());
    let partes: Vec<&str> = t.split(CORTE).collect();
    for (i, parte) in partes.iter().enumerate() {
        if i == 0 {
            out.push_str(parte);
            continue;
        }
        let izq: String = out
            .chars()
            .rev()
            .take_while(|c| c.is_alphanumeric())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let der: String = parte.chars().take_while(|c| c.is_alphanumeric()).collect();
        let junta = format!("{izq}{der}").to_lowercase();
        let con_guion = format!("{izq}-{der}").to_lowercase();
        let guion = if vocab.contains(&junta) {
            false
        } else if vocab.contains(&con_guion) {
            true
        } else {
            PREFIJOS_CON_GUION.contains(&izq.to_lowercase().as_str())
                || izq.chars().any(|c| c.is_ascii_digit())
        };
        if guion {
            out.push('-');
        }
        out.push_str(parte);
    }
    out
}

/// Limpieza final de un párrafo: ligaduras, guiones blandos, espacios raros,
/// rellenos de índice y llamadas de nota en cifras voladas Unicode.
pub(crate) fn pulir(t: &str) -> String {
    use once_cell::sync::Lazy;
    use regex::Regex;
    static RELLENO: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?:\s*[.·…_]){4,}\s*(\d+|[ivxlcdm]+)?\s*$").unwrap());
    static RELLENO_MEDIO: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?:\s?[.·…]){4,}\s?").unwrap());
    static VOLADA: Lazy<Regex> = Lazy::new(|| Regex::new(r"([\p{L}\p{P}])[¹²³⁴⁵⁶⁷⁸⁹⁰]+").unwrap());
    static ANTES_DE_SIGNO: Lazy<Regex> = Lazy::new(|| Regex::new(r" +([,.)\]])").unwrap());
    static ESPACIOS: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"[ \t\u{a0}\u{2000}-\u{200a}\u{202f}]{2,}").unwrap());
    let hangul = t
        .chars()
        .any(|c| ('\u{ac00}'..='\u{d7a3}').contains(&c) && c != '\u{c2a0}');
    let mut s = String::with_capacity(t.len());
    for c in t.chars() {
        match c {
            '\u{fb00}' => s.push_str("ff"),
            '\u{fb01}' => s.push_str("fi"),
            '\u{fb02}' => s.push_str("fl"),
            '\u{fb03}' => s.push_str("ffi"),
            '\u{fb04}' => s.push_str("ffl"),
            '\u{fb05}' | '\u{fb06}' => s.push_str("st"),
            '\u{ad}' | '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{feff}' | CORTE => {}
            '\u{a0}' | '\u{2002}'..='\u{200a}' | '\u{202f}' => s.push(' '),
            // Un NBSP en UTF-8 (C2 A0) mal descodificado como UTF-16 sale
            // como la sílaba hangul «슠» en algunas webs impresas a PDF.
            '\u{c2a0}' if !hangul => s.push(' '),
            '\u{2010}' | '\u{2011}' => s.push('-'),
            c if ('\u{e000}'..='\u{f8ff}').contains(&c) => {} // uso privado: glifos sin Unicode
            c => s.push(c),
        }
    }
    let s = RELLENO.replace(&s, "");
    let s = RELLENO_MEDIO.replace_all(&s, " ");
    let s = VOLADA.replace_all(&s, "$1");
    let s = ANTES_DE_SIGNO.replace_all(&s, "$1");
    let s = ESPACIOS.replace_all(&s, " ");
    s.trim().to_string()
}

/// ¿Es útil la capa de texto? Un PDF escaneado con un OCR antiguo y malo
/// trae «texto» que es sopa de signos («EE F+ (r) F EB»): leerlo es peor
/// que no leer. Solo se juzga la escritura latina.
pub fn capa_util(t: &str) -> bool {
    let fichas: Vec<&str> = t.split_whitespace().collect();
    if fichas.len() < 30 {
        return t.trim().chars().count() > 40;
    }
    let latinas = fichas
        .iter()
        .filter(|f| f.chars().any(|c| c.is_ascii_alphabetic()))
        .count();
    if latinas * 2 < fichas.len() {
        return true; // otra escritura: no se juzga
    }
    let vocal = |c: char| "aeiouyáéíóúàèìòùâêîôûäëïöüãõAEIOUYÁÉÍÓÚÀÈÌÒÙÂÊÎÔÛÄËÏÖÜÃÕ".contains(c);
    // Fichas con aspecto de palabra: tres letras o más (sin contar
    // apóstrofos ni guiones), alguna vocal y mayúsculas solo al principio o
    // en toda la palabra. Un texto real pasa del 45 % (las palabras cortas
    // no cuentan); la sopa de signos de un OCR malo no llega al 10 %.
    let plausibles = fichas
        .iter()
        .filter(|f| {
            let p: &str = f.trim_matches(|c: char| !c.is_alphanumeric());
            let nucleo: Vec<char> = p
                .chars()
                .filter(|c| !matches!(c, '\'' | '’' | '-'))
                .collect();
            if nucleo.len() < 3 || !nucleo.iter().all(|c| c.is_alphabetic()) {
                return false;
            }
            let mayus_dentro = nucleo.iter().skip(1).filter(|c| c.is_uppercase()).count();
            nucleo.iter().any(|c| vocal(*c))
                && (mayus_dentro == 0 || mayus_dentro + 1 == nucleo.len())
        })
        .count();
    plausibles * 4 >= fichas.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folios() {
        for f in [
            "12",
            "- 7 -",
            "xiv",
            "Page 3",
            "Página 12",
            "3 / 19",
            "p. 4",
            "[12]",
        ] {
            assert!(es_folio(f), "{f}");
        }
        for f in ["12 monos", "Introducción", "1.2 Métodos"] {
            assert!(!es_folio(f), "{f}");
        }
    }

    #[test]
    fn cortes_de_linea_con_el_vocabulario() {
        let mut v = HashSet::new();
        v.insert("lenguaje".to_string());
        v.insert("well-known".to_string());
        assert_eq!(
            resolver_cortes(&format!("el len{CORTE}guaje"), &v),
            "el lenguaje"
        );
        assert_eq!(
            resolver_cortes(&format!("a well{CORTE}known"), &v),
            "a well-known"
        );
        assert_eq!(
            resolver_cortes(&format!("self{CORTE}reflection"), &v),
            "self-reflection"
        );
        assert_eq!(
            resolver_cortes(&format!("catás{CORTE}trofe"), &v),
            "catástrofe"
        );
    }

    #[test]
    fn coser_une_guiones_y_cjk() {
        let mut a = "la natu-".to_string();
        coser(&mut a, "raleza");
        assert_eq!(
            pulir(&resolver_cortes(&a, &HashSet::new())),
            "la naturaleza"
        );
        let mut b = "日本".to_string();
        coser(&mut b, "語");
        assert_eq!(b, "日本語");
    }

    #[test]
    fn pulir_ligaduras_y_rellenos() {
        assert_eq!(pulir("de\u{fb01}ning the ﬂow"), "defining the flow");
        assert_eq!(pulir("Capítulo 1 .......... 12"), "Capítulo 1");
        assert_eq!(
            pulir("como dijo Bello¹² en 1847"),
            "como dijo Bello en 1847"
        );
    }

    #[test]
    fn capa_basura() {
        let basura = "EE F+ (r) F EB fi ct)o o C{ t4 .Et EB\" EEE{(J oB\\ ra L. Q).l' c H.lrnrn .x' frc(iHiO o\\ \\o HHoIN U) Eu (.) 6($VH io tr o U ET , E g .I -a E E 6 5 € t";
        assert!(!capa_util(basura));
        let buena = "Munro’s stories are characteristically intense and condensed character studies, intimate psychological portraits of women coming to terms with their complicated life histories, significantly embedded in the dynamic clash between individualism and society.";
        assert!(capa_util(buena));
    }
}
