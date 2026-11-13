// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Деления, подписи и шкалы вида по разрезу. Перенос части `axes.py`
//! модуля: `nice_step`, `nice_ticks`, `tick_label`, `custom_range`,
//! `near_sides`, `tick_marks`, `north_arrow`, `azimuth`,
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

/// Подпись глубины с точностью шага делений `step`: знаков после
/// точки столько, сколько у шага (0.25 - два, 5 - ни одного). Модуль
/// пишет глубину как есть, с хвостом неровной верхней кромки (0.981658),
/// здесь она округляется. Соседние подписи отличаются на шаг и потому
/// не сливаются.
pub fn depth_label(depth: f64, step: f64) -> String {
    let mut digits = 0usize;
    let mut s = step.abs();
    if s.is_finite() && s > 0.0 {
        while digits < 6 && (s - s.round()).abs() > 1e-9 * s.max(1.0) {
            s *= 10.0;
            digits += 1;
        }
    }
    let text = format!("{depth:.digits$}");
    // -0 и -0.0 после округления пишутся без минуса
    if text
        .trim_start_matches('-')
        .chars()
        .all(|c| c == '0' || c == '.')
    {
        text.trim_start_matches('-').to_string()
    } else {
        text
    }
}

/// Охват короба: свой, заданный в окне модуля, или по сцене, как
/// `custom_range` модуля. `rng` - xmin, xmax, ymin, ymax, zmin, zmax.
/// Пара, где наибольшее не больше наименьшего, не задана: по этой оси
/// охват остаётся от сцены.
pub fn custom_range(lo: V3, hi: V3, rng: Option<&[f64]>) -> (V3, V3) {
    let (mut lo, mut hi) = (lo, hi);
    if let Some(r) = rng.filter(|r| r.len() >= 6) {
        for k in 0..3 {
            let (a, b) = (r[2 * k], r[2 * k + 1]);
            if b > a {
                lo[k] = a;
                hi[k] = b;
            }
        }
    }
    (lo, hi)
}

/// Стороны короба для подписей: какое из четырёх параллельных рёбер
/// несёт деления оси. false - наименьшая координата, true - наибольшая.
/// `x` - ребро оси X (сторона по Y, по Z), `y` - ребро оси Y (по X,
/// по Z), `z` - вертикальное ребро (по X, по Y).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sides {
    pub x: [bool; 2],
    pub y: [bool; 2],
    pub z: [bool; 2],
}

/// Подписи у начала координат, как `ORIGIN_SIDES` модуля.
pub const ORIGIN_SIDES: Sides = Sides {
    x: [false; 2],
    y: [false; 2],
    z: [false; 2],
};

/// Рёбра для подписей, ближние к камере `cam` (настоящие координаты),
/// как `near_sides` модуля. Плановые оси по ребру со стороны камеры,
/// вертикальная по ребру ближнему по X и дальнему по Y, чтобы не встать
/// перед моделью. Камера ниже середины короба переносит плановые
/// подписи наверх.
pub fn near_sides(lo: V3, hi: V3, cam: V3) -> Sides {
    let mid = |k: usize| (lo[k] + hi[k]) / 2.0;
    let xn = cam[0] > mid(0);
    let yn = cam[1] > mid(1);
    let zb = cam[2] < mid(2);
    Sides {
        x: [yn, zb],
        y: [xn, zb],
        z: [xn, !yn],
    }
}

fn pick(lo: V3, hi: V3, axis: usize, side: bool) -> f64 {
    if side { hi[axis] } else { lo[axis] }
}

/// Деление короба: ось, значение, штрих от ребра `a` наружу к `b`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tick {
    pub axis: usize,
    pub value: f64,
    pub a: V3,
    pub b: V3,
}

/// Штрихи на рёбрах короба, как `tick_marks` модуля. `steps` - шаг
/// делений по X, Y, Z, ноль - круглый шаг от размаха. Штрих уходит
/// от ребра наружу в плане на 2 % наибольшего размаха.
pub fn tick_marks(lo: V3, hi: V3, want: usize, steps: [f64; 3], sides: Sides) -> Vec<Tick> {
    let span = (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f64::max);
    let ln = if span > 0.0 { span * 0.02 } else { 0.02 };
    // по какой оси уводить штрих наружу: плановые друг в друга,
    // вертикальную по X
    let away = [1usize, 0, 0];
    // две прочие координаты ребра каждой оси
    let fixed: [[(usize, bool); 2]; 3] = [
        [(1, sides.x[0]), (2, sides.x[1])],
        [(0, sides.y[0]), (2, sides.y[1])],
        [(0, sides.z[0]), (1, sides.z[1])],
    ];
    let mut out = Vec::new();
    for axis in 0..3 {
        let other = away[axis];
        let mut base = lo;
        let mut out_dir = -1.0;
        for (k, side) in fixed[axis] {
            base[k] = pick(lo, hi, k, side);
            if k == other && side {
                out_dir = 1.0;
            }
        }
        for value in nice_ticks(lo[axis], hi[axis], want, steps[axis]) {
            let mut a = base;
            a[axis] = value;
            let mut b = a;
            b[other] = a[other] + out_dir * ln;
            out.push(Tick { axis, value, a, b });
        }
    }
    out
}

