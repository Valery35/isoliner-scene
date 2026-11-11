// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Сверка расчётов с прогонами модуля QGIS (fixtures/).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashMap;

use serde_json::Value as J;
use tool_spec::{Body, LineFeature, OutValue, Value, Values, VecReport};
use tools::{run_fn, shell, sweep};

fn json(name: &str) -> J {
    let path = format!("{}/../../fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_slice(&std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1.0)
}

fn pts(v: &J) -> Vec<[f64; 3]> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let c = |k: usize| p[k].as_f64().unwrap_or(f64::NAN);
            [c(0), c(1), c(2)]
        })
        .collect()
}

#[test]
fn sweep_matches_the_module() {
    let data = json("sweep.json");
    assert_eq!(data["max_turn"].as_f64().unwrap(), sweep::MAX_TURN);
    let cases = data["cases"].as_array().unwrap();
    assert!(cases.len() > 50);
    for c in cases {
        let p = &c["profile"];
        let prof = sweep::profile(
            p[0].as_str().unwrap(),
            p[1].as_f64().unwrap(),
            p[2].as_f64(),
            p[3].as_f64(),
            p[4].as_i64().unwrap(),
        )
        .unwrap();
        let want_sec = c["section"].as_array().unwrap();
        assert_eq!(prof.len(), want_sec.len());
        for (a, b) in prof.iter().zip(want_sec) {
            assert!((a[0] - b[0].as_f64().unwrap()).abs() < 1e-12);
            assert!((a[1] - b[1].as_f64().unwrap()).abs() < 1e-12);
        }
        assert!(rel(sweep::section_area(&prof), c["area"].as_f64().unwrap()) < 1e-12);
        let axis = pts(&c["axis"]);
        let sw = sweep::sweep(&axis, &prof, c["anchor"] == "centre").unwrap();
        let tag = format!("{} {} {}", p, c["axis_name"], c["anchor"]);
        assert_eq!(
            sw.verts.len() as u64,
            c["n_verts"].as_u64().unwrap(),
            "{tag}"
        );
        assert_eq!(
            sw.faces.len() as u64,
            c["n_faces"].as_u64().unwrap(),
            "{tag}"
        );
        assert!(
            rel(sw.length, c["length"].as_f64().unwrap()) < 1e-12,
            "{tag}"
        );
        assert!(
            (sw.max_turn - c["max_turn"].as_f64().unwrap()).abs() < 1e-9,
            "{tag}"
        );
        let vol = sweep::signed_volume(&sw.verts, &sw.faces);
        assert!(
            rel(vol, c["volume"].as_f64().unwrap()) < 1e-9,
            "{tag}: {vol}"
        );
        if let Some(vs) = c.get("verts") {
            for (a, b) in sw.verts.iter().zip(pts(vs)) {
                for k in 0..3 {
                    assert!((a[k] - b[k]).abs() < 1e-9, "{tag}");
                }
            }
            let fs = c["faces"].as_array().unwrap();
            for (a, b) in sw.faces.iter().zip(fs) {
                for k in 0..3 {
                    assert_eq!(u64::from(a[k]), b[k].as_u64().unwrap(), "{tag}");
                }
            }
        }
        if c["axis_name"] != "sharp" {
            assert_eq!(shell::defects(&sw.verts, &sw.faces).0, 0, "{tag}");
        }
    }
}

fn values_of(v: &J) -> Values {
    let mut out = Values::new();
    for (k, x) in v.as_object().unwrap().iter().filter(|(_, x)| !x.is_null()) {
        let val = match k.as_str() {
            "PROFILE" | "ANCHOR" | "MODE" | "UNIT" => Value::Enum(x.as_u64().unwrap() as usize),
            "SEGMENTS" => Value::Integer(x.as_i64().unwrap()),
            _ if x.is_string() => Value::Field(x.as_str().unwrap().to_string()),
            _ => Value::Number(x.as_f64().unwrap()),
        };
        out.insert(k.clone(), val);
    }
    out
}

fn report_texts(r: &VecReport) -> Vec<(String, String)> {
    r.lines
        .iter()
        .map(|(k, m)| (k.to_string(), m.text()))
        .collect()
}

