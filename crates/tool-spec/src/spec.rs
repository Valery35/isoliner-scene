// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Описания инструментов из tools.json.

use serde::{Deserialize, Serialize};

use crate::value::{Value, Values};

/// Язык показа.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    Ru,
    En,
}

/// Строка на двух языках.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Text {
    pub ru: String,
    pub en: String,
}

impl Text {
    pub fn get(&self, lang: Lang) -> &str {
        match lang {
            Lang::Ru => &self.ru,
            Lang::En if self.en.is_empty() => &self.ru,
            Lang::En => &self.en,
        }
    }
}

/// Вид поля.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamKind {
    /// Слой полигонов с Z одним телом.
    Mesh,
    /// Линии.
    Lines,
    /// Точки.
    Points,
    /// Поле родительского слоя.
    Field,
    /// Число с дробной частью.
    Number,
    /// Целое.
    Integer,
    /// Пункт списка.
    Enum,
    /// Выход.
    Sink,
}

/// Поле инструмента.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub key: String,
    pub kind: ParamKind,
    pub label: Text,
    pub hint: Text,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    #[serde(default)]
    pub options: Vec<Text>,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub minimum: Option<f64>,
    #[serde(default)]
    pub maximum: Option<f64>,
    #[serde(default)]
    pub numeric: bool,
    #[serde(default)]
    pub geometry: Option<String>,
}

/// Описание инструмента.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tool {
    pub code: String,
    pub name: String,
    pub title: Text,
    pub group: String,
    pub help: Text,
    pub params: Vec<Param>,
    /// Шаблоны сообщений расчёта и их переводы.
    #[serde(default)]
    pub messages: Vec<Text>,
}

/// tools.json целиком.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Catalog {
    pub format: u32,
    pub module: String,
    pub version: String,
    pub tools: Vec<Tool>,
}

/// Формат tools.json, который понимает крейт.
pub const FORMAT: u32 = 1;

/// Прочитать tools.json.
pub fn parse_catalog(data: &[u8]) -> Result<Catalog, String> {
    let cat: Catalog = serde_json::from_slice(data).map_err(|e| e.to_string())?;
    if cat.format > FORMAT {
        return Err(format!("tools.json новее программы: формат {}", cat.format));
    }
    Ok(cat)
}

impl Catalog {
    pub fn tool(&self, name: &str) -> Option<&Tool> {
        self.tools.iter().find(|t| t.name == name)
    }
}

impl Tool {
    /// Название с кодом, как в панели Processing.
    pub fn display_name(&self, lang: Lang) -> String {
        format!("{} {}", self.code, self.title.get(lang))
    }

    pub fn param(&self, key: &str) -> Option<&Param> {
        self.params.iter().find(|p| p.key == key)
    }

    /// Значения полей по умолчанию. Слоёв и выходов здесь нет.
    pub fn defaults(&self) -> Values {
        let mut out = Values::new();
        for p in &self.params {
            let d = p.default.as_ref();
            let v = match p.kind {
                ParamKind::Number => d.and_then(serde_json::Value::as_f64).map(Value::Number),
                ParamKind::Integer => d.and_then(serde_json::Value::as_i64).map(Value::Integer),
                ParamKind::Enum => d
                    .and_then(serde_json::Value::as_u64)
                    .map(|i| Value::Enum(i as usize)),
                ParamKind::Field => Some(Value::Field(String::new())),
                _ => None,
            };
            if let Some(v) = v {
                out.insert(p.key.clone(), v);
            }
        }
        out
    }

    /// Перевод шаблона сообщения: русский шаблон из расчёта на язык
    /// показа. Незнакомый шаблон остаётся как есть.
    pub fn translate<'a>(&'a self, template: &'a str, lang: Lang) -> &'a str {
        self.messages
            .iter()
            .chain(self.params.iter().flat_map(|p| p.options.iter()))
            .find(|m| m.ru == template)
            .map_or(template, |m| m.get(lang))
    }

    /// Проверка значений по описанию: список замечаний, пустой - годно.
    pub fn check(&self, values: &Values, lang: Lang) -> Vec<String> {
        let mut bad = Vec::new();
        for p in &self.params {
            let label = p.label.get(lang);
            let Some(v) = values.get(&p.key) else {
                if p.kind != ParamKind::Sink && !p.optional {
                    bad.push(format!("{label}: нет значения"));
                }
                continue;
            };
            let num = match v {
                Value::Number(x) => Some(*x),
                Value::Integer(x) => Some(*x as f64),
                _ => None,
            };
            if let Some(x) = num
                && (p.minimum.is_some_and(|m| x < m) || p.maximum.is_some_and(|m| x > m))
            {
                bad.push(format!("{label}: {x} вне пределов"));
            }
            if let Value::Enum(i) = v
                && *i >= p.options.len()
            {
                bad.push(format!("{label}: пункта {i} нет"));
            }
            let fits = matches!(
                (p.kind, v),
                (ParamKind::Mesh, Value::Mesh(_))
                    | (ParamKind::Lines, Value::Lines(_))
                    | (ParamKind::Points, Value::Points(_))
                    | (ParamKind::Field, Value::Field(_))
                    | (ParamKind::Number, Value::Number(_))
                    | (ParamKind::Integer, Value::Integer(_))
                    | (ParamKind::Enum, Value::Enum(_))
            );
            if !fits {
                bad.push(format!("{label}: значение не того вида"));
            }
        }
        bad
    }
}
