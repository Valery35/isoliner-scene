// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Isoliner Scene: открыть пакет сцены .i3dscene и смотреть его так,
//! как он выглядел в окне Isoliner3D. Окно - крейт `scene-app`.
//!
//! Запуск: `isoliner-scene [файл.i3dscene] [--pose ...] [--check]
//! [--frames ПАПКА]`. Та же программа собирается страницей для
//! браузера: `trunk build` в этой папке, `index.html`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    scene_app::run(scene_app::Launch::from_args(std::env::args_os().skip(1)))
}

#[cfg(target_arch = "wasm32")]
fn main() {
    scene_app::start_web();
}
