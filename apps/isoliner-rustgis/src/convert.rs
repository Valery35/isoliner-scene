// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Значения полей из JSON запроса RustGIS и выходы расчёта в JSON.
//!
//! Вид значения берётся из описания поля в tools.json, а не из имени
//! поля. Слои - в том же виде, что и прогоны модуля в
//! `fixtures/tool_runs.json`:
//!
//! - линии - `[{"id", "parts": [[[x, y, z], ...]], "attrs": {}}]`,
//!   у точки без Z третья координата `null`;
//! - точки - `[{"id", "xyz": [x, y, z] | null, "attrs": {}}]`;
//! - тело - `{"verts": [[x, y, z]], "faces": [[a, b, c]]}`;
//! - слой тел - `[{"id", "attrs", "verts", "faces"}]`;
//! - объекты любой геометрии - как линии;
//! - таблица - `[{"id", "attrs", "xyz": [x, y, z] | null}]`;
//! - сведения о слое - `"@КЛЮЧ": {"name", "geometry", "hasz",
//!   "fields", "numeric"}`.
//!
//! Объект выхода несёт геометрию `{"verts", "faces"}`, `{"points"}`,
//! `{"xyz"}` или `{"source": id}` - смотря по виду выхода.
//!
//! Входные данные считаются враждебными: номер вершины грани
//! проверяется по числу вершин.

use std::collections::{BTreeMap, HashMap};

use serde_json::{Value as J, json};
use tool_spec::{
    Body, Geom, LayerInfo, LineFeature, MeshFeature, OutGeometry, OutValue, Output, Param,
    ParamKind, PointFeature, Row, Tool, Value, Values,
};

fn point(v: &J) -> Option<[f64; 3]> {
    let a = v.as_array()?;
    if a.len() < 2 || a.len() > 3 {
        return None;
    }
    let z = match a.get(2) {
        None | Some(J::Null) => f64::NAN,
        Some(z) => z.as_f64()?,
    };
    Some([a[0].as_f64()?, a[1].as_f64()?, z])
}

fn points(v: &J) -> Option<Vec<[f64; 3]>> {
    v.as_array()?.iter().map(point).collect()
}

fn attrs(v: &J) -> HashMap<String, J> {
    v.as_object()
        .map(|o| o.iter().map(|(k, x)| (k.clone(), x.clone())).collect())
        .unwrap_or_default()
}

fn lines(v: &J) -> Option<Vec<LineFeature>> {
    v.as_array()?
        .iter()
        .enumerate()
        .map(|(i, f)| {
            Some(LineFeature {
                id: f.get("id").and_then(J::as_i64).unwrap_or(i as i64),
                parts: f
                    .get("parts")?
                    .as_array()?
                    .iter()
                    .map(points)
                    .collect::<Option<_>>()?,
                attrs: attrs(&f["attrs"]),
            })
        })
        .collect()
}

fn point_features(v: &J) -> Option<Vec<PointFeature>> {
    v.as_array()?
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let xyz = match f.get("xyz") {
                None | Some(J::Null) => None,
                Some(p) => Some(point(p)?),
            };
            Some(PointFeature {
                id: f.get("id").and_then(J::as_i64).unwrap_or(i as i64),
                xyz,
                attrs: attrs(&f["attrs"]),
            })
        })
        .collect()
}

