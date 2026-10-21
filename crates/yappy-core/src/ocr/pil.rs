//! Las operaciones de imagen de Pillow que usa Amanuensis, reproducidas al bit en escala de grises de 8 bits:
//! remuestreo con filtro (`Image.resize` con BILINEAR o LANCZOS: `libImaging/Resample.c`, coeficientes en coma fija
//! de 22 bits, pasada horizontal y luego vertical), giro afín con BILINEAR o BICUBIC (`libImaging/Geometry.c`),
//! recorte con relleno negro fuera de la imagen (`Image.crop`), cuartos de vuelta exactos y la conversión RGB -> L.
//! La red se entrenó con lo que da Pillow; cualquier otro remuestreo cambia la lectura.

// Los bucles por índice calcan los de `libImaging` para poder cotejarlos línea a línea.
#![allow(clippy::needless_range_loop)]

/// Imagen gris de 8 bits, fila a fila.
#[derive(Clone, Debug, PartialEq)]
pub struct Gris {
    pub ancho: usize,
    pub alto: usize,
    pub px: Vec<u8>,
}

impl Gris {
    pub fn nueva(ancho: usize, alto: usize, fondo: u8) -> Self {
        Self {
            ancho,
            alto,
            px: vec![fondo; ancho * alto],
        }
    }

    pub fn desde(ancho: usize, alto: usize, px: Vec<u8>) -> Self {
        assert_eq!(px.len(), ancho * alto);
        Self { ancho, alto, px }
    }

    /// RGB -> L con la fórmula de Pillow (`L24`: 19595, 38470, 7471 sobre 2^16, redondeada).
    pub fn desde_rgb(ancho: usize, alto: usize, rgb: &[u8]) -> Self {
        let px = rgb
            .chunks_exact(3)
            .map(|c| {
                ((c[0] as u32 * 19595 + c[1] as u32 * 38470 + c[2] as u32 * 7471 + 0x8000) >> 16)
                    as u8
            })
            .collect();
        Self::desde(ancho, alto, px)
    }

    #[inline]
    pub fn at(&self, x: usize, y: usize) -> u8 {
        self.px[y * self.ancho + x]
    }

    /// `Image.crop((x0, y0, x1, y1))`: lo que cae fuera de la imagen sale negro (0), como en Pillow.
    pub fn recortar(&self, x0: i64, y0: i64, x1: i64, y1: i64) -> Gris {
        let w = (x1 - x0).max(0) as usize;
        let h = (y1 - y0).max(0) as usize;
        let mut out = Gris::nueva(w, h, 0);
        for y in 0..h {
            let sy = y0 + y as i64;
            if sy < 0 || sy >= self.alto as i64 {
                continue;
            }
            for x in 0..w {
                let sx = x0 + x as i64;
                if sx >= 0 && sx < self.ancho as i64 {
                    out.px[y * w + x] = self.at(sx as usize, sy as usize);
                }
            }
        }
        out
    }

    /// `Image.paste(otra, (0, 0))` sobre un lienzo (la parte que cabe).
    pub fn pegar(&mut self, otra: &Gris) {
        let w = otra.ancho.min(self.ancho);
        for y in 0..otra.alto.min(self.alto) {
            self.px[y * self.ancho..y * self.ancho + w]
                .copy_from_slice(&otra.px[y * otra.ancho..y * otra.ancho + w]);
        }
    }

    /// Cuarto de vuelta exacto en sentido antihorario (`Image.rotate(90, expand=True)` = `ROTATE_90`).
    pub fn rotar90(&self) -> Gris {
        let (w, h) = (self.ancho, self.alto);
        let mut out = Gris::nueva(h, w, 0);
        for y in 0..h {
            for x in 0..w {
                // ROTATE_90: (x, y) -> (y, w - 1 - x)
                out.px[(w - 1 - x) * h + y] = self.at(x, y);
            }
        }
        out
    }

    pub fn rotar180(&self) -> Gris {
        let mut px = self.px.clone();
        px.reverse();
        Gris::desde(self.ancho, self.alto, px)
    }

