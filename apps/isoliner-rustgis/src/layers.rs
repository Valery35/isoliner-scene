// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Слои проекта RustGIS - в части сцены и пакет `.i3dscene`.
//!
//! RustGIS отдаёт объекты слоя GeoJSON в градусах WGS 84
//! (`host.features`), высота - третья координата, если она есть. Сцене
//! нужны метры: градусы идут в зону UTM по середине всех слоёв
//! (`scene_core::geo`). Точки - точками, линии и контуры полигонов -
//! линиями, полигоны-треугольники с высотой (так модуль пишет тела,
//! `cadmesh.mesh_wkb`) - телом. Высоты нет - ноль.

use serde_json::Value as J;

use scene_core::geo::Utm;
use scene_format::{Crs, Indices, Package, Part, SceneMeta};

/// Цвета слоёв по порядку: те же доли, что у палитры модуля.
const PALETTE: [[f32; 4]; 6] = [
    [0.80, 0.42, 0.20, 1.0],
    [0.55, 0.62, 0.25, 1.0],
    [0.62, 0.70, 0.92, 1.0],
    [0.45, 0.62, 0.72, 1.0],
    [0.76, 0.24, 0.22, 1.0],
    [0.88, 0.72, 0.30, 1.0],
];

/// Точка GeoJSON: долгота, широта, высота (нет - ноль).
fn point(v: &J) -> Option<[f64; 3]> {
    let a = v.as_array()?;
    let lon = a.first()?.as_f64()?;
    let lat = a.get(1)?.as_f64()?;
    let z = a.get(2).and_then(J::as_f64).unwrap_or(0.0);
    (lon.is_finite() && lat.is_finite() && z.is_finite()).then_some([lon, lat, z])
}

fn ring(v: &J) -> Vec<[f64; 3]> {
    v.as_array()
        .map(|a| a.iter().filter_map(point).collect())
        .unwrap_or_default()
}

