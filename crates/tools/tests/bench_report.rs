// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! 1.09 Отчёт по горизонтам против прогонов модуля
//! (`fixtures/tool_runs.json`): строки уступов и итог до 1e-9, подписи
//! уступов, пустые значения и сообщения слово в слово.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::Value as J;
use tool_spec::{LayerInfo, OutValue, PointFeature, Value, Values, VecReport};

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
    let blocks: Vec<PointFeature> = r["input"]["BLOCKS"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| PointFeature {
            id: b["id"].as_i64().unwrap(),
            xyz: b["xyz"].as_array().map(|p| {
                [
                    p[0].as_f64().unwrap(),
                    p[1].as_f64().unwrap(),
                    p[2].as_f64().unwrap_or(f64::NAN),
                ]
            }),
            attrs: b["attrs"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, x)| (k.clone(), x.clone()))
                .collect(),
        })
        .collect();
    out.insert("BLOCKS".into(), Value::Points(blocks));
    for k in ["GRADE", "VOL", "DENS", "ZTOP", "ZBOT"] {
        if let Some(s) = v[k].as_str() {
            out.insert(k.into(), Value::Field(s.into()));
        }
    }
    for k in [
        "CUTOFF",
        "HEIGHT",
        "BASE",
        "DENSITY",
        "PRICE",
        "RECOVERY",
        "MINE_COST",
        "PROC_COST",
    ] {
        if let Some(x) = v[k].as_f64() {
            out.insert(k.into(), Value::Number(x));
        }
    }
    out.insert(
        "UNIT".into(),
        Value::Enum(v["UNIT"].as_u64().unwrap() as usize),
    );
    let m = &v["@BLOCKS"];
    out.insert(
        "@BLOCKS".into(),
        Value::Layer(LayerInfo {
            name: m["name"].as_str().unwrap_or("").into(),
            geometry: m["geometry"].as_str().unwrap_or("").into(),
            hasz: m["hasz"].as_bool().unwrap_or(false),
            fields: m["fields"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| f.as_str().unwrap().into())
                .collect(),
            numeric: Vec::new(),
        }),
    );
    out
}

#[test]
fn bench_report_runs_like_the_module() {
    let run = tools::run_fn("bench_report").unwrap();
    let all = runs("bench_report");
    assert!(all.len() >= 4);
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
        let o = &out["OUTPUT"];
        assert_eq!(o.geometry, tool_spec::OutGeometry::None);
        let want = r["output"]["OUTPUT"]["features"].as_array().unwrap();
        assert_eq!(o.features.len(), want.len(), "{k}");
        for (row, (f, w)) in o.features.iter().zip(want).enumerate() {
            for (i, (a, e)) in f
                .values
                .iter()
                .zip(w["values"].as_array().unwrap())
                .enumerate()
            {
                let tag = format!("{k} строка {row} [{i}]");
                match a {
                    OutValue::Double(x) => {
                        let y = e.as_f64().unwrap();
                        assert!((x - y).abs() / y.abs().max(1.0) < 1e-9, "{tag}: {x} {y}");
                    }
                    OutValue::Int(x) => assert_eq!(Some(*x), e.as_i64(), "{tag}"),
                    OutValue::Str(s) => assert_eq!(Some(s.as_str()), e.as_str(), "{tag}"),
                    OutValue::Null => assert!(e.is_null(), "{tag}"),
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
