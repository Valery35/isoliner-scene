// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! 3D-плотность: сколько объектов в каждой ячейке объёма. Перенос
//! `density3d.py` модуля.
//!
//! Объект считается один раз, в одной точке: точка - своим положением,
//! линия - серединой по длине, полигон - центром кольца. Длина линии
//! делится между ячейками точно, по пересечениям отрезка с гранями
//! сетки.

use crate::benches::positive;

/// Сетка куба: начало (нижний юго-западный угол первой ячейки), шаг,
/// число ячеек по x, y, z.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lattice {
    pub origin: [f64; 3],
    pub step: [f64; 3],
    pub size: [usize; 3],
}

/// Круглый шаг: 1, 2, 2.5 или 5 на степень десяти.
pub fn round_step(x: f64) -> f64 {
    if !positive(x) {
        return 1.0;
    }
    let p = 10f64.powf(x.log10().floor());
    for m in [1.0, 2.0, 2.5, 5.0, 10.0] {
        if x <= m * p * 1.0001 {
            return m * p;
        }
    }
    10.0 * p
}

impl Lattice {
    /// Сетка на охват данных. Шаг ноль - от охвата: около пятидесяти
    /// ячеек по длинной стороне плана и двадцати по высоте. Сетка крупнее
    /// `limit` ячеек - ошибка с текстом для сообщения.
    pub fn around(
        lo: [f64; 3],
        hi: [f64; 3],
        cell_xy: f64,
        cell_z: f64,
        limit: usize,
    ) -> Result<Self, String> {
        let span: Vec<f64> = (0..3).map(|k| (hi[k] - lo[k]).max(1e-9)).collect();
        let dxy = if cell_xy > 0.0 {
            cell_xy
        } else {
            round_step(span[0].max(span[1]) / 50.0)
        };
        let dz = if cell_z > 0.0 {
            cell_z
        } else if span[2] > 1e-6 {
            round_step(span[2] / 20.0)
        } else {
            dxy
        };
        let step = [dxy, dxy, dz];
        let mut origin = [0.0; 3];
        let mut size = [0usize; 3];
        for k in 0..3 {
            let x0 = (lo[k] / step[k]).floor() * step[k];
            let x1 = (hi[k] / step[k]).ceil() * step[k];
            let mut n = (((x1 - x0) / step[k]).round() as i64).max(1);
            // точка ровно на верхней грани охвата попадает в сетку
            if x0 + n as f64 * step[k] <= hi[k] {
                n += 1;
            }
            origin[k] = x0;
            size[k] = usize::try_from(n).unwrap_or(1);
        }
        let total = size[0].saturating_mul(size[1]).saturating_mul(size[2]);
        if total > limit {
            return Err(format!(
                "cells {} x {} x {} = {} > {}",
                size[0], size[1], size[2], total, limit
            ));
        }
        Ok(Self { origin, step, size })
    }

    /// Номер ячейки точки в плоском массиве (k, j, i) или None - вне сетки.
    pub fn index(&self, p: &[f64; 3]) -> Option<usize> {
        if !p.iter().all(|v| v.is_finite()) {
            return None;
        }
        let mut ijk = [0usize; 3];
        for k in 0..3 {
            let c = ((p[k] - self.origin[k]) / self.step[k]).floor();
            if c < 0.0 || c >= self.size[k] as f64 {
                return None;
            }
            ijk[k] = c as usize;
        }
        Some((ijk[2] * self.size[1] + ijk[1]) * self.size[0] + ijk[0])
    }

    /// Ячеек всего.
    pub fn total(&self) -> usize {
        self.size[0] * self.size[1] * self.size[2]
    }
}

/// Отрезок a-b по ячейкам: (номер ячейки, длина части). Отрезок режется
/// по всем граням сетки, через которые проходит, часть отдаётся ячейке
/// своей середины, части вне сетки отбрасываются.
pub fn segment_cells(a: &[f64; 3], b: &[f64; 3], lat: &Lattice) -> Vec<(usize, f64)> {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let seg = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if !positive(seg) {
        return Vec::new();
    }
    let mut ts = vec![0.0, 1.0];
    for ax in 0..3 {
        if d[ax].abs() < 1e-15 {
            continue;
        }
        let lo = (a[ax].min(b[ax]) - lat.origin[ax]) / lat.step[ax];
        let hi = (a[ax].max(b[ax]) - lat.origin[ax]) / lat.step[ax];
        let mut plane = lo.floor() + 1.0;
        let end = hi.ceil();
        while plane < end {
            let pos = lat.origin[ax] + plane * lat.step[ax];
            ts.push((pos - a[ax]) / d[ax]);
            plane += 1.0;
        }
    }
    let mut t: Vec<f64> = ts.into_iter().map(|x| x.clamp(0.0, 1.0)).collect();
    t.sort_by(f64::total_cmp);
    t.dedup();
    let mut out = Vec::new();
    for w in t.windows(2) {
        let mid = 0.5 * (w[0] + w[1]);
        let part = (w[1] - w[0]) * seg;
        let p = [a[0] + mid * d[0], a[1] + mid * d[1], a[2] + mid * d[2]];
        if let Some(i) = lat.index(&p) {
            out.push((i, part));
        }
    }
    out
}

