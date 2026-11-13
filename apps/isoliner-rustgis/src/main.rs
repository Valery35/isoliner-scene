// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Isoliner3D для RustGIS - расширение-процесс RustGIS, третье
//! приложение линейки (`doc/APPS.md`), одно расширение на всё: сцена,
//! инструменты, документация. Сцена `.i3dscene` открывается окном
//! Isoliner Scene - крейт `scene-app`, общий с программой, без копии.
//! Инструменты считаются крейтом `tools` по описаниям из tools.json
//! модуля.
//!
//! Связь с RustGIS, окна инструментов, системы координат и итоги
//! слоями - крейт `rustgis-kit` (`C:\Dev\rustgis-kit`), своё у продукта -
//! [`product`].
//!
//! Запуск: `isoliner-rustgis rustgis-plugin`, его делает RustGIS
//! по `plugin.json`. Устройство команд - [`plugin`]. Окно сцены -
//! `isoliner-rustgis scene файл.i3dscene`: им команда scene открывает
//! сцену своим процессом.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod convert;
mod i18n;
mod layers;
mod manual;
mod plugin;
mod product;
mod rgtools;

use std::ffi::OsString;
use std::path::Path;
use std::process::ExitCode;

/// Описания инструментов, выгрузка модуля (`tools/export_tools.py`).
/// Обновляются вместе с образцами: `make_fixtures.py` -> `fixtures/`.
const TOOLS_JSON: &[u8] = include_bytes!("../../../fixtures/tools.json");

/// Окно сцены своим процессом: RustGIS останавливает расширение
/// вскоре после ответа, окно должно его пережить. Как глобус
/// RustPlanetX (`planetx-lite`, `desktop::launch_linked`): без консоли,
/// вне задания RustGIS, если задание это разрешает.
fn launch(path: &Path) -> Result<(), String> {
    // окно сцены - этот же exe с ключом scene
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut args: Vec<OsString> = vec!["scene".into(), path.into()];
    // сцена из слоёв: систему сцены можно менять в окне, выбор - файлом
    if path == product::scene_path() {
        args.push("--crs-file".into());
        args.push(product::crs_path().into());
    }
    rustgis_kit::window::spawn_detached(&exe, &args)
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    let first = args.get(1).and_then(|a| a.to_str());
    // окно сцены, его запускает команда scene отдельным процессом
    if first == Some("scene") {
        let launch = scene_app::Launch::from_args(args.into_iter().skip(2));
        // окно сцены из слоёв и окно объёмного итога держат замок своего
        // пакета, пока живы: команда scene и инструмент тогда только
        // переписывают пакет, окно перечитывает его само
        let home = product::home();
        let _lock = launch
            .path
            .as_deref()
            .filter(|p| p.starts_with(&home))
            .and_then(rustgis_kit::window::hold_lock);
        return match scene_app::run(launch) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("Isoliner Scene: {e}");
                ExitCode::FAILURE
            }
        };
    }
    if first != Some("rustgis-plugin") {
        eprintln!("Isoliner3D для RustGIS. Запуск: isoliner-rustgis rustgis-plugin");
        return ExitCode::from(2);
    }
    let catalog = match tool_spec::parse_catalog(TOOLS_JSON) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("tools.json: {e}");
            return ExitCode::FAILURE;
        }
    };
    let ctx = plugin::Context {
        catalog: &catalog,
        launch: &launch,
    };
    // сессия RustGIS или одна команда на запуск - решает крейт
    match product::Isoliner::new(ctx, TOOLS_JSON) {
        Ok(p) => rustgis_kit::serve_stdio(&p),
        Err(e) => {
            eprintln!("tools.json: {e}");
            ExitCode::FAILURE
        }
    }
}
