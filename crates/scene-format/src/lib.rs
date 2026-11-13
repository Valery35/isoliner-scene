// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Пакет сцены Isoliner3D: файл `.i3dscene`.
//!
//! Пакет пишет модуль QGIS Isoliner3D (`scene_package.py`). Это zip
//! из двух частей:
//!
//! - `scene.glb` - геометрия: план сдвинут на круглую точку начала,
//!   высоты настоящие, привязка в `asset.extras.isoliner3d`. Тела,
//!   линии, а с формата 2 и точки с размером значка в пикселях;
//! - `scene.json` - имя, система координат, камера и виды облёта
//!   в настоящих координатах, список частей и состояние окна QGIS.
//!
//! Крейт читает пакет в [`Package`] и пишет его обратно. Состояние окна
//! QGIS (`state`) хранится как есть: пакет, открытый здесь, снова
//! открывается в QGIS с прежними настройками.
//!
//! Входной файл считается враждебным: размеры, смещения и номера
//! вершин проверяются до чтения.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod cad;
pub mod glb;
mod meta;
mod package;

pub use glb::{Georef, Indices, Part};
pub use meta::{Camera, Crs, FORMAT, FORMAT_POINTS, PartInfo, PartKind, SceneMeta};
pub use package::{GLB_NAME, JSON_NAME, Package, read_package, write_package};

use std::fmt;

/// Ошибка чтения или записи пакета.
#[derive(Debug)]
pub enum Error {
    /// Архив zip повреждён или в нём нет нужного файла.
    Zip(String),
    /// scene.json не разбирается.
    Json(String),
    /// GLB повреждён или не того вида.
    Glb(String),
    /// Пакет записан более новой версией формата.
    TooNew(u32),
    /// Файл больше допустимого.
    TooLarge(&'static str, u64),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Zip(s) => write!(f, "архив пакета: {s}"),
            Error::Json(s) => write!(f, "scene.json: {s}"),
            Error::Glb(s) => write!(f, "GLB: {s}"),
            Error::TooNew(n) => write!(
                f,
                "пакет новее программы: формат {n}, поддерживается {FORMAT}"
            ),
            Error::TooLarge(what, n) => write!(f, "{what}: {n} байт, больше допустимого"),
        }
    }
}

impl std::error::Error for Error {}

/// Наибольший размер распакованной части пакета, байт.
pub const MAX_ENTRY: u64 = 2 * 1024 * 1024 * 1024;
