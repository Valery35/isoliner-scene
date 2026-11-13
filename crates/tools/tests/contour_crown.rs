// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! 2.26 Тела по контуру со сводом против прогонов модуля
//! (`fixtures/tool_runs.json`): вершины, грани, объём до 1e-9, площадь,
//! отметки, подъём, сообщения слово в слово, отказ на слое линий.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashMap;

use serde_json::Value as J;
use tool_spec::{Geom, LayerInfo, LineFeature, OutValue, Value, Values, VecReport};

fn runs(tool: &str) -> Vec<J> {
    let p = format!(
        "{}/../../fixtures/tool_runs.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let all: J = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    all["runs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["tool"] == tool)
        .cloned()
        .collect()
}

fn features(v: &J) -> Vec<LineFeature> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|f| LineFeature {
            id: f["id"].as_i64().unwrap(),
            parts: f["parts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|part| {
                    part.as_array()
                        .unwrap()
                        .iter()
                        .map(|p| {
                            [
                                p[0].as_f64().unwrap(),
                                p[1].as_f64().unwrap(),
                                p[2].as_f64().unwrap_or(f64::NAN),
                            ]
                        })
                        .collect()
                })
                .collect(),
            attrs: f["attrs"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, x)| (k.clone(), x.clone()))
                .collect::<HashMap<_, _>>(),
        })
        .collect()
}

fn values(r: &J) -> Values {
    let v = &r["values"];
    let mut out = Values::new();
    out.insert(
        "INPUT".into(),
        Value::Features(features(&r["input"]["INPUT"])),
    );
    if let Some(f) = v["FLOOR_FIELD"].as_str() {
        out.insert("FLOOR_FIELD".into(), Value::Field(f.into()));
    }
    for k in ["FLOOR", "WALL", "STEEP", "RISE_MAX", "CELL"] {
        out.insert(k.into(), Value::Number(v[k].as_f64().unwrap()));
    }
    out.insert(
        "SHAPE".into(),
        Value::Enum(v["SHAPE"].as_u64().unwrap() as usize),
    );
    let m = &v["@INPUT"];
    out.insert(
        "@INPUT".into(),
        Value::Layer(LayerInfo {
            name: m["name"].as_str().unwrap_or("").into(),
            geometry: m["geometry"].as_str().unwrap_or("").into(),
            ..LayerInfo::default()
        }),
    );
    out
}

#[test]
fn contour_crown_runs_like_the_module() {
    let run = tools::run_fn("contour_crown").unwrap();
    let all = runs("contour_crown");
    assert!(all.len() >= 4);
    for (k, r) in all.iter().enumerate() {
        let mut report = VecReport::default();
        let got = run(&values(r), &mut report);
        if let Some(e) = r.get("error") {
            let e2 = got.err().unwrap();
            assert_eq!(e2.0.template, e["template"].as_str().unwrap(), "{k}");
            continue;
        }
        let out = got.unwrap();
        let o = &out["OUTPUT"];
        let want = r["output"]["OUTPUT"]["features"].as_array().unwrap();
        assert_eq!(o.features.len(), want.len(), "{k}");
        let tol = r["tolerance"]["volume_rel"].as_f64().unwrap_or(1e-9);
        for (f, w) in o.features.iter().zip(want) {
            assert_eq!(f.source, w["source"].as_i64(), "{k}");
            let Geom::Mesh(b) = &f.geom else {
                panic!("{k}: не тело")
            };
            assert_eq!(b.verts.len() as u64, w["n_verts"].as_u64().unwrap(), "{k}");
            assert_eq!(b.faces.len() as u64, w["n_faces"].as_u64().unwrap(), "{k}");
            for (i, (a, e)) in f
                .values
                .iter()
                .zip(w["values"].as_array().unwrap())
                .enumerate()
            {
                match a {
                    OutValue::Double(x) => {
                        let y = e.as_f64().unwrap();
                        assert!((x - y).abs() / y.abs().max(1.0) < tol, "{k} [{i}]: {x} {y}");
                    }
                    OutValue::Int(x) => assert_eq!(Some(*x), e.as_i64(), "{k} [{i}]"),
                    other => panic!("{k}: {other:?}"),
                }
            }
        }
        let got_rep: Vec<(String, String)> = report
            .lines
            .iter()
            .map(|(l, m)| (l.to_string(), m.text()))
            .collect();
        let want_rep: Vec<(String, String)> = r["report"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| (x[0].as_str().unwrap().into(), x[1].as_str().unwrap().into()))
            .collect();
        assert_eq!(got_rep, want_rep, "{k}");
    }
}
