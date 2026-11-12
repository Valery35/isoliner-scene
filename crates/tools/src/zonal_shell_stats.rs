// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! 2.14 Зональная статистика оболочек. Перенос `tools/zonal_shell_stats.py`.
//!
//! По каждому телу записывается, сколько объектов в него попало: точки
//! лучом вверх по чётности, линии режутся телом, и в длину идёт часть
//! внутри. Числа ложатся в поля тел (`replace`), второй выход - сами
//! объекты с телом, в которое они попали.

use std::collections::{BTreeMap, HashSet};

use tool_spec::{
    Arg, FieldType, Geom, LayerInfo, LineFeature, MeshFeature, Message, OutFeature, OutGeometry,
    OutValue, Output, Report, ToolError, Value, Values, attr_number,
};

use crate::{boolean, field, shell, zonal};

/// Поля объектов во втором выходе; занятое имя получает номер: zone_2.
const ZONE_FIELDS: [(&str, FieldType); 3] = [
    ("zone", FieldType::String),
    ("zone_ids", FieldType::String),
    ("zone_n", FieldType::Int),
];

fn err(template: &'static str, args: Vec<Arg>) -> ToolError {
    ToolError(Message::new(template, args))
}

/// Латиница по-русски, как `_TRANSLIT` модуля.
fn translit(ch: char) -> Option<&'static str> {
    Some(match ch {
        'а' => "a",
        'б' => "b",
        'в' => "v",
        'г' => "g",
        'д' => "d",
        'е' | 'ё' | 'э' => "e",
        'ж' => "zh",
        'з' => "z",
        'и' => "i",
        'й' => "j",
        'к' => "k",
        'л' => "l",
        'м' => "m",
        'н' => "n",
        'о' => "o",
        'п' => "p",
        'р' => "r",
        'с' => "s",
        'т' => "t",
        'у' => "u",
        'ф' => "f",
        'х' => "h",
        'ц' => "c",
        'ч' => "ch",
        'ш' => "sh",
        'щ' => "sch",
        'ъ' | 'ь' => "",
        'ы' => "y",
        'ю' => "yu",
        'я' => "ya",
        _ => return None,
    })
}

/// Имя поля из имени слоя: латиница, подчёркивания, не длиннее 20.
pub fn slug(name: &str) -> String {
    let mut s = String::new();
    for ch in name.to_lowercase().chars() {
        if let Some(t) = translit(ch) {
            s.push_str(t);
        } else if ch.is_ascii_alphanumeric() {
            s.push(ch);
        } else {
            s.push('_');
        }
    }
    let mut s = s.trim_matches('_').to_string();
    while s.contains("__") {
        s = s.replace("__", "_");
    }
    if s.is_empty() || s.starts_with(|c: char| c.is_ascii_digit()) {
        s = format!("l_{s}");
    }
    s.chars().take(20).collect()
}

/// Значение атрибута строкой, как `str()` в Python.
fn py_str(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(b) => if *b { "True" } else { "False" }.into(),
        serde_json::Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => i.to_string(),
            (None, Some(x)) if x.fract() == 0.0 && x.abs() < 1e16 => format!("{x:.1}"),
            (None, Some(x)) => x.to_string(),
            _ => n.to_string(),
        },
        other => other.to_string(),
    }
}

fn unique(key: &str, have: &HashSet<String>) -> String {
    let mut nm = key.to_string();
    let mut k = 2;
    while have.contains(&nm) {
        nm = format!("{key}_{k}");
        k += 1;
    }
    nm
}

fn layer<'a>(values: &'a Values, key: &str, empty: &'a LayerInfo) -> &'a LayerInfo {
    match values.get(key) {
        Some(Value::Layer(l)) => l,
        _ => empty,
    }
}

/// Объект с частями, у которых есть высота, и его значение атрибута.
struct Obj<'a> {
    item: &'a LineFeature,
    parts: Vec<&'a Vec<[f64; 3]>>,
    val: Option<f64>,
}

