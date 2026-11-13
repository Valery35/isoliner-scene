// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Тело по контуру: плоский пол, стенки и свод от контура. Перенос
//! `crown.py` модуля и `mesh3d.bed_to_mesh_arrays`, `mesh3d.polygon_mask`.
//!
//! Пол - плоскость по отметкам контура наименьшими квадратами. Высота
//! свода в точке растёт с расстоянием до контура: линейный свод -
//! крутизна на расстояние, округлый - дуга той же высоты. Подъём можно
//! ограничить сверху.

/// Не больше стольких ячеек сетки: шаг растёт в 1.25 раза, пока не влезет.
const MAX_CELLS: usize = 250_000;

/// Привязка сетки как у GDAL: (x0, шаг, 0, y1, 0, -шаг).
pub type Gt = [f64; 6];

/// Вершины и грани тела.
pub type Mesh = (Vec<[f64; 3]>, Vec<[u32; 3]>);

/// Плоскость z = a*x + b*y + c по вершинам кольца наименьшими
/// квадратами. У вырожденного кольца - горизонталь на средней Z.
pub fn fit_plane(ring: &[[f64; 3]]) -> (f64, f64, f64) {
    let n = ring.len().max(1) as f64;
    let mut c = [0.0; 3];
    for p in ring {
        for k in 0..3 {
            c[k] += p[k];
        }
    }
    for v in &mut c {
        *v /= n;
    }
    let (mut xx, mut xy, mut yy, mut xz, mut yz) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for p in ring {
        let d = [p[0] - c[0], p[1] - c[1], p[2] - c[2]];
        xx += d[0] * d[0];
        xy += d[0] * d[1];
        yy += d[1] * d[1];
        xz += d[0] * d[2];
        yz += d[1] * d[2];
    }
    let det = xx * yy - xy * xy;
    if det.abs() < 1e-12 {
        return (0.0, 0.0, c[2]);
    }
    let a = (xz * yy - xy * yz) / det;
    let b = (xx * yz - xy * xz) / det;
    (a, b, c[2] - a * c[0] - b * c[1])
}

/// Площадь кольца в плане по формуле шнурования.
pub fn ring_area(ring: &[[f64; 3]]) -> f64 {
    let n = ring.len();
    let mut s = 0.0;
    for i in 0..n {
        let (p, q) = (ring[i], ring[(i + 1) % n]);
        s += p[0] * q[1] - q[0] * p[1];
    }
    0.5 * s.abs()
}

/// Расстояние от точки до ломаной кольца, точно по отрезкам.
fn boundary_distance(x: f64, y: f64, ring: &[[f64; 3]]) -> f64 {
    let mut p: Vec<[f64; 2]> = ring.iter().map(|q| [q[0], q[1]]).collect();
    if let (Some(a), Some(b)) = (p.first().copied(), p.last().copied())
        && ((a[0] - b[0]).abs() > 1e-8 + 1e-5 * b[0].abs()
            || (a[1] - b[1]).abs() > 1e-8 + 1e-5 * b[1].abs())
    {
        p.push(a);
    }
    let mut best = f64::INFINITY;
    for w in p.windows(2) {
        let (a, b) = (w[0], w[1]);
        let ab = [b[0] - a[0], b[1] - a[1]];
        let ll = (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-24);
        let t = (((x - a[0]) * ab[0] + (y - a[1]) * ab[1]) / ll).clamp(0.0, 1.0);
        let (nx, ny) = (a[0] + t * ab[0], a[1] + t * ab[1]);
        let d = ((x - nx).powi(2) + (y - ny).powi(2)).sqrt();
        best = best.min(d);
    }
    best
}

