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

/// Объект слоя тел: тело на объект. Объект без граней - тела нет.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshFeature {
    pub id: i64,
    pub attrs: HashMap<String, serde_json::Value>,
    pub body: Option<Body>,
}

/// Строка таблицы или объект слоя: атрибуты и первая вершина.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub id: i64,
    pub attrs: HashMap<String, serde_json::Value>,
    pub xyz: Option<[f64; 3]>,
}

/// Сведения о слое на входе, `values["@КЛЮЧ"]` модуля: имя слоя,
/// геометрия ("point", "line", "polygon", "none"), есть ли Z, поля.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayerInfo {
    pub name: String,
    pub geometry: String,
    pub hasz: bool,
    pub fields: Vec<String>,
    pub numeric: Vec<String>,
}

/// Значение поля на входе расчёта, как данные `run` модуля
/// (`toolspec.py`).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Mesh(Body),
    Meshes(Vec<MeshFeature>),
    Lines(Vec<LineFeature>),
    Points(Vec<PointFeature>),
    /// Объекты любой геометрии: части - вершины точки, линии или
    /// внешнего кольца полигона.
    Features(Vec<LineFeature>),
    Table(Vec<Row>),
    /// Имя поля, пустое - поле не выбрано.
    Field(String),
    /// Имена полей у поля с multiple.
    Fields(Vec<String>),
    Str(String),
    Bool(bool),
    Number(f64),
    Integer(i64),
    Enum(usize),
    /// Номера пунктов у списка с multiple.
    Enums(Vec<usize>),
    /// Сведения о слое, ключ `@КЛЮЧ`.
    Layer(LayerInfo),
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType {
    Double,
    Int,
    String,
    /// `copy:КЛЮЧ` - поле с тем же именем и типом, что у слоя КЛЮЧ.
    Copy(String),
}

impl FieldType {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "double" => Some(Self::Double),
            "int" => Some(Self::Int),
            "string" => Some(Self::String),
            _ => s.strip_prefix("copy:").map(|k| Self::Copy(k.to_string())),
        }
    }

    /// Запись типа, как в модуле.
    pub fn name(&self) -> String {
        match self {
            Self::Double => "double".into(),
            Self::Int => "int".into(),
            Self::String => "string".into(),
            Self::Copy(k) => format!("copy:{k}"),
        }
    }
}

/// Значение поля выхода. Null - пусто (None модуля).
#[derive(Debug, Clone, PartialEq)]
pub enum OutValue {
    Double(f64),
    Int(i64),
    Str(String),
    Null,
}

/// Геометрия объекта выхода, как `Result.geometry` модуля.
#[derive(Debug, Clone, PartialEq)]
pub enum Geom {
    /// Тело.
    Mesh(Body),
    /// Линия с Z.
    Line(Vec<[f64; 3]>),
    /// Точка с Z.
    Point([f64; 3]),
    /// Геометрия объекта источника как есть.
    Source(i64),
}

/// Объект выхода: геометрия, объект источника и значения полей.
#[derive(Debug, Clone, PartialEq)]
pub struct OutFeature {
    pub geom: Geom,
    pub source: Option<i64>,
    /// Значения своих полей по порядку `Output::fields`.
    pub values: Vec<OutValue>,
    /// При `Output::replace`: значения по имени поля, чего нет -
    /// остаётся от источника.
    pub named: Vec<(String, OutValue)>,
}

impl OutFeature {
    /// Тело объекта, если геометрия - тело.
    pub fn body(&self) -> Option<&Body> {
        match &self.geom {
            Geom::Mesh(b) => Some(b),
            _ => None,
        }
    }
}

/// Вид геометрии выхода.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutGeometry {
    #[default]
    Mesh,
    Line,
    Point,
    Source,
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
    pub geometry: OutGeometry,
    /// Свои поля с именем поля источника пишутся в него.
    pub replace: bool,
}
