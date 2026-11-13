// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! 2.25 3D-плотность против прогонов модуля (`fixtures/tool_runs.json`):
//! сетка, куб по уровням и строкам, ячейки точками, сообщения слово
//! в слово, отказ длины по слою точек.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

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

fn values(r: &J) -> Values {
    let v = &r["values"];
    let mut out = Values::new();
    let feats: Vec<LineFeature> = r["input"]["INPUT"]
        .as_array()
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
                .collect(),
        })
        .collect();
    out.insert("INPUT".into(), Value::Features(feats));
    out.insert(
        "METRIC".into(),
        Value::Enum(v["METRIC"].as_u64().unwrap() as usize),
    );
    if let Some(a) = v["ATTR"].as_str() {
        out.insert("ATTR".into(), Value::Field(a.into()));
    }
    for k in ["CELL_XY", "CELL_Z"] {
        out.insert(k.into(), Value::Number(v[k].as_f64().unwrap()));
    }
    out.insert(
        "OUTPUT_CELLS".into(),
        Value::Bool(v["OUTPUT_CELLS"].as_bool().unwrap_or(false)),
    );
    out.insert(
        "@INPUT".into(),
        Value::Layer(LayerInfo {
            geometry: v["@INPUT"]["geometry"].as_str().unwrap_or("").into(),
            ..LayerInfo::default()
        }),
    );
    out
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() / b.abs().max(1.0) < 1e-9
}

#[test]
fn density_runs_like_the_module() {
    let run = tools::run_fn("density_3d").unwrap();
    let all = runs("density_3d");
    assert!(all.len() >= 6);
    for (k, r) in all.iter().enumerate() {
        let mut report = VecReport::default();
        let got = run(&values(r), &mut report);
        if let Some(e) = r.get("error") {
            assert_eq!(
                got.err().unwrap().0.template,
                e["template"].as_str().unwrap(),
                "{k}"
            );
            continue;
        }
        let out = got.unwrap();
        // куб
        let w = &r["output"]["OUTPUT"]["grid"];
        let Geom::Grid(g) = &out["OUTPUT"].features[0].geom else {
            panic!("{k}: не куб")
        };
        for (i, t) in w["transform"].as_array().unwrap().iter().enumerate() {
            assert!(
                close(g.transform[i], t.as_f64().unwrap()),
                "{k}: transform {i}"
            );
        }
        let meta = |m: &str| w["meta"][m].as_str().unwrap().parse::<f64>().unwrap();
        assert!(
            close(g.z0, meta("Z0")) && close(g.dz, meta("DZ")),
            "{k}: z0 dz"
        );
        let bands = w["bands"].as_array().unwrap();
        assert_eq!(g.bands.len(), bands.len(), "{k}: уровни");
        for (lvl, (gb, wb)) in g.bands.iter().zip(bands).enumerate() {
            let flat: Vec<&J> = wb
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|row| row.as_array().unwrap())
                .collect();
            assert_eq!(gb.len(), flat.len(), "{k}: уровень {lvl}");
            for (a, b) in gb.iter().zip(flat) {
                match (a, b.as_f64()) {
                    (Some(x), Some(y)) => assert!(close(*x, y), "{k} уровень {lvl}: {x} {y}"),
                    (None, None) => {}
                    _ => panic!("{k} уровень {lvl}: {a:?} {b}"),
                }
            }
        }
        // ячейки точками
        if let Some(wc) = r["output"].get("OUTPUT_CELLS") {
            let cells = &out["OUTPUT_CELLS"];
            let wf = wc["features"].as_array().unwrap();
            assert_eq!(cells.features.len(), wf.len(), "{k}: ячейки");
            for (f, w) in cells.features.iter().zip(wf) {
                let Geom::Point(p) = f.geom else {
                    panic!("{k}")
                };
                for i in 0..3 {
                    assert!(close(p[i], w["xyz"][i].as_f64().unwrap()), "{k}: xyz");
                }
                for (a, e) in f.values.iter().zip(w["values"].as_array().unwrap()) {
                    match a {
                        OutValue::Double(x) => assert!(close(*x, e.as_f64().unwrap()), "{k}"),
                        OutValue::Int(x) => assert_eq!(Some(*x), e.as_i64(), "{k}"),
                        OutValue::Null => assert!(e.is_null(), "{k}"),
                        OutValue::Str(_) => panic!("{k}"),
                    }
                }
            }
        } else {
            assert!(!out.contains_key("OUTPUT_CELLS"), "{k}");
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
