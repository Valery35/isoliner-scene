// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! 2.25 3D-плотность. Перенос `tools/density_3d.py`.
//!
//! Куб, в ячейке которого не оценка, а счёт: сколько объектов в ячейке,
//! их сумма или среднее по полю, длина линий внутри ячейки. Ядро -
//! [`crate::density3d`].

use std::collections::BTreeMap;

use tool_spec::{
    Arg, FieldType, Geom, Grid, Message, OutFeature, OutGeometry, OutValue, Output, Report,
    ToolError, Value, Values, attr_number,
};

use crate::density3d::{self, Kind, Lattice};
use crate::{choice, field};

/// Что пишется в куб, по номеру пункта METRIC.
const METRIC: [&str; 4] = ["count", "sum", "mean", "length"];

/// Сетка не больше стольких ячеек.
const LIMIT: usize = 4_000_000;

fn err(t: &'static str, args: Vec<Arg>) -> ToolError {
    ToolError(Message::new(t, args))
}

fn num(values: &Values, key: &str) -> f64 {
    crate::num(values, key).unwrap_or(0.0)
}

pub fn run(
    values: &Values,
    report: &mut dyn Report,
) -> Result<BTreeMap<String, Output>, ToolError> {
    let kind = match values.get("@INPUT") {
        Some(Value::Layer(l)) => match l.geometry.as_str() {
            "line" => Kind::Line,
            "polygon" => Kind::Polygon,
            _ => Kind::Point,
        },
        _ => Kind::Point,
    };
    let which = choice(values, "METRIC");
    let which = if which < METRIC.len() { which } else { 0 };
    let metric = METRIC[which];
    let attr = field(values, "ATTR");
    if matches!(metric, "sum" | "mean") && attr.is_empty() {
        return Err(err("Для суммы и среднего нужен атрибут.", vec![]));
    }
    if metric == "length" && kind != Kind::Line {
        return Err(err("Длина линий считается только по слою линий.", vec![]));
    }

    let items = match values.get("INPUT") {
        Some(Value::Features(f) | Value::Lines(f)) => f.as_slice(),
        _ => &[],
    };
    let mut objs: Vec<Vec<Vec<[f64; 3]>>> = Vec::new();
    let mut vals: Vec<Option<f64>> = Vec::new();
    let mut n_flat = 0usize;
    for item in items {
        if report.canceled() {
            break;
        }
        let parts: Vec<Vec<[f64; 3]>> = item
            .parts
            .iter()
            .filter(|p| !p.is_empty())
            .cloned()
            .collect();
        if parts.is_empty() {
            continue;
        }
        if !parts
            .iter()
            .flatten()
            .all(|p| p.iter().all(|v| v.is_finite()))
        {
            n_flat += 1;
            continue;
        }
        objs.push(parts);
        vals.push(if attr.is_empty() {
            None
        } else {
            attr_number(item.attrs.get(&attr))
        });
    }
    if n_flat > 0 {
        report.warn(Message::new(
            "Объектов без высоты: %d, они пропущены. Плотности в объёме нужна Z у каждой вершины.",
            vec![n_flat.into()],
        ));
    }
    if objs.is_empty() {
        return Err(err(
            "Объектов с высотой не нашлось: у геометрии должна быть Z.",
            vec![],
        ));
    }
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in objs.iter().flatten().flatten() {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let lat = Lattice::around(lo, hi, num(values, "CELL_XY"), num(values, "CELL_Z"), LIMIT)
        .map_err(|e| err("Сетка слишком мелкая (%s). Укрупните шаг.", vec![e.into()]))?;
    let [nx, ny, nz] = lat.size;
    let [dx, dy, dz] = lat.step;
    report.info(Message::new(
        "Сетка %d x %d x %d, шаг %.6g x %.6g x %.6g м.",
        vec![
            nx.into(),
            ny.into(),
            nz.into(),
            dx.into(),
            dy.into(),
            dz.into(),
        ],
    ));

    let value = (!attr.is_empty()).then_some(vals.as_slice());
    let c = density3d::density(&objs, kind, &lat, value);
    report.info(Message::new(
        "Объектов: %d, в сетке %d.",
        vec![c.objects.into(), c.inside.into()],
    ));
    if kind == Kind::Line {
        report.info(Message::new(
            "Длина линий: всего %.2f м, по ячейкам %.2f м.",
            vec![c.len_all.into(), c.len_in.into()],
        ));
    }
    let cube: &[f64] = match metric {
        "sum" => &c.sum,
        "mean" => &c.mean,
        "length" => &c.length,
        _ => &c.count,
    };
    let filled = if metric == "length" {
        c.length.iter().filter(|v| **v > 0.0).count()
    } else {
        c.count.iter().filter(|v| **v > 0.0).count()
    };
    report.info(Message::new(
        "Непустых ячеек: %d из %d.",
        vec![filled.into(), (nx * ny * nz).into()],
    ));

    let [x0, y0, z0] = lat.origin;
    // строки растра идут с севера, куб лежит с юга: переворот по y
    let bands: Vec<Vec<Option<f64>>> = (0..nz)
        .map(|k| {
            (0..ny)
                .rev()
                .flat_map(|j| (0..nx).map(move |i| (k * ny + j) * nx + i))
                .map(|at| Some(cube[at]).filter(|v| !v.is_nan()))
                .collect()
        })
        .collect();
    let grid = Grid {
        transform: [x0, dx, 0.0, y0 + ny as f64 * dy, 0.0, -dy],
        nx,
        ny,
        // как meta модуля: "%.6f"
        z0: format!("{:.6}", z0 + dz / 2.0)
            .parse()
            .unwrap_or(z0 + dz / 2.0),
        dz: format!("{dz:.6}").parse().unwrap_or(dz),
        bands,
    };
    let mut out = BTreeMap::new();
    out.insert(
        "OUTPUT".to_string(),
        Output {
            name: Some("3D-плотность"),
            geometry: OutGeometry::Grid,
            features: vec![OutFeature {
                geom: Geom::Grid(grid),
                source: None,
                values: Vec::new(),
                named: Vec::new(),
            }],
            ..Output::default()
        },
    );

    if matches!(values.get("OUTPUT_CELLS"), Some(Value::Bool(true))) {
        let mut cells = Output {
            fields: vec![
                ("n".into(), FieldType::Int),
                ("sum".into(), FieldType::Double),
                ("mean".into(), FieldType::Double),
                ("len_m".into(), FieldType::Double),
                ("vol".into(), FieldType::Double),
            ],
            name: Some("Ячейки плотности"),
            geometry: OutGeometry::Point,
            ..Output::default()
        };
        let vol = dx * dy * dz;
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let at = (k * ny + j) * nx + i;
                    if !(c.count[at] > 0.0 || c.length[at] > 0.0) {
                        continue;
                    }
                    let xyz = [
                        x0 + (i as f64 + 0.5) * dx,
                        y0 + (j as f64 + 0.5) * dy,
                        z0 + (k as f64 + 0.5) * dz,
                    ];
                    let has = !attr.is_empty();
                    let m = c.mean[at];
                    cells.features.push(OutFeature {
                        geom: Geom::Point(xyz),
                        source: None,
                        values: vec![
                            OutValue::Int(c.count[at] as i64),
                            if has {
                                OutValue::Double(c.sum[at])
                            } else {
                                OutValue::Null
                            },
                            if has && !m.is_nan() {
                                OutValue::Double(m)
                            } else {
                                OutValue::Null
                            },
                            OutValue::Double(c.length[at]),
                            OutValue::Double(vol),
                        ],
                        named: Vec::new(),
                    });
                }
            }
        }
        out.insert("OUTPUT_CELLS".to_string(), cells);
    }
    Ok(out)
}