/// Вид объектов слоя.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Point,
    Line,
    Polygon,
}

fn norm(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt()
}

fn mean(pts: &[[f64; 3]]) -> [f64; 3] {
    let n = pts.len().max(1) as f64;
    let mut m = [0.0; 3];
    for p in pts {
        for k in 0..3 {
            m[k] += p[k];
        }
    }
    m.map(|v| v / n)
}

/// Точка, в которой объект считается один раз.
fn anchor(kind: Kind, parts: &[Vec<[f64; 3]>]) -> Option<[f64; 3]> {
    let pts: Vec<&Vec<[f64; 3]>> = parts.iter().filter(|p| !p.is_empty()).collect();
    let first = pts.first()?;
    match kind {
        Kind::Line => {
            // лучшая часть: вершины, длины отрезков, длина
            type Best<'a> = (&'a Vec<[f64; 3]>, Vec<f64>, f64);
            let mut best: Option<Best> = None;
            let mut best_len = -1.0;
            for p in &pts {
                if p.len() < 2 {
                    continue;
                }
                let seg: Vec<f64> = p.windows(2).map(|w| norm(&w[0], &w[1])).collect();
                let tot: f64 = seg.iter().sum();
                if tot > best_len {
                    best_len = tot;
                    best = Some((p, seg, tot));
                }
            }
            let Some((p, seg, tot)) = best.filter(|_| best_len > 0.0) else {
                return Some(mean(first));
            };
            let mut acc = vec![0.0];
            for s in &seg {
                let last = acc.last().copied().unwrap_or(0.0);
                acc.push(last + s);
            }
            let half = tot / 2.0;
            // searchsorted(side="right") - 1
            let k = acc.iter().filter(|x| **x <= half).count() as i64 - 1;
            let k = k.clamp(0, seg.len() as i64 - 1) as usize;
            let t = if seg[k] > 0.0 {
                (half - acc[k]) / seg[k]
            } else {
                0.0
            };
            Some([0, 1, 2].map(|i| p[k][i] + t * (p[k + 1][i] - p[k][i])))
        }
        Kind::Polygon => {
            let mut ring: &[[f64; 3]] = first;
            let same = |a: &[f64; 3], b: &[f64; 3]| {
                (0..3).all(|k| a[k] == b[k] || (a[k].is_nan() && b[k].is_nan()))
            };
            if ring.len() > 1 && same(&ring[0], &ring[ring.len() - 1]) {
                ring = &ring[..ring.len() - 1];
            }
            Some(mean(ring))
        }
        Kind::Point => {
            let all: Vec<[f64; 3]> = pts.iter().flat_map(|p| p.iter().copied()).collect();
            Some(mean(&all))
        }
    }
}

/// Кубы плотности в порядке (k, j, i) и сводка.
#[derive(Debug, Clone, Default)]
pub struct Cubes {
    pub count: Vec<f64>,
    pub sum: Vec<f64>,
    /// NaN, где значений нет.
    pub mean: Vec<f64>,
    pub length: Vec<f64>,
    pub objects: usize,
    pub inside: usize,
    pub len_all: f64,
    pub len_in: f64,
}

/// Плотность по ячейкам сетки. `value` - число на объект (None - нет).
pub fn density(
    objects: &[Vec<Vec<[f64; 3]>>],
    kind: Kind,
    lat: &Lattice,
    value: Option<&[Option<f64>]>,
) -> Cubes {
    let total = lat.total();
    let mut c = Cubes {
        count: vec![0.0; total],
        sum: vec![0.0; total],
        mean: vec![f64::NAN; total],
        length: vec![0.0; total],
        ..Cubes::default()
    };
    let mut vcnt = vec![0.0; total];
    for (k, parts) in objects.iter().enumerate() {
        let Some(a) = anchor(kind, parts).filter(|a| a.iter().all(|v| v.is_finite())) else {
            continue;
        };
        c.objects += 1;
        let v = value.and_then(|vs| vs.get(k).copied().flatten());
        if kind == Kind::Line {
            for p in parts {
                for s in p.windows(2) {
                    c.len_all += norm(&s[0], &s[1]);
                    for (i, part) in segment_cells(&s[0], &s[1], lat) {
                        c.length[i] += part;
                    }
                }
            }
        }
        if let Some(i) = lat.index(&a) {
            c.inside += 1;
            c.count[i] += 1.0;
            if let Some(v) = v.filter(|v| v.is_finite()) {
                c.sum[i] += v;
                vcnt[i] += 1.0;
            }
        }
    }
    for ((m, s), n) in c.mean.iter_mut().zip(&c.sum).zip(&vcnt) {
        if *n > 0.0 {
            *m = s / n;
        }
    }
    c.len_in = c.length.iter().sum();
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_and_segments() {
        assert_eq!(round_step(0.3), 0.5);
        assert_eq!(round_step(7.0), 10.0);
        let lat = Lattice {
            origin: [0.0; 3],
            step: [1.0; 3],
            size: [4, 1, 1],
        };
        let parts = segment_cells(&[0.5, 0.5, 0.5], &[3.5, 0.5, 0.5], &lat);
        let len: f64 = parts.iter().map(|p| p.1).sum();
        assert!((len - 3.0).abs() < 1e-12 && parts.len() == 4);
    }
}
