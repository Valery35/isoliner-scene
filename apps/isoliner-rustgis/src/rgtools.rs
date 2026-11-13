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
/// числа. 2.17 берёт устья точками или таблицей с X и Y, отметку устья -
/// из поля. Остальным нужны тела (2.11, 2.12, 2.14, 2.23) или линии с Z
/// (2.18), а слой RustGIS их не держит.
pub const OFFERED: &[&str] = &[
    "sweep_body",
    "contour_crown",
    "bench_report",
    "desurvey_holes",
    "fill_to_volume",
    "hole_intervals",
    "select_by_shell",
    "zonal_shell_stats",
    "boolean_shells",
];

/// Слой-таблица с X и Y в метрах своей системы: устья 2.17. Если у слоя
/// нет геометрии, расчёт идёт в его метрах, итог уходит в RustGIS в тех
/// же метрах с кодом системы из поля [`CRS_PARAM`], пересчитывает его
/// сам RustGIS. Слои RustGIS отдаёт расширению только в градусах, а
/// таблица без геометрии хранит X и Y как есть.
const XY_TABLES: &[(&str, &str)] = &[("desurvey_holes", "COLLARS")];

/// Поле окна RustGIS, которого нет в tools.json: система координат X и Y
/// таблицы.
const CRS_PARAM: &str = "crs";
#[cfg(test)]
const CRS_LABEL: (&str, &str) = (
    "Coordinate system of X and Y (EPSG)",
    "Система координат X и Y (EPSG)",
);
#[cfg(test)]
const CRS_HELP: (&str, &str) = (
    "For a collar table without points: the EPSG code of its X and Y, for example EPSG:28410. The traces go to the project in that system, RustGIS draws them in place. Points need no code.",
    "Для таблицы устьев без точек: код EPSG её X и Y, например EPSG:28410. Стволы ложатся в проект в этой системе, RustGIS ставит их на место сам. Точкам код не нужен.",
);

/// Система координат итога: метры зоны UTM, взятой по градусам входа
/// (итог переводится обратно в градусы), или метры таблицы с кодом EPSG.
#[derive(Debug, Clone)]
enum Frame {
    Utm(Utm),
    Native(String),
}

impl Frame {
    fn coord(&self, p: &[f64; 3]) -> J {
        match self {
            Frame::Utm(u) => lonlat(u, p),
            Frame::Native(_) => json!([p[0], p[1]]),
        }
    }
}

