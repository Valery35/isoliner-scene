// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! 1.09 Отчёт по горизонтам. Перенос `tools/bench_report.py`.
//!
//! Блочная модель режется уступами, по каждому - горная масса, руда,
//! вскрыша, содержание, металл, коэффициент вскрыши, выручка, затраты,
//! маржа. Последняя строка - итог. Ядро - [`crate::benches`].

use std::collections::BTreeMap;

use tool_spec::{
    Arg, FieldType, Geom, Message, OutFeature, OutGeometry, OutValue, Output, Report, ToolError,
    Value, Values,
};

use crate::benches::{self, Money, Row, positive};
use crate::{choice, field, num, or};

/// Поля отчёта по порядку, как `FIELDS` модуля.
const FIELDS: [(&str, FieldType); 14] = [
    ("bench", FieldType::String),
    ("z_top", FieldType::Double),
    ("z_bot", FieldType::Double),
    ("n_blocks", FieldType::Int),
    ("vol", FieldType::Double),
    ("ton", FieldType::Double),
    ("ore_t", FieldType::Double),
    ("waste_t", FieldType::Double),
    ("grade", FieldType::Double),
    ("metal", FieldType::Double),
    ("strip", FieldType::Double),
    ("revenue", FieldType::Double),
    ("cost", FieldType::Double),
    ("margin", FieldType::Double),
];

/// Число из атрибута, как `_num` модуля: нет или не число - NaN.
fn number(v: Option<&serde_json::Value>) -> f64 {
    tool_spec::attr_number(v).unwrap_or(f64::NAN)
}

/// Поле, заданное человеком, или первое из угаданных, что есть.
fn pick(name: &str, fields: &[String], guess: &[&str]) -> String {
    if !name.is_empty() {
        return name.to_string();
    }
    for g in guess {
        if let Some(f) = fields.iter().find(|f| f.to_lowercase() == *g) {
            return f.clone();
        }
    }
    String::new()
}

fn err(t: &'static str, args: Vec<Arg>) -> ToolError {
    ToolError(Message::new(t, args))
}

fn opt(x: Option<f64>) -> OutValue {
    x.map_or(OutValue::Null, OutValue::Double)
}

