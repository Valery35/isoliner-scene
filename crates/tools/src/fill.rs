// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Заполнение тела до объёма по колонкам. Перенос `fill.py`
//! и `boolean3d.column_cuts` модуля.
//!
//! Тело режется вертикальными колонками по плану. В колонке берутся
//! отрезки внутри тела: вход, выход, вход, выход. Объём ниже уровня -
//! сумма частей отрезков ниже него на площадь колонки. Уровень ищется
//! делением пополам.

/// Грид плана: x0, шаг x, 0, y верхнего края, 0, минус шаг y -
/// как геопреобразование GDAL.
pub type Gt = [f64; 6];

/// Грид плана по охвату (x0, x1, y0, y1): (gt, ny, nx, шаг).
///
/// `cell` 0 - шаг по охвату, чтобы колонок было около `max_cols`.
/// Меньший шаг, давший больше колонок, укрупняется до предела.
pub fn plan_grid(bounds: [f64; 4], cell: f64, max_cols: usize) -> (Gt, usize, usize, f64) {
    let [x0, x1, y0, y1] = bounds;
    let w = (x1 - x0).max(1e-9);
    let hgt = (y1 - y0).max(1e-9);
    let auto = (w * hgt / max_cols as f64).sqrt();
    let step = if cell > 0.0 { cell.max(auto) } else { auto }.max(1e-9);
    let nx = ((w / step).ceil() as usize).max(1);
    let ny = ((hgt / step).ceil() as usize).max(1);
    (
        [x0, step, 0.0, y0 + ny as f64 * step, 0.0, -step],
        ny,
        nx,
        step,
    )
}

/// Пересечения вертикальных лучей колонок с треугольниками:
/// (колонка, отметка), по колонке, внутри неё по отметке.
pub fn column_cuts(
    verts: &[[f64; 3]],
    faces: &[[u32; 3]],
    gt: &Gt,
    ny: usize,
    nx: usize,
) -> Vec<(usize, f64)> {
    // Луч сдвинут на ничтожную долю ячейки: легший ровно на ребро,
    // он засчитался бы двум треугольникам, и чётность сломалась бы.
    let ex = gt[1].abs() * 1e-6 * 2f64.sqrt();
    let ey = gt[5].abs() * 1e-6 * 3f64.sqrt();
    let xs: Vec<f64> = (0..nx)
        .map(|i| gt[0] + gt[1] * (i as f64 + 0.5) + ex)
        .collect();
    let ys: Vec<f64> = (0..ny)
        .map(|j| gt[3] + gt[5] * (j as f64 + 0.5) + ey)
        .collect();
    let mut cuts = Vec::new();
    for f in faces {
        let (Some(a), Some(b), Some(c)) = (
            verts.get(f[0] as usize),
            verts.get(f[1] as usize),
            verts.get(f[2] as usize),
        ) else {
            continue;
        };
        let det = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
        if det.abs() < 1e-12 {
            continue; // вертикальная грань
        }
        let (xmin, xmax) = (a[0].min(b[0]).min(c[0]), a[0].max(b[0]).max(c[0]));
        let (ymin, ymax) = (a[1].min(b[1]).min(c[1]), a[1].max(b[1]).max(c[1]));
        let i0 = xs.partition_point(|&x| x < xmin);
        let i1 = xs.partition_point(|&x| x <= xmax);
        // строки идут сверху вниз: Y убывает
        let j0 = ys.partition_point(|&y| y > ymax);
        let j1 = ys.partition_point(|&y| y >= ymin);
        for (j, &py) in ys.iter().enumerate().take(j1).skip(j0) {
            for (i, &px) in xs.iter().enumerate().take(i1).skip(i0) {
                let u = ((px - a[0]) * (c[1] - a[1]) - (py - a[1]) * (c[0] - a[0])) / det;
                let w = ((py - a[1]) * (b[0] - a[0]) - (px - a[0]) * (b[1] - a[1])) / det;
                if u >= 0.0 && w >= 0.0 && u + w <= 1.0 {
                    let z = a[2] + u * (b[2] - a[2]) + w * (c[2] - a[2]);
                    cuts.push((j * nx + i, z));
                }
            }
        }
    }
    cuts.sort_by(|p, q| p.0.cmp(&q.0).then(p.1.total_cmp(&q.1)));
    cuts
}

