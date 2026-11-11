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