/// Маска ячеек внутри кольца по центрам ячеек, правило чёт-нечет, как
/// `mesh3d.polygon_mask`.
fn polygon_mask(ring: &[[f64; 3]], gt: &Gt, ny: usize, nx: usize) -> Vec<bool> {
    let mut inside = vec![false; ny * nx];
    if ring.len() < 3 {
        return inside;
    }
    let mut pts: Vec<[f64; 2]> = ring.iter().map(|q| [q[0], q[1]]).collect();
    if pts.first() != pts.last()
        && let Some(f) = pts.first().copied()
    {
        pts.push(f);
    }
    for i in 0..ny {
        let yy = gt[3] + (i as f64 + 0.5) * gt[5];
        for j in 0..nx {
            let xx = gt[0] + (j as f64 + 0.5) * gt[1];
            let mut v = false;
            for w in pts.windows(2) {
                let ([x1, y1], [x2, y2]) = (w[0], w[1]);
                if y1 == y2 {
                    continue;
                }
                if (y1 > yy) != (y2 > yy) && xx < (x2 - x1) * (yy - y1) / (y2 - y1) + x1 {
                    v = !v;
                }
            }
            inside[i * nx + j] = v;
        }
    }
    inside
}

/// Сводка тела: площадь контура и подъём свода.
#[derive(Debug, Clone, Copy)]
pub struct Info {
    pub area: f64,
    pub rise: f64,
}

/// Гриды кровли и пола (NaN - вне контура), их размер, привязка, сводка.
pub struct Grids {
    pub top: Vec<f64>,
    pub bot: Vec<f64>,
    pub ny: usize,
    pub nx: usize,
    pub gt: Gt,
    pub info: Info,
}

/// Гриды тела по контуру. `floor` - плоскость пола (a, b, c) или None:
/// тогда по Z кольца. Ошибка - контур вырожден или ячейка крупнее его.
#[allow(clippy::too_many_arguments)]
pub fn crown_grids(
    ring: &[[f64; 3]],
    floor: Option<(f64, f64, f64)>,
    wall: f64,
    steep: f64,
    rise_max: f64,
    round: bool,
    cell: f64,
) -> Result<Grids, &'static str> {
    let mut p: Vec<[f64; 3]> = ring.to_vec();
    if p.len() > 1 {
        let (a, b) = (p[0], p[p.len() - 1]);
        let close = (0..3).all(|k| {
            (a[k].is_nan() && b[k].is_nan()) || (a[k] - b[k]).abs() <= 1e-8 + 1e-5 * b[k].abs()
        });
        if close {
            p.pop();
        }
    }
    if p.len() < 3 || ring_area(&p) <= 0.0 {
        return Err("degenerate ring");
    }
    let plane = floor.unwrap_or_else(|| fit_plane(&p));
    let fold = |f: fn(f64, f64) -> f64, k: usize, init: f64| p.iter().fold(init, |m, q| f(m, q[k]));
    let (x0, y0) = (
        fold(f64::min, 0, f64::INFINITY),
        fold(f64::min, 1, f64::INFINITY),
    );
    let (x1, y1) = (
        fold(f64::max, 0, f64::NEG_INFINITY),
        fold(f64::max, 1, f64::NEG_INFINITY),
    );
    let span = (x1 - x0).max(y1 - y0);
    let mut step = if cell > 0.0 { cell } else { span / 120.0 };
    let dims = |s: f64| {
        (
            ((x1 - x0) / s).ceil() as usize + 2,
            ((y1 - y0) / s).ceil() as usize + 2,
        )
    };
    let (mut nx, mut ny) = dims(step);
    while nx * ny > MAX_CELLS {
        step *= 1.25;
        (nx, ny) = dims(step);
    }
    let gt: Gt = [x0 - step, step, 0.0, y1 + step, 0.0, -step];
    let inside = polygon_mask(&p, &gt, ny, nx);
    if !inside.iter().any(|&b| b) {
        return Err("cell too coarse");
    }
    let mut d = vec![0.0; ny * nx];
    for i in 0..ny {
        let y = gt[3] - (i as f64 + 0.5) * step;
        for j in 0..nx {
            if inside[i * nx + j] {
                let x = gt[0] + (j as f64 + 0.5) * step;
                d[i * nx + j] = boundary_distance(x, y, &p);
            }
        }
    }
    let dmax = d
        .iter()
        .zip(&inside)
        .filter(|(_, b)| **b)
        .fold(f64::NEG_INFINITY, |m, (v, _)| m.max(*v));
    let k = steep.max(1e-6);
    let (a, b, c) = plane;
    let mut top = vec![f64::NAN; ny * nx];
    let mut bot = vec![f64::NAN; ny * nx];
    let mut rise_top = f64::NEG_INFINITY;
    for i in 0..ny {
        let y = gt[3] - (i as f64 + 0.5) * step;
        for j in 0..nx {
            let at = i * nx + j;
            if !inside[at] {
                continue;
            }
            let x = gt[0] + (j as f64 + 0.5) * step;
            let fl = a * x + b * y + c;
            let mut rise = if round {
                // дуга: у стенки вертикальна, у гребня горизонтальна
                let u = (d[at] / dmax).clamp(0.0, 1.0);
                k * dmax * (u * (2.0 - u)).clamp(0.0, 1.0).sqrt()
            } else {
                k * d[at]
            };
            if rise_max > 0.0 {
                rise = rise.min(rise_max);
            }
            rise_top = rise_top.max(rise);
            top[at] = fl + wall + rise;
            bot[at] = fl;
        }
    }
    Ok(Grids {
        top,
        bot,
        ny,
        nx,
        gt,
        info: Info {
            area: ring_area(&p),
            rise: rise_top,
        },
    })
}

