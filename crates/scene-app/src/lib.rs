// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Окно сцены Isoliner3D библиотекой: пакет `.i3dscene` той же камерой,
//! что в окне модуля QGIS, плашка и окно «Параметры сцены» модуля,
//! обрезка, вид по разрезу, тени, видео, выгрузка.
//!
//! Окно одно на линейку: его открывают программа Isoliner Scene
//! (`apps/isoliner-scene`, и страницей в браузере) и расширение для
//! RustGIS (`apps/isoliner-rustgis`, команда `scene`). Копии кода окна
//! нет ни там, ни там.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod app;
mod i18n;
mod share;
mod shot;
mod toolbar;
#[cfg(not(target_arch = "wasm32"))]
mod video;
#[cfg(not(target_arch = "wasm32"))]
mod watch;
#[cfg(target_arch = "wasm32")]
mod web;

pub use app::SceneApp;

/// С чем открыть окно: ключи командной строки программы.
#[derive(Debug, Default, Clone)]
pub struct Launch {
    /// Пакет `.i3dscene`.
    pub path: Option<std::path::PathBuf>,
    /// Камера из ссылки «Поделиться».
    pub pose: Option<scene_core::Camera>,
    /// Самопроверка с отчётом в вывод.
    pub check: bool,
    /// Кадры оборота PNG в папку и ролик MP4, потом окно закрывается.
    pub frames: Option<std::path::PathBuf>,
}

impl Launch {
    /// Разобрать ключи: `файл.i3dscene`, `--pose x,y,z,...`, `--check`,
    /// `--frames ПАПКА`.
    pub fn from_args(args: impl IntoIterator<Item = std::ffi::OsString>) -> Self {
        let mut l = Launch::default();
        let mut args = args.into_iter();
        while let Some(a) = args.next() {
            match a.to_str() {
                Some("--pose") => {
                    l.pose = args
                        .next()
                        .and_then(|v| v.to_str().and_then(share::parse_pose));
                }
                Some("--check") => l.check = true,
                Some("--frames") => l.frames = args.next().map(std::path::PathBuf::from),
                _ => l.path = Some(std::path::PathBuf::from(a)),
            }
        }
        l
    }
}

/// Открыть окно сцены и ждать, пока его закроют.
#[cfg(not(target_arch = "wasm32"))]
pub fn run(launch: Launch) -> eframe::Result {
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
            let mut app = SceneApp::new(cc);
            if let Some(p) = launch.pose {
                app.set_pose(p);
            }
            if launch.check {
                app.start_check();
            }
            if let Some(dir) = launch.frames {
                app.film_to(dir);
            }
            if let Some(p) = launch.path {
                app.open(&p);
            }
            Ok(Box::new(app))
        }),
    )
}

/// Запустить окно на холсте страницы.
#[cfg(target_arch = "wasm32")]
pub fn start_web() {
    web::start();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_keys() {
        let l = Launch::from_args(
            ["a.i3dscene", "--check", "--frames", "out"].map(std::ffi::OsString::from),
        );
        assert_eq!(l.path.as_deref(), Some(std::path::Path::new("a.i3dscene")));
        assert!(l.check);
        assert_eq!(l.frames.as_deref(), Some(std::path::Path::new("out")));
    }
}
