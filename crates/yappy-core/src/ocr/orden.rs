//! Orden de lectura ligero de Amanuensis (`amanuensis/orden.py`): perceptrón de 3 capas sobre 28 rasgos geométricos
//! por pareja de líneas, con contexto de página, y decodificación por inserciones. Pesos empaquetados
//! (`orden_ligero.json`, convertido del `orden_ligero.npz` original; Apache 2.0).

use once_cell::sync::Lazy;

pub const N_RASGOS: usize = 28;

pub struct ModeloOrden {
    media: Vec<f32>,
    desv: Vec<f32>,
    /// (pesos [entrada x salida] por filas, sesgos)
    capas: Vec<(Vec<f32>, usize, usize, Vec<f32>)>,
}

static MODELO: Lazy<ModeloOrden> = Lazy::new(|| {
    let v: serde_json::Value =
        serde_json::from_str(include_str!("orden_ligero.json")).expect("orden_ligero.json");
    let plano = |k: &str| -> Vec<f32> {
        fn aplanar(v: &serde_json::Value, out: &mut Vec<f32>) {
            match v {
                serde_json::Value::Array(a) => a.iter().for_each(|x| aplanar(x, out)),
                serde_json::Value::Number(n) => out.push(n.as_f64().unwrap_or(0.0) as f32),
                _ => {}
            }
        }
        let mut out = Vec::new();
        aplanar(&v[k], &mut out);
        out
    };
    let forma = |k: &str| -> Vec<usize> {
        v["_forma"][k]
            .as_array()
            .map(|a| a.iter().map(|x| x.as_u64().unwrap_or(0) as usize).collect())
            .unwrap_or_default()
    };
    let n = v["capas"].as_u64().unwrap_or(3) as usize;
    let capas = (0..n)
        .map(|k| {
            let f = forma(&format!("w{k}"));
            (plano(&format!("w{k}")), f[0], f[1], plano(&format!("b{k}")))
        })
        .collect();
    ModeloOrden {
        media: plano("media"),
        desv: plano("desv"),
        capas,
    }
});

pub fn modelo() -> &'static ModeloOrden {
    &MODELO
}

/// Rasgos [n, n, 28] (de i antes que j), en float32 como numpy.
fn rasgos(cajas: &[[f64; 4]], tam: (f64, f64)) -> Vec<f32> {
    let n = cajas.len();
    let b: Vec<[f32; 4]> = cajas
        .iter()
        .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32])
        .collect();
    let (wp, hp) = (tam.0 as f32, tam.1 as f32);
    let x0: Vec<f32> = b.iter().map(|c| c[0]).collect();
    let y0: Vec<f32> = b.iter().map(|c| c[1]).collect();
    let x1: Vec<f32> = b.iter().map(|c| c[2]).collect();
    let y1: Vec<f32> = b.iter().map(|c| c[3]).collect();
    let w: Vec<f32> = (0..n).map(|i| (x1[i] - x0[i]).max(1.0)).collect();
    let h: Vec<f32> = (0..n).map(|i| (y1[i] - y0[i]).max(1.0)).collect();
    let mh = if n > 0 {
        super::numpy::mediana_f32(&h)
    } else {
        1.0
    };
    let cx: Vec<f32> = (0..n).map(|i| (x0[i] + x1[i]) / 2.0).collect();
    let cy: Vec<f32> = (0..n).map(|i| (y0[i] + y1[i]) / 2.0).collect();
    let idx = |i: usize, j: usize| i * n + j;
    let mut hov = vec![0f32; n * n];
    let mut vov = vec![0f32; n * n];
    for i in 0..n {
        for j in 0..n {
            hov[idx(i, j)] = (x1[i].min(x1[j]) - x0[i].max(x0[j])).max(0.0) / w[i].min(w[j]);
            vov[idx(i, j)] = (y1[i].min(y1[j]) - y0[i].max(y0[j])).max(0.0) / h[i].min(h[j]);
        }
    }
    let fila: Vec<f32> = (0..n)
        .map(|i| ((0..n).filter(|&j| vov[idx(i, j)] > 0.5).count() as f32) - 1.0)
        .collect();
    let izq: Vec<f32> = (0..n)
        .map(|i| {
            (0..n)
                .filter(|&j| vov[idx(i, j)] > 0.5 && cx[j] < cx[i])
                .count() as f32
        })
        .collect();
    let mut ent_ij = vec![0f32; n * n];
    let mut sep = vec![0f32; n * n];
    for i in 0..n {
        for j in 0..n {
            let lo = cy[i].min(cy[j]) + 1e-3;
            let hi = cy[i].max(cy[j]) - 1e-3;
            let mxo = x0[i].min(x0[j]) + mh;
            let mxf = x1[i].max(x1[j]) - mh;
            let (mut ci, mut cj, mut s) = (0u32, 0u32, false);
            for k in 0..n {
                let ent = cy[k] > lo && cy[k] < hi;
                if !ent {
                    continue;
                }
                if hov[idx(i, k)] > 0.3 {
                    ci += 1;
                }
                // hov.T[None, :, :][.., k, j] = hov[j, k]
                if hov[idx(j, k)] > 0.3 {
                    cj += 1;
                }
                if x0[k] <= mxo && x1[k] >= mxf {
                    s = true;
                }
            }
            ent_ij[idx(i, j)] = (ci + cj) as f32;
            sep[idx(i, j)] = if s { 1.0 } else { 0.0 };
        }
    }
    let rango: Vec<f32> = (0..n)
        .map(|i| {
            (0..n)
                .filter(|&k| hov[idx(i, k)] > 0.3 && cy[k] < cy[i])
                .count() as f32
        })
        .collect();
    let uno: Vec<[f32; 10]> = (0..n)
        .map(|i| {
            [
                x0[i] / wp,
                y0[i] / hp,
                x1[i] / wp,
                y1[i] / hp,
                w[i] / wp,
                (h[i] / mh).ln(),
                fila[i] / 4.0,
                izq[i] / 4.0,
                rango[i] / 20.0,
                (mh as f64 / tam.1 * 20.0) as f32,
            ]
        })
        .collect();
    let mut f = vec![0f32; n * n * N_RASGOS];
    for i in 0..n {
        for j in 0..n {
            let o = idx(i, j) * N_RASGOS;
            f[o..o + 10].copy_from_slice(&uno[i]);
            f[o + 10..o + 20].copy_from_slice(&uno[j]);
            f[o + 20] = (cx[j] - cx[i]) / wp;
            f[o + 21] = (cy[j] - cy[i]) / hp;
            f[o + 22] = ((y0[j] - y1[i]) / mh).clamp(-20.0, 20.0) / 10.0;
            f[o + 23] = ((x0[j] - x1[i]) / mh).clamp(-40.0, 40.0) / 20.0;
            f[o + 24] = vov[idx(i, j)];
            f[o + 25] = hov[idx(i, j)];
            f[o + 26] = ent_ij[idx(i, j)] / 20.0;
            f[o + 27] = sep[idx(i, j)];
        }
    }
    f
}

