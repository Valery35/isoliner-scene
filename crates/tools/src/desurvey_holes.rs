// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! 2.17 Стволы скважин по инклинометрии. Перенос `tools/desurvey_holes.py`.
//!
//! Математика минимальной кривизны - в [`crate::wells`], угадывание
//! полей по именам - в [`crate::colnames`]. Здесь поля, сведение устьев
//! с замерами и сборка результата: линии с Z, длина каждой равна
//! глубине по стволу.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use tool_spec::{
    Arg, FieldType, Geom, LayerInfo, Message, OutFeature, OutGeometry, OutValue, Output, Report,
    Row, ToolError, Value, Values, attr_number,
};

use crate::{choice, colnames, num, py_str, text, wells};

fn err(template: &'static str, args: Vec<Arg>) -> ToolError {
    ToolError(Message::new(template, args))
}

/// Номер скважины как строка без пробелов по краям.
fn key(v: Option<&serde_json::Value>) -> String {
    v.map(py_str).unwrap_or_default().trim().to_string()
}

fn layer<'a>(values: &'a Values, k: &str, empty: &'a LayerInfo) -> &'a LayerInfo {
    match values.get(k) {
        Some(Value::Layer(l)) => l,
        _ => empty,
    }
}

pub fn run(
    values: &Values,
    report: &mut dyn Report,
) -> Result<BTreeMap<String, Output>, ToolError> {
    let collars: &[Row] = match values.get("COLLARS") {
        Some(Value::Table(t)) => t,
        _ => &[],
    };
    let survey: Option<&[Row]> = match values.get("SURVEY") {
        Some(Value::Table(t)) => Some(t),
        _ => None,
    };
    let empty = LayerInfo::default();
    let meta_c = layer(values, "@COLLARS", &empty);
    let meta_s = layer(values, "@SURVEY", &empty);
    let conv = wells::CONVENTIONS
        .get(choice(values, "CONVENTION"))
        .copied()
        .unwrap_or(wells::CONVENTIONS[0]);
    let step = crate::or(num(values, "STEP"), 1.0);

    let cn = &meta_c.fields;
    let f_hole = colnames::resolve(&text(values, "HOLE"), cn, "hole", &[]);
    let taken: Vec<&str> = f_hole.iter().map(String::as_str).collect();
    let f_eoh = colnames::resolve(&text(values, "EOH"), cn, "eoh", &taken);
    let mut f_caz = colnames::resolve(&text(values, "C_AZI"), cn, "s_azi", &[]);
    let mut f_cang = colnames::resolve(&text(values, "C_ANGLE"), cn, "s_angle", &[]);
    if f_caz.is_none() || f_cang.is_none() {
        f_caz = None;
        f_cang = None;
    }
    let has_geom = !matches!(meta_c.geometry.as_str(), "" | "none");
    let has_z = has_geom && meta_c.hasz;
    let mut f_x = text(values, "XFIELD");
    let mut f_y = text(values, "YFIELD");
    let mut f_z = text(values, "ZFIELD");
    if !has_geom {
        f_x = colnames::resolve(&f_x, cn, "collar_x", &[]).unwrap_or_default();
        f_y = colnames::resolve(&f_y, cn, "collar_y", &[]).unwrap_or_default();
    }
    if f_z.is_empty() && !has_z {
        f_z = colnames::guess(cn, "collar_z", &[]).unwrap_or_default();
    }
    let Some(f_hole) = f_hole else {
        return Err(err(
            "Не найдено поле номера скважины в устьях: выберите его.",
            vec![],
        ));
    };
    let mut s_hole = text(values, "S_HOLE");
    let mut s_d = text(values, "S_DEPTH");
    let mut s_a = text(values, "S_AZI");
    let mut s_g = text(values, "S_ANGLE");
    if survey.is_some() {
        let sn = &meta_s.fields;
        s_hole = colnames::resolve(&s_hole, sn, "hole", &[]).unwrap_or_default();
        s_d = colnames::resolve(&s_d, sn, "s_depth", &[&s_hole]).unwrap_or_default();
        s_a = colnames::resolve(&s_a, sn, "s_azi", &[]).unwrap_or_default();
        s_g = colnames::resolve(&s_g, sn, "s_angle", &[]).unwrap_or_default();
    }
    if let (Some(a), Some(g)) = (&f_caz, &f_cang) {
        report.info(Message::new(
            "Направление у устья: азимут %s, угол %s.",
            vec![Arg::Text(a.clone()), Arg::Text(g.clone())],
        ));
    }
    let point = "точка";
    let show = |s: &str| Arg::Text(if s.is_empty() { "-".into() } else { s.into() });
    report.info(Message::new(
        "Поля устьев: скважина %s, глубина %s, X %s, Y %s, Z %s. Инклинометрия: скважина %s, глубина %s, азимут %s, угол %s.",
        vec![
            show(&f_hole),
            show(f_eoh.as_deref().unwrap_or("")),
            show(if has_geom { point } else { &f_x }),
            show(if has_geom { point } else { &f_y }),
            show(if f_z.is_empty() { point } else { &f_z }),
            show(&s_hole),
            show(&s_d),
            show(&s_a),
            show(&s_g),
        ],
    ));

    // замеры по скважине, в порядке таблицы
    let mut surveys: HashMap<String, Vec<(f64, f64, f64)>> = HashMap::new();
    if let Some(rows) = survey {
        if s_hole.is_empty() || s_d.is_empty() || s_a.is_empty() || s_g.is_empty() {
            return Err(err(
                "У инклинометрии не выбраны поля: скважина, глубина, азимут и угол.",
                vec![],
            ));
        }
        let mut bad = 0usize;
        for row in rows {
            let at = &row.attrs;
            let (d, a, g) = (
                attr_number(at.get(&s_d)),
                attr_number(at.get(&s_a)),
                attr_number(at.get(&s_g)),
            );
            let (Some(d), Some(a), Some(g)) = (d, a, g) else {
                bad += 1;
                continue;
            };
            surveys
                .entry(key(at.get(&s_hole)))
                .or_default()
                .push((d, a, g));
        }
        if bad > 0 {
            report.warn(Message::new(
                "Строк инклинометрии с пустыми или нечисловыми значениями: %d, они пропущены.",
                vec![bad.into()],
            ));
        }
    }

    let mut out = Output {
        fields: vec![
            ("hole".into(), FieldType::String),
            ("eoh".into(), FieldType::Double),
            ("n_survey".into(), FieldType::Int),
        ],
        geometry: OutGeometry::Line,
        name: Some("Стволы скважин"),
        ..Output::default()
    };
    let (mut n_ok, mut n_vert, mut n_noz, mut n_skip) = (0usize, 0usize, 0usize, 0usize);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let none: Vec<(f64, f64, f64)> = Vec::new();
    for item in collars {
        if report.canceled() {
            break;
        }
        let at = &item.attrs;
        let name = key(at.get(&f_hole));
        if name.is_empty() {
            n_skip += 1;
            continue;
        }
        let (mut x, mut y, mut z) = (None, None, None);
        if let Some(p) = item.xyz {
            x = Some(p[0]);
            y = Some(p[1]);
            z = has_z.then_some(p[2]);
        } else if !f_x.is_empty() && !f_y.is_empty() {
            x = attr_number(at.get(&f_x));
            y = attr_number(at.get(&f_y));
        }
        if !f_z.is_empty() {
            z = attr_number(at.get(&f_z));
        }
        let (Some(x), Some(y)) = (x, y) else {
            n_skip += 1;
            continue;
        };
        let z = match z {
            Some(z) if !z.is_nan() => z,
            _ => {
                n_noz += 1;
                0.0
            }
        };
        let rows = surveys.get(&name).unwrap_or(&none);
        seen.insert(name.clone());
        let eoh = f_eoh.as_ref().and_then(|f| attr_number(at.get(f)));
        let caz = f_caz.as_ref().and_then(|f| attr_number(at.get(f)));
        let cang = f_cang.as_ref().and_then(|f| attr_number(at.get(f)));
        let d: Vec<f64> = rows.iter().map(|r| r.0).collect();
        let a: Vec<f64> = rows.iter().map(|r| r.1).collect();
        let g: Vec<f64> = rows.iter().map(|r| r.2).collect();
        let (sd, sa, sg) = wells::with_collar(&d, &a, &g, caz, cang);
        if sd.is_empty() {
            n_vert += 1;
        }
        if sd.len() <= 1 && eoh.is_none() {
            n_skip += 1;
            continue;
        }
        let (pts, mds) = wells::desurvey([x, y, z], &sd, &sa, &sg, eoh, conv, step);
        if pts.len() < 2 {
            n_skip += 1;
            continue;
        }
        let depth = mds.last().copied().unwrap_or(0.0);
        out.features.push(OutFeature {
            geom: Geom::Line(pts),
            source: None,
            values: vec![
                OutValue::Str(name),
                OutValue::Double(depth),
                OutValue::Int(rows.len() as i64),
            ],
            named: Vec::new(),
        });
        n_ok += 1;
    }
    report.info(Message::new("Стволов: %d.", vec![n_ok.into()]));
    if n_vert > 0 {
        report.warn(Message::new(
            "Скважин без инклинометрии: %d. Они построены отвесными, а без глубины забоя пропущены.",
            vec![n_vert.into()],
        ));
    }
    if n_noz > 0 {
        report.warn(Message::new(
            "Устьев без отметки: %d. Они поставлены на ноль: выберите поле отметки или дайте устьям Z.",
            vec![n_noz.into()],
        ));
    }
    if n_skip > 0 {
        report.warn(Message::new(
            "Пропущено устьев: %d (нет номера, координат или глубины).",
            vec![n_skip.into()],
        ));
    }
    let mut orphan: Vec<&String> = surveys.keys().filter(|k| !seen.contains(*k)).collect();
    orphan.sort();
    if !orphan.is_empty() {
        let first: Vec<&str> = orphan.iter().take(5).map(|s| s.as_str()).collect();
        report.warn(Message::new(
            "Инклинометрия без устья: %d скважин, например %s.",
            vec![orphan.len().into(), Arg::Text(first.join(", "))],
        ));
    }
    Ok(BTreeMap::from([("OUTPUT".to_string(), out)]))
}