/// Код EPSG из текста поля: `EPSG:28410`, `epsg:28410` или `28410`.
pub fn epsg(text: &str) -> Option<String> {
    let t = text.trim();
    let digits = t
        .strip_prefix("EPSG:")
        .or_else(|| t.strip_prefix("epsg:"))
        .unwrap_or(t);
    (!digits.is_empty() && digits.len() <= 9 && digits.chars().all(|c| c.is_ascii_digit()))
        .then(|| format!("EPSG:{digits}"))
}

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
        // тела - слой итога Isoliner3D, в RustGIS это полигоны плана
        ParamKind::Mesh | ParamKind::Meshes => "polygonLayer",
        ParamKind::Field if p.numeric => "numericField",
        ParamKind::Field => "field",
        ParamKind::Number | ParamKind::Integer => "number",
        ParamKind::Enum if !p.multiple => "choice",
        // несколько пунктов: у RustGIS такого вида нет, пункты - текстом
        ParamKind::Enum => "text",
        ParamKind::Bool => "bool",
        ParamKind::String => "text",
        _ => return None,
    })
}

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
            if p.kind == ParamKind::Enum && p.multiple {
                // пункты через запятую, их перечень - в подсказке
                let pick = |l: Lang| -> String {
                    let ix: Vec<usize> = match &p.default {
                        Some(J::Array(a)) => {
                            a.iter().filter_map(J::as_u64).map(|i| i as usize).collect()
                        }
                        Some(J::Number(n)) => {
                            n.as_u64().map(|i| vec![i as usize]).unwrap_or_default()
                        }
                        _ => Vec::new(),
                    };
                    ix.iter()
                        .filter_map(|&i| p.options.get(i))
                        .map(|o| o.get(l).to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let all = |l: Lang| -> String {
                    p.options
                        .iter()
                        .map(|o| o.get(l).to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let help_en = format!("{} Comma-separated, from {}.", hint, all(Lang::En));
                let help_ru = format!(
                    "{} Через запятую, из списка {}.",
                    p.hint.get(Lang::Ru),
                    all(Lang::Ru)
                );
                say(&help_en, &help_ru);
                o["help"] = json!(help_en);
                let d = pick(Lang::En);
                if !d.is_empty() {
                    o["default"] = json!(d);
                }
            } else if p.kind == ParamKind::Enum {
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
        if XY_TABLES.iter().any(|(n, _)| *n == t.name) {
            say(CRS_LABEL.0, CRS_LABEL.1);
            say(CRS_HELP.0, CRS_HELP.1);
            params.push(json!({"name": CRS_PARAM, "label": CRS_LABEL.0, "kind": "text", "help": CRS_HELP.0}));
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

/// Метры входа до микрометра: градусы туда и обратно дают шум около
/// 1e-9 м, а ячейка расчёта на ровной границе от него сдвигается.
fn snap(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

/// Тело из треугольников с высотой (кольца по 3 вершины, как модуль пишет
/// тела) в метрах зоны: `{"verts", "faces"}`. Одинаковые вершины
/// склеиваются - по номерам вершин расчёт проверяет замкнутость тела.
/// Треугольника нет - None.
fn body_of(geoms: &[&J], utm: &Utm) -> Option<J> {
    let mut verts: Vec<[f64; 3]> = Vec::new();
    let mut index: HashMap<[u64; 3], u32> = HashMap::new();
    let mut faces: Vec<[u32; 3]> = Vec::new();
    for g in geoms {
        for ring in parts(g) {
            let ring: Vec<[f64; 3]> = if ring.len() == 4 && ring[0] == ring[3] {
                ring[..3].to_vec()
            } else {
                ring
            };
            if ring.len() != 3 || ring.iter().any(|p| !p[2].is_finite()) {
                continue;
            }
            let mut f = [0u32; 3];
            for (k, p) in ring.iter().enumerate() {
                let [e, n] = utm.forward(p[0], p[1]).map(snap);
                let key = [e.to_bits(), n.to_bits(), p[2].to_bits()];
                let next = u32::try_from(verts.len()).ok()?;
                f[k] = *index.entry(key).or_insert_with(|| {
                    verts.push([e, n, p[2]]);
                    next
                });
            }
            faces.push(f);
        }
    }
    (!faces.is_empty()).then(|| json!({"verts": verts, "faces": faces}))
}

/// Слой RustGIS в вид прогонов модуля, в метрах зоны.
fn layer_value(p: &Param, fc: &J, utm: &Utm) -> J {
    let feats = fc["features"].as_array().cloned().unwrap_or_default();
    match p.kind {
        // одно тело из всех объектов слоя
        ParamKind::Mesh => {
            let gs: Vec<&J> = feats.iter().map(|f| &f["geometry"]).collect();
            return body_of(&gs, utm).unwrap_or(J::Null);
        }
        // тело на объект
        ParamKind::Meshes => {
            return J::Array(
                feats
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let mut o = json!({"id": feature_id(f, i), "attrs": f["properties"]});
                        if let Some(b) = body_of(&[&f["geometry"]], utm) {
                            o["verts"] = b["verts"].clone();
                            o["faces"] = b["faces"].clone();
                        }
                        o
                    })
                    .collect(),
            );
        }
        _ => {}
    }
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
                            let [e, n] = utm.forward(x, y).map(snap);
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

/// Сведения о слое `@КЛЮЧ`, как их даёт модуль: имя, геометрия, Z,
/// поля по порядку первого появления, числовые поля. Z у слоёв RustGIS
/// нет никогда.
fn layer_info(name: &str, fc: &J) -> J {
    let feats = fc["features"].as_array().cloned().unwrap_or_default();
    let geometry = feats
        .iter()
        .find_map(|f| f["geometry"]["type"].as_str())
        .map_or("none", |t| match t {
            "Point" | "MultiPoint" => "point",
            "LineString" | "MultiLineString" => "line",
            "Polygon" | "MultiPolygon" => "polygon",
            _ => "none",
        });
    let mut fields: Vec<String> = Vec::new();
    let mut text: Vec<String> = Vec::new();
    for f in &feats {
        for (k, v) in f["properties"].as_object().into_iter().flatten() {
            if !fields.contains(k) {
                fields.push(k.clone());
            }
            if v.is_string() && !text.contains(k) {
                text.push(k.clone());
            }
        }
    }
    let numeric: Vec<&String> = fields.iter().filter(|k| !text.contains(k)).collect();
    // Z есть только у итогов Isoliner3D, слой RustGIS его не держит
    let hasz = feats.iter().any(|f| {
        parts(&f["geometry"])
            .iter()
            .flatten()
            .any(|p| p[2].is_finite())
    });
    json!({"name": name, "geometry": geometry, "hasz": hasz, "fields": fields, "numeric": numeric})
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

/// Геометрии входов по ключу поля-слоя и номеру объекта: их берут
/// выходы «объект источника».
type Sources = HashMap<String, HashMap<i64, J>>;

/// Входная ошибка запроса: ключ поля.
type BadKey = String;

/// Значения полей в виде прогонов модуля и зона пересчёта.
fn values(tool: &Tool, req: &J) -> Result<(J, Utm, Sources), BadKey> {
    let params = &req["params"];
    let layers = &req["layers"];
    // слои входа: имя слоя - в поле, объекты - в layers
    let mut inputs: Vec<(&Param, J)> = Vec::new();
    for p in tool.params.iter().filter(|p| is_layer(p.kind)) {
        let name = params[param_name(&p.key)].as_str().unwrap_or("");
        if name.is_empty() {
            continue;
        }
        let fc = layers.get(name).ok_or_else(|| p.key.clone())?;
        // тела нужны с высотой: слой RustGIS их не держит, берутся из
        // итога Isoliner3D, иначе ошибка с объяснением
        let mesh = matches!(p.kind, ParamKind::Mesh | ParamKind::Meshes);
        if mesh && !has_3d(&bodies_dir(), name) {
            return Err(format!("!{name}"));
        }
        // итог Isoliner3D - геометрия с высотой, поля слоя RustGIS
        inputs.push((p, with_bodies(name, fc.clone())));
    }
    let utm = zone_of(&inputs.iter().map(|(_, fc)| fc).collect::<Vec<_>>());
    let mut out = serde_json::Map::new();
    let mut geoms: Sources = HashMap::new();
    for (p, fc) in &inputs {
        let info = layer_info(params[param_name(&p.key)].as_str().unwrap_or(""), fc);
        out.insert(format!("@{}", p.key), info);
        out.insert(p.key.clone(), layer_value(p, fc, &utm));
        let by_id = geoms.entry(p.key.clone()).or_default();
        for (i, f) in fc["features"].as_array().into_iter().flatten().enumerate() {
            by_id
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
            // несколько пунктов - текстом через запятую или списком
            ParamKind::Enum if p.multiple => {
                let items: Vec<J> = match v {
                    J::Array(a) => a.clone(),
                    J::String(s) => s
                        .split([',', ';'])
                        .map(str::trim)
                        .filter(|x| !x.is_empty())
                        .map(|x| json!(x))
                        .collect(),
                    _ => return Err(p.key.clone()),
                };
                let idx: Option<Vec<usize>> = items.iter().map(|x| choice(p, x)).collect();
                json!(idx.ok_or_else(|| p.key.clone())?)
            }
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

/// Геометрия объекта итога в системе `frame`. Тело - верхними гранями.
fn out_geometry(g: &Geom, frame: &Frame, geoms: &HashMap<i64, J>) -> J {
    let lonlat = |_: &Frame, p: &[f64; 3]| frame.coord(p);
    let utm = frame;
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
        // строка таблицы и куб - без геометрии слоя
        Geom::None | Geom::Grid(_) => J::Null,
    }
}

/// Запуск инструмента по запросу RustGIS. Ответ - `{"ok": ...}` или
/// `{"error": "..."}`.
pub fn run(
    req: &J,
    catalog: &Catalog,
    lang: Lang,
    launch: &dyn Fn(&std::path::Path) -> Result<(), String>,
) -> J {
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
        if let Some(layer) = key.strip_prefix('!') {
            return json!({"error": tr(
                lang,
                "Слой «%s» - не тела Isoliner3D. Тела с высотой дают инструменты 2.24 и 2.26, слой RustGIS их не хранит.",
                &[Arg::Text(layer.to_string())],
            )});
        }
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
    // таблица с X и Y без геометрии - метры своей системы с кодом EPSG
    let table_xy = XY_TABLES.iter().any(|(n, k)| {
        *n == tool.name && given.get(*k).is_some() && given[format!("@{k}")]["geometry"] == "none"
    });
    let frame = if table_xy {
        let code = req["params"][CRS_PARAM].as_str().and_then(epsg);
        match code {
            Some(c) => Frame::Native(c),
            None => {
                return json!({"error": tr(
                    lang,
                    "У таблицы устьев без точек нужен код системы координат X и Y, например EPSG:28410.",
                    &[],
                )});
            }
        }
    } else {
        Frame::Utm(utm)
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
        .map(|(_, m)| crate::i18n::report_text(tool, m, lang))
        .collect();
    let out = match result {
        Ok(o) => o,
        Err(e) => {
            let text = crate::i18n::report_text(tool, &e.0, lang);
            return json!({"error": tr(lang, "Расчёт не удался: %s", &[Arg::Text(text)])});
        }
    };
    let mut layers = Vec::new();
    let mut tables = Vec::new();
    let mut in_scene: Vec<(String, J)> = Vec::new();
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
        // объекты источника - геометрией своего входа
        let empty = HashMap::new();
        let src = o
            .keep_source
            .as_ref()
            .and_then(|k| geoms.get(k))
            .or_else(|| {
                tool.params
                    .iter()
                    .find(|p| is_layer(p.kind) && geoms.contains_key(&p.key))
                    .and_then(|p| geoms.get(&p.key))
            })
            .unwrap_or(&empty);
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
                // отметки тела и линии: высоты слой RustGIS не держит
                if let Some((lo, hi)) = z_range(&f.geom) {
                    props.entry("z_min").or_insert(json!(lo));
                    props.entry("z_max").or_insert(json!(hi));
                }
                json!({
                    "type": "Feature",
                    "properties": props,
                    "geometry": strip_z(&out_geometry(&f.geom, &frame, src)),
                })
            })
            .collect();
        // объекты источника с высотой (итог Isoliner3D на входе) - тоже
        // в сцену и следующим инструментам
        let src_z = o.geometry == OutGeometry::Source
            && o.features.iter().any(|f| match f.geom {
                Geom::Source(id) => src.get(&id).is_some_and(has_z),
                _ => false,
            });
        let solid = src_z
            || matches!(
                o.geometry,
                OutGeometry::Mesh | OutGeometry::Line | OutGeometry::Point
            );
        if solid && let Frame::Utm(u) = &frame {
            let g3: Vec<J> = o
                .features
                .iter()
                .map(|f| match f.geom {
                    Geom::Source(id) => src.get(&id).cloned().unwrap_or(J::Null),
                    ref g => geom_3d(g, u),
                })
                .collect();
            keep_bodies(&bodies_dir(), &name, &features, g3.clone());
            // в сцену - итог с высотой, поля - как у слоя RustGIS
            let feats3: Vec<J> = features
                .iter()
                .zip(g3)
                .map(|(f, g)| json!({"type": "Feature", "properties": f["properties"], "geometry": g}))
                .collect();
            in_scene.push((
                name.clone(),
                json!({"type": "FeatureCollection", "features": feats3}),
            ));
        }
        let mut fc = json!({"type": "FeatureCollection", "features": features});
        // метры таблицы: RustGIS читает член crs и пересчитывает сам
        if let Frame::Native(code) = &frame {
            fc["crs"] = json!({"type": "name", "properties": {"name": code}});
        }
        // таблица без геометрии - в tables, RustGIS кладёт её таблицей
        if o.geometry == OutGeometry::None {
            tables.push(json!({"name": name, "geojson": fc}));
        } else {
            layers.push(json!({"name": name, "geojson": fc}));
        }
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
    // объёмный итог - сразу в окно сцены своим пакетом
    if let Some(m) = show_in_scene(
        &tool.name,
        &tool.display_name(lang),
        &in_scene,
        launch,
        lang,
    ) {
        messages.push(m);
    }
    json!({"ok": {"layers": layers, "tables": tables, "messages": messages}})
}

/// Пакет объёмного итога инструмента `name` и окно сцены с ним: окно
/// открыто - оно перечитывает пакет само, иначе запускается. Сообщение
/// для журнала RustGIS, None - объёмного итога нет.
fn show_in_scene(
    name: &str,
    title: &str,
    layers: &[(String, J)],
    launch: &dyn Fn(&std::path::Path) -> Result<(), String>,
    lang: Lang,
) -> Option<String> {
    if layers.is_empty() {
        return None;
    }
    let path = results_dir().join(format!("{name}.i3dscene"));
    let set = crate::session::Layers {
        list: layers.to_vec(),
        crs: None,
    };
    let written = crate::session::write_scene(&set, &path, title);
    let shown = written.and_then(|_| {
        if crate::session::window_open_for(&path) {
            Ok(())
        } else {
            launch(&path)
        }
    });
    Some(match shown {
        Ok(()) => tr(
            lang,
            "%s: объёмный итог - в окне сцены.",
            &[Arg::Text(title.to_string())],
        ),
        Err(e) => tr(lang, "Окно сцены не запустилось: %s", &[Arg::Text(e)]),
    })
}

/// Где лежат пакеты объёмных итогов инструментов.
pub fn results_dir() -> std::path::PathBuf {
    crate::session::scene_path().with_extension("results")
}

/// Где лежат тела итогов для сцены: рядом с пакетом сцены из слоёв.
fn bodies_dir() -> std::path::PathBuf {
    crate::session::scene_path().with_extension("bodies")
}

/// Имя файла тел слоя: имя слоя без знаков, которых не любят папки.
fn bodies_file(dir: &std::path::Path, layer: &str) -> std::path::PathBuf {
    let safe: String = layer
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    dir.join(format!("{safe}.json"))
}

/// Точка с высотой в градусах GeoJSON. Высоты нет - без третьей координаты.
fn lonlat_z(utm: &Utm, v: &[f64; 3]) -> J {
    let [lon, lat] = utm.inverse(v[0], v[1]);
    if v[2].is_finite() {
        json!([lon, lat, v[2]])
    } else {
        json!([lon, lat])
    }
}

/// Геометрия итога с высотой в градусах: тело - треугольниками
/// MultiPolygon, как модуль пишет тела, линия и точка - с Z. Объект
/// источника - None, у него своя геометрия слоя.
fn geom_3d(g: &Geom, utm: &Utm) -> J {
    match g {
        Geom::Mesh(b) => {
            let tris: Vec<J> = b
                .faces
                .iter()
                .filter_map(|t| {
                    let p = |i: u32| b.verts.get(i as usize).map(|v| lonlat_z(utm, v));
                    Some(json!([[p(t[0])?, p(t[1])?, p(t[2])?, p(t[0])?]]))
                })
                .collect();
            json!({"type": "MultiPolygon", "coordinates": tris})
        }
        Geom::Line(p) => json!({
            "type": "LineString",
            "coordinates": p.iter().map(|q| lonlat_z(utm, q)).collect::<Vec<_>>(),
        }),
        Geom::Point(p) => json!({"type": "Point", "coordinates": lonlat_z(utm, p)}),
        Geom::Source(_) | Geom::None | Geom::Grid(_) => J::Null,
    }
}

/// Есть ли у геометрии высоты.
fn has_z(g: &J) -> bool {
    parts(g).iter().flatten().any(|p| p[2].is_finite())
}

/// Геометрия без высот: план для слоя RustGIS.
fn strip_z(g: &J) -> J {
    fn walk(v: &J) -> J {
        match v.as_array() {
            Some(a) if a.first().is_some_and(J::is_number) => {
                J::Array(a.iter().take(2).cloned().collect())
            }
            Some(a) => J::Array(a.iter().map(walk).collect()),
            None => v.clone(),
        }
    }
    let mut out = g.clone();
    if let Some(c) = g.get("coordinates") {
        out["coordinates"] = walk(c);
    }
    out
}

/// Геометрии итога с высотой по объектам: их берут сцена и следующие
/// инструменты, слой RustGIS высоты не держит.
#[cfg(test)]
fn geoms_3d(o: &tool_spec::Output, utm: &Utm) -> Vec<J> {
    o.features.iter().map(|f| geom_3d(&f.geom, utm)).collect()
}

/// Сохранить 3D итога слоя `layer`: план, который ушёл в RustGIS, и
/// геометрии с высотой. Не вышло - сцена покажет план, расчёт от этого
/// не страдает.
fn keep_bodies(dir: &std::path::Path, layer: &str, plan: &[J], geoms: Vec<J>) {
    let firsts: Vec<J> = plan.iter().map(|f| first_point(&f["geometry"])).collect();
    let doc = json!({"plan": firsts, "geoms": geoms});
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(bodies_file(dir, layer), doc.to_string());
}

/// Отметки геометрии итога: наименьшая и наибольшая Z.
fn z_range(g: &Geom) -> Option<(f64, f64)> {
    let pts: Vec<f64> = match g {
        Geom::Mesh(b) => b.verts.iter().map(|v| v[2]).collect(),
        Geom::Line(p) => p.iter().map(|v| v[2]).collect(),
        _ => return None,
    };
    let z: Vec<f64> = pts.into_iter().filter(|z| z.is_finite()).collect();
    let lo = z.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = z.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    (lo <= hi).then_some((lo, hi))
}

/// Первая точка геометрии - метка объекта плана.
fn first_point(g: &J) -> J {
    let mut c = &g["coordinates"];
    while c
        .as_array()
        .is_some_and(|a| a.first().is_some_and(J::is_array))
    {
        c = &c[0];
    }
    c.clone()
}

/// Слой проекта для сцены: если это итог тел Isoliner3D (то же имя, те же
/// объекты плана), его объекты - тела с высотой, иначе слой как есть.
pub fn with_bodies(layer: &str, fc: J) -> J {
    with_bodies_in(&bodies_dir(), layer, fc)
}

fn with_bodies_in(dir: &std::path::Path, layer: &str, fc: J) -> J {
    let Ok(text) = std::fs::read_to_string(bodies_file(dir, layer)) else {
        return fc;
    };
    let Ok(doc) = serde_json::from_str::<J>(&text) else {
        return fc;
    };
    let feats = fc["features"].as_array().cloned().unwrap_or_default();
    let plan = doc["plan"].as_array().cloned().unwrap_or_default();
    let close = |a: &J, b: &J| {
        let (a, b) = (a.as_array(), b.as_array());
        match (a, b) {
            (Some(a), Some(b)) if a.len() >= 2 && b.len() >= 2 => (0..2).all(|i| {
                let (x, y) = (a[i].as_f64(), b[i].as_f64());
                x.zip(y).is_some_and(|(x, y)| (x - y).abs() < 1e-7)
            }),
            _ => false,
        }
    };
    let same = feats.len() == plan.len()
        && feats
            .iter()
            .zip(&plan)
            .all(|(f, p)| close(&first_point(&f["geometry"]), p));
    if !same {
        return fc;
    }
    // геометрии с высотой на место плана, поля - слоя RustGIS
    let geoms: Vec<J> = match doc.get("geoms") {
        Some(J::Array(g)) => g.clone(),
        _ => doc["bodies"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|f| f["geometry"].clone())
            .collect(),
    };
    let features: Vec<J> = feats
        .into_iter()
        .zip(geoms)
        .map(|(mut f, g)| {
            if !g.is_null() {
                f["geometry"] = g;
            }
            f
        })
        .collect();
    let mut out = fc;
    out["features"] = J::Array(features);
    out
}

/// Есть ли у слоя 3D итога Isoliner3D.
fn has_3d(dir: &std::path::Path, layer: &str) -> bool {
    bodies_file(dir, layer).is_file()
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
        let [e0, n0] = utm.forward(56.0, 54.7).map(f64::round);
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
        let dir = std::env::temp_dir().join(format!("isoliner-bodies-{}", std::process::id()));
        let utm = Utm::at(56.0, 54.7);
        let [e, n] = utm.forward(56.0, 54.7);
        let out = tool_spec::Output {
            fields: Vec::new(),
            features: vec![tool_spec::OutFeature {
                geom: Geom::Mesh(tool_spec::Body {
                    verts: vec![[e, n, -300.0], [e + 10.0, n, -300.0], [e, n + 10.0, -297.0]],
                    faces: vec![[0, 1, 2]],
                }),
                source: Some(1),
                values: Vec::new(),
                named: Vec::new(),
            }],
            keep_source: None,
            name: None,
            geometry: OutGeometry::Mesh,
            replace: false,
        };
        let plan = vec![json!({"type": "Feature", "properties": {},
            "geometry": out_geometry(&out.features[0].geom, &Frame::Utm(utm), &HashMap::new())})];
        keep_bodies(&dir, "Тела выработок", &plan, geoms_3d(&out, &utm));
        let fc = json!({"type": "FeatureCollection", "features": plan});
        let got = with_bodies_in(&dir, "Тела выработок", fc.clone());
        let tri = &got["features"][0]["geometry"]["coordinates"][0][0];
        assert_eq!(tri[2][2], -297.0);
        // другое имя или другие объекты - слой как есть
        assert_eq!(with_bodies_in(&dir, "axes", fc.clone()), fc);
        let moved = json!({"type": "FeatureCollection", "features": [
            {"type": "Feature", "geometry": {"type": "Point", "coordinates": [10.0, 10.0]}}]});
        assert_eq!(with_bodies_in(&dir, "Тела выработок", moved.clone()), moved);
        let _ = std::fs::remove_dir_all(&dir);
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
    fn body_layer(layer: &str, verts: &J, faces: &J) -> (J, Utm, [f64; 2]) {
        let utm = Utm::at(56.0, 54.7);
        let [e0, n0] = utm.forward(56.0, 54.7).map(f64::round);
        let verts: Vec<[f64; 3]> = verts
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                [
                    e0 + v[0].as_f64().unwrap(),
                    n0 + v[1].as_f64().unwrap(),
                    v[2].as_f64().unwrap(),
                ]
            })
            .collect();
        let faces: Vec<[u32; 3]> = faces
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                [
                    f[0].as_u64().unwrap() as u32,
                    f[1].as_u64().unwrap() as u32,
                    f[2].as_u64().unwrap() as u32,
                ]
            })
            .collect();
        let g = Geom::Mesh(tool_spec::Body { verts, faces });
        let plan = vec![json!({"type": "Feature", "row": 0, "properties": {},
            "geometry": out_geometry(&g, &Frame::Utm(utm), &HashMap::new())})];
        keep_bodies(&bodies_dir(), layer, &plan, vec![geom_3d(&g, &utm)]);
        (
            json!({"type": "FeatureCollection", "features": plan}),
            utm,
            [e0, n0],
        )
    }

    /// 2.23 по итогу тел в RustGIS: уровень и объёмы как у модуля, слой
    /// без тел Isoliner3D - понятная ошибка.
    #[test]
    fn fill_to_volume_from_a_body_layer() {
        let cat = catalog();
        let tool = cat.tool("fill_to_volume").unwrap();
        for (k, r) in runs_of("fill_to_volume").iter().enumerate() {
            let layer = format!("Тело {k}");
            let (fc, _, _) = body_layer(&layer, &r["input"]["verts"], &r["input"]["faces"]);
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
        assert!(e.starts_with("Слой «Контур» - не тела Isoliner3D"), "{e}");
    }

    /// Линии с высотой в метрах образца (`[{"id", "attrs", "parts"}]`) -
    /// итогом Isoliner3D в хранилище слоя `layer`. Ответ - план слоя,
    /// каким его отдаст RustGIS.
    fn line_layer(layer: &str, feats: &J) -> J {
        let utm = Utm::at(56.0, 54.7);
        let [e0, n0] = utm.forward(56.0, 54.7).map(f64::round);
        let mut plan = Vec::new();
        let mut g3 = Vec::new();
        for (i, f) in feats.as_array().unwrap().iter().enumerate() {
            let pts: Vec<[f64; 3]> = f["parts"][0]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    [
                        e0 + p[0].as_f64().unwrap(),
                        n0 + p[1].as_f64().unwrap(),
                        p[2].as_f64().unwrap_or(f64::NAN),
                    ]
                })
                .collect();
            let g = Geom::Line(pts);
            plan.push(
                json!({"type": "Feature", "row": i, "properties": f["attrs"],
                "geometry": out_geometry(&g, &Frame::Utm(utm), &HashMap::new())}),
            );
            g3.push(geom_3d(&g, &utm));
        }
        keep_bodies(&bodies_dir(), layer, &plan, g3);
        json!({"type": "FeatureCollection", "features": plan})
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
        let utm = Utm::at(56.0, 54.7);
        let [e0, n0] = utm.forward(56.0, 54.7).map(f64::round);
        let at = |p: &J| -> [f64; 3] {
            [
                e0 + p[0].as_f64().unwrap(),
                n0 + p[1].as_f64().unwrap(),
                p[2].as_f64().unwrap_or(f64::NAN),
            ]
        };
        let body = |v: &J, f: &J| -> Geom {
            Geom::Mesh(tool_spec::Body {
                verts: v.as_array().unwrap().iter().map(at).collect(),
                faces: f
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|t| {
                        [
                            t[0].as_u64().unwrap() as u32,
                            t[1].as_u64().unwrap() as u32,
                            t[2].as_u64().unwrap() as u32,
                        ]
                    })
                    .collect(),
            })
        };
        // объекты образца: (номер, поля, геометрия в метрах)
        let items: Vec<(J, J, Geom)> = match kind {
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
                    let pts: Vec<[f64; 3]> = match f.get("xyz") {
                        Some(p) if !p.is_null() => vec![at(p)],
                        _ => f["parts"][0].as_array().unwrap().iter().map(at).collect(),
                    };
                    let g = if pts.len() == 1 {
                        Geom::Point(pts[0])
                    } else {
                        Geom::Line(pts)
                    };
                    (f["id"].clone(), f["attrs"].clone(), g)
                })
                .collect(),
        };
        let mut plan = Vec::new();
        let mut g3 = Vec::new();
        for (id, attrs, g) in &items {
            plan.push(json!({"type": "Feature", "row": id, "properties": attrs,
                "geometry": out_geometry(g, &Frame::Utm(utm), &HashMap::new())}));
            g3.push(geom_3d(g, &utm));
        }
        keep_bodies(&bodies_dir(), layer, &plan, g3);
        json!({"type": "FeatureCollection", "features": plan})
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
        let [e0, n0] = utm.forward(56.0, 54.7).map(f64::round);
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
