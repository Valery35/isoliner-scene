// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Булевы операции между оболочками по ячейкам. Перенос `boolean3d.py`
//! модуля: `common_box`, `cell_budget`, `shell_occupancy`, `combine`.
//!
//! Обе оболочки переводятся в занятость общего куба лучом вверх
//! из центра каждой колонки: ячейки между первым и вторым
//! пересечением, третьим и четвёртым и так далее лежат внутри.

use crate::fill::{Gt, column_cuts};

/// Операции по номеру пункта, как `OPS` модуля.
pub const OPS: [&str; 3] = ["difference", "union", "intersection"];

/// Охват (x0, x1, y0, y1, z0, z1) вершин.
pub fn bounds(v: &[[f64; 3]]) -> [f64; 6] {
    let mut b = [
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];
    for p in v {
        for k in 0..3 {
            b[2 * k] = b[2 * k].min(p[k]);
            b[2 * k + 1] = b[2 * k + 1].max(p[k]);
        }
    }
    b
}

/// Общий куб для двух тел: (gt, z0, dz, (nz, ny, nx)). У вычитания
/// охват по первому телу, у прочих - по обоим, с запасом в ячейку.
pub fn common_box(a: [f64; 6], b: [f64; 6], cell: f64, op: &str) -> (Gt, f64, f64, [usize; 3]) {
    let [mut x0, mut x1, mut y0, mut y1, mut z0, mut z1] = a;
    if op != "difference" {
        x0 = x0.min(b[0]);
        x1 = x1.max(b[1]);
        y0 = y0.min(b[2]);
        y1 = y1.max(b[3]);
        z0 = z0.min(b[4]);
        z1 = z1.max(b[5]);
    }
    let pad = cell;
    let (x0, x1, y0, y1, z0, z1) = (x0 - pad, x1 + pad, y0 - pad, y1 + pad, z0 - pad, z1 + pad);
    let n = |len: f64| ((len / cell).ceil() as i64).max(1) as usize;
    let nx = n(x1 - x0);
    let ny = n(y1 - y0);
    let nz = (((z1 - z0) / cell).ceil() as i64 + 1).max(1) as usize;
    let gt = [x0, cell, 0.0, y0 + ny as f64 * cell, 0.0, -cell];
    (gt, z0, cell, [nz, ny, nx])
}

/// Сколько ячеек выйдет: считается до выделения памяти. Переполнение -
/// usize::MAX, это больше любого предела.
pub fn cell_budget(shape: [usize; 3]) -> usize {
    shape[0]
        .checked_mul(shape[1])
        .and_then(|x| x.checked_mul(shape[2]))
        .unwrap_or(usize::MAX)
}

/// Занятость ячеек куба замкнутой оболочкой: `[слой][строка * nx +
/// столбец]`, истина там, где центр ячейки внутри.
pub fn shell_occupancy(
    verts: &[[f64; 3]],
    faces: &[[u32; 3]],
    gt: &Gt,
    z0: f64,
    dz: f64,
    shape: [usize; 3],
) -> Vec<Vec<bool>> {
    let [nz, ny, nx] = shape;
    let mut occ = vec![vec![false; ny * nx]; nz];
    let cuts = column_cuts(verts, faces, gt, ny, nx);
    let zlev: Vec<f64> = (0..nz).map(|k| z0 + dz * k as f64).collect();
    let mut s = 0;
    while s < cuts.len() {
        let col = cuts[s].0;
        let mut e = s;
        while e < cuts.len() && cuts[e].0 == col {
            e += 1;
        }
        // нечётный хвост - не пара
        let n = (e - s) - (e - s) % 2;
        for [a, b] in cuts[s..s + n].as_chunks::<2>().0 {
            let (lo, hi) = (a.1, b.1);
            for (k, z) in zlev.iter().enumerate() {
                if *z >= lo
                    && *z <= hi
                    && let Some(c) = occ[k].get_mut(col)
                {
                    *c = true;
                }
            }
        }
        s = e;
    }
    occ
}

