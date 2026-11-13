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
//! Запуск: `isoliner-rustgis rustgis-plugin`, его делает RustGIS
//! по `plugin.json`. Устройство команд - [`plugin`]. Окно сцены -
//! `isoliner-rustgis scene файл.i3dscene`: им команда scene открывает
//! сцену своим процессом.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod convert;
mod i18n;
mod plugin;

use std::io::{BufRead, Write};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

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
    let spawn = |flags: u32| {
        let mut cmd = Command::new(&exe);
        cmd.arg("scene")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(flags);
        }
        #[cfg(not(windows))]
        let _ = flags;
        cmd.spawn().map(|_| ())
    };
    // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP, сначала
    // с CREATE_BREAKAWAY_FROM_JOB
    const DETACHED: u32 = 0x0000_0008 | 0x0000_0200;
    const BREAKAWAY: u32 = 0x0100_0000;
    spawn(DETACHED | BREAKAWAY)
        .or_else(|_| spawn(DETACHED))
        .map_err(|e| e.to_string())
}

fn main() -> ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let first = args.get(1).and_then(|a| a.to_str());
    // окно сцены, его запускает команда scene отдельным процессом
    if first == Some("scene") {
        let launch = scene_app::Launch::from_args(args.into_iter().skip(2));
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
    let mut line = String::new();
    if let Err(e) = std::io::stdin().lock().read_line(&mut line) {
        eprintln!("RustGIS: {e}");
        return ExitCode::FAILURE;
    }
    let ctx = plugin::Context {
        catalog: &catalog,
        launch: &launch,
    };
    let reply = plugin::answer(&line, &ctx);
    let mut out = std::io::stdout().lock();
    match writeln!(out, "{reply}").and_then(|()| out.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("RustGIS: {e}");
            ExitCode::FAILURE
        }
    }
}
