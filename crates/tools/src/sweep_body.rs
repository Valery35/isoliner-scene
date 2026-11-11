// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! 2.24 Тело вдоль выработки. Перенос `tools/sweep_body.py`.

use std::collections::BTreeMap;

use tool_spec::{
    Arg, Body, FieldType, Message, OutFeature, OutValue, Output, Report, ToolError, Value, Values,
    attr_number,
};

use crate::{choice, field, int, num, or, shell, sweep};

/// Пункты поля «Сечение» по порядку.
pub const PROFILES: [&str; 3] = ["arch", "rect", "circle"];

pub fn run(
    values: &Values,
    report: &mut dyn Report,
) -> Result<BTreeMap<String, Output>, ToolError> {
    let lines = match values.get("INPUT") {
        Some(Value::Lines(l)) => l.as_slice(),
        _ => &[],
    };
    let elev_f = field(values, "ELEV_FIELD");
    let elev = num(values, "ELEV").unwrap_or(0.0);
    let kind = PROFILES
        .get(choice(values, "PROFILE"))
        .copied()
        .unwrap_or("arch");
    let width = or(num(values, "WIDTH"), 5.0);
    let width_f = field(values, "WIDTH_FIELD");
    let height = or(num(values, "HEIGHT"), 3.0);
    let height_f = field(values, "HEIGHT_FIELD");
    let rise = or(num(values, "RISE"), 0.0);
    let centre = choice(values, "ANCHOR") == 1;
    let segs = match int(values, "SEGMENTS") {
        Some(s) if s != 0 => s,
        _ => 16,
    };

    let mut out = Output {
        fields: vec![
            ("length".into(), FieldType::Double),
            ("area".into(), FieldType::Double),
            ("volume".into(), FieldType::Double),
            ("closed".into(), FieldType::Int),
        ],
        keep_source: Some("INPUT".into()),
        name: Some("Тела выработок"),
        ..Output::default()
    };
    let total = lines.len().max(1) as f64;
    let (mut n_body, mut n_bad, mut n_skip) = (0usize, 0usize, 0usize);
    let mut vol_sum = 0.0;
    let mut sharp: Vec<i64> = Vec::new();
    for (k, ln) in lines.iter().enumerate() {
        if report.canceled() {
            break;
        }
        if ln.parts.is_empty() {
            n_skip += 1;
            continue;
        }
        let pick = |name: &str| {
            if name.is_empty() {
                None
            } else {
                attr_number(ln.attrs.get(name))
            }
        };
        let w = pick(&width_f).filter(|w| *w > 0.0).unwrap_or(width);
        let h = pick(&height_f).filter(|h| *h > 0.0).unwrap_or(height);
        let z_ft = pick(&elev_f).unwrap_or(elev);
        let rise_opt = if rise != 0.0 { Some(rise) } else { None };
        let prof = sweep::profile(kind, w, Some(h), rise_opt, segs)
            .map_err(|e| ToolError(Message::new("%s", vec![Arg::Text(e)])))?;
        let area = sweep::section_area(&prof);
        for part in &ln.parts {
            let mut pts = part.clone();
            if !pts.is_empty() && !pts.iter().all(|p| p[2].is_finite()) {
                for p in &mut pts {
                    p[2] = z_ft;
                }
            }
            let Ok(sw) = sweep::sweep(&pts, &prof, centre) else {
                n_skip += 1;
                continue;
            };
            if sw.max_turn > sweep::MAX_TURN {
                sharp.push(ln.id);
            }
            let (holes, _pinch) = shell::defects(&sw.verts, &sw.faces);
            let vol = sweep::signed_volume(&sw.verts, &sw.faces);
            out.features.push(OutFeature {
                body: Body {
                    verts: sw.verts,
                    faces: sw.faces,
                },
                source: Some(ln.id),
                values: vec![
                    OutValue::Double(sw.length),
                    OutValue::Double(area),
                    OutValue::Double(vol),
                    OutValue::Int(i64::from(holes == 0)),
                ],
            });
            n_body += 1;
            vol_sum += vol;
            n_bad += usize::from(holes > 0);
        }
        report.progress(100.0 * (k + 1) as f64 / total);
    }
    report.info(Message::new(
        "Тел: %d, объём всех %.1f м3.",
        vec![n_body.into(), vol_sum.into()],
    ));
    if n_skip > 0 {
        report.warn(Message::new(
            "Пропущено линий без двух разных вершин: %d.",
            vec![n_skip.into()],
        ));
    }
    if !sharp.is_empty() {
        let ids: Vec<String> = sharp.iter().take(20).map(i64::to_string).collect();
        report.warn(Message::new(
            "Излом оси круче 100° у объектов %s: тело там самопересекается, объём завышен. Разбейте линию на излом.",
            vec![ids.join(", ").into()],
        ));
    }
    if n_bad > 0 {
        report.warn(Message::new("Незамкнутых тел: %d.", vec![n_bad.into()]));
    }
    Ok(BTreeMap::from([("OUTPUT".to_string(), out)]))
}