/// Какие точки внутри замкнутой оболочки, как `points_inside` модуля:
/// луч из точки вверх, нечётное число пересечений - внутри. Ответ
/// точный, без ячеек.
pub fn points_inside(verts: &[[f64; 3]], faces: &[[u32; 3]], pts: &[[f64; 3]]) -> Vec<bool> {
    if pts.is_empty() {
        return Vec::new();
    }
    // точки по X: у треугольника берётся только своя полоса
    let mut order: Vec<usize> = (0..pts.len()).collect();
    order.sort_by(|&a, &b| pts[a][0].total_cmp(&pts[b][0]));
    let ptp = |k: usize| {
        let (lo, hi) = pts
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), p| {
                (l.min(p[k]), h.max(p[k]))
            });
        hi - lo
    };
    let scale = ptp(0).max(ptp(1)).max(1.0);
    // сдвиг, как у заливки ячеек: точка на ребре двух треугольников
    // засчиталась бы обоим
    let px: Vec<f64> = order
        .iter()
        .map(|&i| pts[i][0] + scale * 1e-9 * 2f64.sqrt())
        .collect();
    let py: Vec<f64> = order
        .iter()
        .map(|&i| pts[i][1] + scale * 1e-9 * 3f64.sqrt())
        .collect();
    let pz: Vec<f64> = order.iter().map(|&i| pts[i][2]).collect();
    let mut above = vec![0u32; pts.len()];
    for t in faces {
        let (Some(a), Some(b), Some(c)) = (
            verts.get(t[0] as usize),
            verts.get(t[1] as usize),
            verts.get(t[2] as usize),
        ) else {
            continue;
        };
        let det = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
        if det.abs() < 1e-12 {
            continue;
        }
        let i0 = px.partition_point(|&x| x < a[0].min(b[0]).min(c[0]));
        let i1 = px.partition_point(|&x| x <= a[0].max(b[0]).max(c[0]));
        let (ylo, yhi) = (a[1].min(b[1]).min(c[1]), a[1].max(b[1]).max(c[1]));
        for i in i0..i1 {
            let (x, y) = (px[i], py[i]);
            if y < ylo || y > yhi {
                continue;
            }
            let u = ((x - a[0]) * (c[1] - a[1]) - (y - a[1]) * (c[0] - a[0])) / det;
            let w = ((y - a[1]) * (b[0] - a[0]) - (x - a[0]) * (b[1] - a[1])) / det;
            if u >= 0.0 && w >= 0.0 && u + w <= 1.0 {
                let z = a[2] + u * (b[2] - a[2]) + w * (c[2] - a[2]);
                if z > pz[i] {
                    above[i] += 1;
                }
            }
        }
    }
    let mut res = vec![false; pts.len()];
    for (k, &i) in order.iter().enumerate() {
        res[i] = above[k] % 2 == 1;
    }
    res
}

/// Охват каждого треугольника: отсечка перед точным пересечением.
pub fn tri_bbox(verts: &[[f64; 3]], faces: &[[u32; 3]]) -> Vec<([f64; 3], [f64; 3])> {
    faces
        .iter()
        .map(|t| {
            let mut lo = [f64::INFINITY; 3];
            let mut hi = [f64::NEG_INFINITY; 3];
            for &i in t {
                if let Some(v) = verts.get(i as usize) {
                    for k in 0..3 {
                        lo[k] = lo[k].min(v[k]);
                        hi[k] = hi[k].max(v[k]);
                    }
                }
            }
            (lo, hi)
        })
        .collect()
}