/// Замкнутое тело из пары гридов: кровля, подошва и боковая юбка по
/// границе области, где валидны обе. Первые n вершин - кровля, следующие
/// n - подошва. Перенос `mesh3d.bed_to_mesh_arrays` (шаг 1, масштаб 1).
pub fn bed_mesh(g: &Grids) -> Result<Mesh, &'static str> {
    let (ny, nx) = (g.ny, g.nx);
    if ny < 2 || nx < 2 {
        return Err("grid too small");
    }
    let valid: Vec<bool> = (0..ny * nx)
        .map(|k| g.top[k].is_finite() && g.bot[k].is_finite())
        .collect();
    let mut idx = vec![-1i64; ny * nx];
    let mut n = 0i64;
    for k in 0..ny * nx {
        if valid[k] {
            idx[k] = n;
            n += 1;
        }
    }
    if n == 0 {
        return Err("no data");
    }
    let at = |i: usize, j: usize| i * nx + j;
    let u = |v: i64| u32::try_from(v).unwrap_or(u32::MAX);
    let n_u = u(n);
    // квадраты, у которых валидны все четыре угла
    let q = |i: usize, j: usize| {
        valid[at(i, j)] && valid[at(i, j + 1)] && valid[at(i + 1, j)] && valid[at(i + 1, j + 1)]
    };
    let mut first: Vec<[u32; 3]> = Vec::new();
    let mut second: Vec<[u32; 3]> = Vec::new();
    for i in 0..ny - 1 {
        for j in 0..nx - 1 {
            if q(i, j) {
                let (n00, n01) = (u(idx[at(i, j)]), u(idx[at(i, j + 1)]));
                let (n10, n11) = (u(idx[at(i + 1, j)]), u(idx[at(i + 1, j + 1)]));
                first.push([n00, n01, n11]);
                second.push([n00, n11, n10]);
            }
        }
    }
    let roof: Vec<[u32; 3]> = first.into_iter().chain(second).collect();
    let floor: Vec<[u32; 3]> = roof
        .iter()
        .map(|t| [t[2] + n_u, t[1] + n_u, t[0] + n_u])
        .collect();
    // квадраты с полем в одну ячейку по краям: qp[i][j] = q(i-1, j-1)
    let qp = |i: usize, j: usize| i >= 1 && j >= 1 && i < ny && j < nx && q(i - 1, j - 1);
    let mut edges: Vec<(u32, u32)> = Vec::new();
    // горизонтальные рёбра (i,j)-(i,j+1): квадраты сверху и снизу
    for i in 0..ny {
        for j in 0..nx - 1 {
            if qp(i, j + 1) != qp(i + 1, j + 1) {
                let (p1, p2) = (idx[at(i, j)], idx[at(i, j + 1)]);
                if p1 >= 0 && p2 >= 0 {
                    edges.push((u(p1), u(p2)));
                }
            }
        }
    }
    // вертикальные рёбра (i,j)-(i+1,j): квадраты слева и справа
    for i in 0..ny - 1 {
        for j in 0..nx {
            if qp(i + 1, j) != qp(i + 1, j + 1) {
                let (p1, p2) = (idx[at(i, j)], idx[at(i + 1, j)]);
                if p1 >= 0 && p2 >= 0 {
                    edges.push((u(p1), u(p2)));
                }
            }
        }
    }
    let sk1 = edges.iter().map(|&(a, b)| [a, b, b + n_u]);
    let sk2 = edges.iter().map(|&(a, b)| [a, b + n_u, a + n_u]);
    let skirt: Vec<[u32; 3]> = sk1.chain(sk2).collect();

    let mut verts = Vec::with_capacity(2 * n as usize);
    let mut lower = Vec::with_capacity(n as usize);
    for i in 0..ny {
        let y = g.gt[3] + (i as f64 + 0.5) * g.gt[5];
        for j in 0..nx {
            if valid[at(i, j)] {
                let x = g.gt[0] + (j as f64 + 0.5) * g.gt[1];
                verts.push([x, y, g.top[at(i, j)]]);
                lower.push([x, y, g.bot[at(i, j)]]);
            }
        }
    }
    verts.extend(lower);
    let faces: Vec<[u32; 3]> = roof.into_iter().chain(floor).chain(skirt).collect();
    Ok((verts, faces))
}

