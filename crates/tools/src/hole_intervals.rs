// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! 2.18 Интервалы по стволу скважины. Перенос `tools/hole_intervals.py`.
//!
//! Ствол - линия с Z, глубина по стволу - её длина от начала. Куски
//! ствола, равные интервалы и композиты считает [`crate::wells`], здесь
//! поля, сведение стволов с таблицей и сборка двух выходов: кусков
//! ствола и середин интервалов.

use std::collections::{BTreeMap, HashMap};

use tool_spec::{
    Arg, FieldType, Geom, LayerInfo, LineFeature, Message, OutFeature, OutGeometry, OutValue,
    Output, Report, Row, ToolError, Value, Values, attr_number,
};

use crate::{choice, colnames, num, or, py_str, text, wells};

type V3 = [f64; 3];

fn err(template: &'static str, args: Vec<Arg>) -> ToolError {
    ToolError(Message::new(template, args))
}

/// Номер скважины как строка без пробелов по краям.
fn key(v: Option<&serde_json::Value>) -> String {
    v.map(py_str).unwrap_or_default().trim().to_string()
}

/// Вершины ствола и глубины по стволу как длина. У составной линии
/// берётся первая часть: ствол один.
pub fn trace_xyz(parts: &[Vec<V3>]) -> Option<(Vec<V3>, Vec<f64>)> {
    let pts = parts.first()?.clone();
    if pts.len() < 2 || !pts.iter().flatten().all(|x| x.is_finite()) {
        return None;
    }
    let mut md = vec![0.0];
    for w in pts.windows(2) {
        let d = ((w[1][0] - w[0][0]).powi(2)
            + (w[1][1] - w[0][1]).powi(2)
            + (w[1][2] - w[0][2]).powi(2))
        .sqrt();
        md.push(md[md.len() - 1] + d);
    }
    Some((pts, md))
}

/// Значение поля таблицы как есть, со своим типом (`copy:TABLE`).
fn copy_value(v: Option<&serde_json::Value>) -> OutValue {
    match v {
        Some(serde_json::Value::Number(n)) => match n.as_i64() {
            Some(i) if !n.is_f64() => OutValue::Int(i),
            _ => n.as_f64().map_or(OutValue::Null, OutValue::Double),
        },
        Some(serde_json::Value::String(s)) => OutValue::Str(s.clone()),
        Some(serde_json::Value::Bool(b)) => OutValue::Int(i64::from(*b)),
        _ => OutValue::Null,
    }
}

fn layer<'a>(values: &'a Values, k: &str, empty: &'a LayerInfo) -> &'a LayerInfo {
    match values.get(k) {
        Some(Value::Layer(l)) => l,
        _ => empty,
    }
}

/// Два выхода: куски ствола и середины интервалов.
struct Outs {
    lines: Output,
    mids: Output,
}

impl Outs {
    /// Кусок ствола от a до b и его середина. Короче двух точек - нет.
    fn put(
        &mut self,
        name: &str,
        pts: &[V3],
        md: &[f64],
        a: f64,
        b: f64,
        extra: Vec<OutValue>,
    ) -> usize {
        let seg = wells::segment(pts, md, a, b);
        if seg.len() < 2 {
            return 0;
        }
        let mut row = vec![
            OutValue::Str(name.to_string()),
            OutValue::Double(a),
            OutValue::Double(b),
            OutValue::Double(b - a),
        ];
        row.extend(extra);
        let mid = wells::at_depth(pts, md, &[(a + b) / 2.0]);
        self.lines.features.push(OutFeature {
            geom: Geom::Line(seg),
            source: None,
            values: row.clone(),
            named: Vec::new(),
        });
        if let Some(m) = mid.first() {
            self.mids.features.push(OutFeature {
                geom: Geom::Point(*m),
                source: None,
                values: row,
                named: Vec::new(),
            });
        }
        1
    }
}

