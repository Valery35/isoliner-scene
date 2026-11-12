// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Сверка облёта и тел с модулем QGIS (fixtures/).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use scene_core::flight;
use scene_core::mesh;
use scene_format::{Camera, read_package};
use serde_json::Value;

fn load(name: &str) -> Vec<u8> {
    let path = format!("{}/../../fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn json(name: &str) -> Value {
    serde_json::from_slice(&load(name)).unwrap()
}

fn cam(v: &Value) -> Camera {
    serde_json::from_value(v.clone()).unwrap()
}

fn same(a: &Camera, b: &Camera) {
    for k in 0..3 {
        assert!((a.center[k] - b.center[k]).abs() < 1e-9, "{a:?} {b:?}");
    }
    assert!((a.distance - b.distance).abs() < 1e-9 * b.distance.max(1.0));
    assert!((a.azimuth - b.azimuth).abs() < 1e-9, "{a:?} {b:?}");
    assert!((a.elevation - b.elevation).abs() < 1e-9);
    assert!((a.fov - b.fov).abs() < 1e-9);
    assert_eq!(a.ortho, b.ortho);
}

#[test]
fn flight_matches_the_module() {
    let f = json("flight.json");
    for c in f["frame_count"].as_array().unwrap() {
        let got = flight::frame_count(c[0].as_f64().unwrap(), c[1].as_u64().unwrap() as u32);
        assert_eq!(got as u64, c[2].as_u64().unwrap(), "{c}");
    }
    for c in f["turn"].as_array().unwrap() {
        let got = flight::turn(c[0].as_f64().unwrap(), c[1].as_f64().unwrap());
        assert!((got - c[2].as_f64().unwrap()).abs() < 1e-12, "{c}");
    }
    for c in f["smoothstep"].as_array().unwrap() {
        let got = flight::smoothstep(c[0].as_f64().unwrap());
        assert!((got - c[1].as_f64().unwrap()).abs() < 1e-15);
    }
    for c in f["orbit"].as_array().unwrap() {
        let got = flight::orbit(
            &cam(&c["cam"]),
            c["degrees"].as_f64().unwrap(),
            c["seconds"].as_f64().unwrap(),
            c["fps"].as_u64().unwrap() as u32,
        );
        let want = c["frames"].as_array().unwrap();
        assert_eq!(got.len(), want.len());
        for (g, w) in got.iter().zip(want) {
            same(g, &cam(w));
        }
    }
    for c in f["tour"].as_array().unwrap() {
        let views: Vec<Camera> = c["views"].as_array().unwrap().iter().map(cam).collect();
        let got = flight::tour(
            &views,
            c["leg"].as_f64().unwrap(),
            c["pause"].as_f64().unwrap(),
            c["fps"].as_u64().unwrap() as u32,
        );
        let want = c["frames"].as_array().unwrap();
        assert_eq!(got.len(), want.len());
        for (g, w) in got.iter().zip(want) {
            same(g, &cam(w));
        }
    }
    assert!(flight::tour(&[], 1.0, 1.0, 25).is_empty());
}

#[test]
fn bodies_match_the_module() {
    let pkg = read_package(&load("scene_demo.i3dscene")).unwrap();
    let exp = json("scene_demo.expected.json");
    for (part, want) in pkg.parts.iter().zip(exp["parts"].as_array().unwrap()) {
        let vol = mesh::body_volume(part);
        match want.get("volume").and_then(Value::as_f64) {
            // GLB хранит сдвиг в float32: объём сходится до сотой процента
            Some(v) => {
                let got = vol.unwrap_or_else(|| panic!("{} не замкнут", part.name));
                assert!((got - v).abs() / v < 1e-4, "{}: {got} {v}", part.name);
            }
            None => assert!(vol.is_none(), "{}", part.name),
        }
    }
    let (lo, hi) = mesh::bounds(&pkg.parts).unwrap();
    for k in 0..3 {
        assert!((lo[k] - exp["bbox"][0][k].as_f64().unwrap()).abs() < 0.01);
        assert!((hi[k] - exp["bbox"][1][k].as_f64().unwrap()).abs() < 0.01);
    }
}

/// Шкалы вида по разрезу против `axes.section_scales` модуля
/// (fixtures/clip.json, раздел section).
#[test]
fn section_scales_like_the_module() {
    let s = &json("clip.json")["section"];
    let p2 = |v: &Value| [v[0].as_f64().unwrap(), v[1].as_f64().unwrap()];
    let p3 = |v: &Value| [0, 1, 2].map(|k| v[k].as_f64().unwrap());
    let near = |a: [f64; 3], b: [f64; 3]| (0..3).all(|k| (a[k] - b[k]).abs() < 1e-9);
    let got = scene_core::axes::section_scales(
        p2(&s["a"]),
        p2(&s["b"]),
        s["zlo"].as_f64().unwrap(),
        s["zhi"].as_f64().unwrap(),
        s["want"].as_u64().unwrap() as usize,
        None,
        s["vex"].as_f64().unwrap(),
    );
    let want = &s["scales"];
    let segs = want["segs"].as_array().unwrap();
    assert_eq!(got.segs.len(), segs.len());
    for (g, w) in got.segs.iter().zip(segs) {
        assert!(near(g.0, p3(&w[0])) && near(g.1, p3(&w[1])), "{g:?} {w}");
    }
    for (side, list) in [("left", &got.left), ("right", &got.right)] {
        let w = want[side].as_array().unwrap();
        assert_eq!(list.len(), w.len(), "{side}");
        for (g, w) in list.iter().zip(w) {
            assert!(near(g.0, p3(&w[0])), "{side}");
            assert_eq!(g.1, w[1].as_str().unwrap(), "{side}");
        }
    }
    assert!(near(got.arrow_label.0, p3(&want["arrow_label"][0])));
    assert_eq!(got.arrow_label.1, want["arrow_label"][1].as_str().unwrap());
    assert!((got.azimuth - want["azimuth"].as_f64().unwrap()).abs() < 1e-9);
}