/// Середина охвата всех слоёв в градусах: по ней выбирается зона UTM.
pub fn middle(layers: &[(String, J)]) -> Option<[f64; 2]> {
    let mut lo = [f64::MAX; 2];
    let mut hi = [f64::MIN; 2];
    let mut any = false;
    let mut visit = |p: [f64; 3]| {
        any = true;
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    };
    for (_, fc) in layers {
        for f in fc["features"].as_array().into_iter().flatten() {
            walk(&f["geometry"], &mut visit);
        }
    }
    any.then_some([(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0])
}

/// Все точки геометрии.
fn walk(g: &J, visit: &mut dyn FnMut([f64; 3])) {
    match &g["coordinates"] {
        J::Array(a) => {
            fn deep(v: &J, visit: &mut dyn FnMut([f64; 3])) {
                if let Some(p) = point(v) {
                    visit(p);
                } else if let Some(a) = v.as_array() {
                    for x in a {
                        deep(x, visit);
                    }
                }
            }
            for x in a {
                deep(x, visit);
            }
            if let Some(p) = point(&g["coordinates"]) {
                visit(p);
            }
        }
        _ => {
            for sub in g["geometries"].as_array().into_iter().flatten() {
                walk(sub, visit);
            }
        }
    }
}

/// Накопитель частей одного слоя: точки, линии, треугольники.
#[derive(Default)]
struct Acc {
    pts: Vec<[f64; 3]>,
    lines: (Vec<[f64; 3]>, Vec<[u32; 2]>),
    mesh: (Vec<[f64; 3]>, Vec<[u32; 3]>),
}

impl Acc {
    fn line(&mut self, pts: &[[f64; 3]], closed: bool) {
        if pts.len() < 2 {
            return;
        }
        let (v, idx) = &mut self.lines;
        let base = v.len() as u32;
        v.extend_from_slice(pts);
        let n = pts.len() as u32;
        for i in 0..n - 1 {
            idx.push([base + i, base + i + 1]);
        }
        if closed && n > 2 && pts.first() != pts.last() {
            idx.push([base + n - 1, base]);
        }
    }

    /// Полигон: треугольник с высотой - грань тела, иначе контуры.
    fn polygon(&mut self, rings: &[Vec<[f64; 3]>]) {
        if let [outer] = rings {
            let tri = outer.len() == 4 && outer.first() == outer.last();
            let has_z = outer.iter().any(|p| p[2] != 0.0);
            if tri && has_z {
                let (v, idx) = &mut self.mesh;
                let base = v.len() as u32;
                v.extend_from_slice(&outer[..3]);
                idx.push([base, base + 1, base + 2]);
                return;
            }
        }
        for r in rings {
            self.line(r, true);
        }
    }

    fn geometry(&mut self, g: &J) {
        let c = &g["coordinates"];
        let rings = |v: &J| -> Vec<Vec<[f64; 3]>> {
            v.as_array()
                .map(|a| a.iter().map(ring).collect())
                .unwrap_or_default()
        };
        match g["type"].as_str().unwrap_or("") {
            "Point" => self.pts.extend(point(c)),
            "MultiPoint" => self.pts.extend(ring(c)),
            "LineString" => self.line(&ring(c), false),
            "MultiLineString" => {
                for l in rings(c) {
                    self.line(&l, false);
                }
            }
            "Polygon" => self.polygon(&rings(c)),
            "MultiPolygon" => {
                for p in c.as_array().into_iter().flatten() {
                    self.polygon(&rings(p));
                }
            }
            "GeometryCollection" => {
                for sub in g["geometries"].as_array().into_iter().flatten() {
                    self.geometry(sub);
                }
            }
            _ => {}
        }
    }
}

/// Слои проекта в части сцены: имя слоя и его GeoJSON. Каждый слой -
/// до трёх частей (тело, линии, точки) своего цвета, метры зоны `utm`.
pub fn parts(layers: &[(String, J)], utm: Utm) -> Vec<Part> {
    parts_with(layers, &|x, y| utm.forward(x, y))
}

/// Части сцены с переводом координат слоя `to_m` в метры сцены.
fn parts_with(layers: &[(String, J)], to_m: &dyn Fn(f64, f64) -> [f64; 2]) -> Vec<Part> {
    let mut out = Vec::new();
    for (k, (name, fc)) in layers.iter().enumerate() {
        let mut acc = Acc::default();
        for f in fc["features"].as_array().into_iter().flatten() {
            acc.geometry(&f["geometry"]);
        }
        let color = PALETTE[k % PALETTE.len()];
        let to_m = |v: Vec<[f64; 3]>| -> Vec<[f64; 3]> {
            v.into_iter()
                .map(|p| {
                    let [x, y] = to_m(p[0], p[1]);
                    [x, y, p[2]]
                })
                .collect()
        };
        let mut push = |verts: Vec<[f64; 3]>, indices: Indices| {
            if verts.is_empty() || indices.is_empty() {
                return;
            }
            let n = verts.len();
            out.push(Part {
                name: name.clone(),
                verts: to_m(verts),
                indices,
                colors: Some(vec![color; n]),
                sizes: None,
            });
        };
        push(acc.mesh.0, Indices::Triangles(acc.mesh.1));
        push(acc.lines.0, Indices::Lines(acc.lines.1));
        let n = acc.pts.len() as u32;
        push(acc.pts, Indices::Points((0..n).collect()));
    }
    out
}

/// Пакет сцены из слоёв проекта в градусах: метры зоны UTM по середине.
/// Слоёв с объектами нет - None.
pub fn package(layers: &[(String, J)], title: &str) -> Option<Package> {
    let [lon, lat] = middle(layers)?;
    let utm = Utm::at(lon, lat);
    wrap(parts(layers, utm), title, &format!("EPSG:{}", utm.epsg()))
}

/// Пакет сцены из слоёв, уже лежащих в метрах системы `authid` (выгрузка
/// RustGIS в системе слоя).
pub fn package_native(layers: &[(String, J)], title: &str, authid: &str) -> Option<Package> {
    wrap(parts_with(layers, &|x, y| [x, y]), title, authid)
}

fn wrap(parts: Vec<Part>, title: &str, authid: &str) -> Option<Package> {
    if parts.is_empty() {
        return None;
    }
    let meta = SceneMeta {
        module: "isoliner-rustgis".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        name: title.to_string(),
        crs: Crs {
            authid: authid.to_string(),
            wkt: String::new(),
        },
        ..SceneMeta::default()
    };
    Some(Package {
        meta,
        parts,
        georef: Default::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Слой с телом-треугольником, линией и точкой: три части, метры
    /// зоны UTM, высоты сохраняются; пакет пишется и читается.
    #[test]
    fn project_layers_to_scene() {
        let fc = json!({"type": "FeatureCollection", "features": [
            {"type": "Feature", "geometry": {"type": "MultiPolygon", "coordinates": [
                [[[57.0, 56.0, -100.0], [57.001, 56.0, -100.0], [57.0, 56.001, -90.0], [57.0, 56.0, -100.0]]]
            ]}},
            {"type": "Feature", "geometry": {"type": "LineString", "coordinates": [[57.0, 56.0], [57.002, 56.002]]}},
            {"type": "Feature", "geometry": {"type": "Point", "coordinates": [57.001, 56.001, 5.0]}}
        ]});
        let layers = vec![("Тела".to_string(), fc)];
        let utm = Utm::at(57.0, 56.0);
        assert_eq!(utm.epsg(), 32640);
        let p = parts(&layers, utm);
        assert_eq!(p.len(), 3);
        assert!(matches!(p[0].indices, Indices::Triangles(ref t) if t.len() == 1));
        assert!(matches!(p[1].indices, Indices::Lines(ref l) if l.len() == 1));
        assert!(matches!(p[2].indices, Indices::Points(ref x) if x.len() == 1));
        // 57° - осевой меридиан зоны 40: восток 500 000
        assert!((p[0].verts[0][0] - 500_000.0).abs() < 1e-6);
        assert_eq!(p[0].verts[2][2], -90.0);
        let pkg = package(&layers, "Проект").unwrap();
        assert_eq!(pkg.meta.crs.authid, "EPSG:32640");
        let bytes = scene_format::write_package(&pkg).unwrap();
        let back = scene_format::read_package(&bytes).unwrap();
        assert_eq!(back.parts.len(), 3);
        assert!(package(&[("пусто".into(), json!({"features": []}))], "x").is_none());
    }
}
