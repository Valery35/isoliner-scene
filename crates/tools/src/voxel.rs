// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Тело из ячеек: видимые грани занятых ячеек, по грани на ячейку.
//! Перенос `voxel.voxel_mesh` модуля без слияния граней: у такого
//! тела каждое ребро принадлежит двум граням, объём считается точно.

use tool_spec::Body;

use crate::fill::Gt;

/// Меш тела из занятости `occ[слой][строка * nx + столбец]`.
/// None - граней больше `max_quads`.
pub fn voxel_mesh(
    occ: &[Vec<bool>],
    ny: usize,
    nx: usize,
    gt: &Gt,
    z0: f64,
    dz: f64,
    max_quads: usize,
) -> Option<Body> {
    let nz = occ.len();
    let at = |k: isize, j: isize, i: isize| -> bool {
        if k < 0 || j < 0 || i < 0 || k >= nz as isize || j >= ny as isize || i >= nx as isize {
            return false;
        }
        occ[k as usize][j as usize * nx + i as usize]
    };
    let xs = |i: usize| gt[0] + i as f64 * gt[1];
    let ys = |j: usize| gt[3] + j as f64 * gt[5];
    let zs = |k: usize| z0 + (k as f64 - 0.5) * dz;
    let mut verts: Vec<[f64; 3]> = Vec::new();
    let mut faces: Vec<[u32; 3]> = Vec::new();
    let mut quads = 0usize;
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                if !at(k as isize, j as isize, i as isize) {
                    continue;
                }
                let (x0, x1) = (xs(i), xs(i + 1));
                let (y0, y1) = (ys(j), ys(j + 1));
                let (z0c, z1c) = (zs(k), zs(k + 1));
                let (ki, ji, ii) = (k as isize, j as isize, i as isize);
                // (сосед, четыре угла, наружу)
                let sides: [(bool, [[f64; 3]; 4], [f64; 3]); 6] = [
                    (
                        at(ki - 1, ji, ii),
                        [[x0, y0, z0c], [x1, y0, z0c], [x1, y1, z0c], [x0, y1, z0c]],
                        [0.0, 0.0, -dz],
                    ),
                    (
                        at(ki + 1, ji, ii),
                        [[x0, y0, z1c], [x1, y0, z1c], [x1, y1, z1c], [x0, y1, z1c]],
                        [0.0, 0.0, dz],
                    ),
                    (
                        at(ki, ji - 1, ii),
                        [[x0, y0, z0c], [x1, y0, z0c], [x1, y0, z1c], [x0, y0, z1c]],
                        [0.0, -gt[5], 0.0],
                    ),
                    (
                        at(ki, ji + 1, ii),
                        [[x0, y1, z0c], [x1, y1, z0c], [x1, y1, z1c], [x0, y1, z1c]],
                        [0.0, gt[5], 0.0],
                    ),
                    (
                        at(ki, ji, ii - 1),
                        [[x0, y0, z0c], [x0, y1, z0c], [x0, y1, z1c], [x0, y0, z1c]],
                        [-gt[1], 0.0, 0.0],
                    ),
                    (
                        at(ki, ji, ii + 1),
                        [[x1, y0, z0c], [x1, y1, z0c], [x1, y1, z1c], [x1, y0, z1c]],
                        [gt[1], 0.0, 0.0],
                    ),
                ];
                for (nb, q, want) in sides {
                    if nb {
                        continue;
                    }
                    quads += 1;
                    if quads > max_quads {
                        return None;
                    }
                    let b = verts.len() as u32;
                    verts.extend_from_slice(&q);
                    for t in [[b, b + 1, b + 2], [b, b + 2, b + 3]] {
                        let (p, r, s) = (
                            q[(t[0] - b) as usize],
                            q[(t[1] - b) as usize],
                            q[(t[2] - b) as usize],
                        );
                        let e1 = [r[0] - p[0], r[1] - p[1], r[2] - p[2]];
                        let e2 = [s[0] - p[0], s[1] - p[1], s[2] - p[2]];
                        let n = [
                            e1[1] * e2[2] - e1[2] * e2[1],
                            e1[2] * e2[0] - e1[0] * e2[2],
                            e1[0] * e2[1] - e1[1] * e2[0],
                        ];
                        let d = n[0] * want[0] + n[1] * want[1] + n[2] * want[2];
                        faces.push(if d < 0.0 { [t[0], t[2], t[1]] } else { t });
                    }
                }
            }
        }
    }
    Some(Body { verts, faces })
}