/// Параметры t в (0, 1), где отрезок a-b пересекает оболочку,
/// как `_segment_hits` модуля. Точки на общем ребре двух треугольников
/// сливаются.
fn segment_hits(
    verts: &[[f64; 3]],
    faces: &[[u32; 3]],
    a: [f64; 3],
    b: [f64; 3],
    bbox: &[([f64; 3], [f64; 3])],
) -> Vec<f64> {
    let sub = |p: [f64; 3], q: [f64; 3]| [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
    let cross = |p: [f64; 3], q: [f64; 3]| {
        [
            p[1] * q[2] - p[2] * q[1],
            p[2] * q[0] - p[0] * q[2],
            p[0] * q[1] - p[1] * q[0],
        ]
    };
    let dot = |p: [f64; 3], q: [f64; 3]| p[0] * q[0] + p[1] * q[1] + p[2] * q[2];
    let d = sub(b, a);
    let s_lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
    let s_hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
    let mut ts = Vec::new();
    for (t, (lo, hi)) in faces.iter().zip(bbox) {
        if (0..3).any(|k| hi[k] < s_lo[k] || lo[k] > s_hi[k]) {
            continue;
        }
        let (Some(p0), Some(p1), Some(p2)) = (
            verts.get(t[0] as usize),
            verts.get(t[1] as usize),
            verts.get(t[2] as usize),
        ) else {
            continue;
        };
        let (e1, e2) = (sub(*p1, *p0), sub(*p2, *p0));
        let h = cross(d, e2);
        let det = dot(e1, h);
        if det.abs() <= 1e-14 {
            continue;
        }
        let inv = 1.0 / det;
        let s = sub(a, *p0);
        let u = dot(s, h) * inv;
        let q = cross(s, e1);
        let w = dot(d, q) * inv;
        let tt = dot(e2, q) * inv;
        if u >= 0.0 && w >= 0.0 && u + w <= 1.0 && tt > 0.0 && tt < 1.0 {
            ts.push(tt);
        }
    }
    ts.sort_by(f64::total_cmp);
    let mut out: Vec<f64> = Vec::with_capacity(ts.len());
    for t in ts {
        if out.last().is_none_or(|l| t - l > 1e-9) {
            out.push(t);
        }
    }
    out
}

/// Длина ломаной внутри замкнутой оболочки, как `polyline_inside_length`
/// модуля: (длина внутри, длина всего). `start_inside` - уже известно,
/// внутри ли начало ломаной.
pub fn polyline_inside_length(
    verts: &[[f64; 3]],
    faces: &[[u32; 3]],
    pts: &[[f64; 3]],
    bbox: &[([f64; 3], [f64; 3])],
    start_inside: Option<bool>,
) -> (f64, f64) {
    if pts.len() < 2 {
        return (0.0, 0.0);
    }
    let mut inside = start_inside
        .unwrap_or_else(|| points_inside(verts, faces, &pts[..1]).first() == Some(&true));
    let (mut got, mut total) = (0.0, 0.0);
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
        if len <= 0.0 || !len.is_finite() {
            continue;
        }
        total += len;
        let ts = segment_hits(verts, faces, a, b, bbox);
        let mut edges = Vec::with_capacity(ts.len() + 2);
        edges.push(0.0);
        edges.extend(ts);
        edges.push(1.0);
        for e in edges.windows(2) {
            if inside {
                got += (e[1] - e[0]) * len;
            }
            inside = !inside;
        }
        // цикл переключил состояние и на последнем фиктивном краю
        inside = !inside;
    }
    (got, total)
}

/// Логическая операция над занятостью двух тел.
pub fn combine(a: &[Vec<bool>], b: &[Vec<bool>], op: &str) -> Vec<Vec<bool>> {
    a.iter()
        .zip(b)
        .map(|(ra, rb)| {
            ra.iter()
                .zip(rb)
                .map(|(&x, &y)| match op {
                    "union" => x || y,
                    "intersection" => x && y,
                    _ => x && !y,
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Куб со стороной 2 с центром в `c`, грани наружу.
    fn cube(c: [f64; 3]) -> (Vec<[f64; 3]>, Vec<[u32; 3]>) {
        let mut v = Vec::new();
        for i in 0..8u32 {
            let s = |b: u32| if i & b == 0 { -1.0 } else { 1.0 };
            v.push([c[0] + s(1), c[1] + s(2), c[2] + s(4)]);
        }
        let quads = [
            [0, 2, 3, 1],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [2, 6, 7, 3],
            [0, 4, 6, 2],
            [1, 3, 7, 5],
        ];
        let f = quads
            .iter()
            .flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]])
            .collect();
        (v, f)
    }

    #[test]
    fn cube_fills_its_cells() {
        let (v, f) = cube([0.0; 3]);
        let b = bounds(&v);
        let (gt, z0, dz, shape) = common_box(b, b, 0.5, "union");
        let occ = shell_occupancy(&v, &f, &gt, z0, dz, shape);
        let n: usize = occ.iter().flatten().filter(|x| **x).count();
        // центры колонок внутри куба: 4 x 4, слоёв в отрезке -1..1: 5
        assert_eq!(n, 4 * 4 * 5);
        let none = combine(&occ, &occ, "difference");
        assert!(none.iter().flatten().all(|x| !x));
        assert_eq!(cell_budget([usize::MAX, 2, 1]), usize::MAX);
    }
}
