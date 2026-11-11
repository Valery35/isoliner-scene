// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! scene.json: описание сцены.

use serde::{Deserialize, Serialize};

/// Номер формата пакета, который понимает крейт.
pub const FORMAT: u32 = 1;

fn d_distance() -> f64 {
    10.0
}
fn d_elevation() -> f64 {
    30.0
}
fn d_fov() -> f64 {
    60.0
}

/// Камера в настоящих координатах.
///
/// Как в окне Isoliner3D (pyqtgraph): точка взгляда `center`, удаление
/// `distance`, азимут и наклон в градусах, раствор `fov` в градусах.
/// Недостающие поля берут значения окна по умолчанию.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    #[serde(default)]
    pub center: [f64; 3],
    #[serde(default = "d_distance")]
    pub distance: f64,
    #[serde(default)]
    pub azimuth: f64,
    #[serde(default = "d_elevation")]
    pub elevation: f64,
    #[serde(default = "d_fov")]
    pub fov: f64,
    #[serde(default)]
    pub ortho: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            center: [0.0; 3],
            distance: d_distance(),
            azimuth: 0.0,
            elevation: d_elevation(),
            fov: d_fov(),
            ortho: false,
        }
    }
}

/// Система координат сцены.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Crs {
    #[serde(default)]
    pub authid: String,
    #[serde(default)]
    pub wkt: String,
}

/// Вид части: тело из треугольников или линии.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PartKind {
    Mesh,
    Lines,
}

/// Часть сцены в scene.json: узел GLB и признаки показа.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartInfo {
    pub node: usize,
    pub name: String,
    pub kind: PartKind,
    #[serde(default)]
    pub verts: u64,
    #[serde(default)]
    pub elements: u64,
    /// Оформление вида (короб, подписи), не тело.
    #[serde(default)]
    pub decor: bool,
    /// id слоя QGIS, из которого собрана часть.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
}

fn d_vex() -> f64 {
    1.0
}
fn d_glb() -> String {
    crate::GLB_NAME.to_string()
}

/// scene.json целиком.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneMeta {
    pub format: u32,
    #[serde(default)]
    pub module: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub crs: Crs,
    /// Точка начала плана в GLB, настоящие координаты.
    #[serde(default)]
    pub origin: [f64; 3],
    /// Преувеличение по вертикали в окне при записи.
    #[serde(default = "d_vex")]
    pub vex: f64,
    #[serde(default)]
    pub camera: Option<Camera>,
    #[serde(default)]
    pub views: Vec<Camera>,
    #[serde(default)]
    pub parts: Vec<PartInfo>,
    #[serde(default = "d_glb")]
    pub glb: String,
    /// Состояние окна QGIS, хранится как есть.
    #[serde(default)]
    pub state: serde_json::Value,
}

impl SceneMeta {
    /// Показан ли короб в окне QGIS при записи (`state.axes_on`).
    ///
    /// Модуль кладёт короб и его подписи в пакет всегда, а в окне
    /// рисует их только при включённом коробе. Без флага в состоянии
    /// короб считается показанным.
    pub fn box_shown(&self) -> bool {
        self.state
            .get("axes_on")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true)
    }

    /// Показана ли часть при открытии пакета, как в окне QGIS:
    /// оформление скрыто, если короб в окне был выключен.
    pub fn part_shown(&self, info: Option<&PartInfo>) -> bool {
        !info.is_some_and(|i| i.decor) || self.box_shown()
    }
}

impl Default for SceneMeta {
    fn default() -> Self {
        Self {
            format: FORMAT,
            module: "isoliner-scene".to_string(),
            version: String::new(),
            name: String::new(),
            crs: Crs::default(),
            origin: [0.0; 3],
            vex: 1.0,
            camera: None,
            views: Vec::new(),
            parts: Vec::new(),
            glb: d_glb(),
            state: serde_json::Value::Null,
        }
    }
}
