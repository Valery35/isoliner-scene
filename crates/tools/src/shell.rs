// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Дыры и защипы оболочки, склейка вершин. Перенос `shell_defects`
//! и `weld` модуля.

use std::collections::HashMap;

/// Склейка совпадающих вершин по округлению: (вершины, грани).
pub fn weld(verts: &[[f64; 3]], faces: &[[u32; 3]], snap: f64) -> (Vec<[f64; 3]>, Vec<[u32; 3]>) {
    if verts.is_empty() {
        return (Vec::new(), faces.to_vec());
    }
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for v in verts {
        for k in 0..3 {
            lo[k] = lo[k].min(v[k]);
            hi[k] = hi[k].max(v[k]);
        }
    }
    let span = (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f64::max);
    let step = snap.max(if span > 0.0 { span / 1.5e6 } else { snap });
    let mut map: HashMap<[i64; 3], u32> = HashMap::new();
    let mut out = Vec::new();
    let mut remap = Vec::with_capacity(verts.len());
    for v in verts {
        let key = [
            ((v[0] - lo[0]) / step).round() as i64,
            ((v[1] - lo[1]) / step).round() as i64,
            ((v[2] - lo[2]) / step).round() as i64,
        ];
        let id = *map.entry(key).or_insert_with(|| {
            out.push(*v);
            (out.len() - 1) as u32
        });
        remap.push(id);
    }
    let faces = faces
        .iter()
        .filter_map(|f| {
            let g = [
                *remap.get(f[0] as usize)?,
                *remap.get(f[1] as usize)?,
                *remap.get(f[2] as usize)?,
            ];
            Some(g)
        })
        .collect();
    (out, faces)
}

/// (рёбер с одной гранью, рёбер с тремя и более) по склеенным вершинам.
/// Дыра - первое, защип - второе.
pub fn defects(verts: &[[f64; 3]], faces: &[[u32; 3]]) -> (usize, usize) {
    if faces.is_empty() {
        return (0, 0);
    }
    let (_v, f) = weld(verts, faces, 1e-6);
    let mut count: HashMap<(u32, u32), u32> = HashMap::new();
    for t in f
        .iter()
        .filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2])
    {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            *count.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    let holes = count.values().filter(|&&n| n == 1).count();
    let pinch = count.values().filter(|&&n| n > 2).count();
    (holes, pinch)
}
