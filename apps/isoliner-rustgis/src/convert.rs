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
//! - тело - `{"verts": [[x, y, z]], "faces": [[a, b, c]]}`.
//!
//! Входные данные считаются враждебными: номер вершины грани
//! проверяется по числу вершин.

use std::collections::{BTreeMap, HashMap};

use serde_json::{Value as J, json};
use tool_spec::{
    Body, LineFeature, OutValue, Output, ParamKind, PointFeature, Tool, Value, Values,
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

/// Значение одного поля по его виду. None - значение не того вида.
fn value(kind: ParamKind, v: &J) -> Option<Value> {
    Some(match kind {
        ParamKind::Number => Value::Number(v.as_f64()?),
        ParamKind::Integer => Value::Integer(v.as_i64()?),
        ParamKind::Enum => Value::Enum(usize::try_from(v.as_u64()?).ok()?),
        ParamKind::Field => Value::Field(v.as_str()?.to_string()),
        ParamKind::Lines => Value::Lines(lines(v)?),
        ParamKind::Points => Value::Points(point_features(v)?),
        ParamKind::Mesh => Value::Mesh(body(v)?),
        ParamKind::Sink => return None,
    })
}

/// Значения полей: умолчания инструмента, поверх - значения запроса.
/// Ошибка - ключ поля, значение которого не того вида.
pub fn values(tool: &Tool, given: &J) -> Result<Values, String> {
    let mut out = tool.defaults();
    let Some(given) = given.as_object() else {
        return Ok(out);
    };
    for p in &tool.params {
        match given.get(&p.key) {
            None | Some(J::Null) => {}
            Some(_) if p.kind == ParamKind::Sink => {}
            Some(v) => {
                let val = value(p.kind, v).ok_or_else(|| p.key.clone())?;
                out.insert(p.key.clone(), val);
            }
        }
    }
    Ok(out)
}

fn out_value(v: &OutValue) -> J {
    match v {
        OutValue::Double(x) if x.is_finite() => json!(x),
        OutValue::Double(_) => J::Null,
        OutValue::Int(i) => json!(i),
        OutValue::Str(s) => json!(s),
    }
}

/// Выходы расчёта: поля, объекты с телами и значениями.
pub fn outputs(tool: &Tool, out: &BTreeMap<String, Output>, lang: tool_spec::Lang) -> J {
    let mut map = serde_json::Map::new();
    for (key, o) in out {
        let fields: Vec<J> = o
            .fields
            .iter()
            .map(|(name, t)| json!({"name": name, "type": format!("{t:?}").to_lowercase()}))
            .collect();
        let features: Vec<J> = o
            .features
            .iter()
            .map(|f| {
                json!({
                    "source": f.source,
                    "values": f.values.iter().map(out_value).collect::<Vec<_>>(),
                    "verts": f.body.verts,
                    "faces": f.body.faces,
                })
            })
            .collect();
        map.insert(
            key.clone(),
            json!({
                "name": o.name.map(|n| tool.translate(n, lang).to_string()),
                "keep_source": o.keep_source,
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
