// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Охват, объём и замкнутость тел сцены.

use std::collections::HashMap;

use scene_format::{Indices, Part};

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

/// Сводка по части: замкнута ли и её объём, если замкнута.
pub fn body_volume(part: &Part) -> Option<f64> {
    match &part.indices {
        Indices::Triangles(t) if !t.is_empty() && open_edges(t) == 0 => {
            Some(signed_volume(&part.verts, t).abs())
        }
        _ => None,
    }
}
