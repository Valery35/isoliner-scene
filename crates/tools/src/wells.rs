// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Стволы скважин по инклинометрии и интервалы по стволу (2.17, 2.18).
//! Перенос `wells.py` модуля.
//!
//! Ствол строится методом минимальной кривизны: между двумя замерами он
//! идёт дугой окружности, касательные на концах которой совпадают
//! с замерами. Угол наклона записывают по-разному:
//!
//! - `dip_down_neg` - от горизонта, вниз отрицательный (-90 отвесно);
//! - `dip_down_pos` - от горизонта, вниз положительный (90 отвесно);
//! - `zenith` - зенитный угол от вертикали (0 отвесно вниз).
//!
//! Азимут - от севера сетки по часовой стрелке.

type V3 = [f64; 3];

/// Соглашения об угле, как `CONVENTIONS` модуля.
pub const CONVENTIONS: [&str; 3] = ["dip_down_neg", "dip_down_pos", "zenith"];

/// Угол в зенитный: 0 - вниз отвесно, 90 - горизонтально.
pub fn inclination(angle: f64, convention: &str) -> f64 {
    match convention {
        "dip_down_pos" => 90.0 - angle,
        "zenith" => angle,
        _ => 90.0 + angle,
    }
}

/// Единичное направление ствола: X - восток, Y - север, Z - вверх.
pub fn tangent(inc_deg: f64, azi_deg: f64) -> V3 {
    let (i, a) = (inc_deg.to_radians(), azi_deg.to_radians());
    [i.sin() * a.sin(), i.sin() * a.cos(), -i.cos()]
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(a: V3) -> f64 {
    dot(a, a).sqrt()
}

/// Касательные на дуге между t1 и t2 на долях f.
fn slerp(t1: V3, t2: V3, f: &[f64]) -> Vec<V3> {
    let beta = dot(t1, t2).clamp(-1.0, 1.0).acos();
    f.iter()
        .map(|&f| {
            let v = if beta < 1e-9 {
                [
                    t1[0] + f * (t2[0] - t1[0]),
                    t1[1] + f * (t2[1] - t1[1]),
                    t1[2] + f * (t2[2] - t1[2]),
                ]
            } else {
                let s = beta.sin();
                let (k1, k2) = (((1.0 - f) * beta).sin() / s, (f * beta).sin() / s);
                [
                    k1 * t1[0] + k2 * t2[0],
                    k1 * t1[1] + k2 * t2[1],
                    k1 * t1[2] + k2 * t2[2],
                ]
            };
            let n = norm(v);
            [v[0] / n, v[1] / n, v[2] / n]
        })
        .collect()
}

/// Шаг минимальной кривизны между двумя касательными.
fn mc_step(t1: V3, t2: V3, dmd: f64) -> V3 {
    let beta = dot(t1, t2).clamp(-1.0, 1.0).acos();
    let rf = if beta < 1e-9 {
        1.0
    } else {
        (2.0 / beta) * (beta / 2.0).tan()
    };
    let k = 0.5 * dmd * rf;
    [
        k * (t1[0] + t2[0]),
        k * (t1[1] + t2[1]),
        k * (t1[2] + t2[2]),
    ]
}

/// Замеры по возрастанию глубины, без повторов и пустот:
/// (глубины, зенитные углы, азимуты). Повтор глубины оставляет
/// последний замер.
pub fn clean_survey(
    depth: &[f64],
    azimuth: &[f64],
    angle: &[f64],
    convention: &str,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut rows: Vec<(f64, f64, f64)> = depth
        .iter()
        .zip(azimuth)
        .zip(angle)
        .map(|((d, a), g)| (*d, inclination(*g, convention), *a))
        .filter(|(d, i, a)| d.is_finite() && a.is_finite() && i.is_finite())
        .collect();
    // устойчивая сортировка, как argsort(kind="stable")
    rows.sort_by(|p, q| p.0.total_cmp(&q.0));
    let mut out: (Vec<f64>, Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new(), Vec::new());
    for k in 0..rows.len() {
        let last = k + 1 == rows.len();
        if last || rows[k + 1].0 - rows[k].0 > 1e-9 {
            out.0.push(rows[k].0);
            out.1.push(rows[k].1);
            out.2.push(rows[k].2);
        }
    }
    out
}

/// Замеры с направлением из паспорта скважины на глубине 0, если
/// своего замера там нет: (глубины, азимуты, углы).
pub fn with_collar(
    depth: &[f64],
    azimuth: &[f64],
    angle: &[f64],
    c_azi: Option<f64>,
    c_angle: Option<f64>,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let (d, a, g) = (depth.to_vec(), azimuth.to_vec(), angle.to_vec());
    let (Some(ca), Some(cg)) = (c_azi, c_angle) else {
        return (d, a, g);
    };
    if !(ca.is_finite() && cg.is_finite()) || d.iter().any(|v| v.abs() <= 1e-6) {
        return (d, a, g);
    }
    let pre = |x: f64, v: Vec<f64>| std::iter::once(x).chain(v).collect();
    (pre(0.0, d), pre(ca, a), pre(cg, g))
}

/// Доли от 0 до 1 на n отрезков, как `np.linspace(0, 1, n + 1)`.
fn linspace01(n: usize) -> Vec<f64> {
    let step = 1.0 / n as f64;
    let mut f: Vec<f64> = (0..=n).map(|k| k as f64 * step).collect();
    if let Some(last) = f.last_mut() {
        *last = 1.0;
    }
    f
}

/// Ствол скважины: точки и глубины по стволу. Без замеров ствол
/// отвесный, за последним замером - прямо по нему. `max_depth` - забой,
/// без него ствол кончается на последнем замере. `step` - шаг узлов
/// вдоль дуги, метры.
pub fn desurvey(
    collar: V3,
    depth: &[f64],
    azimuth: &[f64],
    angle: &[f64],
    max_depth: Option<f64>,
    convention: &str,
    step: f64,
) -> (Vec<V3>, Vec<f64>) {
    let (mut d, mut inc, mut azi) = clean_survey(depth, azimuth, angle, convention);
    if d.is_empty() {
        d = vec![0.0];
        inc = vec![0.0];
        azi = vec![0.0];
    }
    let end = match max_depth {
        Some(m) if m.is_finite() => m,
        _ => d[d.len() - 1],
    }
    .max(0.0);
    // станции: устье, замеры внутри (0, end), забой
    let mut md = vec![0.0];
    let mut tg = vec![tangent(inc[0], azi[0])];
    for k in 0..d.len() {
        if 1e-9 < d[k] && d[k] < end - 1e-9 {
            md.push(d[k]);
            tg.push(tangent(inc[k], azi[k]));
        }
    }
    let last_md = md[md.len() - 1];
    if end > last_md + 1e-9 {
        let t_end = match d.iter().position(|&x| x >= end - 1e-9) {
            Some(k) => {
                let t_k = tangent(inc[k], azi[k]);
                if (d[k] - end).abs() < 1e-9 || k == 0 {
                    t_k
                } else {
                    let f = (end - last_md) / (d[k] - last_md);
                    slerp(tg[tg.len() - 1], t_k, &[f])[0]
                }
            }
            None => tg[tg.len() - 1],
        };
        md.push(end);
        tg.push(t_end);
    }
    let step = step.max(1e-3);
    let mut pts = vec![collar];
    let mut mds = vec![0.0];
    for i in 0..md.len() - 1 {
        let len = md[i + 1] - md[i];
        let n = ((len / step).ceil() as usize).max(1);
        let f = linspace01(n);
        let ts = slerp(tg[i], tg[i + 1], &f);
        let mut p = pts[pts.len() - 1];
        for k in 0..n {
            let s = mc_step(ts[k], ts[k + 1], len / n as f64);
            p = [p[0] + s[0], p[1] + s[1], p[2] + s[2]];
            pts.push(p);
            mds.push(md[i] + len * f[k + 1]);
        }
    }
    (pts, mds)
}

/// Линейная интерполяция, как `np.interp`.
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

/// Точки ствола на глубинах, линейно между узлами; глубины вне ствола
/// обрезаются по устью и забою.
pub fn at_depth(points: &[V3], mds: &[f64], depth: &[f64]) -> Vec<V3> {
    let (Some(&m0), Some(&mn)) = (mds.first(), mds.last()) else {
        return Vec::new();
    };
    depth
        .iter()
        .map(|&q| {
            let q = q.clamp(m0.min(mn), mn.max(m0));
            let col = |k: usize| points.iter().map(|p| p[k]).collect::<Vec<_>>();
            [
                interp(q, mds, &col(0)),
                interp(q, mds, &col(1)),
                interp(q, mds, &col(2)),
            ]
        })
        .collect()
}

/// Кусок ствола от глубины d0 до d1: узлы внутри плюс концы.
pub fn segment(points: &[V3], mds: &[f64], d0: f64, d1: f64) -> Vec<V3> {
    let (lo, hi) = if d0 <= d1 { (d0, d1) } else { (d1, d0) };
    let ends = at_depth(points, mds, &[lo, hi]);
    let mut out = Vec::new();
    if let Some(e) = ends.first() {
        out.push(*e);
    }
    out.extend(
        points
            .iter()
            .zip(mds)
            .filter(|(_, m)| **m > lo && **m < hi)
            .map(|(p, _)| *p),
    );
    if let Some(e) = ends.get(1) {
        out.push(*e);
    }
    out
}

/// Равные интервалы от `start` до забоя `end`. Последний короче, если
/// длина не делится нацело.
pub fn equal_intervals(end: f64, length: f64, start: f64) -> Vec<[f64; 2]> {
    if length <= 0.0 || end <= start || !length.is_finite() {
        return Vec::new();
    }
    // np.arange(start, end, length): числа start + k * length
    let n = ((end - start) / length).ceil() as usize;
    let mut edges: Vec<f64> = (0..n).map(|k| start + k as f64 * length).collect();
    edges.push(end);
    if edges.len() >= 2 && edges[edges.len() - 1] - edges[edges.len() - 2] < 1e-9 {
        edges.pop();
    }
    edges.windows(2).map(|w| [w[0], w[1]]).collect()
}

/// Композиты по длине: (интервалы, значения [интервал][столбец],
/// опробованная доля). Среднее взвешено по длине пересечения, пустые
/// значения в него не идут; композит без проб - NaN и доля 0.
pub fn composite(
    froms: &[f64],
    tos: &[f64],
    values: &[Vec<f64>],
    length: f64,
    start: f64,
    end: Option<f64>,
) -> (Vec<[f64; 2]>, Vec<Vec<f64>>, Vec<f64>) {
    let lo: Vec<f64> = froms.iter().zip(tos).map(|(f, t)| f.min(*t)).collect();
    let hi: Vec<f64> = froms.iter().zip(tos).map(|(f, t)| f.max(*t)).collect();
    let end = end.unwrap_or_else(|| {
        if hi.is_empty() {
            start
        } else {
            hi.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        }
    });
    let iv = equal_intervals(end, length, start);
    let ncol = values.first().map_or(0, Vec::len);
    let mut out = vec![vec![f64::NAN; ncol]; iv.len()];
    let mut cover = vec![0.0; iv.len()];
    for (k, [a, b]) in iv.iter().enumerate() {
        let ov: Vec<f64> = lo
            .iter()
            .zip(&hi)
            .map(|(l, h)| (h.min(*b) - l.max(*a)).max(0.0))
            .collect();
        let sum: f64 = ov.iter().filter(|o| **o > 0.0).sum();
        if sum <= 0.0 {
            continue;
        }
        cover[k] = (sum / (b - a)).min(1.0);
        for (j, cell) in out[k].iter_mut().enumerate() {
            let (mut vw, mut w) = (0.0, 0.0);
            for (i, o) in ov.iter().enumerate() {
                let x = values
                    .get(i)
                    .and_then(|r| r.get(j))
                    .copied()
                    .unwrap_or(f64::NAN);
                if *o > 0.0 && x.is_finite() {
                    vw += x * o;
                    w += o;
                }
            }
            if w > 0.0 {
                *cell = vw / w;
            }
        }
    }
    (iv, out, cover)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plumb_hole_goes_straight_down() {
        let (p, m) = desurvey(
            [10.0, 20.0, 100.0],
            &[],
            &[],
            &[],
            Some(30.0),
            "dip_down_neg",
            5.0,
        );
        assert_eq!(p.len(), 7);
        assert!((m[m.len() - 1] - 30.0).abs() < 1e-12);
        let e = p[p.len() - 1];
        assert!((e[0] - 10.0).abs() < 1e-9 && (e[1] - 20.0).abs() < 1e-9);
        assert!((e[2] - 70.0).abs() < 1e-9);
    }

    #[test]
    fn conventions_agree() {
        // -60 от горизонта вниз = 60 вниз = 30 от вертикали
        let a = inclination(-60.0, "dip_down_neg");
        assert_eq!(a, inclination(60.0, "dip_down_pos"));
        assert_eq!(a, inclination(30.0, "zenith"));
    }

    #[test]
    fn intervals_and_composites() {
        assert_eq!(
            equal_intervals(10.0, 4.0, 0.0),
            vec![[0.0, 4.0], [4.0, 8.0], [8.0, 10.0]]
        );
        assert_eq!(equal_intervals(8.0, 4.0, 0.0), vec![[0.0, 4.0], [4.0, 8.0]]);
        let (iv, v, c) = composite(
            &[0.0, 1.0],
            &[1.0, 2.0],
            &[vec![2.0], vec![4.0]],
            2.0,
            0.0,
            Some(4.0),
        );
        assert_eq!(iv.len(), 2);
        assert_eq!(v[0][0], 3.0);
        assert!(v[1][0].is_nan());
        assert_eq!(c, vec![1.0, 0.0]);
    }
}
