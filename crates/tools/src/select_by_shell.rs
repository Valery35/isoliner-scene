// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! 2.12 Выборка оболочкой. Перенос `tools/select_by_shell.py`.
//!
//! Луч из точки вверх: если выше неё оболочка пересекается нечётное
//! число раз, точка внутри (`boolean::points_inside`). Выход - те же
//! объекты, что пришли, с той же геометрией и полями. Сводка - объём,
//! тоннаж и содержание отобранного - идёт в отчёт.

use std::collections::BTreeMap;

use tool_spec::{
    Arg, Body, Geom, LineFeature, Message, OutFeature, OutGeometry, Output, Report, ToolError,
    Value, Values, attr_number,
};

use crate::{boolean, choice, field, num, shell};

/// Что оставить, как `MODES` модуля.
const MODES: [&str; 3] = ["inside", "cross", "outside"];

fn err(template: &'static str, args: Vec<Arg>) -> ToolError {
    ToolError(Message::new(template, args))
}

/// Числовое поле объекта: пустое и текстовое дают ноль.
fn num_attr(f: &LineFeature, name: &str) -> f64 {
    attr_number(f.attrs.get(name)).unwrap_or(0.0)
}

/// Точки опроса объекта: вершины, при шаге - и промежуточные вдоль
/// отрезков. Точки без высоты выбрасываются.
pub fn feature_points(parts: &[Vec<[f64; 3]>], step: f64) -> Vec<[f64; 3]> {
    let mut out = Vec::new();
    for a in parts {
        out.extend_from_slice(a);
        if step > 0.0 && a.len() > 1 {
            for k in 0..a.len() - 1 {
                let (p, q) = (a[k], a[k + 1]);
                let d =
                    ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2)).sqrt();
                let m = if d.is_finite() {
                    (d / step).floor() as usize
                } else {
                    0
                };
                for t in 1..=m {
                    let s = t as f64 * step / d;
                    out.push([
                        p[0] + (q[0] - p[0]) * s,
                        p[1] + (q[1] - p[1]) * s,
                        p[2] + (q[2] - p[2]) * s,
                    ]);
                }
            }
        }
    }
    out.retain(|p| p.iter().all(|x| x.is_finite()));
    out
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
    let (v, f) = (&mesh.verts, &mesh.faces);
    let mode = MODES
        .get(choice(values, "MODE"))
        .copied()
        .unwrap_or("inside");
    let step = num(values, "STEP").unwrap_or(0.0);
    let vfield = field(values, "VOLUME");
    let gfield = field(values, "GRADE");
    let dens = num(values, "DENSITY").unwrap_or(0.0);

    let (holes, _pinch) = shell::defects(v, f);
    if holes > 0 {
        return Err(err(
            "Оболочка не замкнута, рваных рёбер %d. У незамкнутой внутренности нет.",
            vec![holes.into()],
        ));
    }
    let shell_vol = shell::mesh_volume(v, f);
    report.info(Message::new(
        "Оболочка: граней %d, объём %.1f м3.",
        vec![f.len().into(), shell_vol.into()],
    ));

    let input: &[LineFeature] = match values.get("INPUT") {
        Some(Value::Features(x) | Value::Lines(x)) => x,
        _ => &[],
    };
    let mut feats: Vec<&LineFeature> = Vec::new();
    let mut pts: Vec<[f64; 3]> = Vec::new();
    let mut owner: Vec<usize> = Vec::new();
    for item in input {
        if report.canceled() {
            break;
        }
        let own = feature_points(&item.parts, step);
        if own.is_empty() {
            continue;
        }
        owner.extend(std::iter::repeat_n(feats.len(), own.len()));
        feats.push(item);
        pts.extend(own);
    }
    if pts.is_empty() {
        return Err(err(
            "Объектов с высотой не нашлось: у геометрии должна быть Z.",
            vec![],
        ));
    }
    report.info(Message::new(
        "Объектов %d, точек опроса %d.",
        vec![feats.len().into(), pts.len().into()],
    ));
    report.progress(30.0);
    let inside = boolean::points_inside(v, f, &pts);
    let mut n_in = vec![0usize; feats.len()];
    let mut n_all = vec![0usize; feats.len()];
    for (o, ins) in owner.iter().zip(&inside) {
        n_all[*o] += 1;
        if *ins {
            n_in[*o] += 1;
        }
    }
    report.progress(70.0);

    let mut out = Output {
        keep_source: Some("INPUT".into()),
        geometry: OutGeometry::Source,
        name: Some("Отобранные объекты"),
        ..Output::default()
    };
    let (mut vol, mut grade_w) = (0.0, 0.0);
    let mut n = 0usize;
    for (k, item) in feats.iter().enumerate() {
        let take = match mode {
            "inside" => n_in[k] == n_all[k],
            "cross" => n_in[k] > 0,
            _ => n_in[k] == 0,
        };
        if !take {
            continue;
        }
        n += 1;
        out.features.push(OutFeature {
            geom: Geom::Source(item.id),
            source: Some(item.id),
            values: Vec::new(),
            named: Vec::new(),
        });
        if !vfield.is_empty() {
            let q = num_attr(item, &vfield);
            vol += q;
            if !gfield.is_empty() {
                grade_w += q * num_attr(item, &gfield);
            }
        }
    }

    report.info(Message::new(
        "Объектов всего %d, отобрано %d (%.1f процента).",
        vec![
            feats.len().into(),
            n.into(),
            (100.0 * n as f64 / feats.len().max(1) as f64).into(),
        ],
    ));
    let done = |out: Output| Ok(BTreeMap::from([("OUTPUT".to_string(), out)]));
    if vfield.is_empty() {
        report.info(Message::new(
            "Поле объёма не задано: объём и тоннаж не считаются. У блочной модели из 2.03 и 1.03 это поле vol.",
            vec![],
        ));
        return done(out);
    }
    report.info(Message::new(
        "Объём отобранного: %.1f м3.",
        vec![vol.into()],
    ));
    if mode != "outside" && shell_vol > 0.0 {
        report.info(Message::new(
            "Объём оболочки %.1f м3, расхождение с блоками %+.1f процента. Это грубость блочной модели для этого тела: блок либо целиком внутри, либо целиком снаружи.",
            vec![
                shell_vol.into(),
                (100.0 * (vol - shell_vol) / shell_vol).into(),
            ],
        ));
    }
    if !gfield.is_empty() && vol > 0.0 {
        report.info(Message::new(
            "Содержание, взвешенное по объёму: %.3f.",
            vec![(grade_w / vol).into()],
        ));
    }
    if dens > 0.0 {
        report.info(Message::new(
            "Тоннаж руды: %.0f т.",
            vec![(vol * dens).into()],
        ));
        if !gfield.is_empty() && vol > 0.0 {
            report.info(Message::new(
                "Тоннаж металла: %.0f т.",
                vec![(vol * dens * (grade_w / vol) / 100.0).into()],
            ));
        }
    }
    done(out)
}
