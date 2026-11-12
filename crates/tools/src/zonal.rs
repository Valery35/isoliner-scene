// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Статистика атрибута по объектам внутри тела (2.14). Перенос
//! `zonal.py` модуля.
//!
//! Вес объекта: у точек и полигонов единица, у линий длина их части
//! внутри тела. Разброс - стандартное отклонение со взвешиванием, без
//! поправки на число проб. Квантиль взвешенный, линейно между
//! серединами долей веса; при равных весах это квантиль по Хазену.

/// Ключ показателя и суффиксы его полей, в порядке `STATS` модуля.
pub const STATS: [(&str, &[&str]); 8] = [
    ("sum", &["_sum"]),
    ("mean", &["_mean"]),
    ("min", &["_min"]),
    ("max", &["_max"]),
    ("std", &["_std"]),
    ("med", &["_med"]),
    ("quart", &["_p25", "_p75"]),
    ("decile", &["_p10", "_p90"]),
];

/// Ключ показателя по номеру пункта списка.
pub fn key(i: usize) -> Option<&'static str> {
    STATS.get(i).map(|(k, _)| *k)
}

/// Суффиксы полей выбранных показателей, в порядке `STATS`.
pub fn suffixes(keys: &[&str]) -> Vec<&'static str> {
    STATS
        .iter()
        .filter(|(k, _)| keys.contains(k))
        .flat_map(|(_, s)| s.iter().copied())
        .collect()
}

/// Взвешенный квантиль `q` (0..1) или None без значений.
pub fn wquantile(values: &[f64], weights: &[f64], q: f64) -> Option<f64> {
    let mut vw: Vec<(f64, f64)> = values
        .iter()
        .zip(weights)
        .filter(|(v, w)| v.is_finite() && w.is_finite() && **w > 0.0)
        .map(|(v, w)| (*v, *w))
        .collect();
    match vw.len() {
        0 => return None,
        1 => return Some(vw[0].0),
        _ => {}
    }
    // устойчивая сортировка по значению, как argsort(kind="stable")
    vw.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = vw.iter().map(|p| p.1).sum();
    let mut acc = 0.0;
    let mid: Vec<f64> = vw
        .iter()
        .map(|(_, w)| {
            acc += w;
            (acc - 0.5 * w) / total
        })
        .collect();
    Some(interp(q, &mid, &vw.iter().map(|p| p.0).collect::<Vec<_>>()))
}

/// Линейная интерполяция, как `np.interp`: за краями - крайние значения.
fn interp(x: f64, xp: &[f64], fp: &[f64]) -> f64 {
    let (Some(&x0), Some(&xn)) = (xp.first(), xp.last()) else {
        return f64::NAN;
    };
    if x <= x0 {
        return fp[0];
    }
    if x >= xn {
        return fp[fp.len() - 1];
    }
    let i = xp.partition_point(|&p| p <= x).max(1);
    let (a, b) = (xp[i - 1], xp[i]);
    if b == a {
        return fp[i];
    }
    fp[i - 1] + (x - a) * (fp[i] - fp[i - 1]) / (b - a)
}

/// Показатели по значениям с весами: (суффикс, число или None), в порядке
/// `STATS`. Пустые значения не участвуют ни в чём, кроме суммы, где
/// просто не прибавляются. Без значений всё пусто, кроме суммы: у тела
/// без проб сумма ноль.
pub fn stats(
    values: &[Option<f64>],
    weights: &[f64],
    keys: &[&str],
) -> Vec<(&'static str, Option<f64>)> {
    let v: Vec<f64> = values.iter().map(|x| x.unwrap_or(f64::NAN)).collect();
    let wok = |w: f64| w.is_finite() && w > 0.0;
    let (vv, ww): (Vec<f64>, Vec<f64>) = v
        .iter()
        .zip(weights)
        .filter(|(x, w)| x.is_finite() && wok(**w))
        .map(|(x, w)| (*x, *w))
        .unzip();
    let has = !vv.is_empty();
    let wsum: f64 = ww.iter().sum();
    let mean = has.then(|| vv.iter().zip(&ww).map(|(x, w)| x * w).sum::<f64>() / wsum);
    let mut out = Vec::new();
    let on = |k: &str| keys.contains(&k);
    if on("sum") {
        // сумма без весов: запас тела - сумма металла блоков
        let s: f64 = v
            .iter()
            .zip(weights)
            .filter(|(x, w)| wok(**w) && !x.is_nan())
            .map(|(x, _)| *x)
            .sum();
        out.push(("_sum", Some(s)));
    }
    if on("mean") {
        out.push(("_mean", mean));
    }
    if on("min") {
        out.push((
            "_min",
            has.then(|| vv.iter().copied().fold(f64::INFINITY, f64::min)),
        ));
    }
    if on("max") {
        out.push((
            "_max",
            has.then(|| vv.iter().copied().fold(f64::NEG_INFINITY, f64::max)),
        ));
    }
    if on("std") {
        out.push((
            "_std",
            mean.map(|m| {
                (vv.iter()
                    .zip(&ww)
                    .map(|(x, w)| (x - m).powi(2) * w)
                    .sum::<f64>()
                    / wsum)
                    .sqrt()
            }),
        ));
    }
    if on("med") {
        out.push(("_med", wquantile(&vv, &ww, 0.5)));
    }
    if on("quart") {
        out.push(("_p25", wquantile(&vv, &ww, 0.25)));
        out.push(("_p75", wquantile(&vv, &ww, 0.75)));
    }
    if on("decile") {
        out.push(("_p10", wquantile(&vv, &ww, 0.10)));
        out.push(("_p90", wquantile(&vv, &ww, 0.90)));
    }
    out
}

/// Поля объекта по телам, в которые он попал: (первое тело, все тела
/// через «; », число тел). Повторы убираются, порядок сохраняется.
pub fn zone_fields(labels: &[String]) -> (Option<String>, Option<String>, usize) {
    let mut seen: Vec<&String> = Vec::new();
    for x in labels {
        if !seen.contains(&x) {
            seen.push(x);
        }
    }
    match seen.first() {
        None => (None, None, 0),
        Some(first) => (
            Some((*first).clone()),
            Some(
                seen.iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join("; "),
            ),
            seen.len(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hazen_quantile_and_weights() {
        let v = [2.0, 4.0];
        let w = [1.0, 1.0];
        assert_eq!(wquantile(&v, &w, 0.25), Some(2.0));
        assert_eq!(wquantile(&v, &w, 0.5), Some(3.0));
        assert_eq!(wquantile(&[], &[], 0.5), None);
        let s = stats(
            &[Some(2.0), None, Some(4.0)],
            &[1.0, 1.0, 1.0],
            &["sum", "mean", "std"],
        );
        assert_eq!(
            s,
            vec![
                ("_sum", Some(6.0)),
                ("_mean", Some(3.0)),
                ("_std", Some(1.0))
            ]
        );
        let empty = stats(&[], &[], &["sum", "mean"]);
        assert_eq!(empty, vec![("_sum", Some(0.0)), ("_mean", None)]);
        assert_eq!(suffixes(&["quart", "sum"]), vec!["_sum", "_p25", "_p75"]);
        let z = zone_fields(&["A".into(), "B".into(), "A".into()]);
        assert_eq!(z, (Some("A".into()), Some("A; B".into()), 2));
    }
}