    pub fn rotar270(&self) -> Gris {
        let (w, h) = (self.ancho, self.alto);
        let mut out = Gris::nueva(h, w, 0);
        for y in 0..h {
            for x in 0..w {
                // ROTATE_270: (x, y) -> (h - 1 - y, x)
                out.px[x * h + (h - 1 - y)] = self.at(x, y);
            }
        }
        out
    }

    /// `Image.rotate(grados, expand=True)` para múltiplos de 90.
    pub fn rotar_cuartos(&self, grados: i32) -> Gris {
        match grados.rem_euclid(360) {
            90 => self.rotar90(),
            180 => self.rotar180(),
            270 => self.rotar270(),
            _ => self.clone(),
        }
    }

    pub fn media(&self) -> f64 {
        if self.px.is_empty() {
            return f64::NAN;
        }
        self.px.iter().map(|&v| v as f64).sum::<f64>() / self.px.len() as f64
    }
}

// ---------------------------------------------------------------- remuestreo (Resample.c)

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filtro {
    Bilineal,
    Lanczos,
}

impl Filtro {
    fn soporte(self) -> f64 {
        match self {
            Filtro::Bilineal => 1.0,
            Filtro::Lanczos => 3.0,
        }
    }
    fn valor(self, x: f64) -> f64 {
        match self {
            Filtro::Bilineal => {
                let x = x.abs();
                if x < 1.0 {
                    1.0 - x
                } else {
                    0.0
                }
            }
            Filtro::Lanczos => {
                if (-3.0..3.0).contains(&x) {
                    sinc(x) * sinc(x / 3.0)
                } else {
                    0.0
                }
            }
        }
    }
}

fn sinc(x: f64) -> f64 {
    if x == 0.0 {
        return 1.0;
    }
    let x = x * std::f64::consts::PI;
    x.sin() / x
}

const PRECISION_BITS: u32 = 32 - 8 - 2;

/// `precompute_coeffs` + `normalize_coeffs_8bpc`: -> (ksize, límites [xmin, n] por salida, coeficientes enteros).
fn coeficientes(
    tam_in: usize,
    in0: f32,
    in1: f32,
    tam_out: usize,
    f: Filtro,
) -> (usize, Vec<(usize, usize)>, Vec<i32>) {
    let scale = (in1 as f64 - in0 as f64) / tam_out as f64;
    let filterscale = if scale < 1.0 { 1.0 } else { scale };
    let support = f.soporte() * filterscale;
    let ksize = support.ceil() as usize * 2 + 1;
    let mut limites = Vec::with_capacity(tam_out);
    let mut kk = vec![0i32; tam_out * ksize];
    let mut k = vec![0f64; ksize];
    for xx in 0..tam_out {
        let center = in0 as f64 + (xx as f64 + 0.5) * scale;
        let ss = 1.0 / filterscale;
        let mut xmin = (center - support + 0.5) as i64; // (int) trunca hacia cero, como en C
        if xmin < 0 {
            xmin = 0;
        }
        let mut xmax = (center + support + 0.5) as i64;
        if xmax > tam_in as i64 {
            xmax = tam_in as i64;
        }
        let xmax = (xmax - xmin).max(0) as usize;
        let xmin = xmin as usize;
        let mut ww = 0.0;
        for x in 0..xmax {
            let w = f.valor((x as f64 + xmin as f64 - center + 0.5) * ss);
            k[x] = w;
            ww += w;
        }
        for (x, kv) in k.iter().enumerate().take(xmax) {
            let v = if ww != 0.0 { kv / ww } else { *kv };
            let e = v * (1u32 << PRECISION_BITS) as f64;
            kk[xx * ksize + x] = if v < 0.0 {
                (-0.5 + e) as i32
            } else {
                (0.5 + e) as i32
            };
        }
        limites.push((xmin, xmax));
    }
    (ksize, limites, kk)
}

#[inline]
fn clip8(v: i64) -> u8 {
    if v >= (1i64 << PRECISION_BITS << 8) {
        255
    } else if v <= 0 {
        0
    } else {
        (v >> PRECISION_BITS) as u8
    }
}

