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

fn body_of(v: &J) -> Body {
    Body {
        verts: pts(&v["verts"]),
        faces: v["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| [0, 1, 2].map(|k| f[k].as_u64().unwrap() as u32))
            .collect(),
    }
}

fn want_report(r: &J) -> Vec<(String, String)> {
    r["report"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| (x[0].as_str().unwrap().into(), x[1].as_str().unwrap().into()))
        .collect()
}

/// Значения объекта выхода против модуля: целые и строки точно,
/// числа - в относительном допуске.
fn same_values(got: &[OutValue], want: &J, tol: f64, tag: &str) {
    let want = want.as_array().unwrap();
    assert_eq!(got.len(), want.len(), "{tag}");
    for (a, b) in got.iter().zip(want) {
        match a {
            OutValue::Double(x) => assert!(rel(*x, b.as_f64().unwrap()) < tol, "{tag}: {x} {b}"),
            OutValue::Int(i) => assert_eq!(*i, b.as_i64().unwrap(), "{tag}"),
            OutValue::Str(s) => assert_eq!(s, b.as_str().unwrap(), "{tag}"),
            OutValue::Null => assert!(b.is_null(), "{tag}"),
        }
    }
}

#[test]
fn boolean_runs_like_the_module() {
    let runs = json("tool_runs.json");
    let run = run_fn("boolean_shells").unwrap();
    let mut n = 0;
    for r in runs["runs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["tool"] == "boolean_shells")
    {
        let mut vals = values_of(&r["values"]);
        vals.insert(
            "OP".into(),
            Value::Enum(r["values"]["OP"].as_u64().unwrap() as usize),
        );
        vals.insert(
            "SHELL_A".into(),
            Value::Mesh(body_of(&r["input"]["SHELL_A"])),
        );
        vals.insert(
            "SHELL_B".into(),
            Value::Mesh(body_of(&r["input"]["SHELL_B"])),
        );
        let mut rep = VecReport::default();
        let out = run(&vals, &mut rep).unwrap();
        let o = &out["OUTPUT"];
        let want = r["output"]["features"].as_array().unwrap();
        assert_eq!(o.features.len(), want.len());
        for (g, w) in o.features.iter().zip(want) {
            let b = g.body().unwrap();
            let tag = format!("{} {}", r["values"]["OP"], w["values"]);
            assert_eq!(
                b.verts.len() as u64,
                w["n_verts"].as_u64().unwrap(),
                "{tag}"
            );
            assert_eq!(
                b.faces.len() as u64,
                w["n_faces"].as_u64().unwrap(),
                "{tag}"
            );
            same_values(&g.values, &w["values"], 1e-9, &tag);
            assert_eq!(shell::defects(&b.verts, &b.faces).0, 0);
        }
        assert_eq!(report_texts(&rep), want_report(r));
        n += 1;
    }
    assert_eq!(n, 3);
}

/// Объекты слоя из прогона: `[{"id", "parts", "attrs"}]`.
fn features_of(v: &J) -> Vec<LineFeature> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|l| LineFeature {
            id: l["id"].as_i64().unwrap(),
            parts: l["parts"].as_array().unwrap().iter().map(pts).collect(),
            attrs: l["attrs"]
                .as_object()
                .map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default(),
        })
        .collect()
}

