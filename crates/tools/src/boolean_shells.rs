// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! 2.11 Булевы операции с оболочками. Перенос `tools/boolean_shells.py`.
//!
//! Обе оболочки переводятся в занятость общего куба, операция идёт над
//! занятостью, результат снова становится телом из ячеек. Тело
//! ступенчатое, зато замкнуто всегда, и объём по нему считается.

use std::collections::BTreeMap;

use tool_spec::{
    Arg, Body, FieldType, Geom, Message, OutFeature, OutValue, Output, Report, ToolError, Value,
    Values,
};

use crate::{boolean, choice, num, or, shell, voxel};

/// Предел ячеек куба, как `MAX_CELLS` модуля.
pub const MAX_CELLS: usize = 60_000_000;

fn err(template: &'static str, args: Vec<Arg>) -> ToolError {
    ToolError(Message::new(template, args))
}

pub fn run(
    values: &Values,
    report: &mut dyn Report,
) -> Result<BTreeMap<String, Output>, ToolError> {
    let op = boolean::OPS
        .get(choice(values, "OP"))
        .copied()
        .unwrap_or(boolean::OPS[0]);
    let cell = or(num(values, "CELL"), 1.0);
    if !(cell.is_finite() && cell > 0.0) {
        return Err(err("Сторона ячейки должна быть больше нуля.", vec![]));
    }
    let empty = Body::default();
    let mut shells: Vec<&Body> = Vec::new();
    for (key, tag) in [("SHELL_A", "Первое тело"), ("SHELL_B", "Второе тело")] {
        let mesh = match values.get(key) {
            Some(Value::Mesh(b)) => b,
            _ => &empty,
        };
        if mesh.faces.is_empty() {
            return Err(err(
                "%s: треугольников не нашлось. Нужен слой полигонов с высотой, какой даёт кнопка оболочек в окне просмотра.",
                vec![Arg::Text(tag.into())],
            ));
        }
        let (holes, _pinch) = shell::defects(&mesh.verts, &mesh.faces);
        if holes > 0 {
            return Err(err(
                "%s: оболочка не замкнута, рваных рёбер %d. У незамкнутой внутренности нет, и определить, что внутри, нечем.",
                vec![Arg::Text(tag.into()), holes.into()],
            ));
        }
        report.info(Message::new(
            "%s: граней %d, объём %.1f м3.",
            vec![
                Arg::Text(tag.into()),
                mesh.faces.len().into(),
                shell::mesh_volume(&mesh.verts, &mesh.faces).into(),
            ],
        ));
        shells.push(mesh);
    }
    let (a, b) = (shells[0], shells[1]);

    let (gt, z0, dz, shape) = boolean::common_box(
        boolean::bounds(&a.verts),
        boolean::bounds(&b.verts),
        cell,
        op,
    );
    let cells = boolean::cell_budget(shape);
    report.info(Message::new(
        "Куб %d x %d x %d, ячеек %d.",
        vec![
            shape[2].into(),
            shape[1].into(),
            shape[0].into(),
            cells.into(),
        ],
    ));
    if cells > MAX_CELLS {
        return Err(err(
            "Ячеек слишком много: %d. Память растёт кубом, укрупните ячейку.",
            vec![cells.into()],
        ));
    }
    let occ_a = boolean::shell_occupancy(&a.verts, &a.faces, &gt, z0, dz, shape);
    report.progress(35.0);
    let occ_b = boolean::shell_occupancy(&b.verts, &b.faces, &gt, z0, dz, shape);
    report.progress(60.0);
    let occ = boolean::combine(&occ_a, &occ_b, op);
    let n_cell = occ.iter().flatten().filter(|x| **x).count();
    if n_cell == 0 {
        return Err(err(
            "От результата не осталось ни одной ячейки. Для пересечения это значит, что тела не перекрываются, для вычитания - что второе поглотило первое.",
            vec![],
        ));
    }
    report.info(Message::new(
        "Занято ячеек: %d, объём по ним %.1f м3.",
        vec![
            n_cell.into(),
            (n_cell as f64 * (gt[1] * gt[5]).abs() * dz).into(),
        ],
    ));

    let Some(mesh) = voxel::voxel_mesh(&occ, shape[1], shape[2], &gt, z0, dz, 2_000_000) else {
        return Err(err("Граней вышло слишком много: укрупните ячейку.", vec![]));
    };
    report.progress(80.0);

    let mut out = Output {
        fields: vec![
            ("body".into(), FieldType::Int),
            ("op".into(), FieldType::String),
            ("volume".into(), FieldType::Double),
            ("faces".into(), FieldType::Int),
            ("closed".into(), FieldType::Int),
        ],
        name: Some("Результат (тела)"),
        ..Output::default()
    };
    let mut total = 0.0;
    let mut n_body = 0i64;
    for (bv, bf) in shell::split_bodies(&mesh.verts, &mesh.faces) {
        let (holes, _pinch) = shell::defects(&bv, &bf);
        let vol = shell::mesh_volume(&bv, &bf);
        total += vol;
        n_body += 1;
        let n_faces = bf.len() as i64;
        out.features.push(OutFeature {
            geom: Geom::Mesh(Body {
                verts: bv,
                faces: bf,
            }),
            source: None,
            values: vec![
                OutValue::Int(n_body),
                OutValue::Str(op.into()),
                OutValue::Double(vol),
                OutValue::Int(n_faces),
                OutValue::Int(i64::from(holes == 0)),
            ],
            named: Vec::new(),
        });
    }
    report.info(Message::new(
        "Тел: %d, объём по оболочкам %.1f м3. Точность ограничена ячейкой: ошибка идёт по площади поверхности и убывает вместе с ячейкой.",
        vec![n_body.into(), total.into()],
    ));
    Ok(BTreeMap::from([("OUTPUT".to_string(), out)]))
}