/// Стрелка севера сбоку от короба, как `north_arrow` модуля: древко
/// и наконечник. Север - возрастание Y.
pub fn north_arrow(lo: V3, hi: V3) -> [(V3, V3); 3] {
    let span = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1.0);
    let ln = span * 0.18;
    let x = lo[0] - span * 0.12;
    let (y0, z) = (lo[1], lo[2]);
    let tip = [x, y0 + ln, z];
    let wing = ln * 0.28;
    [
        ([x, y0, z], tip),
        (tip, [x - wing * 0.6, y0 + ln - wing, z]),
        (tip, [x + wing * 0.6, y0 + ln - wing, z]),
    ]
}

/// Ось плана смотрит на камеру торцом, как `_axes_edge_on` модуля:
/// все её подписи легли бы в одну точку, их незачем ставить.
/// Углы камеры в градусах.
pub fn edge_on(axis: usize, azimuth: f64, elevation: f64) -> bool {
    let (az, el) = (azimuth.to_radians(), elevation.to_radians());
    let look = [el.cos() * az.cos(), el.cos() * az.sin()];
    axis < 2 && look[axis].abs() > 0.9
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
    let ticks = nice_ticks(zlo, zhi, want, 0.0);
    // шаг делений: разность соседних, при одном делении - весь охват
    let step = ticks
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .find(|d| *d > 0.0)
        .unwrap_or(span);
    for z in ticks {
        let pl = at(out_l, -tick);
        let pr = at(out_r, tick);
        segs.push(([out_l[0], out_l[1], z], [pl[0], pl[1], z]));
        segs.push(([out_r[0], out_r[1], z], [pr[0], pr[1], z]));
        let lab_l = at(out_l, -tick * 2.2);
        let lab_r = at(out_r, tick * 1.4);
        left.push(([lab_l[0], lab_l[1], z], tick_label(z, 1.0)));
        right.push(([lab_r[0], lab_r[1], z], depth_label(zhi - z, step)));
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

    #[test]
    fn depth_labels_rounded_by_step() {
        assert_eq!(depth_label(0.981658, 0.2), "1.0");
        assert_eq!(depth_label(0.781658, 0.2), "0.8");
        assert_eq!(depth_label(12.481658, 5.0), "12");
        assert_eq!(depth_label(0.981658, 0.25), "0.98");
        assert_eq!(depth_label(-0.0004, 0.1), "0.0");
        assert_eq!(depth_label(300.0, 100.0), "300");
        assert_eq!(depth_label(1.5, 0.0), "2");
    }

    /// Короб 0..100 x 0..50 x -30..0 с камерой у начала: деления
    /// на рёбрах у начала, штрихи наружу, как у модуля.
    #[test]
    fn box_ticks_like_the_module() {
        let (lo, hi) = ([0.0, 0.0, -30.0], [100.0, 50.0, 0.0]);
        let t = tick_marks(lo, hi, 5, [0.0; 3], ORIGIN_SIDES);
        let xs: Vec<f64> = t.iter().filter(|t| t.axis == 0).map(|t| t.value).collect();
        assert_eq!(xs, [0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);
        let first = t[0];
        assert_eq!(first.a, [0.0, 0.0, -30.0]);
        assert_eq!(first.b, [0.0, -2.0, -30.0]);
        let z = t.iter().find(|t| t.axis == 2).map(|t| (t.a, t.b));
        assert_eq!(z, Some(([0.0, 0.0, -30.0], [-2.0, 0.0, -30.0])));
        // камера над дальним углом: плановые рёбра со стороны камеры,
        // вертикаль ближняя по X, дальняя по Y
        let s = near_sides(lo, hi, [500.0, 500.0, 100.0]);
        assert_eq!(
            s,
            Sides {
                x: [true, false],
                y: [true, false],
                z: [true, false],
            }
        );
        let t = tick_marks(lo, hi, 5, [0.0; 3], s);
        assert_eq!(t[0].b, [0.0, 52.0, -30.0]);
        assert_eq!(
            custom_range(lo, hi, Some(&[0.0, 0.0, 0.0, 0.0, -100.0, 10.0])),
            ([0.0, 0.0, -100.0], [100.0, 50.0, 10.0])
        );
        assert!(edge_on(0, 0.0, 0.0));
        assert!(!edge_on(0, 90.0, 0.0));
        assert!(!edge_on(2, 0.0, 0.0));
        assert_eq!(north_arrow(lo, hi)[0].0, [-12.0, 0.0, -30.0]);
    }

    /// Неровная верхняя кромка: глубины справа округлены по шагу,
    /// все разные, отметки слева как у модуля.
    #[test]
    fn section_depths_do_not_merge() {
        let s = section_scales([0.0, 0.0], [100.0, 0.0], -1.4, 0.981658, 6, None, 1.0);
        let right: Vec<&str> = s.right.iter().map(|r| r.1.as_str()).collect();
        // деления -1, -0.5, 0, 0.5, глубины 1.981658 и дальше
        assert_eq!(right, ["2.0", "1.5", "1.0", "0.5"]);
        let left: Vec<&str> = s.left.iter().map(|r| r.1.as_str()).collect();
        assert_eq!(left, ["-1", "-0.5", "0", "0.5"]);
    }
}
