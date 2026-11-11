// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Isoliner3D для RustGIS - расширение-процесс RustGIS, третье
//! приложение линейки (`doc/APPS.md`). Сцена `.i3dscene` открывается
//! в окне Isoliner Scene, инструменты 2.23 и 2.24 считаются крейтом
//! `tools` по описаниям из tools.json модуля.
//!
//! Запуск: `isoliner-rustgis rustgis-plugin`, его делает RustGIS
//! по `plugin.json`. Устройство команд - [`plugin`].

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod convert;
mod i18n;
mod plugin;

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

/// Описания инструментов, выгрузка модуля (`tools/export_tools.py`).
/// Обновляются вместе с образцами: `make_fixtures.py` -> `fixtures/`.
const TOOLS_JSON: &[u8] = include_bytes!("../../../fixtures/tools.json");

/// Окно сцены - программа isoliner-scene рядом с этой.
fn scene_exe() -> Result<PathBuf, String> {
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = me.parent().ok_or("нет папки программы")?;
    let exe = dir.join(format!("isoliner-scene{}", std::env::consts::EXE_SUFFIX));
    if exe.is_file() {
        Ok(exe)
    } else {
        Err(exe.display().to_string())
    }
}

/// Окно сцены своим процессом: RustGIS останавливает расширение
/// вскоре после ответа, окно должно его пережить. Как глобус
/// RustPlanetX (`planetx-lite`, `desktop::launch_linked`): без консоли,
/// вне задания RustGIS, если задание это разрешает.
fn launch(path: &Path) -> Result<(), String> {
    let exe = scene_exe()?;
    let spawn = |flags: u32| {
        let mut cmd = Command::new(&exe);
        cmd.arg(path)
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
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some("rustgis-plugin") {
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
