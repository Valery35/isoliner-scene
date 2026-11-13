// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Инструменты Isoliner3D как инструменты RustGIS.
//!
//! Окно инструмента рисует сам RustGIS (панель Tools) по разделу
//! `tools` в `plugin.json`. Раздел строится здесь из tools.json модуля
//! ([`manifest_tools`]), тест сверяет его с `plugin.json`. Подписи,
//! подсказки и пункты списков - те же, что у модуля QGIS, русские -
//! в `i18n.ru` манифеста.
//!
//! Запуск - запрос `{"kind": "tool", "id", "params", "layers", "lang"}`
//! (docs/plugins.md RustGIS, «The ABI»): поле-слой несёт имя слоя,
//! его объекты - GeoJSON в `layers`. Ответ - `{"layers": [{"name",
//! "geojson"}], "messages": [...]}`, RustGIS кладёт слои в проект
//! с отменой.
//!
//! RustGIS хранит слои в градусах WGS 84 и без Z (проверено
//! `rustgis-cli`: «RustGIS keeps x and y only»). Поэтому:
//!
//! - градусы входа переводятся в метры зоны UTM по середине входа
//!   (`scene_core::geo`), итог - обратно в градусы;
//! - Z у входа нет, отметка берётся из поля или числа инструмента;
//! - тело в проект RustGIS ложится верхними гранями - контуром в плане,
//!   с полями расчёта (длина, площадь, объём).
//!
//! Предлагаются только инструменты, которым хватает слоёв RustGIS:
//! [`OFFERED`].

use std::collections::HashMap;

use scene_core::geo::Utm;
use serde_json::{Value as J, json};
use tool_spec::{Arg, Catalog, Geom, Lang, OutGeometry, Param, ParamKind, Tool, VecReport};

use crate::convert;
use crate::i18n::tr;

/// Инструменты в RustGIS. 2.24 берёт оси без Z: отметка - из поля или
/// числа. Остальным нужны тела (2.11, 2.12, 2.14, 2.23) или линии с Z
/// (2.18), а слой RustGIS их не держит. 2.17 ждёт выбора системы
/// координат таблицы устьев.
pub const OFFERED: &[&str] = &["sweep_body"];

/// Группа инструментов в панели RustGIS.
#[cfg(test)]
const TOOLBOX: &str = "Isoliner3D";

/// Имя инструмента RustGIS: строчные буквы, цифры и дефис.
#[cfg(test)]
fn tool_id(name: &str) -> String {
    name.to_lowercase().replace('_', "-")
}

/// Имя поля RustGIS: со строчной буквы.
fn param_name(key: &str) -> String {
    key.to_lowercase()
}

/// Вид поля RustGIS. None - поле выхода или вид, которого у RustGIS нет.
#[cfg(test)]
fn rg_kind(p: &Param) -> Option<&'static str> {
    Some(match p.kind {
        ParamKind::Lines | ParamKind::Features => "vectorLayer",
        ParamKind::Points => "pointLayer",
        ParamKind::Table => "layer",
        ParamKind::Field if p.numeric => "numericField",
        ParamKind::Field => "field",
        ParamKind::Number | ParamKind::Integer => "number",
        ParamKind::Enum if !p.multiple => "choice",
        ParamKind::Bool => "bool",
        ParamKind::String => "text",
        _ => return None,
    })
}

fn is_layer(kind: ParamKind) -> bool {
    matches!(
        kind,
        ParamKind::Lines | ParamKind::Features | ParamKind::Points | ParamKind::Table
    )
}

/// Первое предложение подсказки: строка «summary» RustGIS.
#[cfg(test)]
fn summary(help: &str) -> &str {
    let line = help.lines().next().unwrap_or("").trim();
    match line.find(". ") {
        Some(i) => &line[..=i],
        None => line,
    }
}

