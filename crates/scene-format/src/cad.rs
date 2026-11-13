// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Оболочки в STL и OBJ - форматы, которые понимает CAD, как
//! `cadmesh.py` модуля.
//!
//! Пишутся только треугольники: линии и точки CAD-оболочкой не бывают.
//! STL двоичный, частей в нём нет, все оболочки сливаются в одну. OBJ
//! текстовый, у каждой части своя группа с именем. Отметки настоящие:
//! преувеличение - способ смотреть, а не свойство модели. Координаты
//! STL - float32, как у модуля и как велит формат.

use std::fmt::Write as _;

use crate::{Indices, Part};

/// Треугольники части или None, если у части их нет.
fn tris(part: &Part) -> Option<&[[u32; 3]]> {
    match &part.indices {
        Indices::Triangles(t) if !t.is_empty() && !part.verts.is_empty() => Some(t),
        _ => None,
    }
}

/// Нормаль грани. Вырожденной грани - вверх: с нулевой нормалью CAD
/// спорит. Номер за краем вершин - тоже вверх, без паники.
fn normal(v: &[[f64; 3]], t: [u32; 3]) -> [f64; 3] {
    let p = |i: u32| v.get(i as usize).copied().unwrap_or([0.0; 3]);
    let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let w = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        u[1] * w[2] - u[2] * w[1],
        u[2] * w[0] - u[0] * w[2],
        u[0] * w[1] - u[1] * w[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len < 1e-30 || !len.is_finite() {
        [0.0, 0.0, 1.0]
    } else {
        [n[0] / len, n[1] / len, n[2] / len]
    }
}

/// Двоичный STL из частей, как `build_stl` модуля. Писать нечего -
/// None.
pub fn stl(parts: &[Part], name: &str) -> Option<Vec<u8>> {
    let total: usize = parts.iter().filter_map(tris).map(<[_]>::len).sum();
    let count = u32::try_from(total).ok().filter(|n| *n > 0)?;
    let mut out = Vec::with_capacity(84 + total * 50);
    // заголовок - 80 байт ASCII, не-ASCII заменяется знаком вопроса
    let mut head: Vec<u8> = name
        .chars()
        .map(|c| if c.is_ascii() { c as u8 } else { b'?' })
        .take(79)
        .collect();
    head.resize(80, b' ');
    out.extend_from_slice(&head);
    out.extend_from_slice(&count.to_le_bytes());
    let f = |out: &mut Vec<u8>, x: f64| out.extend_from_slice(&(x as f32).to_le_bytes());
    for part in parts {
        let Some(t) = tris(part) else { continue };
        for tri in t {
            for x in normal(&part.verts, *tri) {
                f(&mut out, x);
            }
            for i in tri {
                let p = part.verts.get(*i as usize).copied().unwrap_or([0.0; 3]);
                for x in p {
                    f(&mut out, x);
                }
            }
            out.extend_from_slice(&0u16.to_le_bytes());
        }
    }
    Some(out)
}

/// Текстовый OBJ из частей, как `build_obj` модуля: группа на часть,
/// вершины с единицы, номера каждой следующей части сдвигаются.
/// Писать нечего - None.
pub fn obj(parts: &[Part], name: &str) -> Option<String> {
    let mut s = String::new();
    let _ = writeln!(s, "# {name}");
    let mut base = 0usize;
    let mut wrote = 0usize;
    for (k, part) in parts.iter().enumerate() {
        let Some(t) = tris(part) else { continue };
        let group = if part.name.is_empty() {
            format!("part_{}", k + 1)
        } else {
            part.name.clone()
        };
        let _ = writeln!(s, "g {group}");
        for p in &part.verts {
            let _ = writeln!(s, "v {:.6} {:.6} {:.6}", p[0], p[1], p[2]);
        }
        for tri in t {
            let _ = writeln!(
                s,
                "f {} {} {}",
                base + tri[0] as usize + 1,
                base + tri[1] as usize + 1,
                base + tri[2] as usize + 1
            );
        }
        base += part.verts.len();
        wrote += t.len();
    }
    (wrote > 0).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tri(name: &str, z: f64) -> Part {
        Part {
            name: name.into(),
            verts: vec![
                [500000.0, 6200000.0, z],
                [500010.0, 6200000.0, z],
                [500000.0, 6200010.0, z],
            ],
            indices: Indices::Triangles(vec![[0, 1, 2]]),
            colors: None,
            sizes: None,
        }
    }

    fn line() -> Part {
        Part {
            name: "линия".into(),
            verts: vec![[0.0; 3], [1.0; 3]],
            indices: Indices::Lines(vec![[0, 1]]),
            colors: None,
            sizes: None,
        }
    }

    /// Двоичный STL: заголовок 80 байт, число граней, на грань 50 байт,
    /// нормаль вверх, настоящие координаты, линии не пишутся.
    #[test]
    fn stl_like_the_module() {
        let parts = [tri("а", -100.0), line(), tri("б", -50.0)];
        let b = stl(&parts, "Сцена 1").unwrap();
        assert_eq!(b.len(), 84 + 2 * 50);
        assert_eq!(&b[..2], b"??");
        assert_eq!(u32::from_le_bytes([b[80], b[81], b[82], b[83]]), 2);
        let f = |o: usize| f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        assert_eq!((f(84), f(88), f(92)), (0.0, 0.0, 1.0));
        assert_eq!((f(96), f(100), f(104)), (500000.0, 6200000.0, -100.0));
        assert!(stl(&[line()], "x").is_none());
    }

    /// Демо-сцена модуля: STL и OBJ из тел пакета, GLB той же сцены
    /// читается обратно с теми же настоящими вершинами.
    #[test]
    fn demo_scene_exports() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/scene_demo.i3dscene"
        );
        let pkg = crate::read_package(&std::fs::read(path).unwrap()).unwrap();
        let n: usize = pkg.parts.iter().filter_map(tris).map(<[_]>::len).sum();
        assert!(n > 0);
        assert_eq!(stl(&pkg.parts, &pkg.meta.name).unwrap().len(), 84 + 50 * n);
        let o = obj(&pkg.parts, &pkg.meta.name).unwrap();
        assert_eq!(o.lines().filter(|l| l.starts_with("f ")).count(), n);
        let glb = crate::glb::write(&pkg.parts, &pkg.meta.name, &pkg.georef);
        let (back, _) = crate::glb::read(&glb).unwrap();
        assert_eq!(back.len(), pkg.parts.len());
        for (a, b) in back.iter().zip(&pkg.parts) {
            assert_eq!(a.verts.len(), b.verts.len());
            for (p, q) in a.verts.iter().zip(&b.verts) {
                assert!((0..3).all(|k| (p[k] - q[k]).abs() < 1e-3), "{p:?} {q:?}");
            }
        }
    }

    /// OBJ: группа на часть, номера второй части сдвинуты на вершины
    /// первой, линии не пишутся.
    #[test]
    fn obj_like_the_module() {
        let parts = [tri("а", -100.0), line(), tri("", -50.0)];
        let s = obj(&parts, "Сцена").unwrap();
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines[0], "# Сцена");
        assert_eq!(lines[1], "g а");
        assert_eq!(lines[2], "v 500000.000000 6200000.000000 -100.000000");
        assert_eq!(lines[5], "f 1 2 3");
        assert_eq!(lines[6], "g part_3");
        assert_eq!(lines[10], "f 4 5 6");
        assert!(obj(&[line()], "x").is_none());
    }
}