#[test]
fn select_runs_like_the_module() {
    let runs = json("tool_runs.json");
    let run = run_fn("select_by_shell").unwrap();
    let mut n = 0;
    for r in runs["runs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["tool"] == "select_by_shell")
    {
        let mut vals = values_of(&r["values"]);
        vals.insert(
            "MODE".into(),
            Value::Enum(r["values"]["MODE"].as_u64().unwrap() as usize),
        );
        vals.insert("SHELL".into(), Value::Mesh(body_of(&r["input"]["SHELL"])));
        vals.insert(
            "INPUT".into(),
            Value::Features(features_of(&r["input"]["INPUT"])),
        );
        let mut rep = VecReport::default();
        let out = run(&vals, &mut rep).unwrap();
        let o = &out["OUTPUT"];
        let got: Vec<i64> = o.features.iter().map(|f| f.source.unwrap()).collect();
        let want: Vec<i64> = r["output"]["features"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["source"].as_i64().unwrap())
            .collect();
        assert_eq!(got, want, "{}", r["values"]);
        assert_eq!(report_texts(&rep), want_report(r), "{}", r["values"]);
        n += 1;
    }
    assert_eq!(n, 6);
}

/// Значения прогона по видам полей из описания инструмента в tools.json:
/// так не надо помнить, какой ключ какого вида.
fn typed_values(tool: &str, v: &J) -> Values {
    let cat = json("tools.json");
    let spec = cat["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == tool)
        .unwrap();
    let mut out = Values::new();
    for (k, x) in v.as_object().unwrap().iter().filter(|(_, x)| !x.is_null()) {
        if let Some(meta) = k.strip_prefix('@') {
            let _ = meta;
            let s = |f: &str| x[f].as_str().unwrap_or_default().to_string();
            let list = |f: &str| {
                x[f].as_array()
                    .map(|a| a.iter().map(|s| s.as_str().unwrap().to_string()).collect())
                    .unwrap_or_default()
            };
            out.insert(
                k.clone(),
                Value::Layer(tool_spec::LayerInfo {
                    name: s("name"),
                    geometry: s("geometry"),
                    hasz: x["hasz"].as_bool().unwrap_or(false),
                    fields: list("fields"),
                    numeric: list("numeric"),
                }),
            );
            continue;
        }
        let p = spec["params"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["key"] == k.as_str())
            .unwrap_or_else(|| panic!("{tool}: нет поля {k}"));
        let multiple = p["multiple"].as_bool().unwrap_or(false);
        let val = match p["kind"].as_str().unwrap() {
            "enum" if multiple => Value::Enums(
                x.as_array()
                    .unwrap()
                    .iter()
                    .map(|i| i.as_u64().unwrap() as usize)
                    .collect(),
            ),
            "enum" => Value::Enum(x.as_u64().unwrap() as usize),
            "field" if multiple => Value::Fields(
                x.as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s.as_str().unwrap().to_string())
                    .collect(),
            ),
            "field" => Value::Field(x.as_str().unwrap().to_string()),
            "string" => Value::Str(x.as_str().unwrap().to_string()),
            "bool" | "sink" => Value::Bool(x.as_bool().unwrap()),
            "integer" => Value::Integer(x.as_i64().unwrap()),
            "number" => Value::Number(x.as_f64().unwrap()),
            other => panic!("{tool}: {k} вида {other} задаётся входом"),
        };
        out.insert(k.clone(), val);
    }
    out
}

fn mesh_features_of(v: &J) -> Vec<tool_spec::MeshFeature> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|m| tool_spec::MeshFeature {
            id: m["id"].as_i64().unwrap(),
            attrs: m["attrs"]
                .as_object()
                .map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default(),
            body: (!m["verts"].is_null() && !m["faces"].is_null()).then(|| body_of(m)),
        })
        .collect()
}

fn out_json(v: &OutValue) -> J {
    match v {
        OutValue::Double(x) => serde_json::json!(x),
        OutValue::Int(i) => serde_json::json!(i),
        OutValue::Str(s) => serde_json::json!(s),
        OutValue::Null => J::Null,
    }
}

/// Значение против образца: числа в относительном допуске.
fn same_json(a: &J, b: &J, tag: &str) {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => assert!(rel(x, y) < 1e-9, "{tag}: {x} {y}"),
        _ => assert_eq!(a, b, "{tag}"),
    }
}

#[test]
fn zonal_runs_like_the_module() {
    let runs = json("tool_runs.json");
    let run = run_fn("zonal_shell_stats").unwrap();
    let mut n = 0;
    for r in runs["runs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["tool"] == "zonal_shell_stats")
    {
        let tag = r["values"].to_string();
        let mut vals = typed_values("zonal_shell_stats", &r["values"]);
        vals.insert(
            "SHELLS".into(),
            Value::Meshes(mesh_features_of(&r["input"]["SHELLS"])),
        );
        vals.insert(
            "INPUT".into(),
            Value::Features(features_of(&r["input"]["INPUT"])),
        );
        let mut rep = VecReport::default();
        let out = run(&vals, &mut rep).unwrap();
        for (key, want) in r["output"].as_object().unwrap() {
            let o = &out[key];
            let fields: Vec<(String, String)> = o
                .fields
                .iter()
                .map(|(n, t)| (n.clone(), t.name()))
                .collect();
            let want_fields: Vec<(String, String)> = want["fields"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| (f[0].as_str().unwrap().into(), f[1].as_str().unwrap().into()))
                .collect();
            assert_eq!(fields, want_fields, "{tag} {key}");
            assert_eq!(o.replace, want["replace"].as_bool().unwrap(), "{tag}");
            let wf = want["features"].as_array().unwrap();
            assert_eq!(o.features.len(), wf.len(), "{tag} {key}");
            for (g, w) in o.features.iter().zip(wf) {
                assert_eq!(g.source, w["source"].as_i64(), "{tag}");
                if o.replace {
                    let want_named = w["values"].as_object().unwrap();
                    let names: Vec<&String> = g.named.iter().map(|(k, _)| k).collect();
                    let want_names: Vec<&String> = want_named.keys().collect();
                    let mut a = names.clone();
                    let mut b = want_names.clone();
                    a.sort();
                    b.sort();
                    assert_eq!(a, b, "{tag}");
                    for (k, v) in &g.named {
                        same_json(&out_json(v), &want_named[k], &format!("{tag} {k}"));
                    }
                } else {
                    let wv = w["values"].as_array().unwrap();
                    assert_eq!(g.values.len(), wv.len(), "{tag}");
                    for (a, b) in g.values.iter().zip(wv) {
                        same_json(&out_json(a), b, &tag);
                    }
                }
            }
        }
        assert_eq!(out.len(), r["output"].as_object().unwrap().len(), "{tag}");
        assert_eq!(report_texts(&rep), want_report(r), "{tag}");
        n += 1;
    }
    assert_eq!(n, 3);
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