/// Раздел `tools` манифеста и русские строки к нему. Нужен тесту,
/// который сверяет и переписывает `plugin.json`.
#[cfg(test)]
fn manifest_tools(catalog: &Catalog) -> (Vec<J>, serde_json::Map<String, J>) {
    let mut ru = serde_json::Map::new();
    let mut say = |en: &str, r: &str| {
        if !en.is_empty() && en != r {
            ru.insert(en.to_string(), json!(r));
        }
    };
    let mut tools = Vec::new();
    for name in OFFERED {
        let Some(t) = catalog.tool(name) else {
            continue;
        };
        let title = t.display_name(Lang::En);
        say(&title, &t.display_name(Lang::Ru));
        let sum = summary(t.help.get(Lang::En));
        say(sum, summary(t.help.get(Lang::Ru)));
        let mut params = Vec::new();
        for p in &t.params {
            let Some(kind) = rg_kind(p) else { continue };
            let label = p.label.get(Lang::En);
            say(label, p.label.get(Lang::Ru));
            let mut o = json!({"name": param_name(&p.key), "label": label, "kind": kind});
            if is_layer(p.kind) && !p.optional {
                o["required"] = json!(true);
            }
            let hint = p.hint.get(Lang::En);
            if !hint.is_empty() {
                o["help"] = json!(hint);
                say(hint, p.hint.get(Lang::Ru));
            }
            if p.kind == ParamKind::Enum {
                let choices: Vec<&str> = p.options.iter().map(|c| c.get(Lang::En)).collect();
                for c in &p.options {
                    say(c.get(Lang::En), c.get(Lang::Ru));
                }
                let i = p.default.as_ref().and_then(J::as_u64).unwrap_or(0) as usize;
                if let Some(d) = choices.get(i) {
                    o["default"] = json!(d);
                }
                o["choices"] = json!(choices);
            } else if let Some(d) = p
                .default
                .as_ref()
                .filter(|d| !d.is_null() && d.as_str() != Some("") && !is_layer(p.kind))
            {
                o["default"] = d.clone();
            }
            params.push(o);
        }
        tools.push(json!({
            "id": tool_id(&t.name),
            "name": title,
            "summary": sum,
            "toolbox": TOOLBOX,
            "params": params,
        }));
    }
    (tools, ru)
}

/// Координаты GeoJSON: `[x, y]` или `[x, y, z]`.
fn position(v: &J) -> Option<[f64; 3]> {
    let a = v.as_array()?;
    if a.len() < 2 {
        return None;
    }
    let z = a.get(2).and_then(J::as_f64).unwrap_or(f64::NAN);
    Some([a[0].as_f64()?, a[1].as_f64()?, z])
}

/// Части геометрии GeoJSON: линии - по линии на часть, точки и полигоны -
/// вершины по кольцам.
fn parts(g: &J) -> Vec<Vec<[f64; 3]>> {
    let c = &g["coordinates"];
    let line = |v: &J| -> Vec<[f64; 3]> {
        v.as_array()
            .into_iter()
            .flatten()
            .filter_map(position)
            .collect()
    };
    match g["type"].as_str().unwrap_or("") {
        "Point" => position(c).map(|p| vec![vec![p]]).unwrap_or_default(),
        "MultiPoint" | "LineString" => vec![line(c)],
        "MultiLineString" | "Polygon" => c.as_array().into_iter().flatten().map(line).collect(),
        "MultiPolygon" => c
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|p| p.as_array().into_iter().flatten().map(line))
            .collect(),
        _ => Vec::new(),
    }
}

/// Номер объекта: `row` RustGIS, иначе `id`, иначе порядковый.
fn feature_id(f: &J, i: usize) -> i64 {
    f["row"]
        .as_i64()
        .or_else(|| f["id"].as_i64())
        .unwrap_or(i as i64)
}

/// Зона UTM по середине всех вершин входа.
fn zone_of(layers: &[&J]) -> Utm {
    let (mut sx, mut sy, mut n) = (0.0, 0.0, 0.0);
    for fc in layers {
        for f in fc["features"].as_array().into_iter().flatten() {
            for p in parts(&f["geometry"]).iter().flatten() {
                if p[0].is_finite() && p[1].is_finite() {
                    sx += p[0];
                    sy += p[1];
                    n += 1.0;
                }
            }
        }
    }
    if n > 0.0 {
        Utm::at(sx / n, sy / n)
    } else {
        Utm::at(0.0, 0.0)
    }
}