pub fn run(
    values: &Values,
    report: &mut dyn Report,
) -> Result<BTreeMap<String, Output>, ToolError> {
    let mode = choice(values, "MODE");
    let tab: Option<&[Row]> = match values.get("TABLE") {
        Some(Value::Table(t)) => Some(t),
        _ => None,
    };
    let empty = LayerInfo::default();
    let meta_t = layer(values, "@TRACES", &empty);
    let meta_b = layer(values, "@TABLE", &empty);
    let length = or(num(values, "LENGTH"), 1.0);
    let mut picked: Vec<String> = match values.get("VALUES") {
        Some(Value::Fields(f)) => f.clone(),
        _ => Vec::new(),
    };

    let Some(t_hole) = colnames::resolve(&text(values, "T_HOLE"), &meta_t.fields, "hole", &[])
    else {
        return Err(err(
            "Не найдено поле номера скважины в стволах: выберите его.",
            vec![],
        ));
    };
    let mut f_hole = text(values, "F_HOLE");
    let mut f_from = text(values, "F_FROM");
    let mut f_to = text(values, "F_TO");
    let tn = &meta_b.fields;
    if tab.is_some() {
        f_hole = colnames::resolve(&f_hole, tn, "hole", &[]).unwrap_or_default();
        f_from = colnames::resolve(&f_from, tn, "from", &[]).unwrap_or_default();
        f_to = colnames::resolve(&f_to, tn, "to", &[]).unwrap_or_default();
        let show = |s: &str| Arg::Text(if s.is_empty() { "-".into() } else { s.into() });
        report.info(Message::new(
            "Поля таблицы: скважина %s, от %s, до %s.",
            vec![show(&f_hole), show(&f_from), show(&f_to)],
        ));
    }
    if (mode == 0 || mode == 2)
        && (tab.is_none() || f_hole.is_empty() || f_from.is_empty() || f_to.is_empty())
    {
        return Err(err(
            "Для этого способа нужна таблица интервалов с полями скважины, начала и конца.",
            vec![],
        ));
    }

    // стволы по номеру, в порядке слоя; повтор номера заменяет ствол
    let trace_in: &[LineFeature] = match values.get("TRACES") {
        Some(Value::Lines(x) | Value::Features(x)) => x,
        _ => &[],
    };
    let mut order: Vec<String> = Vec::new();
    let mut traces: HashMap<String, (Vec<V3>, Vec<f64>)> = HashMap::new();
    for item in trace_in {
        if let Some(t) = trace_xyz(&item.parts) {
            let k = key(item.attrs.get(&t_hole));
            if !traces.contains_key(&k) {
                order.push(k.clone());
            }
            traces.insert(k, t);
        }
    }
    if traces.is_empty() {
        return Err(err("Стволов с отметками не нашлось.", vec![]));
    }

    let mut fields: Vec<(String, FieldType)> = vec![
        ("hole".into(), FieldType::String),
        ("from_m".into(), FieldType::Double),
        ("to_m".into(), FieldType::Double),
        ("length_m".into(), FieldType::Double),
    ];
    let mut names: Vec<String> = fields.iter().map(|(n, _)| n.clone()).collect();
    let mut carry: Vec<String> = Vec::new();
    if mode == 0 {
        // поля таблицы переходят как есть, со своим типом
        for nm in tn {
            if [&f_hole, &f_from, &f_to].contains(&nm) || names.contains(nm) {
                continue;
            }
            fields.push((nm.clone(), FieldType::Copy("TABLE".into())));
            names.push(nm.clone());
            carry.push(nm.clone());
        }
    } else if mode == 2 {
        if picked.is_empty() {
            picked = tn
                .iter()
                .filter(|nm| meta_b.numeric.contains(nm) && **nm != f_from && **nm != f_to)
                .cloned()
                .collect();
        }
        for nm in &picked {
            if !names.contains(nm) {
                fields.push((nm.clone(), FieldType::Double));
                names.push(nm.clone());
            }
        }
        fields.push(("cover".into(), FieldType::Double));
    }

    // строки таблицы по скважине, в порядке таблицы
    let mut rows: HashMap<String, Vec<(f64, f64, &Row)>> = HashMap::new();
    let mut row_order: Vec<String> = Vec::new();
    let mut n_bad = 0usize;
    if let (Some(t), true) = (tab, mode == 0 || mode == 2) {
        for item in t {
            let at = &item.attrs;
            let (a, b) = (attr_number(at.get(&f_from)), attr_number(at.get(&f_to)));
            let (Some(a), Some(b)) = (a, b) else {
                n_bad += 1;
                continue;
            };
            if b <= a {
                n_bad += 1;
                continue;
            }
            let k = key(at.get(&f_hole));
            if !rows.contains_key(&k) {
                row_order.push(k.clone());
            }
            rows.entry(k).or_default().push((a, b, item));
        }
    }

    let mut outs = Outs {
        lines: Output {
            fields: fields.clone(),
            geometry: OutGeometry::Line,
            name: Some("Интервалы"),
            ..Output::default()
        },
        mids: Output {
            fields,
            geometry: OutGeometry::Point,
            name: Some("Середины интервалов"),
            ..Output::default()
        },
    };
    let (mut made, mut beyond) = (0usize, 0usize);
    let none: Vec<(f64, f64, &Row)> = Vec::new();
    for name in &order {
        if report.canceled() {
            break;
        }
        let Some((pts, md)) = traces.get(name) else {
            continue;
        };
        let end = md.last().copied().unwrap_or(0.0);
        match mode {
            0 => {
                for (a, b, row) in rows.get(name).unwrap_or(&none) {
                    // хорды узлов короче дуги на доли миллиметра: проба
                    // до самого забоя кончается чуть дальше линии
                    if *b > end + 0.05 {
                        beyond += 1;
                    }
                    let extra = carry.iter().map(|c| copy_value(row.attrs.get(c))).collect();
                    made += outs.put(name, pts, md, *a, b.min(end), extra);
                }
            }
            1 => {
                for [a, b] in wells::equal_intervals(end, length, 0.0) {
                    made += outs.put(name, pts, md, a, b, Vec::new());
                }
            }
            _ => {
                let r = rows.get(name).unwrap_or(&none);
                if r.is_empty() {
                    continue;
                }
                let vv: Vec<Vec<f64>> = r
                    .iter()
                    .map(|(_, _, row)| {
                        picked
                            .iter()
                            .map(|c| attr_number(row.attrs.get(c)).unwrap_or(f64::NAN))
                            .collect()
                    })
                    .collect();
                let froms: Vec<f64> = r.iter().map(|x| x.0).collect();
                let tos: Vec<f64> = r.iter().map(|x| x.1).collect();
                let (iv, cv, cover) = wells::composite(&froms, &tos, &vv, length, 0.0, Some(end));
                for (k, [a, b]) in iv.iter().enumerate() {
                    if cover[k] <= 0.0 {
                        continue;
                    }
                    let mut extra: Vec<OutValue> = cv[k]
                        .iter()
                        .map(|x| {
                            if x.is_finite() {
                                OutValue::Double(*x)
                            } else {
                                OutValue::Null
                            }
                        })
                        .collect();
                    extra.push(OutValue::Double(cover[k]));
                    made += outs.put(name, pts, md, *a, *b, extra);
                }
            }
        }
    }
    report.info(Message::new(
        "Стволов: %d, интервалов: %d.",
        vec![traces.len().into(), made.into()],
    ));
    if n_bad > 0 {
        report.warn(Message::new(
            "Строк таблицы без начала и конца или с концом не дальше начала: %d, они пропущены.",
            vec![n_bad.into()],
        ));
    }
    if beyond > 0 {
        report.warn(Message::new(
            "Интервалов глубже забоя: %d. Они обрезаны по забою: проверьте глубину скважины в устьях.",
            vec![beyond.into()],
        ));
    }
    let mut orphan: Vec<&String> = row_order
        .iter()
        .filter(|k| !traces.contains_key(*k))
        .collect();
    orphan.sort();
    if !orphan.is_empty() {
        let first: Vec<&str> = orphan.iter().take(5).map(|s| s.as_str()).collect();
        report.warn(Message::new(
            "Интервалы без ствола: %d скважин, например %s.",
            vec![orphan.len().into(), Arg::Text(first.join(", "))],
        ));
    }
    Ok(BTreeMap::from([
        ("OUTPUT".to_string(), outs.lines),
        ("OUTPUT_POINTS".to_string(), outs.mids),
    ]))
}