impl Gris {
    /// `Image.resize((ancho, alto), filtro)` de Pillow para el modo L.
    pub fn redimensionar(&self, ancho: usize, alto: usize, f: Filtro) -> Gris {
        if ancho == self.ancho && alto == self.alto {
            return self.clone();
        }
        let need_h = ancho != self.ancho;
        let need_v = alto != self.alto;
        let (kh, lim_h, kk_h) = coeficientes(self.ancho, 0.0, self.ancho as f32, ancho, f);
        let (kv, mut lim_v, kk_v) = coeficientes(self.alto, 0.0, self.alto as f32, alto, f);
        let y_primera = lim_v[0].0;
        let y_ultima = lim_v[alto - 1].0 + lim_v[alto - 1].1;
        let mut actual: std::borrow::Cow<Gris> = std::borrow::Cow::Borrowed(self);
        if need_h {
            for l in lim_v.iter_mut() {
                l.0 -= y_primera;
            }
            let filas = y_ultima - y_primera;
            let mut tmp = Gris::nueva(ancho, filas, 0);
            for yy in 0..filas {
                let fila =
                    &self.px[(yy + y_primera) * self.ancho..(yy + y_primera + 1) * self.ancho];
                for xx in 0..ancho {
                    let (xmin, n) = lim_h[xx];
                    let k = &kk_h[xx * kh..xx * kh + n];
                    let mut ss: i64 = 1 << (PRECISION_BITS - 1);
                    for x in 0..n {
                        ss += fila[x + xmin] as i64 * k[x] as i64;
                    }
                    tmp.px[yy * ancho + xx] = clip8(ss);
                }
            }
            actual = std::borrow::Cow::Owned(tmp);
        }
        if need_v {
            let src = actual.as_ref();
            let mut out = Gris::nueva(src.ancho, alto, 0);
            for yy in 0..alto {
                let (ymin, n) = lim_v[yy];
                let k = &kk_v[yy * kv..yy * kv + n];
                for xx in 0..src.ancho {
                    let mut ss: i64 = 1 << (PRECISION_BITS - 1);
                    for y in 0..n {
                        ss += src.px[(y + ymin) * src.ancho + xx] as i64 * k[y] as i64;
                    }
                    out.px[yy * src.ancho + xx] = clip8(ss);
                }
            }
            return out;
        }
        actual.into_owned()
    }
}

// ---------------------------------------------------------------- giro afín (Geometry.c)

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FiltroGiro {
    Bilineal,
    Bicubico,
}

/// `round(x, 15)` de Python (redondeo decimal correcto).
fn redondear15(x: f64) -> f64 {
    format!("{x:.15}").parse().unwrap_or(x)
}

impl Gris {
    /// `Image.rotate(grados, resample, expand=False, fillcolor=relleno)` (relleno `None` = 0 fuera de la imagen).
    pub fn rotar(&self, grados: f64, filtro: FiltroGiro, relleno: Option<u8>) -> Gris {
        let angulo = grados.rem_euclid(360.0);
        let angulo = if angulo == 360.0 { 0.0 } else { angulo };
        if angulo == 0.0 {
            return self.clone();
        }
        if angulo == 180.0 {
            return self.rotar180();
        }
        if (angulo == 90.0 || angulo == 270.0) && self.ancho == self.alto {
            return self.rotar_cuartos(angulo as i32);
        }
        let (w, h) = (self.ancho as f64, self.alto as f64);
        let centro = (w / 2.0, h / 2.0);
        let a = -(angulo * (std::f64::consts::PI / 180.0));
        let mut m = [
            redondear15(a.cos()),
            redondear15(a.sin()),
            0.0,
            redondear15(-a.sin()),
            redondear15(a.cos()),
            0.0,
        ];
        let tr =
            |x: f64, y: f64, m: &[f64; 6]| (m[0] * x + m[1] * y + m[2], m[3] * x + m[4] * y + m[5]);
        let (c2, c5) = tr(-centro.0, -centro.1, &m);
        m[2] = c2 + centro.0;
        m[5] = c5 + centro.1;
        let mut out = Gris::nueva(self.ancho, self.alto, relleno.unwrap_or(0));
        for y in 0..self.alto {
            for x in 0..self.ancho {
                let xin = x as f64 + 0.5;
                let yin = y as f64 + 0.5;
                let xo = m[0] * xin + m[1] * yin + m[2];
                let yo = m[3] * xin + m[4] * yin + m[5];
                let v = match filtro {
                    FiltroGiro::Bilineal => self.bilineal(xo, yo),
                    FiltroGiro::Bicubico => self.bicubico(xo, yo),
                };
                if let Some(v) = v {
                    out.px[y * self.ancho + x] = v;
                }
            }
        }
        out
    }