/// Слой RustGIS в вид прогонов модуля, в метрах зоны.
fn layer_value(p: &Param, fc: &J, utm: &Utm) -> J {
    let feats = fc["features"].as_array().cloned().unwrap_or_default();
    let list: Vec<J> = feats
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let id = feature_id(f, i);
            let attrs = f["properties"].clone();
            let ps: Vec<Vec<[f64; 3]>> = parts(&f["geometry"])
                .into_iter()
                .map(|part| {
                    part.into_iter()
                        .map(|[x, y, z]| {
                            let [e, n] = utm.forward(x, y);
                            [e, n, z]
                        })
                        .collect()
                })
                .collect();
            let xyz = |pt: [f64; 3]| {
                json!([pt[0], pt[1], if pt[2].is_finite() { json!(pt[2]) } else { J::Null }])
            };
            match p.kind {
                ParamKind::Points | ParamKind::Table => {
                    let first = ps.first().and_then(|q| q.first()).copied();
                    json!({"id": id, "attrs": attrs, "xyz": first.map(xyz)})
                }
                _ => json!({
                    "id": id,
                    "attrs": attrs,
                    "parts": ps.iter().map(|q| q.iter().map(|&pt| xyz(pt)).collect::<Vec<_>>()).collect::<Vec<_>>(),
                }),
            }
        })
        .collect();
    J::Array(list)
}

/// Пункт списка: номер, английский или русский текст пункта.
fn choice(p: &Param, v: &J) -> Option<usize> {
    if let Some(i) = v.as_u64() {
        return usize::try_from(i).ok();
    }
    let s = v.as_str()?.trim();
    p.options
        .iter()
        .position(|o| o.en == s || o.ru == s)
        .or_else(|| s.parse().ok())
}

/// Число: число или текст числа (поле RustGIS могло прийти строкой).
fn number(v: &J) -> Option<f64> {
    v.as_f64().or_else(|| {
        v.as_str()
            .and_then(|s| s.trim().replace(',', ".").parse().ok())
    })
}

/// Входная ошибка запроса: ключ поля.
type BadKey = String;

/// Значения полей в виде прогонов модуля и зона пересчёта.
fn values(tool: &Tool, req: &J) -> Result<(J, Utm, HashMap<i64, J>), BadKey> {
    let params = &req["params"];
    let layers = &req["layers"];
    // слои входа: имя слоя - в поле, объекты - в layers
    let mut inputs: Vec<(&Param, &J)> = Vec::new();
    for p in tool.params.iter().filter(|p| is_layer(p.kind)) {
        let name = params[param_name(&p.key)].as_str().unwrap_or("");
        if name.is_empty() {
            continue;
        }
        let fc = layers.get(name).ok_or_else(|| p.key.clone())?;
        inputs.push((p, fc));
    }
    let utm = zone_of(&inputs.iter().map(|(_, fc)| *fc).collect::<Vec<_>>());
    let mut out = serde_json::Map::new();
    let mut geoms = HashMap::new();
    for (p, fc) in &inputs {
        out.insert(p.key.clone(), layer_value(p, fc, &utm));
        for (i, f) in fc["features"].as_array().into_iter().flatten().enumerate() {
            geoms
                .entry(feature_id(f, i))
                .or_insert_with(|| f["geometry"].clone());
        }
    }
    for p in &tool.params {
        let v = &params[param_name(&p.key)];
        if v.is_null() || is_layer(p.kind) {
            continue;
        }
        let got = match p.kind {
            ParamKind::Enum => json!(choice(p, v).ok_or_else(|| p.key.clone())?),
            ParamKind::Number => json!(number(v).ok_or_else(|| p.key.clone())?),
            ParamKind::Integer => {
                let x = number(v).ok_or_else(|| p.key.clone())?;
                json!(x.round() as i64)
            }
            ParamKind::Field | ParamKind::String => match v.as_str().map(str::trim) {
                Some("") if p.kind == ParamKind::Field => continue,
                Some(s) if p.multiple => json!(
                    s.split([',', ';'])
                        .map(str::trim)
                        .filter(|x| !x.is_empty())
                        .collect::<Vec<_>>()
                ),
                Some(s) => json!(s),
                None => return Err(p.key.clone()),
            },
            _ => v.clone(),
        };
        out.insert(p.key.clone(), got);
    }
    Ok((J::Object(out), utm, geoms))
}

/// Точку зоны - в градусы GeoJSON.
fn lonlat(utm: &Utm, p: &[f64; 3]) -> J {
    let [lon, lat] = utm.inverse(p[0], p[1]);
    json!([lon, lat])
}

