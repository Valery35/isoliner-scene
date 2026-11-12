// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Деления, подписи и шкалы вида по разрезу. Перенос части `axes.py`
//! модуля: `nice_step`, `nice_ticks`, `tick_label`, `azimuth`,
//! `section_scales`. Координаты настоящие, метры.

type V3 = [f64; 3];

/// Потолок по делениям на ось, как `MAX_TICKS` модуля.
pub const MAX_TICKS: f64 = 200.0;

/// Круглый шаг, дающий примерно `want` делений на размах.
pub fn nice_step(span: f64, want: usize) -> f64 {
    let span = span.abs();
    if span <= 0.0 || !span.is_finite() {
        return 1.0;
    }
    let raw = span / want.max(1) as f64;
    let mag = 10f64.powf(raw.log10().floor());
    for m in [1.0, 2.0, 5.0, 10.0] {
        if raw <= m * mag * 1.000_000_1 {
            return m * mag;
        }
    }
    10.0 * mag
}

/// Округление до 10 знаков, как `round(x, 10)` в Python.
fn round10(x: f64) -> f64 {
    let r = (x * 1e10).round_ties_even() / 1e10;
    if r.is_finite() { r } else { x }
}

/// Деления по круглым числам внутри размаха, без краёв. `step` больше
/// нуля - шаг, заданный человеком; дал бы больше `MAX_TICKS` - удваивается.
pub fn nice_ticks(lo: f64, hi: f64, want: usize, step: f64) -> Vec<f64> {
    let (lo, hi) = if hi < lo { (hi, lo) } else { (lo, hi) };
    if hi - lo <= 0.0 || !(hi - lo).is_finite() {
        return vec![lo];
    }
    let mut step = if step > 0.0 {
        step
    } else {
        nice_step(hi - lo, want)
    };
    while (hi - lo) / step > MAX_TICKS {
        step *= 2.0;
    }
    let mut out = Vec::new();
    let mut x = (lo / step).ceil() * step;
    while x <= hi + step * 1e-9 {
        out.push(round10(x));
        x += step;
    }
    if out.is_empty() { vec![lo, hi] } else { out }
}

/// Подпись деления без хвоста нулей, как `tick_label` модуля.
pub fn tick_label(value: f64, mult: f64) -> String {
    let mut v = value * mult;
    if v.abs() < 1e-12 {
        v = 0.0;
    }
    if (v - v.round()).abs() < 1e-9 {
        return format!("{}", v.round() as i64);
    }
    let s = format!("{v:.6}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Азимут направления от a к b в плане, градусы 0-360 от севера.
pub fn azimuth(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((b[0] - a[0]).atan2(b[1] - a[1]).to_degrees() + 360.0) % 360.0
}

/// Шкалы вида по разрезу: отрезки, подписи слева и справа, подпись
/// стрелки и азимут.
#[derive(Debug, Clone, PartialEq)]
pub struct SectionScales {
    pub segs: Vec<(V3, V3)>,
    /// Абсолютные отметки у начала линии.
    pub left: Vec<(V3, String)>,
    /// Глубины от верхней кромки у конца линии.
    pub right: Vec<(V3, String)>,
    pub arrow_label: (V3, String),
    pub azimuth: f64,
}

/// Шкалы вида по разрезу на линии a-b в плане при отметках сцены
/// zlo..zhi, как `axes.section_scales` модуля. `tick` - длина штриха,
/// None - полтора процента длины линии. Усы стрелки по высоте делятся
/// на преувеличение `vex`.
pub fn section_scales(
    a: [f64; 2],
    b: [f64; 2],
    zlo: f64,
    zhi: f64,
    want: usize,
    tick: Option<f64>,
    vex: f64,
) -> SectionScales {
    let (zlo, zhi) = (zlo.min(zhi), zlo.max(zhi));
    let mut d = [b[0] - a[0], b[1] - a[1]];
    let mut length = d[0].hypot(d[1]);
    if length <= 0.0 || !length.is_finite() {
        d = [1.0, 0.0];
        length = 1.0;
    }
    let n = d[0].hypot(d[1]);
    let t = [d[0] / n, d[1] / n];
    let span = (zhi - zlo).max(1e-9);
    let tick = tick.filter(|x| *x > 0.0).unwrap_or(0.015 * length);
    let at = |p: [f64; 2], k: f64| [p[0] + t[0] * k, p[1] + t[1] * k];
    let out_l = at(a, -tick * 0.5);
    let out_r = at(b, tick * 0.5);
    let mut segs = vec![
        ([out_l[0], out_l[1], zlo], [out_l[0], out_l[1], zhi]),
        ([out_r[0], out_r[1], zlo], [out_r[0], out_r[1], zhi]),
    ];
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for z in nice_ticks(zlo, zhi, want, 0.0) {
        let pl = at(out_l, -tick);
        let pr = at(out_r, tick);
        segs.push(([out_l[0], out_l[1], z], [pl[0], pl[1], z]));
        segs.push(([out_r[0], out_r[1], z], [pr[0], pr[1], z]));
        let lab_l = at(out_l, -tick * 2.2);
        let lab_r = at(out_r, tick * 1.4);
        left.push(([lab_l[0], lab_l[1], z], tick_label(z, 1.0)));
        right.push(([lab_r[0], lab_r[1], z], tick_label(zhi - z, 1.0)));
    }
    // стрелка направления над серединой, длиной в десятую линии
    let mid = [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])];
    let zt = zhi + 0.06 * span;
    let half = 0.05 * length;
    let s0 = at(mid, -half);
    let s1 = at(mid, half);
    let head = 0.35 * half;
    let tip = [s1[0], s1[1], zt];
    segs.push(([s0[0], s0[1], zt], tip));
    let q = at(s1, -head);
    // усы в вертикальной плоскости разреза, по высоте делятся
    // на преувеличение
    let up = head / vex.max(1e-9);
    for side in [1.0, -1.0] {
        segs.push((tip, [q[0], q[1], zt + side * up]));
    }
    let az = azimuth(a, b);
    SectionScales {
        segs,
        left,
        right,
        arrow_label: ([mid[0], mid[1], zt + 0.04 * span], format!("{az:.0}°")),
        azimuth: az,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_like_the_module() {
        assert_eq!(
            nice_ticks(-320.0, 0.0, 6, 0.0),
            vec![-300.0, -200.0, -100.0, 0.0]
        );
        assert_eq!(nice_step(7.0, 5), 2.0);
        assert_eq!(tick_label(-300.0, 1.0), "-300");
        assert_eq!(tick_label(2.5, 1.0), "2.5");
        assert_eq!(tick_label(-0.0, 1.0), "0");
        assert!((azimuth([0.0, 0.0], [1.0, 1.0]) - 45.0).abs() < 1e-12);
        assert!((azimuth([0.0, 0.0], [-1.0, 0.0]) - 270.0).abs() < 1e-12);
    }
}
