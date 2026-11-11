// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Охват, объём и замкнутость тел сцены.

use std::collections::HashMap;

use glam::DVec3;
use scene_format::{Indices, Part};

use crate::Frame;

/// Охват частей: (низ, верх). Пустой набор - None.
pub fn bounds<'a>(parts: impl IntoIterator<Item = &'a Part>) -> Option<([f64; 3], [f64; 3])> {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for v in parts.into_iter().flat_map(|p| p.verts.iter()) {
        for k in 0..3 {
            lo[k] = lo[k].min(v[k]);
            hi[k] = hi[k].max(v[k]);
        }
    }
    lo[0].is_finite().then_some((lo, hi))
}

/// Объём замкнутой сетки со знаком: плюс у наружных нормалей.
///
/// Считается от середины охвата: на координатах в миллионы метров
/// сумма от нуля теряла бы знаки.
pub fn signed_volume(verts: &[[f64; 3]], faces: &[[u32; 3]]) -> f64 {
    let n = verts.len().max(1) as f64;
    let mut c = [0.0; 3];
    for v in verts {
        for k in 0..3 {
            c[k] += v[k] / n;
        }
    }
    let p = |i: u32| -> [f64; 3] {
        let v = verts.get(i as usize).copied().unwrap_or(c);
        [v[0] - c[0], v[1] - c[1], v[2] - c[2]]
    };
    faces
        .iter()
        .map(|f| {
            let (a, b, d) = (p(f[0]), p(f[1]), p(f[2]));
            let cross = [
                b[1] * d[2] - b[2] * d[1],
                b[2] * d[0] - b[0] * d[2],
                b[0] * d[1] - b[1] * d[0],
            ];
            a[0] * cross[0] + a[1] * cross[1] + a[2] * cross[2]
        })
        .sum::<f64>()
        / 6.0
}

/// Рёбер, у которых не две грани: 0 у замкнутого тела.
pub fn open_edges(faces: &[[u32; 3]]) -> usize {
    let mut count: HashMap<(u32, u32), u32> = HashMap::new();
    for f in faces {
        for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
            *count.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    count.values().filter(|&&n| n != 2).count()
}

/// Попадание луча в тело сцены.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    /// Номер части в переданном наборе.
    pub part: usize,
    /// Точка в координатах просмотра.
    pub point: [f64; 3],
    /// Доля пути луча до точки.
    pub t: f64,
}

/// Ближайшее попадание луча (`origin` + `dir` * t, t от 0 до 1)
/// в треугольники частей. Луч в координатах просмотра `frame`.
/// Линии не ловятся: у них нет площади.
pub fn raycast<'a>(
    parts: impl IntoIterator<Item = (usize, &'a Part)>,
    frame: &Frame,
    origin: DVec3,
    dir: DVec3,
) -> Option<Hit> {
    let mut best: Option<Hit> = None;
    for (n, part) in parts {
        let Indices::Triangles(tris) = &part.indices else {
            continue;
        };
        let v = |i: u32| {
            part.verts
                .get(i as usize)
                .map(|p| DVec3::from(frame.to_view(*p)))
        };
        for f in tris {
            let (Some(a), Some(b), Some(c)) = (v(f[0]), v(f[1]), v(f[2])) else {
                continue;
            };
            if let Some(t) = hit_triangle(origin, dir, a, b, c)
                && best.is_none_or(|h| t < h.t)
            {
                best = Some(Hit {
                    part: n,
                    point: (origin + dir * t).into(),
                    t,
                });
            }
        }
    }
    best
}

/// Луч и треугольник, алгоритм Мёллера - Трумбора. Обе стороны
/// грани: у поверхности пласта нет изнанки.
fn hit_triangle(o: DVec3, d: DVec3, a: DVec3, b: DVec3, c: DVec3) -> Option<f64> {
    let e1 = b - a;
    let e2 = c - a;
    let p = d.cross(e2);
    let det = e1.dot(p);
    // относительный порог: координаты просмотра бывают в километрах
    if det.abs() <= 1e-12 * e1.length() * e2.length() * d.length() {
        return None;
    }
    let s = o - a;
    let u = s.dot(p) / det;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let w = d.dot(q) / det;
    if w < 0.0 || u + w > 1.0 {
        return None;
    }
    let t = e2.dot(q) / det;
    (0.0..=1.0).contains(&t).then_some(t)
}

/// Сводка по части: замкнута ли и её объём, если замкнута.
pub fn body_volume(part: &Part) -> Option<f64> {
    match &part.indices {
        Indices::Triangles(t) if !t.is_empty() && open_edges(t) == 0 => {
            Some(signed_volume(&part.verts, t).abs())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Куб 2 x 2 x 2 с центром в точке `c`.
    fn cube(c: [f64; 3]) -> Part {
        let mut verts = Vec::new();
        for i in 0..8 {
            let s = |b: usize| if i & b == 0 { -1.0 } else { 1.0 };
            verts.push([c[0] + s(1), c[1] + s(2), c[2] + s(4)]);
        }
        let quads = [
            [0, 1, 3, 2],
            [4, 6, 7, 5],
            [0, 4, 5, 1],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 5, 7, 3],
        ];
        let tris = quads
            .iter()
            .flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]])
            .collect();
        Part {
            name: "куб".into(),
            verts,
            indices: Indices::Triangles(tris),
            colors: None,
        }
    }

    #[test]
    fn ray_hits_the_nearest_face_in_real_coordinates() {
        // настоящие координаты в миллионах метров, преувеличение 2
        let near = cube([512_000.0, 6_401_000.0, -100.0]);
        let far = cube([512_000.0, 6_401_000.0, -110.0]);
        let frame = Frame {
            cx: 512_000.0,
            cy: 6_401_000.0,
            cz: -100.0,
            vex: 2.0,
        };
        assert_eq!(body_volume(&near), Some(8.0));
        // луч сверху вниз через середину
        let o = DVec3::new(0.0, 0.0, 50.0);
        let d = DVec3::new(0.0, 0.0, -100.0);
        let hit = raycast([(0, &far), (1, &near)], &frame, o, d).unwrap();
        assert_eq!(hit.part, 1);
        // верх ближнего куба: отметка -99, в просмотре (+1) * 2
        assert!((hit.point[2] - 2.0).abs() < 1e-9, "{hit:?}");
        assert!((frame.to_world(hit.point)[2] + 99.0).abs() < 1e-9);
        // мимо
        let miss = raycast([(0, &near)], &frame, DVec3::new(5.0, 0.0, 50.0), d);
        assert!(miss.is_none());
    }
}