/// Геометрия объекта итога в градусах. Тело - верхними гранями.
fn out_geometry(g: &Geom, utm: &Utm, geoms: &HashMap<i64, J>) -> J {
    match g {
        Geom::Mesh(b) => {
            let tri = |f: &[u32; 3]| -> Option<[[f64; 3]; 3]> {
                Some([
                    *b.verts.get(f[0] as usize)?,
                    *b.verts.get(f[1] as usize)?,
                    *b.verts.get(f[2] as usize)?,
                ])
            };
            let up = |t: &[[f64; 3]; 3]| {
                let (a, b, c) = (t[0], t[1], t[2]);
                (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) > 0.0
            };
            let all: Vec<[[f64; 3]; 3]> = b.faces.iter().filter_map(tri).collect();
            let top: Vec<&[[f64; 3]; 3]> = all.iter().filter(|t| up(t)).collect();
            let polys: Vec<J> = top
                .iter()
                .map(|t| {
                    json!([[
                        lonlat(utm, &t[0]),
                        lonlat(utm, &t[1]),
                        lonlat(utm, &t[2]),
                        lonlat(utm, &t[0])
                    ]])
                })
                .collect();
            json!({"type": "MultiPolygon", "coordinates": polys})
        }
        Geom::Line(p) => json!({
            "type": "LineString",
            "coordinates": p.iter().map(|q| lonlat(utm, q)).collect::<Vec<_>>(),
        }),
        Geom::Point(p) => json!({"type": "Point", "coordinates": lonlat(utm, p)}),
        Geom::Source(id) => geoms.get(id).cloned().unwrap_or(J::Null),
    }
}

/// Запуск инструмента по запросу RustGIS. Ответ - `{"ok": ...}` или
/// `{"error": "..."}`.
pub fn run(req: &J, catalog: &Catalog, lang: Lang) -> J {
    let id = req["id"].as_str().unwrap_or("");
    let name = id.replace('-', "_");
    let found = OFFERED
        .contains(&name.as_str())
        .then(|| catalog.tool(&name).zip(tools::run_fn(&name)))
        .flatten();
    let Some((tool, run)) = found else {
        return json!({"error": tr(lang, "Инструмента «%s» нет.", &[Arg::Text(id.to_string())])});
    };
    let bad = |key: String| {
        let label = tool
            .param(&key)
            .map_or(key.clone(), |p| p.label.get(lang).to_string());
        let why = tr(lang, "%s: значение не того вида", &[Arg::Text(label)]);
        json!({"error": tr(lang, "Значения не годятся: %s", &[Arg::Text(why)])})
    };
    let (given, utm, geoms) = match values(tool, req) {
        Ok(v) => v,
        Err(key) => return bad(key),
    };
    let values = match convert::values(tool, &given) {
        Ok(v) => v,
        Err(key) => return bad(key),
    };
    let wrong = tool.check(&values, lang);
    if !wrong.is_empty() {
        return json!({"error": tr(lang, "Значения не годятся: %s", &[Arg::Text(wrong.join(", "))])});
    }
    let mut report = VecReport::default();
    let result = run(&values, &mut report);
    let mut messages: Vec<String> = report
        .lines
        .iter()
        .map(|(_, m)| tool_spec::format(tool.translate(m.template, lang), &m.args))
        .collect();
    let out = match result {
        Ok(o) => o,
        Err(e) => {
            let text = tool_spec::format(tool.translate(e.0.template, lang), &e.0.args);
            return json!({"error": tr(lang, "Расчёт не удался: %s", &[Arg::Text(text)])});
        }
    };
    let mut layers = Vec::new();
    for (key, o) in &out {
        // имя слоя - подпись выхода на языке RustGIS, у выхода без поля -
        // имя из расчёта
        let name = tool.param(key).map_or_else(
            || {
                o.name
                    .map_or(key.clone(), |n| tool.translate(n, lang).to_string())
            },
            |p| p.label.get(lang).to_string(),
        );
        let features: Vec<J> = o
            .features
            .iter()
            .map(|f| {
                let mut props = serde_json::Map::new();
                for ((field, _), v) in o.fields.iter().zip(&f.values) {
                    props.insert(field.clone(), out_value(v));
                }
                for (field, v) in &f.named {
                    props.insert(field.clone(), out_value(v));
                }
                if let Some(s) = f.source {
                    props.entry("source").or_insert(json!(s));
                }
                json!({
                    "type": "Feature",
                    "properties": props,
                    "geometry": out_geometry(&f.geom, &utm, &geoms),
                })
            })
            .collect();
        layers.push(json!({
            "name": name,
            "geojson": {"type": "FeatureCollection", "features": features},
        }));
        if o.geometry == OutGeometry::Mesh {
            messages.push(tr(
                lang,
                "%s: тела в плане, верхними гранями, метры зоны UTM %s.",
                &[
                    Arg::Text(tool.display_name(lang)),
                    Arg::Text(format!("{}{}", utm.zone, if utm.north { "N" } else { "S" })),
                ],
            ));
        }
    }
    json!({"ok": {"layers": layers, "messages": messages}})
}