/// Отрезок колонки внутри тела.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    pub col: usize,
    pub lo: f64,
    pub hi: f64,
}

/// Отрезки внутри тела по пересечениям. Нечётный хвост колонки
/// отбрасывается: у рваного тела это не внутренность.
pub fn intervals(cuts: &[(usize, f64)]) -> Vec<Interval> {
    let mut out = Vec::new();
    let mut s = 0;
    while s < cuts.len() {
        let col = cuts[s].0;
        let mut e = s;
        while e < cuts.len() && cuts[e].0 == col {
            e += 1;
        }
        let mut r = s;
        while r + 1 < e {
            out.push(Interval {
                col,
                lo: cuts[r].1,
                hi: cuts[r + 1].1,
            });
            r += 2;
        }
        s = e;
    }
    out
}

/// Объём внутри тела ниже уровня `h`.
pub fn volume_below(iv: &[Interval], area: f64, h: f64) -> f64 {
    area * iv
        .iter()
        .map(|i| (i.hi.min(h) - i.lo).max(0.0))
        .sum::<f64>()
}

/// Весь объём тела по колонкам.
pub fn total_volume(iv: &[Interval], area: f64) -> f64 {
    area * iv.iter().map(|i| (i.hi - i.lo).max(0.0)).sum::<f64>()
}

/// Уровень, ниже которого (`from_top` - выше которого) объём `target`:
/// (уровень, объём на этом уровне).
pub fn level_for_volume(
    iv: &[Interval],
    area: f64,
    target: f64,
    from_top: bool,
    tol: f64,
) -> Option<(f64, f64)> {
    if iv.is_empty() {
        return None;
    }
    let full = total_volume(iv, area);
    let zmin = iv.iter().map(|i| i.lo).fold(f64::INFINITY, f64::min);
    let zmax = iv.iter().map(|i| i.hi).fold(f64::NEG_INFINITY, f64::max);
    let target = target.max(0.0).min(full);
    if target >= full {
        return Some((if from_top { zmin } else { zmax }, full));
    }
    if target <= 0.0 {
        return Some((if from_top { zmax } else { zmin }, 0.0));
    }
    let want = if from_top { full - target } else { target };
    let (mut a, mut b) = (zmin, zmax);
    let span = (zmax - zmin).max(1e-12);
    for _ in 0..200 {
        let mid = 0.5 * (a + b);
        if volume_below(iv, area, mid) < want {
            a = mid;
        } else {
            b = mid;
        }
        if b - a <= tol * span {
            break;
        }
    }
    let h = 0.5 * (a + b);
    let got = volume_below(iv, area, h);
    Some((h, if from_top { full - got } else { got }))
}

/// Куб заполненной части: занятость [слой][колонка], отметка центра
/// нижнего слоя, высота слоя. Уровень приходится на границу слоёв.
pub fn filled_occupancy(
    iv: &[Interval],
    ny: usize,
    nx: usize,
    level: f64,
    dz: f64,
    from_top: bool,
) -> (Vec<Vec<bool>>, f64, f64) {
    let zmin = iv.iter().map(|i| i.lo).fold(f64::INFINITY, f64::min);
    let zmax = iv.iter().map(|i| i.hi).fold(f64::NEG_INFINITY, f64::max);
    let (bottom, top) = if from_top {
        (level, zmax)
    } else {
        (zmin, level)
    };
    let nz = (((top - bottom) / dz).ceil() as usize).max(1);
    let zc: Vec<f64> = (0..nz)
        .map(|k| {
            if from_top {
                bottom + dz * (k as f64 + 0.5)
            } else {
                top - dz * ((nz - k) as f64 - 0.5)
            }
        })
        .collect();
    let mut occ = vec![vec![false; ny * nx]; nz];
    for i in iv {
        // zc растёт: слои отрезка ищутся делением пополам
        let k0 = zc.partition_point(|&z| z < i.lo);
        let k1 = zc.partition_point(|&z| z <= i.hi);
        for row in occ.iter_mut().take(k1).skip(k0) {
            if let Some(c) = row.get_mut(i.col) {
                *c = true;
            }
        }
    }
    (occ, zc.first().copied().unwrap_or(bottom), dz)
}