/// Объём замкнутой оболочки, как `cleanup.mesh_volume` модуля:
/// вырожденные грани выбрасываются, обход граней согласуется по общим
/// рёбрам, сумма тетраэдров - от середины вершин, по модулю. У тела
/// из гридов юбка обходится вразнобой с кровлей, без согласования объём
/// выходит меньше настоящего.
pub fn mesh_volume(verts: &[[f64; 3]], faces: &[[u32; 3]]) -> f64 {
    let mut f: Vec<[u32; 3]> = faces
        .iter()
        .copied()
        .filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2])
        .collect();
    if f.is_empty() || verts.is_empty() {
        return 0.0;
    }
    // согласование обхода: у соседних граней общее ребро проходится
    // в разные стороны
    let key = |a: u32, b: u32| if a < b { (a, b) } else { (b, a) };
    let mut edges: std::collections::HashMap<(u32, u32), Vec<usize>> =
        std::collections::HashMap::new();
    for (i, t) in f.iter().enumerate() {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            edges.entry(key(a, b)).or_default().push(i);
        }
    }
    let mut seen = vec![false; f.len()];
    for start in 0..f.len() {
        if seen[start] {
            continue;
        }
        seen[start] = true;
        let mut stack = vec![start];
        while let Some(i) = stack.pop() {
            let t = f[i];
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                for &j in edges.get(&key(a, b)).into_iter().flatten() {
                    if seen[j] {
                        continue;
                    }
                    seen[j] = true;
                    let t2 = f[j];
                    let same = (0..3).any(|k| t2[k] == a && t2[(k + 1) % 3] == b);
                    if same {
                        f[j] = [t2[2], t2[1], t2[0]];
                    }
                    stack.push(j);
                }
            }
        }
    }
    let n = verts.len() as f64;
    let mut c = [0.0; 3];
    for v in verts {
        for k in 0..3 {
            c[k] += v[k] / n;
        }
    }
    let p = |i: u32| {
        verts
            .get(i as usize)
            .map(|v| [v[0] - c[0], v[1] - c[1], v[2] - c[2]])
    };
    let mut sum = 0.0;
    for t in &f {
        let (Some(a), Some(b), Some(d)) = (p(t[0]), p(t[1]), p(t[2])) else {
            continue;
        };
        sum += a[0] * (b[1] * d[2] - b[2] * d[1])
            + a[1] * (b[2] * d[0] - b[0] * d[2])
            + a[2] * (b[0] * d[1] - b[1] * d[0]);
    }
    (sum / 6.0).abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_and_area() {
        let sq = [
            [0.0, 0.0, 1.0],
            [2.0, 0.0, 3.0],
            [2.0, 2.0, 3.0],
            [0.0, 2.0, 1.0],
        ];
        let (a, b, c) = fit_plane(&sq);
        assert!((a - 1.0).abs() < 1e-12 && b.abs() < 1e-12 && (c - 1.0).abs() < 1e-12);
        assert!((ring_area(&sq) - 4.0).abs() < 1e-12);
        assert!(crown_grids(&sq[..2], None, 0.0, 0.5, 0.0, false, 0.0).is_err());
    }
}