fn out_value(v: &tool_spec::OutValue) -> J {
    use tool_spec::OutValue;
    match v {
        OutValue::Double(x) if x.is_finite() => json!(x),
        OutValue::Double(_) | OutValue::Null => J::Null,
        OutValue::Int(i) => json!(i),
        OutValue::Str(s) => json!(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Catalog {
        tool_spec::parse_catalog(include_bytes!("../../../fixtures/tools.json")).unwrap()
    }

    fn manifest() -> J {
        let path = format!("{}/plugin.json", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    /// Вставить раздел tools и новые строки перевода в текст манифеста,
    /// не трогая порядок и вид остального: раздел - перед i18n, по
    /// инструменту на строку, переводы - в начало `ru`.
    fn splice(text: &str, tools: &[J], ru: &serde_json::Map<String, J>) -> String {
        let mut text = text.to_string();
        if let Some(s) = text.find("\n  \"tools\": [") {
            let e = s + text[s..].find("\n  ],").unwrap() + "\n  ],".len();
            text.replace_range(s..e, "");
        }
        let lines: Vec<String> = tools.iter().map(|t| format!("    {t}")).collect();
        let block = format!("\n  \"tools\": [\n{}\n  ],", lines.join(",\n"));
        let at = text.find("\n  \"i18n\": {").unwrap();
        text.insert_str(at, &block);
        let m: J = serde_json::from_str(&text).unwrap();
        let mut add = String::new();
        for (k, v) in ru {
            if m["i18n"]["ru"].get(k).is_none() {
                add.push_str(&format!("      {}: {v},\n", json!(k)));
            }
        }
        let at = text.find("\"ru\": {\n").unwrap() + "\"ru\": {\n".len();
        text.insert_str(at, &add);
        text
    }

    /// Раздел tools манифеста - из tools.json, у каждой строки перевод.
    /// `ISOLINER_MANIFEST=1 cargo test -p isoliner-rustgis` переписывает
    /// раздел и строки в plugin.json.
    #[test]
    fn manifest_tools_follow_tools_json() {
        let (tools, ru) = manifest_tools(&catalog());
        let mut m = manifest();
        if std::env::var_os("ISOLINER_MANIFEST").is_some() {
            let path = format!("{}/plugin.json", env!("CARGO_MANIFEST_DIR"));
            let text = std::fs::read_to_string(&path).unwrap();
            std::fs::write(&path, splice(&text, &tools, &ru)).unwrap();
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
        let [e0, n0] = utm.forward(56.0, 54.7);
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
        let got = run(&req, &cat, Lang::Ru);
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

    #[test]
    fn bad_requests_get_an_error() {
        let cat = catalog();
        let r = run(&json!({"id": "fill-to-volume"}), &cat, Lang::En);
        assert_eq!(r["error"], "No tool 'fill-to-volume'.");
        // слоя с таким именем нет в запросе
        let r = run(
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
        let r = run(
            &json!({"id": "sweep-body", "params": {"input": "a", "profile": "Square"}, "layers": {"a": fc}}),
            &cat,
            Lang::En,
        );
        assert!(r["error"].is_string(), "{r}");
        let r = run(
            &json!({"id": "sweep-body", "params": {"input": "a", "width": "wide"}, "layers": {"a": fc}}),
            &cat,
            Lang::En,
        );
        assert!(r["error"].is_string(), "{r}");
        // порченая геометрия - пропуск, не падение
        let junk = json!({"type": "FeatureCollection", "features": [
            {"type": "Feature", "geometry": {"type": "LineString", "coordinates": [["a"], [1]]}},
            {"type": "Feature", "geometry": null}, 5]});
        let r = run(
            &json!({"id": "sweep-body", "params": {"input": "a"}, "layers": {"a": junk}}),
            &cat,
            Lang::En,
        );
        assert!(r.get("ok").is_some() || r.get("error").is_some());
    }
}