fn body(v: &J) -> Option<Body> {
    let verts = points(v.get("verts")?)?;
    let n = verts.len() as u64;
    let faces = v
        .get("faces")?
        .as_array()?
        .iter()
        .map(|f| {
            let a = f.as_array()?;
            if a.len() != 3 {
                return None;
            }
            let k = |i: usize| {
                a.get(i)
                    .and_then(J::as_u64)
                    .filter(|&x| x < n)
                    .and_then(|x| u32::try_from(x).ok())
            };
            Some([k(0)?, k(1)?, k(2)?])
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Body { verts, faces })
}

/// Слой тел: `[{"id", "attrs", "verts", "faces"}]`, у объекта без
/// граней verts и faces - null.
fn mesh_features(v: &J) -> Option<Vec<MeshFeature>> {
    v.as_array()?
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let body = match (f.get("verts"), f.get("faces")) {
                (None | Some(J::Null), _) | (_, None | Some(J::Null)) => None,
                _ => Some(body(f)?),
            };
            Some(MeshFeature {
                id: f.get("id").and_then(J::as_i64).unwrap_or(i as i64),
                attrs: attrs(&f["attrs"]),
                body,
            })
        })
        .collect()
}

/// Таблица: `[{"id", "attrs", "xyz": [x, y, z] | null}]`.
fn rows(v: &J) -> Option<Vec<Row>> {
    v.as_array()?
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let xyz = match f.get("xyz") {
                None | Some(J::Null) => None,
                Some(p) => Some(point(p)?),
            };
            Some(Row {
                id: f.get("id").and_then(J::as_i64).unwrap_or(i as i64),
                attrs: attrs(&f["attrs"]),
                xyz,
            })
        })
        .collect()
}

fn strings(v: &J) -> Option<Vec<String>> {
    v.as_array()?
        .iter()
        .map(|s| s.as_str().map(str::to_string))
        .collect()
}

/// Сведения о слое `@КЛЮЧ`: имя, геометрия, Z, поля.
fn layer(v: &J) -> Option<LayerInfo> {
    let o = v.as_object()?;
    let s = |k: &str| o.get(k).and_then(J::as_str).unwrap_or_default().to_string();
    Some(LayerInfo {
        name: s("name"),
        geometry: s("geometry"),
        hasz: o.get("hasz").and_then(J::as_bool).unwrap_or(false),
        fields: o.get("fields").and_then(strings).unwrap_or_default(),
        numeric: o.get("numeric").and_then(strings).unwrap_or_default(),
    })
}

/// Значение одного поля по его описанию. None - значение не того вида.
fn value(p: &Param, v: &J) -> Option<Value> {
    Some(match p.kind {
        ParamKind::Number => Value::Number(v.as_f64()?),
        ParamKind::Integer | ParamKind::Band => Value::Integer(v.as_i64()?),
        ParamKind::Enum if p.multiple => Value::Enums(
            v.as_array()?
                .iter()
                .map(|i| usize::try_from(i.as_u64()?).ok())
                .collect::<Option<_>>()?,
        ),
        ParamKind::Enum => Value::Enum(usize::try_from(v.as_u64()?).ok()?),
        ParamKind::Field if p.multiple => Value::Fields(strings(v)?),
        ParamKind::Field => Value::Field(v.as_str()?.to_string()),
        ParamKind::String => Value::Str(v.as_str()?.to_string()),
        ParamKind::Bool => Value::Bool(v.as_bool()?),
        ParamKind::Lines => Value::Lines(lines(v)?),
        ParamKind::Features => Value::Features(lines(v)?),
        ParamKind::Points => Value::Points(point_features(v)?),
        ParamKind::Mesh => Value::Mesh(body(v)?),
        ParamKind::Meshes => Value::Meshes(mesh_features(v)?),
        ParamKind::Table => Value::Table(rows(v)?),
        // растра среди значений пока нет: ждёт переноса 2.13
        ParamKind::Raster => return None,
        // необязательный выход: заказан или нет
        ParamKind::Sink if p.optional => Value::Bool(v.as_bool()?),
        ParamKind::Sink => return None,
    })
}

