// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Инструмент Isoliner3D как объект, сторона Rust.
//!
//! В модуле QGIS инструмент - описание и расчёт (`toolspec.py`).
//! Описания выгружаются в `tools.json` (`tools/export_tools.py`),
//! этот крейт их читает: по ним строится окно инструмента в RustGIS
//! и проверяются значения полей. Расчёты на Rust лежат в крейте
//! `tools` и пользуются отсюда значениями ([`Value`]), отчётом
//! ([`Report`]) и результатом ([`Output`]).
//!
//! Строки описания и сообщений - на двух языках ([`Text`]). Шаблоны
//! сообщений в стиле printf, как в модуле: `%d`, `%s`, `%.1f`, `%.3g`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod format;
mod spec;
mod value;

pub use format::{Arg, format};
pub use spec::{Catalog, Lang, Param, ParamKind, Text, Tool, parse_catalog};
pub use value::{
    Body, FieldType, Geom, Grid, LayerInfo, LineFeature, MeshFeature, Message, OutFeature,
    OutGeometry, OutValue, Output, PointFeature, Report, Row, ToolError, Value, Values, VecReport,
    attr_number,
};
