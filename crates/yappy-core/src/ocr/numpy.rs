//! Las pocas reducciones de numpy cuyo redondeo importa para reproducir Amanuensis: la suma por parejas de float32
//! (`np.add.reduce` sobre el eje contiguo), la mediana y el percentil lineal en float32.

/// Suma por parejas de numpy (`pairwise_sum`, bloques de 8 acumuladores y corte a 128).
pub fn suma_pares_f32(a: &[f32]) -> f32 {
    let n = a.len();
    if n < 8 {
        let mut res = 0.0f32;
        for &v in a {
            res += v;
        }
        res
    } else if n <= 128 {
        let mut r = [a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]];
        let mut i = 8;
        while i < n - (n % 8) {
            for k in 0..8 {
                r[k] += a[i + k];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        while i < n {
            res += a[i];
            i += 1;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        suma_pares_f32(&a[..n2]) + suma_pares_f32(&a[n2..])
    }
}

/// `np.median` de un vector float32 (la media de los dos centrales en float32).
pub fn mediana_f32(v: &[f32]) -> f32 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = s.len();
    if n == 0 {
        return f32::NAN;
    }
    if n % 2 == 1 {
        s[n / 2]
    } else {
        (s[n / 2 - 1] + s[n / 2]) / 2.0
    }
}

/// `np.median` de un vector float64.
pub fn mediana_f64(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = s.len();
    if n == 0 {
        return f64::NAN;
    }
    if n % 2 == 1 {
        s[n / 2]
    } else {
        (s[n / 2 - 1] + s[n / 2]) / 2.0
    }
}

/// `np.percentile(v, q)` (método lineal) sobre un vector float32 YA ORDENADO, con la interpolación de numpy
/// (`_lerp`, en float32, que resta desde el extremo superior cuando la fracción es >= 0,5).
pub fn percentil_ordenado_f32(s: &[f32], q: f64) -> f32 {
    let n = s.len();
    if n == 0 {
        return f32::NAN;
    }
    let virtual_ = (q / 100.0) * (n as f64 - 1.0);
    let prev = virtual_.floor();
    let gamma = (virtual_ - prev) as f32;
    let i0 = (prev as usize).min(n - 1);
    let i1 = (i0 + 1).min(n - 1);
    let (a, b) = (s[i0], s[i1]);
    let d = b - a;
    if gamma >= 0.5 {
        b - d * (1.0 - gamma)
    } else {
        a + d * gamma
    }
}