pub fn run(
    values: &Values,
    report: &mut dyn Report,
) -> Result<BTreeMap<String, Output>, ToolError> {
    let empty = LayerInfo::default();
    let meta_in = layer(values, "@INPUT", &empty);
    let meta_sh = layer(values, "@SHELLS", &empty);
    let is_pt = meta_in.geometry == "point";
    let is_ln = meta_in.geometry == "line";
    let mut name = match values.get("NAME") {
        Some(Value::Str(s)) => s.trim().to_string(),
        _ => String::new(),
    };
    if name.is_empty() {
        name = slug(if meta_in.name.is_empty() {
            "layer"
        } else {
            &meta_in.name
        });
    }
    let attr = field(values, "ATTR");
    let picked: Vec<usize> = match values.get("STATS") {
        Some(Value::Enums(list)) if !list.is_empty() => list.clone(),
        _ => vec![0],
    };
    let mut keys: Vec<&str> = picked.iter().filter_map(|&i| zonal::key(i)).collect();
    if keys.is_empty() {
        keys.push("sum");
    }
    let overwrite = matches!(values.get("OVERWRITE"), Some(Value::Bool(true)));
    let zone_field = field(values, "ZONE_FIELD");

    // поля результата; столкновение имён - отказ, если не разрешено
    let mut new_fields = vec![(format!("{name}_n"), FieldType::Int)];
    if is_ln {
        new_fields.push((format!("{name}_len"), FieldType::Double));
    }
    if !attr.is_empty() {
        for suf in zonal::suffixes(&keys) {
            new_fields.push((format!("{name}{suf}"), FieldType::Double));
        }
    }
    let have: HashSet<&String> = meta_sh.fields.iter().collect();
    let clash: Vec<&str> = new_fields
        .iter()
        .filter(|(nm, _)| have.contains(nm))
        .map(|(nm, _)| nm.as_str())
        .collect();
    if !clash.is_empty() && !overwrite {
        return Err(err(
            "Поля %s уже есть у слоя тел. Задайте другое имя или разрешите перезапись.",
            vec![Arg::Text(clash.join(", "))],
        ));
    }

    let shells_in: &[MeshFeature] = match values.get("SHELLS") {
        Some(Value::Meshes(m)) => m,
        _ => &[],
    };
    let mut shells: Vec<(&MeshFeature, bool)> = Vec::new();
    for item in shells_in {
        if report.canceled() {
            break;
        }
        let closed = item
            .body
            .as_ref()
            .is_some_and(|b| !b.faces.is_empty() && shell::defects(&b.verts, &b.faces).0 == 0);
        shells.push((item, closed));
    }
    let n_open = shells.iter().filter(|(_, c)| !c).count();
    report.info(Message::new(
        "Тел: %d, незамкнутых %d.",
        vec![shells.len().into(), n_open.into()],
    ));
    if n_open > 0 {
        report.warn(Message::new(
            "Незамкнутых тел %d: им записано пусто, а не ноль. У незамкнутого тела нет внутренности.",
            vec![n_open.into()],
        ));
    }

    let input: &[LineFeature] = match values.get("INPUT") {
        Some(Value::Features(x) | Value::Lines(x)) => x,
        _ => &[],
    };
    let mut objs: Vec<Obj> = Vec::new();
    for item in input {
        if report.canceled() {
            break;
        }
        let parts: Vec<&Vec<[f64; 3]>> = item
            .parts
            .iter()
            .filter(|p| !p.is_empty() && p.iter().flatten().all(|x| x.is_finite()))
            .collect();
        if parts.is_empty() {
            continue;
        }
        let val = if attr.is_empty() {
            None
        } else {
            attr_number(item.attrs.get(&attr))
        };
        objs.push(Obj { item, parts, val });
    }
    if objs.is_empty() {
        return Err(err(
            "Объектов с высотой не нашлось: у геометрии должна быть Z.",
            vec![],
        ));
    }
    let seg_len = |p: &[[f64; 3]]| -> f64 {
        p.windows(2)
            .map(|w| {
                ((w[1][0] - w[0][0]).powi(2)
                    + (w[1][1] - w[0][1]).powi(2)
                    + (w[1][2] - w[0][2]).powi(2))
                .sqrt()
            })
            .sum()
    };
    let total_len: f64 = if is_ln {
        objs.iter()
            .flat_map(|o| o.parts.iter())
            .map(|p| seg_len(p))
            .sum()
    } else {
        0.0
    };
    let mut all_pts: Vec<[f64; 3]> = Vec::new();
    let mut owner: Vec<usize> = Vec::new();
    let mut starts_xyz: Vec<[f64; 3]> = Vec::new();
    for (oi, o) in objs.iter().enumerate() {
        for p in &o.parts {
            all_pts.extend_from_slice(p);
            owner.extend(std::iter::repeat_n(oi, p.len()));
            starts_xyz.push(p[0]);
        }
    }
    report.info(Message::new("Объектов: %d.", vec![objs.len().into()]));

    let mut out = Output {
        fields: new_fields.clone(),
        keep_source: Some("SHELLS".into()),
        geometry: OutGeometry::Source,
        replace: true,
        name: Some("Тела со статистикой"),
        ..Output::default()
    };
    // тела, в которые попал каждый объект, по порядку слоя тел
    let mut zones: Vec<Vec<String>> = vec![Vec::new(); objs.len()];
    let mut grand_n = 0usize;
    let mut grand_len = 0.0;
    let n_shells = shells.len().max(1) as f64;
    for (si, (item, closed)) in shells.iter().enumerate() {
        let label = if zone_field.is_empty() {
            item.id.to_string()
        } else {
            item.attrs.get(&zone_field).map(py_str).unwrap_or_default()
        };
        let mut row: Vec<(String, OutValue)> = Vec::new();
        if let (true, Some(b)) = (*closed, item.body.as_ref()) {
            let (v, f) = (&b.verts, &b.faces);
            let bbox = boolean::tri_bbox(v, f);
            let mut n_in = 0usize;
            let mut len_in = 0.0;
            let mut in_vals: Vec<Option<f64>> = Vec::new();
            let mut in_w: Vec<f64> = Vec::new();
            if is_ln {
                // начала всех ломаных - одним пакетом
                let starts = boolean::points_inside(v, f, &starts_xyz);
                let mut pi = 0;
                for (oi, o) in objs.iter().enumerate() {
                    let mut hit = false;
                    let mut got_obj = 0.0;
                    for p in &o.parts {
                        let (got, _tot) = boolean::polyline_inside_length(
                            v,
                            f,
                            p,
                            &bbox,
                            starts.get(pi).copied(),
                        );
                        pi += 1;
                        if got > 0.0 {
                            hit = true;
                            len_in += got;
                            got_obj += got;
                        }
                    }
                    if hit {
                        n_in += 1;
                        in_vals.push(o.val);
                        in_w.push(got_obj);
                        zones[oi].push(label.clone());
                    }
                }
            } else {
                // точки всех объектов - одним пакетом
                let ins = boolean::points_inside(v, f, &all_pts);
                for (oi, o) in objs.iter().enumerate() {
                    let sel: Vec<bool> = owner
                        .iter()
                        .zip(&ins)
                        .filter(|(w, _)| **w == oi)
                        .map(|(_, i)| *i)
                        .collect();
                    let hit = if is_pt {
                        sel.iter().all(|x| *x)
                    } else {
                        sel.iter().any(|x| *x)
                    };
                    if hit {
                        n_in += 1;
                        in_vals.push(o.val);
                        in_w.push(1.0);
                        zones[oi].push(label.clone());
                    }
                }
            }
            row.push((format!("{name}_n"), OutValue::Int(n_in as i64)));
            if is_ln {
                row.push((format!("{name}_len"), OutValue::Double(len_in)));
                grand_len += len_in;
            }
            if !attr.is_empty() {
                for (suf, x) in zonal::stats(&in_vals, &in_w, &keys) {
                    row.push((
                        format!("{name}{suf}"),
                        x.map_or(OutValue::Null, OutValue::Double),
                    ));
                }
            }
            grand_n += n_in;
        }
        out.features.push(OutFeature {
            geom: Geom::Source(item.id),
            source: Some(item.id),
            values: Vec::new(),
            named: row,
        });
        report.progress(100.0 * (si + 1) as f64 / n_shells);
    }
    let mut result = BTreeMap::from([("OUTPUT".to_string(), out)]);
    if matches!(values.get("OUTPUT_OBJECTS"), Some(Value::Bool(true))) {
        let mut have_in: HashSet<String> = meta_in.fields.iter().cloned().collect();
        let mut zone_out = Vec::new();
        for (key, typ) in ZONE_FIELDS {
            let nm = unique(key, &have_in);
            have_in.insert(nm.clone());
            zone_out.push((nm, typ));
        }
        let mut objs_out = Output {
            fields: zone_out,
            keep_source: Some("INPUT".into()),
            geometry: OutGeometry::Source,
            name: Some("Объекты с телом"),
            ..Output::default()
        };
        let mut n_hit = 0usize;
        for (o, zs) in objs.iter().zip(&zones) {
            let (first, ids, n) = zonal::zone_fields(zs);
            let s = |x: Option<String>| x.map_or(OutValue::Null, OutValue::Str);
            objs_out.features.push(OutFeature {
                geom: Geom::Source(o.item.id),
                source: Some(o.item.id),
                values: vec![s(first), s(ids), OutValue::Int(n as i64)],
                named: Vec::new(),
            });
            n_hit += usize::from(n > 0);
        }
        report.info(Message::new(
            "Объектов в телах: %d из %d.",
            vec![n_hit.into(), objs.len().into()],
        ));
        result.insert("OUTPUT_OBJECTS".to_string(), objs_out);
    }
    report.info(Message::new(
        "Попаданий по всем телам: %d.",
        vec![grand_n.into()],
    ));
    if is_ln {
        report.info(Message::new(
            "Длина линий: всего %.2f м, внутри тел %.2f м, снаружи %.2f м.",
            vec![
                total_len.into(),
                grand_len.into(),
                (total_len - grand_len).into(),
            ],
        ));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_like_the_module() {
        assert_eq!(slug("Пробы Au"), "proby_au");
        assert_eq!(slug("Борозды"), "borozdy");
        assert_eq!(slug("2024 керн"), "l_2024_kern");
        assert_eq!(slug("!!!"), "l_");
        assert_eq!(py_str(&serde_json::json!(7.0)), "7.0");
        assert_eq!(py_str(&serde_json::json!(7)), "7");
    }
}
