// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Сцена без окна и без видеокарты.
//!
//! - [`axes`] - деления, подписи и шкалы вида по разрезу;
//! - [`camera`] - камера окна Isoliner3D: матрицы вида и проекции
//!   как у pyqtgraph, управление мышью, перевод между настоящими
//!   координатами и координатами просмотра;
//! - [`flight`] - оборот и облёт по видам для видео, перенос
//!   `flight.py` модуля;
//! - [`measure`] - длина, уклон и площадь по точкам сцены;
//! - [`mesh`] - охват, объём и замкнутость тел, луч в тела и значки.
//!
//! Мировые координаты считаются в double. В float32 переходят только
//! координаты просмотра: сдвиг на центр сцены уже вычтен.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod axes;
pub mod camera;
pub mod clip;
pub mod flight;
pub mod geo;
pub mod measure;
pub mod mesh;

pub use camera::{Frame, View};
pub use scene_format::Camera;
