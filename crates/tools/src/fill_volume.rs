// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! 2.23 Заполнение тела до объёма. Перенос `tools/fill_volume.py`.

use std::collections::BTreeMap;

use tool_spec::{
    Body, FieldType, Message, OutFeature, OutValue, Output, Report, ToolError, Value, Values,
};

use crate::{choice, fill, num, shell, sweep, voxel};

fn err(template: &'static str, args: Vec<tool_spec::Arg>) -> ToolError {
    ToolError(Message::new(template, args))
}

pub fn run(
    values: &Values,
    report: &mut dyn Report,
) -> Result<BTreeMap<String, Output>, ToolError> {
    let empty = Body::default();
    let mesh = match values.get("SHELL") {
        Some(Value::Mesh(b)) => b,
        _ => &empty,
    };
    if mesh.faces.is_empty() {
        return Err(err("В слое нет ни одного треугольника.", vec![]));
    }
    let (v, f) = (&mesh.verts, &mesh.faces);
    let from_top = choice(values, "MODE") == 1;
    let target = num(values, "TARGET").unwrap_or(0.0);
    let in_pct = choice(values, "UNIT") == 0;
    let cell = num(values, "CELL").unwrap_or(0.0);

    let (holes, _pinch) = shell::defects(v, f);
    if holes > 0 {
        return Err(err(
            "Оболочка не замкнута, рваных рёбер %d. У незамкнутой внутренности нет, и заполнять нечего.",
            vec![holes.into()],
        ));
    }
    let exact = sweep::signed_volume(v, f).abs();
    let fold = |k: usize, lo: bool| {
        v.iter().map(|p| p[k]).fold(
            if lo { f64::INFINITY } else { f64::NEG_INFINITY },
            |a, b| {
                if lo { a.min(b) } else { a.max(b) }
            },
        )
    };
    let bounds = [fold(0, true), fold(0, false), fold(1, true), fold(1, false)];
    let (gt, ny, nx, step) = fill::plan_grid(bounds, cell, 400_000);
    if cell > 0.0 && step > cell * 1.0001 {
        report.warn(Message::new(
            "Шаг %.3g м дал бы слишком много колонок, взят %.3g м.",
            vec![cell.into(), step.into()],
        ));
    }
    report.info(Message::new(
        "Колонок %d x %d, шаг %.3g м.",
        vec![nx.into(), ny.into(), step.into()],
    ));
    let iv = fill::intervals(&fill::column_cuts(v, f, &gt, ny, nx));
    if iv.is_empty() {
        return Err(err(
            "Колонки не задели тело: шаг крупнее самого тела.",
            vec![],
        ));
    }
    let area = step * step;
    let full = fill::total_volume(&iv, area);
    report.info(Message::new(
        "Объём тела: по колонкам %.1f м3, точный по оболочке %.1f м3, расхождение %.2f %%.",
        vec![
            full.into(),
            exact.into(),
            (if exact != 0.0 {
                100.0 * (full - exact) / exact
            } else {
                0.0
            })
            .into(),
        ],
    ));
    let want = if in_pct {
        full * target / 100.0
    } else {
        target
    };
    if want > full {
        report.warn(Message::new(
            "Задано %.1f м3, а в теле только %.1f м3: тело заполняется целиком.",
            vec![want.into(), full.into()],
        ));
    }
    let Some((level, got)) = fill::level_for_volume(&iv, area, want, from_top, 1e-6) else {
        return Err(err(
            "Колонки не задели тело: шаг крупнее самого тела.",
            vec![],
        ));
    };
    let share = if full != 0.0 { 100.0 * got / full } else { 0.0 };
    report.info(Message::new(
        "Уровень %.3f м. Объём %s уровня %.1f м3, %.1f %% тела.",
        vec![
            level.into(),
            (if from_top { "выше" } else { "ниже" }).into(),
            got.into(),
            share.into(),
        ],
    ));
    // Тело заполнения - на сетке грубее: уровень уже найден по мелким
    // колонкам, крышка всё равно ровная на уровне.
    let (gt2, ny2, nx2, step2) = fill::plan_grid(bounds, cell.max(step), 40_000);
    let iv2 = fill::intervals(&fill::column_cuts(v, f, &gt2, ny2, nx2));
    let zlo = iv.iter().map(|i| i.lo).fold(f64::INFINITY, f64::min);
    let zhi = iv.iter().map(|i| i.hi).fold(f64::NEG_INFINITY, f64::max);
    let zspan = zhi - zlo;
    let dz = step2.min(zspan / 50.0).max(zspan / 100.0).max(1e-6);
    let (occ, z0, dz) = fill::filled_occupancy(&iv2, ny2, nx2, level, dz, from_top);
    if occ.len().saturating_mul(ny2 * nx2) > 60_000_000 {
        return Err(err("Ячеек слишком много: укрупните шаг колонок.", vec![]));
    }
    let Some(body) = voxel::voxel_mesh(&occ, ny2, nx2, &gt2, z0, dz, 2_000_000) else {
        return Err(err(
            "Граней вышло слишком много: укрупните шаг колонок.",
            vec![],
        ));
    };
    let mut out = Output {
        fields: vec![
            ("mode".into(), FieldType::String),
            ("level".into(), FieldType::Double),
            ("volume".into(), FieldType::Double),
            ("share_pct".into(), FieldType::Double),
            ("body_vol".into(), FieldType::Double),
        ],
        name: Some("Заполненная часть"),
        ..Output::default()
    };
    if body.faces.is_empty() {
        report.warn(Message::new(
            "Заполненная часть пуста: объём меньше одной ячейки.",
            vec![],
        ));
    } else {
        out.features.push(OutFeature {
            body,
            source: None,
            values: vec![
                OutValue::Str(if from_top { "top" } else { "bottom" }.into()),
                OutValue::Double(level),
                OutValue::Double(got),
                OutValue::Double(share),
                OutValue::Double(full),
            ],
        });
    }
    Ok(BTreeMap::from([("OUTPUT".to_string(), out)]))
}