/// Значения полей: умолчания инструмента, поверх - значения запроса.
/// Ключи `@КЛЮЧ` - сведения о слоях, как в модуле. Ошибка - ключ
/// поля, значение которого не того вида.
pub fn values(tool: &Tool, given: &J) -> Result<Values, String> {
    let mut out = tool.defaults();
    let Some(given) = given.as_object() else {
        return Ok(out);
    };
    for p in &tool.params {
        match given.get(&p.key) {
            None | Some(J::Null) => {}
            Some(_) if p.kind == ParamKind::Sink && !p.optional => {}
            Some(v) => {
                let val = value(p, v).ok_or_else(|| p.key.clone())?;
                out.insert(p.key.clone(), val);
            }
        }
    }
    for (k, v) in given.iter().filter(|(k, _)| k.starts_with('@')) {
        let info = layer(v).ok_or_else(|| k.clone())?;
        out.insert(k.clone(), Value::Layer(info));
    }
    Ok(out)
}

fn out_value(v: &OutValue) -> J {
    match v {
        OutValue::Double(x) if x.is_finite() => json!(x),
        OutValue::Double(_) | OutValue::Null => J::Null,
        OutValue::Int(i) => json!(i),
        OutValue::Str(s) => json!(s),
    }
}

/// Геометрия объекта выхода: тело, линия, точка или объект источника.
fn geometry(g: &Geom) -> J {
    match g {
        Geom::Mesh(b) => json!({"verts": b.verts, "faces": b.faces}),
        Geom::Line(p) => json!({"points": p}),
        Geom::Point(p) => json!({"xyz": p}),
        Geom::Source(id) => json!({"source": id}),
        Geom::None => J::Null,
        Geom::Grid(g) => json!({"grid": {"transform": g.transform, "nx": g.nx, "ny": g.ny,
            "z0": g.z0, "dz": g.dz, "bands": g.bands}}),
    }
}

/// Выходы расчёта: поля, объекты с телами и значениями.
pub fn outputs(tool: &Tool, out: &BTreeMap<String, Output>, lang: tool_spec::Lang) -> J {
    let mut map = serde_json::Map::new();
    for (key, o) in out {
        let fields: Vec<J> = o
            .fields
            .iter()
            .map(|(name, t)| json!({"name": name, "type": t.name()}))
            .collect();
        let features: Vec<J> = o
            .features
            .iter()
            .map(|f| {
                let mut x = json!({
                    "source": f.source,
                    "values": f.values.iter().map(out_value).collect::<Vec<_>>(),
                    "geometry": geometry(&f.geom),
                });
                if o.replace {
                    let named: serde_json::Map<String, J> = f
                        .named
                        .iter()
                        .map(|(k, v)| (k.clone(), out_value(v)))
                        .collect();
                    x["named"] = J::Object(named);
                }
                x
            })
            .collect();
        let geom = match o.geometry {
            OutGeometry::Mesh => "mesh",
            OutGeometry::Line => "line",
            OutGeometry::Point => "point",
            OutGeometry::Source => "source",
            OutGeometry::None => "none",
            OutGeometry::Grid => "grid",
        };
        map.insert(
            key.clone(),
            json!({
                "name": o.name.map(|n| tool.translate(n, lang).to_string()),
                "keep_source": o.keep_source,
                "geometry": geom,
                "replace": o.replace,
                "fields": fields,
                "features": features,
            }),
        );
    }
    J::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_layers_are_refused() {
        // номер вершины за краем
        let bad = json!({"verts": [[0, 0, 0], [1, 0, 0], [0, 1, 0]], "faces": [[0, 1, 3]]});
        assert!(body(&bad).is_none());
        let good = json!({"verts": [[0, 0, 0], [1, 0, 0], [0, 1, 0]], "faces": [[0, 1, 2]]});
        assert_eq!(body(&good).map(|b| b.faces.len()), Some(1));
        // точка из одной координаты, текст вместо числа
        assert!(point(&json!([1.0])).is_none());
        assert!(point(&json!(["a", 1.0, 2.0])).is_none());
        // Z не задан - NaN, как у линии без Z в модуле
        assert!(point(&json!([1.0, 2.0, null])).is_some_and(|p| p[2].is_nan()));
        assert!(lines(&json!([{"id": 1}])).is_none());
    }
}
