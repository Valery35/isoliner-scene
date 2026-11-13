// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Инструменты Isoliner3D как инструменты RustGIS.
//!
//! Окно инструмента рисует сам RustGIS (панель Tools) по разделу
//! `tools` в `plugin.json`. Раздел строит крейт `rustgis-kit` из
//! tools.json модуля (`rustgis_kit::manifest`), тест сверяет его
//! с `plugin.json`. Подписи, подсказки и пункты списков - те же, что
//! у модуля QGIS, русские - в `i18n.ru` манифеста.
//!
//! Разбор запроса `{"kind": "tool", ...}`, перевод градусов RustGIS
//! в метры, слои и таблицы итога, хранилище высот итогов - в крейте
//! (`rustgis_kit::tool`). Здесь - вход в него для команд
//! [`crate::plugin`]: расчёт делает [`crate::product::Isoliner`].

use serde_json::{Value as J, json};
use tool_spec::{Catalog, Lang};

use crate::plugin::Context;
use crate::product::Isoliner;

/// Язык показа для крейта.
pub fn kit_lang(lang: Lang) -> rustgis_kit::Lang {
    match lang {
        Lang::Ru => rustgis_kit::Lang::Ru,
        Lang::En => rustgis_kit::Lang::En,
    }
}

/// Инструмент RustGIS `req` (`{"kind": "tool", ...}`): ответ
/// `{"ok": {"layers", "tables", "messages"}}` или `{"error"}`.
pub fn run(
    req: &J,
    cat: &Catalog,
    lang: Lang,
    launch: &dyn Fn(&std::path::Path) -> Result<(), String>,
) -> J {
    let ctx = Context {
        catalog: cat,
        launch,
    };
    match Isoliner::new(ctx, crate::TOOLS_JSON) {
        Ok(p) => rustgis_kit::runner::run_tool(&p, req, kit_lang(lang)),
        Err(e) => json!({"error": format!("tools.json: {e}")}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use rustgis_kit::geo::Utm;
    use rustgis_kit::geom::{Frame, plan, solid};
    use tool_spec::{ParamKind, Tool};

    /// Итог Isoliner3D в хранилище высот слоя `layer`: объекты (номер,
    /// поля, геометрия прогона модуля в метрах зоны `utm`). Ответ - план
    /// слоя, каким его отдаст RustGIS.
    fn keep_result(layer: &str, items: &[(J, J, J)], utm: &Utm) -> J {
        let none = HashMap::new();
        let mut feats = Vec::new();
        let mut g3 = Vec::new();
        for (id, attrs, g) in items {
            feats.push(json!({"type": "Feature", "row": id, "properties": attrs,
                "geometry": plan(g, &Frame::Utm(*utm), &none)}));
            g3.push(solid(g, utm, &none));
        }
        crate::product::store().keep(layer, &feats, g3);
        json!({"type": "FeatureCollection", "features": feats})
    }

    /// Имя поля RustGIS: со строчной буквы.
    fn param_name(key: &str) -> String {
        key.to_lowercase()
    }

    /// Поле - слой проекта.
    fn is_layer(kind: ParamKind) -> bool {
        matches!(
            kind,
            ParamKind::Lines
                | ParamKind::Features
                | ParamKind::Points
                | ParamKind::Table
                | ParamKind::Mesh
                | ParamKind::Meshes
        )
    }

    use rustgis_kit::geo::epsg;

    /// Начало координат образцов: около Уфы, круглые километры зоны.
    fn origin() -> (Utm, [f64; 2]) {
        let utm = Utm::at(56.0, 54.7);
        let o = utm
            .forward(56.0, 54.7)
            .map(|v| (v / 1000.0).round() * 1000.0);
        (utm, o)
    }

    /// Точка образца `[x, y, z|null]` - в метры зоны.
    fn shift(p: &J, o: [f64; 2]) -> J {
        json!([
            o[0] + p[0].as_f64().unwrap(),
            o[1] + p[1].as_f64().unwrap(),
            p[2].clone()
        ])
    }

    /// Запуск без окна сцены.
    fn run_t(req: &J, cat: &Catalog, lang: Lang) -> J {
        run(req, cat, lang, &|_| Ok(()))
    }

    fn catalog() -> Catalog {
        tool_spec::parse_catalog(include_bytes!("../../../fixtures/tools.json")).unwrap()
    }

    fn manifest() -> J {
        let path = format!("{}/plugin.json", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    /// Раздел tools манифеста - из tools.json, у каждой строки перевод.
    /// `ISOLINER_MANIFEST=1 cargo test -p isoliner-rustgis` переписывает
    /// раздел и строки в plugin.json.
    #[test]
    fn manifest_tools_follow_tools_json() {
        let kit = rustgis_kit::Catalog::parse(crate::TOOLS_JSON).unwrap();
        let (tools, ru) = rustgis_kit::manifest::tools_section(&kit, &crate::product::OPTIONS);
        let mut m = manifest();
        if std::env::var_os("ISOLINER_MANIFEST").is_some() {
            let path = format!("{}/plugin.json", env!("CARGO_MANIFEST_DIR"));
            let text = std::fs::read_to_string(&path).unwrap();
            let text = rustgis_kit::manifest::splice(&text, &tools, &ru).unwrap();
            std::fs::write(&path, text).unwrap();
            m = manifest();
        }
        assert_eq!(m["tools"], json!(tools), "plugin.json: ISOLINER_MANIFEST=1");
        for (k, v) in &ru {
            assert_eq!(&m["i18n"]["ru"][k], v, "нет перевода: {k}");
        }
        // правила RustGIS: имена, число полей, пункты списков
        for t in &tools {
            let id = t["id"].as_str().unwrap();
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            );
            let ps = t["params"].as_array().unwrap();
            assert!(!ps.is_empty() && ps.len() <= 32);
            for p in ps {
                let n = p["name"].as_str().unwrap();
                assert!(n.starts_with(|c: char| c.is_ascii_lowercase()), "{n}");
                assert!(
                    n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
                    "{n}"
                );
                if p["kind"] == "choice" {
                    let ch = p["choices"].as_array().unwrap();
                    assert!((1..=64).contains(&ch.len()));
                    assert!(ch.contains(&p["default"]));
                }
            }
        }
    }

    /// Прогон 2.24 модуля через RustGIS: оси в градусах без Z, отметка
    /// из поля. Длина, площадь сечения и объём - как у модуля.
    #[test]
    fn sweep_body_from_rustgis_layers() {
        let runs: J = serde_json::from_slice(
            &std::fs::read(format!(
                "{}/../../fixtures/tool_runs.json",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
        )
        .unwrap();
        let r = runs["runs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["tool"] == "sweep_body")
            .unwrap();
        // ось 1 прогона: Z из поля z, её и берём; кладём в Уфу
        let utm = Utm::at(56.0, 54.7);
        let [e0, n0] = utm
            .forward(56.0, 54.7)
            .map(|v| (v / 1000.0).round() * 1000.0);
        let f = &r["input"][0];
        let coords: Vec<J> = f["parts"][0]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                let [lon, lat] =
                    utm.inverse(e0 + p[0].as_f64().unwrap(), n0 + p[1].as_f64().unwrap());
                json!([lon, lat])
            })
            .collect();
        let fc = json!({"type": "FeatureCollection", "features": [
            {"type": "Feature", "row": 1, "properties": f["attrs"],
             "geometry": {"type": "LineString", "coordinates": coords}}
        ]});
        let v = &r["values"];
        let cat = catalog();
        let profile = &cat
            .tool("sweep_body")
            .unwrap()
            .param("PROFILE")
            .unwrap()
            .options[v["PROFILE"].as_u64().unwrap() as usize]
            .en;
        let req = json!({
            "abi": 1, "kind": "tool", "id": "sweep-body", "lang": "ru",
            "params": {"input": "Оси", "elev_field": "z", "elev": v["ELEV"],
                "profile": profile, "width": v["WIDTH"], "width_field": "w",
                "height": v["HEIGHT"], "height_field": "h", "rise": v["RISE"],
                "anchor": 0, "segments": v["SEGMENTS"]},
            "layers": {"Оси": fc},
        });
        let got = run_t(&req, &cat, Lang::Ru);
        assert!(got["ok"].is_object(), "{got}");
        let layers = got["ok"]["layers"].as_array().unwrap();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0]["name"], "Тела выработок");
        let feats = layers[0]["geojson"]["features"].as_array().unwrap();
        assert_eq!(feats.len(), 1);
        let want = &r["output"]["features"][0]["values"];
        let props = &feats[0]["properties"];
        for (k, i) in [("length", 0), ("area", 1), ("volume", 2)] {
            let (a, b) = (props[k].as_f64().unwrap(), want[i].as_f64().unwrap());
            // градусы туда и обратно рядами UTM - до миллиметра на вершину
            assert!((a - b).abs() / b < 1e-4, "{k}: {a} {b}");
        }
        assert_eq!(props["closed"], 1);
        // контур в плане - в градусах около Уфы
        let g = &feats[0]["geometry"];
        assert_eq!(g["type"], "MultiPolygon");
        let p = &g["coordinates"][0][0][0];
        assert!((p[0].as_f64().unwrap() - 56.0).abs() < 0.01);
        assert!((p[1].as_f64().unwrap() - 54.7).abs() < 0.01);
        let msgs = got["ok"]["messages"].as_array().unwrap();
        assert!(msgs[0].as_str().unwrap().starts_with("Тел: 1"), "{msgs:?}");
    }

    /// Прогон 2.17 модуля через RustGIS: устья точками в градусах,
    /// отметка из поля, инклинометрия таблицей без геометрии. Стволы в плане
    /// и поля - как у модуля, сообщения - слово в слово.
    #[test]
    fn desurvey_from_rustgis_layers() {
        let runs: J = serde_json::from_slice(
            &std::fs::read(format!(
                "{}/../../fixtures/tool_runs.json",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
        )
        .unwrap();
        let r = runs["runs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["tool"] == "desurvey_holes")
            .unwrap();
        let utm = Utm::at(57.0, 54.7);
        let [e0, n0] = utm.forward(57.0, 54.7);
        let collars: Vec<J> = r["input"]["COLLARS"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let a = &row["attrs"];
                let geometry = match (a["x"].as_f64(), a["y"].as_f64()) {
                    (Some(x), Some(y)) => {
                        let [lon, lat] = utm.inverse(e0 + x, n0 + y);
                        json!({"type": "Point", "coordinates": [lon, lat]})
                    }
                    _ => J::Null,
                };
                json!({"type": "Feature", "row": row["id"], "properties": a, "geometry": geometry})
            })
            .collect();
        let survey: Vec<J> = r["input"]["SURVEY"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| json!({"type": "Feature", "row": row["id"], "properties": row["attrs"], "geometry": null}))
            .collect();
        let req = json!({
            "kind": "tool", "id": "desurvey-holes", "lang": "ru",
            "params": {"collars": "Устья", "hole": "hole", "zfield": "z", "eoh": "eoh",
                "survey": "Инклинометрия", "s_hole": "hole", "s_depth": "depth",
                "s_azi": "azimuth", "s_angle": "dip", "convention": 0, "step": r["values"]["STEP"]},
            "layers": {
                "Устья": {"type": "FeatureCollection", "features": collars},
                "Инклинометрия": {"type": "FeatureCollection", "features": survey},
            },
        });
        let got = run_t(&req, &catalog(), Lang::Ru);
        assert!(got["ok"].is_object(), "{got}");
        let feats = got["ok"]["layers"][0]["geojson"]["features"]
            .as_array()
            .unwrap();
        let want = r["output"]["features"].as_array().unwrap();
        assert_eq!(feats.len(), want.len());
        let names: Vec<&str> = r["output"]["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f[0].as_str().unwrap())
            .collect();
        for (g, w) in feats.iter().zip(want) {
            for (i, n) in names.iter().enumerate() {
                let (a, b) = (&g["properties"][*n], &w["values"][i]);
                match (a.as_f64(), b.as_f64()) {
                    (Some(x), Some(y)) => assert!((x - y).abs() < 1e-6, "{n}: {x} {y}"),
                    _ => assert_eq!(a, b, "{n}"),
                }
            }
            let pts = g["geometry"]["coordinates"].as_array().unwrap();
            let wp = w["points"].as_array().unwrap();
            assert_eq!(pts.len(), wp.len());
            for (p, q) in pts.iter().zip(wp) {
                let [x, y] = utm.forward(p[0].as_f64().unwrap(), p[1].as_f64().unwrap());
                assert!((x - e0 - q[0].as_f64().unwrap()).abs() < 1e-3);
                assert!((y - n0 - q[1].as_f64().unwrap()).abs() < 1e-3);
            }
        }
        let msgs: Vec<&str> = got["ok"]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(J::as_str)
            .collect();
        // поля устьев называются по-другому: X и Y - точка слоя
        for (m, w) in msgs.iter().zip(r["report"].as_array().unwrap()).skip(1) {
            assert_eq!(*m, w[1].as_str().unwrap());
        }
        assert!(msgs[0].contains("X точка, Y точка, Z z"), "{}", msgs[0]);

        // строковые аргументы сообщений переводятся, как в QGIS
        let en = run_t(&req, &catalog(), Lang::En);
        let first = en["ok"]["messages"][0].as_str().unwrap_or("");
        assert!(first.contains("X point, Y point"), "{first}");

        // устья таблицей с X и Y: расчёт в метрах таблицы, как у модуля,
        // итог в тех же метрах с кодом системы
        let table: Vec<J> = r["input"]["COLLARS"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| json!({"type": "Feature", "row": row["id"], "properties": row["attrs"], "geometry": null}))
            .collect();
        let mut req = req;
        req["layers"]["Устья"] = json!({"type": "FeatureCollection", "features": table});
        req["params"]["zfield"] = J::Null;
        // без кода системы - ошибка, а не стволы неизвестно где
        assert!(run_t(&req, &catalog(), Lang::En)["error"].is_string());
        req["params"]["crs"] = json!("28410");
        let got = run_t(&req, &catalog(), Lang::Ru);
        let fc = &got["ok"]["layers"][0]["geojson"];
        assert_eq!(fc["crs"]["properties"]["name"], "EPSG:28410", "{got}");
        let msgs: Vec<&J> = got["ok"]["messages"].as_array().unwrap().iter().collect();
        for (m, w) in msgs.iter().zip(r["report"].as_array().unwrap()) {
            assert_eq!(*m, &w[1]);
        }
        for (g, w) in fc["features"].as_array().unwrap().iter().zip(want) {
            let pts = g["geometry"]["coordinates"].as_array().unwrap();
            for (p, q) in pts.iter().zip(w["points"].as_array().unwrap()) {
                assert!((p[0].as_f64().unwrap() - q[0].as_f64().unwrap()).abs() < 1e-9);
                assert!((p[1].as_f64().unwrap() - q[1].as_f64().unwrap()).abs() < 1e-9);
            }
        }
        assert_eq!(epsg("EPSG:3857").as_deref(), Some("EPSG:3857"));
        assert_eq!(epsg("epsg:4326").as_deref(), Some("EPSG:4326"));
        assert!(epsg("Pulkovo").is_none() && epsg("").is_none());
    }

    /// Тела итога идут в сцену телами: слой проекта с тем же именем и теми
    /// же объектами плана заменяется телами с высотой. Другой слой или
    /// слой после правки - как есть.
    #[test]
    fn scene_gets_bodies_of_a_result_layer() {
        let utm = Utm::at(56.0, 54.7);
        let [e, n] = utm.forward(56.0, 54.7);
        let body = json!({"verts": [[e, n, -300.0], [e + 10.0, n, -300.0], [e, n + 10.0, -297.0]],
            "faces": [[0, 1, 2]]});
        let fc = keep_result("Тела выработок", &[(json!(0), json!({}), body)], &utm);
        let store = crate::product::store();
        let got = store.with_3d("Тела выработок", fc.clone());
        let tri = &got["features"][0]["geometry"]["coordinates"][0][0];
        assert_eq!(tri[2][2], -297.0);
        // другое имя или другие объекты - слой как есть
        assert_eq!(store.with_3d("axes", fc.clone()), fc);
        let moved = json!({"type": "FeatureCollection", "features": [
            {"type": "Feature", "geometry": {"type": "Point", "coordinates": [10.0, 10.0]}}]});
        assert_eq!(store.with_3d("Тела выработок", moved.clone()), moved);
    }

    /// Прогоны модуля для инструмента `tool`.
    fn runs_of(tool: &str) -> Vec<J> {
        let runs: J = serde_json::from_slice(
            &std::fs::read(format!(
                "{}/../../fixtures/tool_runs.json",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
        )
        .unwrap();
        runs["runs"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["tool"] == tool)
            .cloned()
            .collect()
    }

    /// Тело в метрах образца - итогом Isoliner3D в хранилище слоя
    /// `layer` около Уфы. Ответ - план слоя, каким его отдаст RustGIS.
    fn body_layer(layer: &str, verts: &J, faces: &J) -> J {
        let (utm, o) = origin();
        let verts: Vec<J> = verts
            .as_array()
            .unwrap()
            .iter()
            .map(|v| shift(v, o))
            .collect();
        let g = json!({"verts": verts, "faces": faces});
        keep_result(layer, &[(json!(0), json!({}), g)], &utm)
    }

    /// 2.23 по итогу тел в RustGIS: уровень и объёмы как у модуля, слой
    /// без тел Isoliner3D - понятная ошибка.
    #[test]
    fn fill_to_volume_from_a_body_layer() {
        let cat = catalog();
        let tool = cat.tool("fill_to_volume").unwrap();
        for (k, r) in runs_of("fill_to_volume").iter().enumerate() {
            let layer = format!("Тело {k}");
            let fc = body_layer(&layer, &r["input"]["verts"], &r["input"]["faces"]);
            let v = &r["values"];
            let opt = |key: &str| {
                let p = tool.param(key).unwrap();
                json!(p.options[v[key].as_u64().unwrap() as usize].en)
            };
            let req = json!({"kind": "tool", "id": "fill-to-volume", "lang": "ru",
                "params": {"shell": layer, "mode": opt("MODE"), "target": v["TARGET"],
                    "unit": opt("UNIT"), "cell": v["CELL"]},
                "layers": {layer.clone(): fc}});
            let got = run_t(&req, &cat, Lang::Ru);
            assert!(got["ok"].is_object(), "{k}: {got}");
            let f = &got["ok"]["layers"][0]["geojson"]["features"][0]["properties"];
            let want = &r["output"]["features"][0]["values"];
            let tol = &r["tolerance"];
            let lvl = (f["level"].as_f64().unwrap() - want[1].as_f64().unwrap()).abs();
            assert!(lvl < tol["level"].as_f64().unwrap(), "{k}: уровень {f}");
            let (a, b) = (f["volume"].as_f64().unwrap(), want[2].as_f64().unwrap());
            assert!(
                (a - b).abs() / b.abs().max(1.0) < tol["volume_rel"].as_f64().unwrap(),
                "{k}: {a} {b}"
            );
            assert_eq!(f["mode"], want[0]);
            // заполненная часть - в проект планом с отметками, в сцену телом
            assert!(f["z_min"].is_number() && f["z_max"].is_number(), "{f}");
        }
        // слой RustGIS без тел - объяснение, а не «значение не того вида»
        let plain = json!({"type": "FeatureCollection", "features": [
            {"type": "Feature", "properties": {}, "geometry": {"type": "Polygon",
             "coordinates": [[[56.0, 54.7], [56.001, 54.7], [56.0, 54.701], [56.0, 54.7]]]}}]});
        let req = json!({"kind": "tool", "id": "fill-to-volume", "lang": "ru",
            "params": {"shell": "Контур"}, "layers": {"Контур": plain}});
        let e = run_t(&req, &cat, Lang::Ru)["error"]
            .as_str()
            .unwrap_or("")
            .to_string();
        assert!(e.starts_with("Слой «Контур» без тел с высотой"), "{e}");
    }

    /// Линии с высотой в метрах образца (`[{"id", "attrs", "parts"}]`) -
    /// итогом Isoliner3D в хранилище слоя `layer`. Ответ - план слоя,
    /// каким его отдаст RustGIS.
    fn line_layer(layer: &str, feats: &J) -> J {
        let (utm, o) = origin();
        let items: Vec<(J, J, J)> = feats
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let pts: Vec<J> = f["parts"][0]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| shift(p, o))
                    .collect();
                (json!(i), f["attrs"].clone(), json!({"points": pts}))
            })
            .collect();
        keep_result(layer, &items, &utm)
    }

    /// Таблица образца (`[{"id", "attrs"}]`) - слоем RustGIS без геометрии.
    fn table_layer(rows: &J) -> J {
        let feats: Vec<J> = rows
            .as_array()
            .unwrap()
            .iter()
            .map(|r| json!({"type": "Feature", "row": r["id"], "properties": r["attrs"], "geometry": null}))
            .collect();
        json!({"type": "FeatureCollection", "features": feats})
    }

    /// Объекты выхода против образца: число, значения полей по порядку,
    /// числа - до 1e-6, строки - слово в слово.
    fn same_values(name: &str, got: &J, want: &J) {
        let names: Vec<&str> = want["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f[0].as_str().unwrap())
            .collect();
        let feats = got["features"].as_array().unwrap();
        let wf = want["features"].as_array().unwrap();
        if feats.len() != wf.len() {
            for f in feats {
                eprintln!("got {}", f["properties"]);
            }
            for w in wf {
                eprintln!("want {} {}", w["values"], w["xyz"]);
            }
        }
        assert_eq!(feats.len(), wf.len(), "{name}");
        for (g, w) in feats.iter().zip(wf) {
            // выход, заменяющий поля источника, - значения по именам
            let pairs: Vec<(&str, &J)> = match &w["values"] {
                J::Object(m) => m.iter().map(|(k, x)| (k.as_str(), x)).collect(),
                vals => names
                    .iter()
                    .enumerate()
                    .map(|(i, n)| (*n, &vals[i]))
                    .collect(),
            };
            for (n, b) in pairs {
                let a = &g["properties"][n];
                match (a.as_f64(), b.as_f64()) {
                    (Some(x), Some(y)) => {
                        assert!(
                            (x - y).abs() / y.abs().max(1.0) < 1e-6,
                            "{name} {n}: {x} {y}"
                        )
                    }
                    _ => assert_eq!(a, b, "{name} {n}"),
                }
            }
        }
    }

    /// 2.18 по стволам - итогу 2.17 в RustGIS: интервалы и середины как
    /// у модуля, отметки стволов сохраняются, сообщения слово в слово.
    #[test]
    fn hole_intervals_from_trace_layer() {
        let cat = catalog();
        let tool = cat.tool("hole_intervals").unwrap();
        for (k, r) in runs_of("hole_intervals").iter().enumerate() {
            let layer = format!("Стволы {k}");
            let traces = line_layer(&layer, &r["input"]["TRACES"]);
            let v = &r["values"];
            let mode =
                &tool.param("MODE").unwrap().options[v["MODE"].as_u64().unwrap() as usize].en;
            let req = json!({"kind": "tool", "id": "hole-intervals", "lang": "ru",
                "params": {"traces": layer, "t_hole": v["T_HOLE"], "mode": mode,
                    "table": "Пробы", "f_hole": v["F_HOLE"], "f_from": v["F_FROM"],
                    "f_to": v["F_TO"], "length": v["LENGTH"]},
                "layers": {layer.clone(): traces, "Пробы": table_layer(&r["input"]["TABLE"])}});
            let got = run_t(&req, &cat, Lang::Ru);
            assert!(got["ok"].is_object(), "{k}: {got}");
            let ls = got["ok"]["layers"].as_array().unwrap();
            // выходы - по порядку ключей: OUTPUT, OUTPUT_POINTS
            same_values(
                &format!("{k} OUTPUT"),
                &ls[0]["geojson"],
                &r["output"]["OUTPUT"],
            );
            same_values(
                &format!("{k} POINTS"),
                &ls[1]["geojson"],
                &r["output"]["OUTPUT_POINTS"],
            );
            // отметки интервала - из ствола с высотой
            for (g, w) in ls[0]["geojson"]["features"]
                .as_array()
                .unwrap()
                .iter()
                .zip(r["output"]["OUTPUT"]["features"].as_array().unwrap())
            {
                let zs: Vec<f64> = w["geometry"]["points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| p[2].as_f64().unwrap())
                    .collect();
                let lo = zs.iter().copied().fold(f64::INFINITY, f64::min);
                assert!((g["properties"]["z_min"].as_f64().unwrap() - lo).abs() < 1e-9);
            }
            let msgs: Vec<&J> = got["ok"]["messages"].as_array().unwrap().iter().collect();
            for (m, w) in msgs.iter().zip(r["report"].as_array().unwrap()) {
                assert_eq!(*m, &w[1], "{k}");
            }
        }
    }

    /// Слой образца модуля - слоем RustGIS по виду поля: тела и линии
    /// с высотой - итогом Isoliner3D в хранилище, таблица - без геометрии.
    fn sample_layer(kind: ParamKind, layer: &str, input: &J) -> J {
        let (utm, o) = origin();
        let at = |p: &J| shift(p, o);
        let body = |v: &J, f: &J| -> J {
            let verts: Vec<J> = v.as_array().unwrap().iter().map(at).collect();
            json!({"verts": verts, "faces": f})
        };
        // объекты образца: (номер, поля, геометрия прогона в метрах)
        let items: Vec<(J, J, J)> = match kind {
            ParamKind::Table => return table_layer(input),
            ParamKind::Mesh => vec![(json!(0), json!({}), body(&input["verts"], &input["faces"]))],
            ParamKind::Meshes => input
                .as_array()
                .unwrap()
                .iter()
                .map(|f| {
                    (
                        f["id"].clone(),
                        f["attrs"].clone(),
                        body(&f["verts"], &f["faces"]),
                    )
                })
                .collect(),
            _ => input
                .as_array()
                .unwrap()
                .iter()
                .map(|f| {
                    let pts: Vec<J> = match f.get("xyz") {
                        Some(p) if !p.is_null() => vec![at(p)],
                        _ => f["parts"][0].as_array().unwrap().iter().map(at).collect(),
                    };
                    let g = if pts.len() == 1 {
                        json!({"xyz": pts[0]})
                    } else {
                        json!({"points": pts})
                    };
                    (f["id"].clone(), f["attrs"].clone(), g)
                })
                .collect(),
        };
        keep_result(layer, &items, &utm)
    }

    /// Все прогоны модуля для `name` через RustGIS: вход - слоями, выходы
    /// и сообщения - как у модуля (числа до 1e-6).
    fn runs_through_rustgis(name: &str) {
        let cat = catalog();
        let tool = cat.tool(name).unwrap();
        let runs = runs_of(name);
        assert!(!runs.is_empty(), "{name}: нет прогонов");
        for (k, r) in runs.iter().enumerate() {
            let v = &r["values"];
            let mut params = serde_json::Map::new();
            let mut layers = serde_json::Map::new();
            let input = &r["input"];
            let by_key = input
                .as_object()
                .is_some_and(|o| o.keys().all(|key| tool.param(key).is_some()));
            for p in &tool.params {
                let pn = param_name(&p.key);
                if is_layer(p.kind) {
                    let data = if by_key { &input[&p.key] } else { input };
                    if data.is_null() || (!by_key && p.key != first_layer_key(tool)) {
                        continue;
                    }
                    let lname = v[format!("@{}", p.key)]["name"]
                        .as_str()
                        .map_or_else(|| format!("{} {k}", p.key), str::to_string);
                    layers.insert(lname.clone(), sample_layer(p.kind, &lname, data));
                    params.insert(pn, json!(lname));
                    continue;
                }
                let x = &v[&p.key];
                if x.is_null() || p.kind == ParamKind::Sink && !p.optional {
                    continue;
                }
                let val = match p.kind {
                    ParamKind::Enum if !p.multiple => {
                        json!(p.options[x.as_u64().unwrap() as usize].en)
                    }
                    ParamKind::Field if p.multiple => {
                        json!(
                            x.as_array()
                                .unwrap()
                                .iter()
                                .filter_map(J::as_str)
                                .collect::<Vec<_>>()
                                .join(",")
                        )
                    }
                    _ => x.clone(),
                };
                params.insert(pn, val);
            }
            let req = json!({"kind": "tool", "id": name.replace('_', "-"), "lang": "ru",
                "params": params, "layers": layers});
            let got = run_t(&req, &cat, Lang::Ru);
            if let Some(e) = r.get("error") {
                // отказ модуля - отказ и здесь
                let t = e["template"].as_str().unwrap_or("");
                let g = got["error"].as_str().unwrap_or("");
                assert!(!g.is_empty() && !t.is_empty(), "{name} {k}: {got}");
                continue;
            }
            assert!(got["ok"].is_object(), "{name} {k}: {got}");
            // слои и таблицы итога вместе
            let ls: Vec<J> = got["ok"]["layers"]
                .as_array()
                .into_iter()
                .chain(got["ok"]["tables"].as_array())
                .flatten()
                .cloned()
                .collect();
            let outs: Vec<(&str, &J)> = if r["output"].get("features").is_some() {
                vec![("OUTPUT", &r["output"])]
            } else {
                r["output"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(a, b)| (a.as_str(), b))
                    .collect()
            };
            for (key, want) in outs {
                // куб слоем не ложится, его сверяет тест расчёта
                if want.get("grid").is_some() {
                    continue;
                }
                let label = tool.param(key).map(|p| p.label.get(Lang::Ru).to_string());
                let found = ls
                    .iter()
                    .find(|l| label.as_deref().is_none_or(|t| l["name"] == t));
                assert!(found.is_some(), "{name} {k}: нет выхода {key}");
                let layer = found.unwrap();
                if want.get("fields").is_some() {
                    same_values(&format!("{name} {k} {key}"), &layer["geojson"], want);
                } else {
                    let n = layer["geojson"]["features"].as_array().unwrap().len();
                    assert_eq!(
                        n,
                        want["features"].as_array().unwrap().len(),
                        "{name} {k} {key}"
                    );
                }
            }
            let msgs: Vec<&J> = got["ok"]["messages"].as_array().unwrap().iter().collect();
            for (m, w) in msgs.iter().zip(r["report"].as_array().unwrap()) {
                assert_eq!(*m, &w[1], "{name} {k}");
            }
        }
    }

    fn first_layer_key(tool: &Tool) -> String {
        tool.params
            .iter()
            .find(|p| is_layer(p.kind))
            .map(|p| p.key.clone())
            .unwrap_or_default()
    }

    /// 2.26 по полигонам RustGIS: Г-образный контур без Z (пол - полем
    /// или постоянной) даёт то же тело, что модуль. Слой линий - отказ.
    #[test]
    fn contour_crown_from_polygons() {
        let cat = catalog();
        let tool = cat.tool("contour_crown").unwrap();
        let utm = Utm::at(56.0, 54.7);
        let [e0, n0] = utm
            .forward(56.0, 54.7)
            .map(|v| (v / 1000.0).round() * 1000.0);
        for (k, r) in runs_of("contour_crown").iter().enumerate() {
            let f8 = r["input"]["INPUT"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["id"] == 8)
                .unwrap();
            // контур - от его угла: прогоны модуля бывают и в метрах
            // проекта (435 000, 6 062 000), в зону кладётся только форма
            let pts = f8["parts"][0].as_array().unwrap();
            let min = |k: usize| {
                pts.iter()
                    .map(|p| p[k].as_f64().unwrap())
                    .fold(f64::INFINITY, f64::min)
            };
            let (mx, my) = (min(0), min(1));
            let ring: Vec<J> = pts
                .iter()
                .map(|p| {
                    let [lon, lat] = utm.inverse(
                        e0 + p[0].as_f64().unwrap() - mx,
                        n0 + p[1].as_f64().unwrap() - my,
                    );
                    json!([lon, lat])
                })
                .collect();
            let fc = json!({"type": "FeatureCollection", "features": [
                {"type": "Feature", "row": 8, "properties": f8["attrs"],
                 "geometry": {"type": "Polygon", "coordinates": [ring]}}]});
            let v = &r["values"];
            let shape =
                &tool.param("SHAPE").unwrap().options[v["SHAPE"].as_u64().unwrap() as usize].en;
            let mut params = json!({"input": "Контуры", "floor": v["FLOOR"], "wall": v["WALL"],
                "shape": shape, "steep": v["STEEP"], "rise_max": v["RISE_MAX"], "cell": v["CELL"]});
            if let Some(f) = v["FLOOR_FIELD"].as_str() {
                params["floor_field"] = json!(f);
            }
            let req = json!({"kind": "tool", "id": "contour-crown", "lang": "ru",
                "params": params, "layers": {"Контуры": fc}});
            let got = run_t(&req, &cat, Lang::Ru);
            if r.get("error").is_some() {
                // у RustGIS слой полигонов, отказ модуля - на слое линий
                continue;
            }
            assert!(got["ok"].is_object(), "{k}: {got}");
            let props = &got["ok"]["layers"][0]["geojson"]["features"][0]["properties"];
            let want = r["output"]["OUTPUT"]["features"]
                .as_array()
                .unwrap()
                .iter()
                .find(|w| w["source"] == 8)
                .unwrap();
            for (i, n) in ["volume", "plan_area", "zmin", "zmax", "rise"]
                .iter()
                .enumerate()
            {
                let (a, b) = (
                    props[*n].as_f64().unwrap(),
                    want["values"][i].as_f64().unwrap(),
                );
                assert!((a - b).abs() / b.abs().max(1.0) < 1e-6, "{k} {n}: {a} {b}");
            }
            assert_eq!(props["closed"], 1);
        }
        // слой линий - отказ модуля
        let lines = json!({"type": "FeatureCollection", "features": [
            {"type": "Feature", "properties": {}, "geometry": {"type": "LineString",
             "coordinates": [[56.0, 54.7], [56.001, 54.7], [56.001, 54.701]]}}]});
        let req = json!({"kind": "tool", "id": "contour-crown", "lang": "ru",
            "params": {"input": "Линии"}, "layers": {"Линии": lines}});
        let e = run_t(&req, &cat, Lang::Ru)["error"]
            .as_str()
            .unwrap_or("")
            .to_string();
        assert!(e.contains("Нужен слой полигонов"), "{e}");
    }

    #[test]
    fn density_3d_through_rustgis() {
        runs_through_rustgis("density_3d");
    }

    #[test]
    fn bench_report_through_rustgis() {
        runs_through_rustgis("bench_report");
    }

    #[test]
    fn select_by_shell_through_rustgis() {
        runs_through_rustgis("select_by_shell");
    }

    #[test]
    fn zonal_shell_stats_through_rustgis() {
        runs_through_rustgis("zonal_shell_stats");
    }

    #[test]
    fn boolean_shells_through_rustgis() {
        runs_through_rustgis("boolean_shells");
    }

    #[test]
    fn bad_requests_get_an_error() {
        let cat = catalog();
        let r = run_t(&json!({"id": "grid-to-shell"}), &cat, Lang::En);
        assert_eq!(r["error"], "No tool 'grid-to-shell'.");
        // слоя с таким именем нет в запросе
        let r = run_t(
            &json!({"id": "sweep-body", "params": {"input": "x"}, "layers": {}}),
            &cat,
            Lang::En,
        );
        assert!(
            r["error"]
                .as_str()
                .unwrap()
                .starts_with("The values do not fit")
        );
        // пункт списка не из списка, число текстом не число
        let fc = json!({"type": "FeatureCollection", "features": []});
        let r = run_t(
            &json!({"id": "sweep-body", "params": {"input": "a", "profile": "Square"}, "layers": {"a": fc}}),
            &cat,
            Lang::En,
        );
        assert!(r["error"].is_string(), "{r}");
        let r = run_t(
            &json!({"id": "sweep-body", "params": {"input": "a", "width": "wide"}, "layers": {"a": fc}}),
            &cat,
            Lang::En,
        );
        assert!(r["error"].is_string(), "{r}");
        // порченая геометрия - пропуск, не падение
        let junk = json!({"type": "FeatureCollection", "features": [
            {"type": "Feature", "geometry": {"type": "LineString", "coordinates": [["a"], [1]]}},
            {"type": "Feature", "geometry": null}, 5]});
        let r = run_t(
            &json!({"id": "sweep-body", "params": {"input": "a"}, "layers": {"a": junk}}),
            &cat,
            Lang::En,
        );
        assert!(r.get("ok").is_some() || r.get("error").is_some());
    }
}