pub fn run(
    values: &Values,
    report: &mut dyn Report,
) -> Result<BTreeMap<String, Output>, ToolError> {
    let fields: Vec<String> = match values.get("@BLOCKS") {
        Some(Value::Layer(l)) => l.fields.clone(),
        _ => Vec::new(),
    };
    let grade_f = field(values, "GRADE");
    if grade_f.is_empty() {
        return Err(err("Нужно поле содержания.", vec![]));
    }
    let vol_f = pick(&field(values, "VOL"), &fields, &["vol", "volume"]);
    let dens_f = pick(&field(values, "DENS"), &fields, &["dens", "density"]);
    let top_f = pick(&field(values, "ZTOP"), &fields, &["z_from"]);
    let bot_f = pick(&field(values, "ZBOT"), &fields, &["z_to"]);
    let both = !top_f.is_empty() && !bot_f.is_empty();
    let dz_f = if both {
        String::new()
    } else {
        pick("", &fields, &["dz"])
    };
    if vol_f.is_empty() {
        return Err(err("Нет поля объёма блока (vol). Задайте его.", vec![]));
    }
    let dens_c = num(values, "DENSITY").unwrap_or(0.0);
    let dens_shown = if dens_f.is_empty() {
        format!("{dens_c:.3}")
    } else {
        dens_f.clone()
    };
    let height_shown = if both {
        format!("{top_f}..{bot_f}")
    } else if dz_f.is_empty() {
        "нет, по центру".to_string()
    } else {
        dz_f.clone()
    };
    report.info(Message::new(
        "Поля: содержание %s, объём %s, плотность %s, высота блока %s.",
        vec![
            grade_f.clone().into(),
            vol_f.clone().into(),
            dens_shown.into(),
            height_shown.into(),
        ],
    ));

    let blocks = match values.get("BLOCKS") {
        Some(Value::Points(p)) => p.as_slice(),
        _ => &[],
    };
    let (mut lo, mut hi, mut vol, mut g, mut dens) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut n_nogeo = 0usize;
    for item in blocks {
        let a = &item.attrs;
        let z = item.xyz.map_or(f64::NAN, |p| p[2]);
        let (zt, zb) = if both {
            (number(a.get(&top_f)), number(a.get(&bot_f)))
        } else {
            let h = if dz_f.is_empty() {
                0.0
            } else {
                number(a.get(&dz_f))
            };
            let h = if h.is_nan() { 0.0 } else { h.abs() };
            (z + h / 2.0, z - h / 2.0)
        };
        let v = number(a.get(&vol_f));
        if !(zt.is_finite() && zb.is_finite() && v.is_finite()) {
            n_nogeo += 1;
            continue;
        }
        let mut d = if dens_f.is_empty() {
            f64::NAN
        } else {
            number(a.get(&dens_f))
        };
        if !positive(d) {
            d = dens_c;
        }
        lo.push(zb);
        hi.push(zt);
        vol.push(v);
        g.push(number(a.get(&grade_f)));
        dens.push(d);
    }
    if n_nogeo > 0 {
        report.warn(Message::new(
            "Блоков без отметки или объёма: %d, они пропущены.",
            vec![n_nogeo.into()],
        ));
    }
    if vol.is_empty() {
        return Err(err("Блоков с отметкой и объёмом не нашлось.", vec![]));
    }
    let no_dens = dens.iter().filter(|d| !positive(**d)).count();
    if no_dens > 0 {
        return Err(err(
            "У %d блоков нет плотности: задайте поле плотности или постоянную плотность.",
            vec![no_dens.into()],
        ));
    }
    let no_grade = g.iter().filter(|x| !x.is_finite()).count();
    if no_grade > 0 {
        report.warn(Message::new(
            "Блоков без содержания: %d, они считаются вскрышей.",
            vec![no_grade.into()],
        ));
    }
    let money = Money {
        unit: benches::UNITS
            .get(choice(values, "UNIT"))
            .copied()
            .unwrap_or(0.01),
        price: num(values, "PRICE").unwrap_or(0.0),
        recovery: or(num(values, "RECOVERY"), 100.0) / 100.0,
        mine_cost: num(values, "MINE_COST").unwrap_or(0.0),
        proc_cost: num(values, "PROC_COST").unwrap_or(0.0),
    };
    let Some((rows, tot)) = benches::report(
        &lo,
        &hi,
        &vol,
        &g,
        &dens,
        or(num(values, "HEIGHT"), 10.0),
        num(values, "BASE").unwrap_or(0.0),
        num(values, "CUTOFF").unwrap_or(0.0),
        &money,
    ) else {
        return Err(err("Высота уступа должна быть больше нуля.", vec![]));
    };

    let mut out = Output {
        fields: FIELDS
            .iter()
            .map(|(n, t)| (n.to_string(), t.clone()))
            .collect(),
        name: Some("Отчёт по горизонтам"),
        geometry: OutGeometry::None,
        ..Output::default()
    };
    let line = |label: String, r: &Row| OutFeature {
        geom: Geom::None,
        source: None,
        values: vec![
            OutValue::Str(label),
            OutValue::Double(r.z_top),
            OutValue::Double(r.z_bot),
            OutValue::Int(r.n_blocks),
            OutValue::Double(r.vol),
            OutValue::Double(r.ton),
            OutValue::Double(r.ore_t),
            OutValue::Double(r.waste_t),
            opt(r.grade),
            OutValue::Double(r.metal),
            opt(r.strip),
            OutValue::Double(r.revenue),
            OutValue::Double(r.cost),
            OutValue::Double(r.margin),
        ],
        named: Vec::new(),
    };
    for r in &rows {
        out.features
            .push(line(format!("{:.2}..{:.2}", r.z_top, r.z_bot), r));
    }
    out.features.push(line("Итого".to_string(), &tot));
    report.info(Message::new(
        "Уступов: %d. Горная масса %.0f т, руда %.0f т, вскрыша %.0f т, металл %.6g.",
        vec![
            rows.len().into(),
            tot.ton.into(),
            tot.ore_t.into(),
            tot.waste_t.into(),
            tot.metal.into(),
        ],
    ));
    if tot.revenue != 0.0 || tot.cost != 0.0 {
        report.info(Message::new(
            "Выручка %.0f, затраты %.0f, маржа %.0f.",
            vec![tot.revenue.into(), tot.cost.into(), tot.margin.into()],
        ));
    }
    Ok(BTreeMap::from([("OUTPUT".to_string(), out)]))
}
