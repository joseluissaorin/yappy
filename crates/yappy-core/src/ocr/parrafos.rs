//! Párrafos de una página leída (`amanuensis/salidas.py`: `_partir_parrafos` y `unir`): corte por hueco vertical,
//! cambio de cuerpo, sangría tras fin de frase y vuelta atrás (otra columna); el texto corrido quita los guiones
//! de corte de línea.

use super::Linea;

/// `_partir_parrafos(p, idx)` sobre las líneas con texto, en orden de lectura -> grupos de índices.
pub fn partir(lineas: &[&Linea]) -> Vec<Vec<usize>> {
    if lineas.is_empty() {
        return Vec::new();
    }
    let mut hs: Vec<f64> = lineas.iter().map(|l| l.caja[3] - l.caja[1]).collect();
    hs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let hmed = hs[hs.len() / 2];
    let mut xs: Vec<f64> = lineas.iter().map(|l| l.caja[0]).collect();
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let izq = xs[lineas.len() / 4];
    let mut grupos = Vec::new();
    let mut act = vec![0usize];
    for k in 0..lineas.len() - 1 {
        let (a, b) = (lineas[k], lineas[k + 1]);
        let hueco = b.caja[1] - a.caja[3];
        let (ha, hb) = (a.caja[3] - a.caja[1], b.caja[3] - b.caja[1]);
        let fin_frase = a
            .texto
            .trim_end()
            .chars()
            .last()
            .map(|c| ".:!?»\"".contains(c))
            .unwrap_or(true);
        let nuevo = (ha - hb).abs() > 0.25 * ha.max(hb)
            || hueco > 0.8 * hmed
            || b.caja[1] < a.caja[1] - hmed
            || (b.caja[0] - izq > 1.5 * hmed && fin_frase);
        if nuevo {
            grupos.push(std::mem::take(&mut act));
        }
        act.push(k + 1);
    }
    grupos.push(act);
    grupos
}

/// `unir(lineas)`: texto corrido; quita el guion de corte («-», «¬», «⸗») entre letras.
pub fn unir<'a>(textos: impl IntoIterator<Item = &'a str>) -> String {
    let mut act = String::new();
    for t in textos {
        let t = t.trim();
        if t.is_empty() {
            continue;
        }
        let cs: Vec<char> = act.chars().collect();
        let n = cs.len();
        if n > 1 && "-¬⸗".contains(cs[n - 1]) && cs[n - 2].is_alphabetic() {
            act = cs[..n - 1].iter().collect::<String>() + t;
        } else if act.is_empty() {
            act = t.to_string();
        } else {
            act.push(' ');
            act.push_str(t);
        }
    }
    act
}