fn puntuar(m: &ModeloOrden, f: &[f32]) -> Vec<f32> {
    let filas = f.len() / N_RASGOS;
    let mut x: Vec<f32> = f
        .iter()
        .enumerate()
        .map(|(k, v)| (v - m.media[k % N_RASGOS]) / m.desv[k % N_RASGOS])
        .collect();
    let mut ancho = N_RASGOS;
    let nc = m.capas.len();
    for (c, (w, ent, sal, b)) in m.capas.iter().enumerate() {
        debug_assert_eq!(*ent, ancho);
        let mut y = vec![0f32; filas * sal];
        for r in 0..filas {
            let xr = &x[r * ancho..(r + 1) * ancho];
            for s in 0..*sal {
                let mut acc = 0f32;
                for e in 0..*ent {
                    acc += xr[e] * w[e * sal + s];
                }
                let v = acc + b[s];
                y[r * sal + s] = if c + 1 < nc { v.max(0.0) } else { v };
            }
        }
        x = y;
        ancho = *sal;
    }
    x
}

/// log(1 + e^x) estable (np.logaddexp(0, x)).
fn logaddexp0(x: f32) -> f32 {
    let x = x as f64;
    (if x > 0.0 {
        x + (-x).exp().ln_1p()
    } else {
        x.exp().ln_1p()
    }) as f32
}

fn decodificar(a: &[f32], n: usize, barridos: usize) -> Vec<usize> {
    if n <= 1 {
        return (0..n).collect();
    }
    let lp: Vec<f32> = a.iter().map(|&v| -logaddexp0(-v)).collect();
    let fila = |i: usize| -> f32 { super::numpy::suma_pares_f32(&lp[i * n..(i + 1) * n]) };
    let col = |j: usize| -> f32 {
        let mut s = 0f32;
        for i in 0..n {
            s += lp[i * n + j];
        }
        s
    };
    let punt: Vec<f32> = (0..n).map(|i| fila(i) - col(i)).collect();
    let mut orden: Vec<usize> = (0..n).collect();
    orden.sort_by(|&p, &q| {
        punt[q]
            .partial_cmp(&punt[p])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for _ in 0..barridos {
        let mut mejora = false;
        let copia = orden.clone();
        for &x in &copia {
            let resto: Vec<usize> = orden.iter().copied().filter(|&y| y != x).collect();
            let m = resto.len();
            let antes: Vec<f64> = resto.iter().map(|&r| lp[r * n + x] as f64).collect();
            let despues: Vec<f64> = resto.iter().map(|&r| lp[x * n + r] as f64).collect();
            let mut pref = vec![0f64; m + 1];
            for q in 0..m {
                pref[q + 1] = (pref[q] as f32 + antes[q] as f32) as f64; // cumsum en float32
            }
            let mut suf = vec![0f64; m + 1];
            let mut acc = 0f32;
            for q in (0..m).rev() {
                acc += despues[q] as f32;
                suf[q] = acc as f64;
            }
            let mut p = 0;
            let mut mejor = f64::NEG_INFINITY;
            for k in 0..=m {
                let v = pref[k] + suf[k];
                if v > mejor {
                    mejor = v;
                    p = k;
                }
            }
            let mut nuevo = resto[..p].to_vec();
            nuevo.push(x);
            nuevo.extend_from_slice(&resto[p..]);
            if nuevo != orden {
                orden = nuevo;
                mejora = true;
            }
        }
        if !mejora {
            break;
        }
    }
    orden
}

/// `orden.ordenar(m, cajas, tam)`.
pub fn ordenar(cajas: &[[f64; 4]], tam: (f64, f64)) -> Vec<usize> {
    let n = cajas.len();
    if n <= 1 {
        return (0..n).collect();
    }
    let m = modelo();
    let f = puntuar(m, &rasgos(cajas, tam));
    let a: Vec<f32> = (0..n * n).map(|k| f[k] - f[(k % n) * n + k / n]).collect();
    decodificar(&a, n, 4)
}
