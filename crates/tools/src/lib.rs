// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Расчёты инструментов Isoliner3D на Rust.
//!
//! Каждый расчёт - перенос `tools/<имя>.py` модуля QGIS с теми же
//! полями, выходами и сообщениями. Сверка - `fixtures/tool_runs.json`
//! (прогоны модуля) и `fixtures/sweep.json`.
//!
//! - [`boolean_shells`] - 2.11 Булевы операции с оболочками, ядро
//!   в [`boolean`] и [`voxel`];
//! - [`sweep_body`] - 2.24 Тело вдоль выработки, ядро в [`sweep`];
//! - [`fill_volume`] - 2.23 Заполнение тела до объёма, ядро в [`fill`]
//!   и [`voxel`].

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod boolean;
pub mod boolean_shells;
pub mod fill;
pub mod fill_volume;
pub mod shell;
pub mod sweep;
pub mod sweep_body;
pub mod voxel;

use std::collections::BTreeMap;

use tool_spec::{Output, Report, ToolError, Values};

/// Расчёт: значения полей и отчёт на входе, выходы по ключу.
pub type RunFn = fn(&Values, &mut dyn Report) -> Result<BTreeMap<String, Output>, ToolError>;

/// Расчёты по имени Processing.
pub const RUNS: &[(&str, RunFn)] = &[
    ("boolean_shells", boolean_shells::run),
    ("fill_to_volume", fill_volume::run),
    ("sweep_body", sweep_body::run),
];

/// Расчёт инструмента по имени.
pub fn run_fn(name: &str) -> Option<RunFn> {
    RUNS.iter().find(|(n, _)| *n == name).map(|(_, f)| *f)
}

pub(crate) fn num(values: &Values, key: &str) -> Option<f64> {
    match values.get(key)? {
        tool_spec::Value::Number(x) => Some(*x),
        tool_spec::Value::Integer(i) => Some(*i as f64),
        _ => None,
    }
}

pub(crate) fn int(values: &Values, key: &str) -> Option<i64> {
    match values.get(key)? {
        tool_spec::Value::Integer(i) => Some(*i),
        tool_spec::Value::Number(x) => Some(*x as i64),
        _ => None,
    }
}

pub(crate) fn choice(values: &Values, key: &str) -> usize {
    match values.get(key) {
        Some(tool_spec::Value::Enum(i)) => *i,
        _ => 0,
    }
}

pub(crate) fn field(values: &Values, key: &str) -> String {
    match values.get(key) {
        Some(tool_spec::Value::Field(s)) => s.clone(),
        _ => String::new(),
    }
}

/// Значение как `values.get(k) or d` в Python: ноль и пусто - d.
pub(crate) fn or(v: Option<f64>, d: f64) -> f64 {
    match v {
        Some(x) if x != 0.0 => x,
        _ => d,
    }
}