    #[inline]
    fn xclip(&self, x: i64) -> usize {
        x.clamp(0, self.ancho as i64 - 1) as usize
    }

    #[inline]
    fn yclip(&self, y: i64) -> usize {
        y.clamp(0, self.alto as i64 - 1) as usize
    }

    fn bilineal(&self, xin: f64, yin: f64) -> Option<u8> {
        if xin < 0.0 || xin >= self.ancho as f64 || yin < 0.0 || yin >= self.alto as f64 {
            return None;
        }
        let (xin, yin) = (xin - 0.5, yin - 0.5);
        let x = suelo(xin);
        let y = suelo(yin);
        let dx = xin - x as f64;
        let dy = yin - y as f64;
        let x0 = self.xclip(x);
        let x1 = self.xclip(x + 1);
        let fila = self.yclip(y);
        let a = self.at(x0, fila) as f64;
        let b = self.at(x1, fila) as f64;
        let v1 = a + (b - a) * dx;
        let v2 = if y + 1 >= 0 && y + 1 < self.alto as i64 {
            let f = (y + 1) as usize;
            let a = self.at(x0, f) as f64;
            let b = self.at(x1, f) as f64;
            a + (b - a) * dx
        } else {
            v1
        };
        let v = v1 + (v2 - v1) * dy;
        Some(v as u8)
    }

    fn bicubico(&self, xin: f64, yin: f64) -> Option<u8> {
        if xin < 0.0 || xin >= self.ancho as f64 || yin < 0.0 || yin >= self.alto as f64 {
            return None;
        }
        let (xin, yin) = (xin - 0.5, yin - 0.5);
        let mut x = suelo(xin);
        let mut y = suelo(yin);
        let dx = xin - x as f64;
        let dy = yin - y as f64;
        x -= 1;
        y -= 1;
        let xs = [
            self.xclip(x),
            self.xclip(x + 1),
            self.xclip(x + 2),
            self.xclip(x + 3),
        ];
        let fila = |f: usize| -> f64 {
            cubica(
                self.at(xs[0], f) as f64,
                self.at(xs[1], f) as f64,
                self.at(xs[2], f) as f64,
                self.at(xs[3], f) as f64,
                dx,
            )
        };
        let v1 = fila(self.yclip(y));
        let dentro = |yy: i64| yy >= 0 && yy < self.alto as i64;
        let v2 = if dentro(y + 1) {
            fila((y + 1) as usize)
        } else {
            v1
        };
        let v3 = if dentro(y + 2) {
            fila((y + 2) as usize)
        } else {
            v2
        };
        let v4 = if dentro(y + 3) {
            fila((y + 3) as usize)
        } else {
            v3
        };
        let v = cubica(v1, v2, v3, v4, dy);
        Some(if v <= 0.0 {
            0
        } else if v >= 255.0 {
            255
        } else {
            v as u8
        })
    }
}

#[inline]
fn suelo(v: f64) -> i64 {
    if v < 0.0 {
        v.floor() as i64
    } else {
        v as i64
    }
}

/// Núcleo cúbico de Pillow (a = -0,5).
#[inline]
fn cubica(v1: f64, v2: f64, v3: f64, v4: f64, d: f64) -> f64 {
    let p1 = v2;
    let p2 = -v1 + v3;
    let p3 = 2.0 * (v1 - v2) + v3 - v4;
    let p4 = -v1 + v2 - v3 + v4;
    p1 + d * (p2 + d * (p3 + d * p4))
}
