// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Isoliner Scene: открыть пакет сцены .i3dscene и смотреть его так,
//! как он выглядел в окне Isoliner3D.
//!
//! Запуск: `isoliner-scene [файл.i3dscene]`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod app;
mod i18n;

fn main() -> eframe::Result {
    let path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Isoliner Scene")
            .with_inner_size([1280.0, 800.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "isoliner-scene",
        options,
        Box::new(move |cc| Ok(Box::new(app::SceneApp::new(cc, path)))),
    )
}
