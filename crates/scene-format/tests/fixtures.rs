// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Сверка с пакетом, записанным модулем QGIS (fixtures/).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use scene_format::{Indices, PartKind, read_package, write_package};
use serde_json::Value;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/../../fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn expected() -> Value {
    serde_json::from_slice(&fixture("scene_demo.expected.json")).unwrap()
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn reads_the_module_package() {
    let pkg = read_package(&fixture("scene_demo.i3dscene")).unwrap();
    let exp = expected();
    assert_eq!(pkg.meta.name, exp["name"].as_str().unwrap());
    assert_eq!(pkg.meta.crs.authid, "EPSG:32640");
    assert_eq!(pkg.georef.crs, "EPSG:32640");
    assert_eq!(pkg.meta.vex, exp["vex"].as_f64().unwrap());
    for k in 0..3 {
        assert_eq!(pkg.meta.origin[k], exp["origin"][k].as_f64().unwrap());
        assert_eq!(pkg.georef.origin[k], pkg.meta.origin[k]);
    }
    let cam = pkg.meta.camera.unwrap();
    for k in 0..3 {
        assert!(close(
            cam.center[k],
            exp["camera"]["center"][k].as_f64().unwrap(),
            1e-9
        ));
    }
    assert_eq!(pkg.meta.views.len(), exp["views"].as_array().unwrap().len());
    let parts = exp["parts"].as_array().unwrap();
    assert_eq!(pkg.parts.len(), parts.len());
    assert_eq!(pkg.meta.parts.len(), parts.len());
    for ((got, info), want) in pkg.parts.iter().zip(&pkg.meta.parts).zip(parts) {
        assert_eq!(got.name, want["name"].as_str().unwrap());
        assert_eq!(info.name, got.name);
        assert_eq!(got.verts.len() as u64, want["verts"].as_u64().unwrap());
        assert_eq!(got.indices.len() as u64, want["elements"].as_u64().unwrap());
        let lines = want["kind"] == "lines";
        assert_eq!(matches!(got.indices, Indices::Lines(_)), lines);
        assert_eq!(info.kind == PartKind::Lines, lines);
        // настоящие координаты после float32 и сдвига: сантиметр
        for k in 0..3 {
            let lo = got.verts.iter().map(|v| v[k]).fold(f64::INFINITY, f64::min);
            let hi = got
                .verts
                .iter()
                .map(|v| v[k])
                .fold(f64::NEG_INFINITY, f64::max);
            assert!(
                close(lo, want["bbox"][0][k].as_f64().unwrap(), 0.01),
                "{}",
                got.name
            );
            assert!(
                close(hi, want["bbox"][1][k].as_f64().unwrap(), 0.01),
                "{}",
                got.name
            );
        }
        let c = got.colors.as_ref().unwrap()[0];
        for k in 0..4 {
            assert!(close(
                f64::from(c[k]),
                want["first_color"][k].as_f64().unwrap(),
                1e-6
            ));
        }
    }
    assert!(pkg.meta.parts.iter().any(|p| p.decor));
    assert_eq!(pkg.meta.parts[0].layer.as_deref(), Some("drifts_1"));
    assert!(pkg.meta.state.get("checked").is_some());
}

#[test]
fn write_and_read_back_keeps_everything() {
    let pkg = read_package(&fixture("scene_demo.i3dscene")).unwrap();
    let bytes = write_package(&pkg).unwrap();
    let back = read_package(&bytes).unwrap();
    assert_eq!(back.meta.origin, pkg.meta.origin);
    assert_eq!(back.meta.parts, pkg.meta.parts);
    assert_eq!(back.meta.state, pkg.meta.state);
    assert_eq!(back.meta.views, pkg.meta.views);
    for (a, b) in back.parts.iter().zip(&pkg.parts) {
        assert_eq!(a.indices, b.indices);
        assert_eq!(a.colors, b.colors);
        for (p, q) in a.verts.iter().zip(&b.verts) {
            for k in 0..3 {
                assert!(close(p[k], q[k], 1e-9));
            }
        }
    }
}

#[test]
fn hostile_input_is_refused_not_panicking() {
    let good = fixture("scene_demo.i3dscene");
    assert!(read_package(&good[..good.len() / 2]).is_err());
    assert!(read_package(b"not a zip").is_err());
    let pkg = read_package(&good).unwrap();
    let glb = scene_format::glb::write(&pkg.parts, "x", &pkg.georef);
    for cut in [0, 4, 12, 30, glb.len() / 2, glb.len() - 3] {
        assert!(scene_format::glb::read(&glb[..cut]).is_err(), "cut {cut}");
    }
    // номер вершины за краем
    let mut bad = pkg.parts.clone();
    if let Indices::Lines(l) = &mut bad[4].indices {
        l[0][1] = 9999;
    }
    let glb = scene_format::glb::write(&bad, "x", &pkg.georef);
    assert!(scene_format::glb::read(&glb).is_err());
}

#[test]
fn newer_format_is_refused() {
    let mut pkg = read_package(&fixture("scene_demo.i3dscene")).unwrap();
    pkg.meta.format = 99;
    let bytes = write_package(&pkg).unwrap();
    // запись ставит свой формат
    assert!(read_package(&bytes).is_ok());
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default();
    zw.start_file("scene.json", opts).unwrap();
    std::io::Write::write_all(&mut zw, br#"{"format": 2}"#).unwrap();
    let data = zw.finish().unwrap().into_inner();
    assert!(matches!(
        read_package(&data),
        Err(scene_format::Error::TooNew(2))
    ));
}
