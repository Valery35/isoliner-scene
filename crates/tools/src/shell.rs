// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Дыры и защипы оболочки, склейка вершин. Перенос `shell_defects`
//! и `weld` модуля.

use std::collections::HashMap;

/// Склейка совпадающих вершин по округлению: (вершины, грани).
///
/// Как `iso3d.weld` модуля: вершины склеенного меша идут по возрастанию
/// ключа округления (`np.unique`), у каждой - первая из совпавших.
/// От этого порядка зависит нумерация тел в `split_bodies`.
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
    // np.round - к чётному
    let keys: Vec<[i64; 3]> = verts
        .iter()
        .map(|v| {
            [
                ((v[0] - lo[0]) / step).round_ties_even() as i64,
                ((v[1] - lo[1]) / step).round_ties_even() as i64,
                ((v[2] - lo[2]) / step).round_ties_even() as i64,
            ]
        })
        .collect();
    let mut first: HashMap<[i64; 3], usize> = HashMap::new();
    for (i, k) in keys.iter().enumerate() {
        first.entry(*k).or_insert(i);
    }
    let mut uniq: Vec<[i64; 3]> = first.keys().copied().collect();
    uniq.sort_unstable();
    let id: HashMap<[i64; 3], u32> = uniq
        .iter()
        .enumerate()
        .map(|(n, k)| (*k, n as u32))
        .collect();
    let out: Vec<[f64; 3]> = uniq.iter().map(|k| verts[first[k]]).collect();
    let remap: Vec<u32> = keys.iter().map(|k| id[k]).collect();
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

/// Связный кусок меша: вершины и грани.
pub type Piece = (Vec<[f64; 3]>, Vec<[u32; 3]>);

/// Номер связного куска на каждую грань, как `cleanup._labels` модуля:
/// тот же союз вершин, тот же корень куска.
fn labels(n: usize, faces: &[[u32; 3]]) -> Vec<usize> {
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut [usize], mut a: usize) -> usize {
        while parent[a] != a {
            parent[a] = parent[parent[a]];
            a = parent[a];
        }
        a
    }
    for t in faces {
        let ra = find(&mut parent, t[0] as usize);
        let rb = find(&mut parent, t[1] as usize);
        let rc = find(&mut parent, t[2] as usize);
        parent[rb] = ra;
        parent[rc] = ra;
    }
    faces
        .iter()
        .map(|t| find(&mut parent, t[0] as usize))
        .collect()
}

/// Разбить меш на связные тела, как `cleanup.split_bodies` модуля:
/// склейка, куски по возрастанию корня, потом от большего числа граней
/// к меньшему.
pub fn split_bodies(verts: &[[f64; 3]], faces: &[[u32; 3]]) -> Vec<Piece> {
    if faces.is_empty() {
        return Vec::new();
    }
    let (v, f) = weld(verts, faces, 1e-6);
    if f.iter().flatten().any(|&i| i as usize >= v.len()) {
        return Vec::new();
    }
    let lab = labels(v.len(), &f);
    let mut keys: Vec<usize> = lab.clone();
    keys.sort_unstable();
    keys.dedup();
    let mut out = Vec::with_capacity(keys.len());
    for key in keys {
        let fs: Vec<[u32; 3]> = f
            .iter()
            .zip(&lab)
            .filter(|(_, l)| **l == key)
            .map(|(t, _)| *t)
            .collect();
        let mut used: Vec<u32> = fs.iter().flatten().copied().collect();
        used.sort_unstable();
        used.dedup();
        let remap: HashMap<u32, u32> = used
            .iter()
            .enumerate()
            .map(|(n, &i)| (i, n as u32))
            .collect();
        let bv: Vec<[f64; 3]> = used.iter().map(|&i| v[i as usize]).collect();
        let bf: Vec<[u32; 3]> = fs.iter().map(|t| t.map(|i| remap[&i])).collect();
        out.push((bv, bf));
    }
    // сортировка устойчивая, как sort в Python
    out.sort_by_key(|(_, f)| std::cmp::Reverse(f.len()));
    out
}

/// Объём замкнутой оболочки от середины вершин, по модулю, как
/// `cleanup.mesh_volume` модуля без согласования обхода: у оболочек
/// из ячеек и у тел окна обход согласован.
pub fn mesh_volume(verts: &[[f64; 3]], faces: &[[u32; 3]]) -> f64 {
    if verts.is_empty() {
        return 0.0;
    }
    // от середины: в настоящих координатах слагаемые огромны,
    // и сумма теряла бы значащие цифры
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
    for t in faces {
        let (Some(a), Some(b), Some(d)) = (p(t[0]), p(t[1]), p(t[2])) else {
            continue;
        };
        sum += a[0] * (b[1] * d[2] - b[2] * d[1])
            + a[1] * (b[2] * d[0] - b[0] * d[2])
            + a[2] * (b[0] * d[1] - b[1] * d[0]);
    }
    (sum / 6.0).abs()
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
