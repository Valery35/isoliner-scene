// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! 2.26 Тела по контуру со сводом. Перенос `tools/crown_body.py`.
//!
//! Контур в плане даёт замкнутое тело: плоский пол, стенки и свод,
//! высота которого растёт с расстоянием до контура. Камера, линза,
//! купол, отвал по контуру основания. Ядро - [`crate::crown`].

use std::collections::BTreeMap;

use tool_spec::{
    Body, FieldType, Geom, Message, OutFeature, OutValue, Output, Report, ToolError, Value, Values,
    attr_number,
};

use crate::{choice, crown, field, num, or, shell};

fn err(t: &'static str) -> ToolError {
    ToolError(Message::new(t, vec![]))
}

pub fn run(
    values: &Values,
    report: &mut dyn Report,
) -> Result<BTreeMap<String, Output>, ToolError> {
    let geometry = match values.get("@INPUT") {
        Some(Value::Layer(l)) if !l.geometry.is_empty() => l.geometry.as_str(),
        _ => "polygon",
    };
    if geometry != "polygon" {
        return Err(err("Нужен слой полигонов: контуры тел в плане."));
    }
    let floor_f = field(values, "FLOOR_FIELD");
    let floor_c = num(values, "FLOOR").unwrap_or(0.0);
    let wall = num(values, "WALL").unwrap_or(0.0).max(0.0);
    let steep = or(num(values, "STEEP"), 0.45);
    // NaN тоже отказ, как `not steep > 0` в модуле
    if steep.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
        return Err(err("Крутизна свода должна быть больше нуля."));
    }
    let rise_max = num(values, "RISE_MAX").unwrap_or(0.0);
    let round = choice(values, "SHAPE") == 1;
    let cell = num(values, "CELL").unwrap_or(0.0);

    let mut out = Output {
        fields: vec![
            ("volume".into(), FieldType::Double),
            ("plan_area".into(), FieldType::Double),
            ("zmin".into(), FieldType::Double),
            ("zmax".into(), FieldType::Double),
            ("rise".into(), FieldType::Double),
            ("closed".into(), FieldType::Int),
        ],
        keep_source: Some("INPUT".into()),
        name: Some("Тела по контуру"),
        ..Output::default()
    };
    let items = match values.get("INPUT") {
        Some(Value::Features(f) | Value::Lines(f)) => f.as_slice(),
        _ => &[],
    };
    let (mut n_body, mut n_skip, mut n_open) = (0usize, 0usize, 0usize);
    let mut vol_sum = 0.0;
    let total = items.len().max(1) as f64;
    for (k, item) in items.iter().enumerate() {
        if report.canceled() {
            break;
        }
        for ring in &item.parts {
            if ring.len() < 3 {
                continue;
            }
            let z_field = if floor_f.is_empty() {
                None
            } else {
                attr_number(item.attrs.get(&floor_f))
            };
            let plane = if let Some(z) = z_field {
                Some((0.0, 0.0, z))
            } else if !ring.iter().all(|p| p[2].is_finite()) {
                Some((0.0, 0.0, floor_c))
            } else {
                let ptp = |k: usize| {
                    let lo = ring.iter().fold(f64::INFINITY, |m, p| m.min(p[k]));
                    let hi = ring.iter().fold(f64::NEG_INFINITY, |m, p| m.max(p[k]));
                    hi - lo
                };
                let span = ptp(0).max(ptp(1)).max(1.0);
                if ptp(2) > 0.35 * span {
                    // отвесный контур - это разрез, а не план
                    n_skip += 1;
                    continue;
                }
                None
            };
            let built = crown::crown_grids(ring, plane, wall, steep, rise_max, round, cell)
                .and_then(|g| crown::bed_mesh(&g).map(|m| (m, g.info)));
            let Ok(((verts, faces), info)) = built else {
                n_skip += 1;
                continue;
            };
            let (holes, _pinch) = shell::defects(&verts, &faces);
            let vol = crown::mesh_volume(&verts, &faces);
            // обход наружу: тело принимают 2.23 и 2.11, сцена освещает
            // его верно. Вершины, грани и объём те же, что у модуля.
            let faces = crown::outward(&verts, &faces);
            let zmin = verts.iter().fold(f64::INFINITY, |m, v| m.min(v[2]));
            let zmax = verts.iter().fold(f64::NEG_INFINITY, |m, v| m.max(v[2]));
            vol_sum += vol;
            n_body += 1;
            n_open += usize::from(holes > 0);
            out.features.push(OutFeature {
                geom: Geom::Mesh(Body { verts, faces }),
                source: Some(item.id),
                values: vec![
                    OutValue::Double(vol),
                    OutValue::Double(info.area),
                    OutValue::Double(zmin),
                    OutValue::Double(zmax),
                    OutValue::Double(info.rise),
                    OutValue::Int(i64::from(holes == 0)),
                ],
                named: Vec::new(),
            });
        }
        report.progress(100.0 * (k + 1) as f64 / total);
    }
    if n_skip > 0 {
        report.warn(Message::new(
            "Пропущено контуров: %d. Вырожденный контур или отвесный, как на разрезе: свода в плане у него нет.",
            vec![n_skip.into()],
        ));
    }
    if n_body == 0 {
        return Err(err(
            "Ни одного тела не построилось: нужны контуры в плане с площадью.",
        ));
    }
    report.info(Message::new(
        "Тел: %d, объём всего %.1f м3.",
        vec![n_body.into(), vol_sum.into()],
    ));
    if n_open > 0 {
        report.warn(Message::new(
            "Незамкнутых тел: %d. Их не примут 2.11 и 2.12.",
            vec![n_open.into()],
        ));
    }
    Ok(BTreeMap::from([("OUTPUT".to_string(), out)]))
}