#[test]
fn sweep_body_runs_like_the_module() {
    let runs = json("tool_runs.json");
    let run = run_fn("sweep_body").unwrap();
    let mut n = 0;
    for r in runs["runs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["tool"] == "sweep_body")
    {
        let mut vals = values_of(&r["values"]);
        let lines: Vec<LineFeature> = r["input"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| LineFeature {
                id: l["id"].as_i64().unwrap(),
                parts: l["parts"].as_array().unwrap().iter().map(pts).collect(),
                attrs: l["attrs"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<HashMap<_, _>>(),
            })
            .collect();
        vals.insert("INPUT".into(), Value::Lines(lines));
        let mut rep = VecReport::default();
        let out = run(&vals, &mut rep).unwrap();
        let o = &out["OUTPUT"];
        let want = r["output"]["features"].as_array().unwrap();
        assert_eq!(o.features.len(), want.len());
        for (g, w) in o.features.iter().zip(want) {
            assert_eq!(g.source, w["source"].as_i64());
            assert_eq!(
                g.body().unwrap().verts.len() as u64,
                w["n_verts"].as_u64().unwrap()
            );
            assert_eq!(
                g.body().unwrap().faces.len() as u64,
                w["n_faces"].as_u64().unwrap()
            );
            for (a, b) in g.values.iter().zip(w["values"].as_array().unwrap()) {
                match a {
                    OutValue::Double(x) => assert!(rel(*x, b.as_f64().unwrap()) < 1e-9),
                    OutValue::Int(i) => assert_eq!(*i, b.as_i64().unwrap()),
                    OutValue::Str(s) => assert_eq!(s, b.as_str().unwrap()),
                    OutValue::Null => assert!(b.is_null()),
                }
            }
        }
        // сообщения те же, слово в слово
        let want_rep: Vec<(String, String)> = r["report"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| (x[0].as_str().unwrap().into(), x[1].as_str().unwrap().into()))
            .collect();
        assert_eq!(report_texts(&rep), want_rep);
        n += 1;
    }
    assert_eq!(n, 6);
}

#[test]
fn fill_runs_like_the_module() {
    let runs = json("tool_runs.json");
    let run = run_fn("fill_to_volume").unwrap();
    let mut n = 0;
    for r in runs["runs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["tool"] == "fill_to_volume")
    {
        let mut vals = values_of(&r["values"]);
        let body = Body {
            verts: pts(&r["input"]["verts"]),
            faces: r["input"]["faces"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| [0, 1, 2].map(|k| f[k].as_u64().unwrap() as u32))
                .collect(),
        };
        vals.insert("SHELL".into(), Value::Mesh(body));
        let mut rep = VecReport::default();
        let out = run(&vals, &mut rep).unwrap();
        let o = &out["OUTPUT"];
        let w = &r["output"]["features"][0]["values"];
        let g = &o.features[0];
        let num = |i: usize| match &g.values[i] {
            OutValue::Double(x) => *x,
            _ => panic!("не число"),
        };
        assert_eq!(g.values[0], OutValue::Str(w[0].as_str().unwrap().into()));
        let tol = &r["tolerance"];
        assert!((num(1) - w[1].as_f64().unwrap()).abs() < tol["level"].as_f64().unwrap());
        for i in 2..5 {
            assert!(rel(num(i), w[i].as_f64().unwrap()) < tol["volume_rel"].as_f64().unwrap());
        }
        // заполненная часть - замкнутое тело с объёмом около заданного
        assert_eq!(
            shell::defects(&g.body().unwrap().verts, &g.body().unwrap().faces).0,
            0
        );
        let vol = sweep::signed_volume(&g.body().unwrap().verts, &g.body().unwrap().faces);
        assert!(vol > 0.0 && rel(vol, num(2)) < 0.15, "{vol} {}", num(2));
        assert_eq!(rep.lines.len(), r["report"].as_array().unwrap().len());
        n += 1;
    }
    assert_eq!(n, 4);
}

#[test]
fn open_shell_is_refused() {
    let prof = sweep::profile("rect", 10.0, Some(10.0), None, 16).unwrap();
    let mut sw = sweep::sweep(&[[0.0, 5.0, 0.0], [10.0, 5.0, 0.0]], &prof, false).unwrap();
    sw.faces.truncate(sw.faces.len() - 2);
    let mut vals = Values::new();
    vals.insert(
        "SHELL".into(),
        Value::Mesh(Body {
            verts: sw.verts,
            faces: sw.faces,
        }),
    );
    let err = run_fn("fill_to_volume").unwrap()(&vals, &mut VecReport::default()).unwrap_err();
    assert!(err.0.template.contains("не замкнута"));
}
