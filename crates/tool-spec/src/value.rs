// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Значения полей, отчёт и результат расчёта.

use std::collections::{BTreeMap, HashMap};

use crate::format::Arg;

/// Тело: вершины и треугольники.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Body {
    pub verts: Vec<[f64; 3]>,
    pub faces: Vec<[u32; 3]>,
}

/// Объект линейного слоя. У линии без Z третья координата NaN.
#[derive(Debug, Clone, PartialEq)]
pub struct LineFeature {
    pub id: i64,
    pub parts: Vec<Vec<[f64; 3]>>,
    pub attrs: HashMap<String, serde_json::Value>,
}

/// Объект точечного слоя.
#[derive(Debug, Clone, PartialEq)]
pub struct PointFeature {
    pub id: i64,
    pub xyz: Option<[f64; 3]>,
    pub attrs: HashMap<String, serde_json::Value>,
}

/// Значение поля на входе расчёта.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Mesh(Body),
    Lines(Vec<LineFeature>),
    Points(Vec<PointFeature>),
    /// Имя поля, пустое - поле не выбрано.
    Field(String),
    Number(f64),
    Integer(i64),
    Enum(usize),
}

/// Значения полей по ключу.
pub type Values = BTreeMap<String, Value>;

/// Число из атрибута, как `float()` в Python: число или строка
/// с числом. Пустое, текст и NaN - None.
pub fn attr_number(v: Option<&serde_json::Value>) -> Option<f64> {
    let x = match v? {
        serde_json::Value::Number(n) => n.as_f64()?,
        serde_json::Value::String(s) => s.trim().parse::<f64>().ok()?,
        serde_json::Value::Bool(b) => f64::from(u8::from(*b)),
        _ => return None,
    };
    x.is_finite().then_some(x)
}

/// Сообщение: русский шаблон и аргументы. Переводится при показе.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub template: &'static str,
    pub args: Vec<Arg>,
}

impl Message {
    pub fn new(template: &'static str, args: Vec<Arg>) -> Self {
        Self { template, args }
    }

    /// Текст на русском, без перевода.
    pub fn text(&self) -> String {
        crate::format(self.template, &self.args)
    }
}

/// Ошибка расчёта: сообщение для пользователя.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolError(pub Message);

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.text())
    }
}

impl std::error::Error for ToolError {}

/// Отчёт расчёта: сведения, предупреждения, ход, отмена.
pub trait Report {
    fn info(&mut self, msg: Message);
    fn warn(&mut self, msg: Message);
    fn progress(&mut self, _percent: f64) {}
    fn canceled(&self) -> bool {
        false
    }
}

/// Отчёт, который копит сообщения: для тестов и для журнала.
#[derive(Debug, Default)]
pub struct VecReport {
    pub lines: Vec<(&'static str, Message)>,
    pub cancel: bool,
}

impl Report for VecReport {
    fn info(&mut self, msg: Message) {
        self.lines.push(("info", msg));
    }
    fn warn(&mut self, msg: Message) {
        self.lines.push(("warn", msg));
    }
    fn canceled(&self) -> bool {
        self.cancel
    }
}

/// Тип поля выхода.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Double,
    Int,
    String,
}

impl FieldType {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "double" => Some(Self::Double),
            "int" => Some(Self::Int),
            "string" => Some(Self::String),
            _ => None,
        }
    }
}

/// Значение поля выхода.
#[derive(Debug, Clone, PartialEq)]
pub enum OutValue {
    Double(f64),
    Int(i64),
    Str(String),
}

/// Объект выхода: тело, объект источника и значения полей.
#[derive(Debug, Clone, PartialEq)]
pub struct OutFeature {
    pub body: Body,
    pub source: Option<i64>,
    pub values: Vec<OutValue>,
}

/// Выход инструмента, как `Result` модуля.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Output {
    pub fields: Vec<(String, FieldType)>,
    pub features: Vec<OutFeature>,
    /// Ключ входа: выход начинается полями его объектов.
    pub keep_source: Option<String>,
    /// Имя выходного слоя (русский шаблон).
    pub name: Option<&'static str>,
}
