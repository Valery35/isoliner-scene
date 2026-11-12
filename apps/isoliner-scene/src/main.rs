// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Isoliner Scene: открыть пакет сцены .i3dscene и смотреть его так,
//! как он выглядел в окне Isoliner3D.
//!
//! Запуск: `isoliner-scene [файл.i3dscene] [--pose ...] [--check]`.
//! Та же программа собирается
//! страницей для браузера: `trunk build` в этой папке, `index.html`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod app;
mod i18n;
mod share;
mod shot;
#[cfg(not(target_arch = "wasm32"))]
mod video;
#[cfg(not(target_arch = "wasm32"))]
mod watch;
#[cfg(target_arch = "wasm32")]
mod web;

/// Ключи командной строки: `файл.i3dscene`, `--pose x,y,z,...` -
/// камера как в ссылке «Поделиться», `--check` - самопроверка
/// с отчётом в вывод, `--frames ПАПКА` - кадры оборота PNG в папку
/// и ролик MP4, если в PATH есть ffmpeg, потом окно закрывается.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let mut path = None;
    let mut pose = None;
    let mut check = false;
    let mut frames = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(a) = args.next() {
        match a.to_str() {
            Some("--pose") => {
                pose = args
                    .next()
                    .and_then(|v| v.to_str().and_then(share::parse_pose));
            }
            Some("--check") => check = true,
            Some("--frames") => frames = args.next().map(std::path::PathBuf::from),
            _ => path = Some(std::path::PathBuf::from(a)),
        }
    }
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
        Box::new(move |cc| {
            let mut app = app::SceneApp::new(cc);
            if let Some(p) = pose {
                app.set_pose(p);
            }
            if check {
                app.start_check();
            }
            if let Some(dir) = frames {
                app.film_to(dir);
            }
            if let Some(p) = path {
                app.open(&p);
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}
