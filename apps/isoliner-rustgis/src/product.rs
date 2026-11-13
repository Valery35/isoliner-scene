// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Isoliner3D как продукт крейта `rustgis-kit`: что у расширения своё.
//!
//! Общее с другими расширениями линейки (протокол RustGIS, окно
//! инструмента по tools.json, системы координат, итоги слоями, высоты
//! итогов, упаковка) - в `rustgis-kit` (`C:\Dev\rustgis-kit`). Здесь:
//!
//! - расчёт инструментов крейтом `tools` по значениям прогона модуля
//!   ([`Isoliner::compute`]);
//! - окно сцены Isoliner Scene: сцена из слоёв проекта, пакет `.i3dscene`,
//!   итоги с высотой своим пакетом ([`Isoliner::show`]);
//! - команды `scene`, `tools`, `run`, `about` ([`crate::plugin`]).

use std::path::{Path, PathBuf};

use rustgis_kit::catalog::Tool as KitTool;
use rustgis_kit::project::{Layers, project_layers};
use rustgis_kit::{Call, Failure, Lang as KitLang, Options, Product, Store};
use serde_json::{Value as J, json};
use tool_spec::{Arg, Lang, VecReport};

use crate::plugin::{self, Context};
use crate::{convert, i18n, layers};

/// Инструменты Isoliner3D в RustGIS, в порядке панели.
pub const OPTIONS: Options = Options {
    toolbox: "Isoliner3D",
    offered: &[
        "sweep_body",
        "contour_crown",
        "bench_report",
        "density_3d",
        "desurvey_holes",
        "fill_to_volume",
        "hole_intervals",
        "select_by_shell",
        "zonal_shell_stats",
        "boolean_shells",
    ],
    // устья 2.17 таблицей с X и Y в метрах своей системы
    xy_tables: &[("desurvey_holes", "COLLARS")],
};

/// Папка данных расширения во временной папке системы.
pub fn home() -> PathBuf {
    rustgis_kit::store::temp_home("isoliner3d")
}

/// Пакет сцены из слоёв проекта.
pub fn scene_path() -> PathBuf {
    home().join("RustGIS.i3dscene")
}

/// Файл выбора системы сцены (поле `scene_crs` окна, ключ `--crs-file`).
pub fn crs_path() -> PathBuf {
    scene_path().with_extension("crs")
}

/// Пакеты объёмных итогов инструментов.
pub fn results_dir() -> PathBuf {
    scene_path().with_extension("results")
}

/// Хранилище высот итогов.
pub fn store() -> Store {
    Store::new(scene_path().with_extension("bodies"))
}

fn lang(l: KitLang) -> Lang {
    match l {
        KitLang::Ru => Lang::Ru,
        KitLang::En => Lang::En,
    }
}

/// Записать пакет сцены из слоёв заменой: окно не прочтёт его наполовину.
pub fn write_scene(layers: &Layers, path: &Path, title: &str) -> Result<usize, String> {
    let pkg = match &layers.crs {
        Some(c) => layers::package_native(&layers.list, title, c),
        None => layers::package(&layers.list, title),
    }
    .ok_or_else(|| "no features in visible layers".to_string())?;
    let bytes = scene_format::write_package(&pkg).map_err(|e| e.to_string())?;
    rustgis_kit::window::write_replace(path, &bytes)?;
    Ok(pkg.parts.len())
}

/// Isoliner3D для RustGIS.
pub struct Isoliner<'a> {
    pub ctx: Context<'a>,
    /// Описания инструментов для крейта (те же tools.json).
    pub kit: rustgis_kit::Catalog,
    /// Открыто ли окно с пакетом: замок окна. Тесту - своя проверка.
    pub window_open: fn(&Path) -> bool,
}

impl<'a> Isoliner<'a> {
    pub fn new(ctx: Context<'a>, tools_json: &[u8]) -> Result<Self, String> {
        Ok(Self {
            ctx,
            kit: rustgis_kit::Catalog::parse(tools_json)?,
            window_open: rustgis_kit::window::window_open_for,
        })
    }

    /// Сцена из слоёв проекта: пакет и окно. Окно открыто - только пакет,
    /// окно перечитает его само.
    fn scene_from_layers(&self, host: &mut dyn Call, req: &J) -> J {
        let path = scene_path();
        let layers = match project_layers(
            host,
            &store(),
            &crs_path(),
            &scene_path().with_extension("export"),
        ) {
            Ok(l) => l,
            Err(e) => return json!({"error": e}),
        };
        let n = match write_scene(&layers, &path, "RustGIS") {
            Ok(n) => n,
            Err(e) => return json!({"error": e}),
        };
        if (self.window_open)(&path) {
            return json!({"ok": {"name": "RustGIS", "parts": n, "path": path.display().to_string()}});
        }
        let mut req = req.clone();
        req["params"] = json!({"path": path.display().to_string()});
        plugin::answer(&req.to_string(), &self.ctx)
    }
}

impl Product for Isoliner<'_> {
    fn catalog(&self) -> &rustgis_kit::Catalog {
        &self.kit
    }

    fn options(&self) -> &Options {
        &OPTIONS
    }

    fn store(&self) -> Store {
        store()
    }

    /// Расчёт крейтом `tools` по значениям прогона модуля.
    fn compute(&self, t: &KitTool, values: &J, l: KitLang) -> Result<(J, Vec<String>), Failure> {
        let lang = lang(l);
        let (Some(tool), Some(run)) = (self.ctx.catalog.tool(&t.name), tools::run_fn(&t.name))
        else {
            return Err(Failure::Run(t.name.clone()));
        };
        let values = convert::values(tool, values).map_err(|key| {
            let label = tool
                .param(&key)
                .map_or(key.clone(), |p| p.label.get(lang).to_string());
            Failure::Values(i18n::tr(
                lang,
                "%s: значение не того вида",
                &[Arg::Text(label)],
            ))
        })?;
        let wrong = tool.check(&values, lang);
        if !wrong.is_empty() {
            return Err(Failure::Values(wrong.join(", ")));
        }
        let mut report = VecReport::default();
        let result = run(&values, &mut report);
        let messages: Vec<String> = report
            .lines
            .iter()
            .map(|(_, m)| i18n::report_text(tool, m, lang))
            .collect();
        match result {
            Ok(out) => Ok((convert::outputs(tool, &out, lang), messages)),
            Err(e) => Err(Failure::Run(i18n::report_text(tool, &e.0, lang))),
        }
    }

    fn command(&self, host: &mut dyn Call, req: &J, _lang: KitLang) -> J {
        let params = rustgis_kit::host::params_of(req);
        if req["id"] == "scene" && params["path"].as_str().is_none_or(str::is_empty) {
            return self.scene_from_layers(host, req);
        }
        plugin::answer(&req.to_string(), &self.ctx)
    }

    /// Итоги с высотой - своим пакетом сцены, окно открыто - перечитает.
    fn show(&self, tool: &KitTool, solids: &[(String, J)], l: KitLang) -> Option<String> {
        let lang = lang(l);
        let title = tool.display_name(l);
        let path = results_dir().join(format!("{}.i3dscene", tool.name));
        let set = Layers {
            list: solids.to_vec(),
            crs: None,
        };
        let shown = write_scene(&set, &path, &title).and_then(|_| {
            if (self.window_open)(&path) {
                Ok(())
            } else {
                (self.ctx.launch)(&path)
            }
        });
        Some(match shown {
            Ok(()) => i18n::tr(
                lang,
                "%s: объёмный итог - в окне сцены.",
                &[Arg::Text(title)],
            ),
            Err(e) => i18n::tr(lang, "Окно сцены не запустилось: %s", &[Arg::Text(e)]),
        })
    }

    /// Слои изменились: открытое окно сцены из слоёв получает новый пакет.
    fn event(&self, host: &mut dyn Call, name: &str) {
        let path = scene_path();
        if name == "layers"
            && (self.window_open)(&path)
            && let Ok(l) =
                project_layers(host, &store(), &crs_path(), &path.with_extension("export"))
        {
            let _ = write_scene(&l, &path, "RustGIS");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// Тесты сцены пишут один пакет во временной папке: по очереди.
    static SCENE: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn catalog() -> tool_spec::Catalog {
        tool_spec::parse_catalog(crate::TOOLS_JSON).unwrap()
    }

    /// Строки вывода расширения.
    fn said(out: Vec<u8>) -> Vec<J> {
        String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    /// RustGIS без сессий: команда scene одним запуском, ответы host.*
    /// без id. Сцена строится из слоёв, окно запускается.
    #[test]
    fn scene_in_one_run() {
        let _turn = SCENE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let fc = json!({"type": "FeatureCollection", "features": [
            {"type": "Feature", "geometry": {"type": "LineString", "coordinates": [[56.0, 54.7], [56.001, 54.7]]}}
        ]});
        let lines = [
            json!({"abi": 1, "kind": "command", "id": "scene", "params": {}, "lang": "ru"}),
            json!({"ok": [{"id": 1, "name": "axes", "visible": true, "raster": false}]}),
            json!({"ok": fc}),
            json!({"ok": {"projection": "mercator"}}),
        ];
        let input: String = lines.iter().map(|l| format!("{l}\n")).collect();
        let launched = Cell::new(0);
        let launch = |_: &Path| {
            launched.set(launched.get() + 1);
            Ok(())
        };
        let cat = catalog();
        let mut p = Isoliner::new(
            Context {
                catalog: &cat,
                launch: &launch,
            },
            crate::TOOLS_JSON,
        )
        .unwrap();
        p.window_open = |_| false;
        let mut out = Vec::new();
        rustgis_kit::serve(&p, input.as_bytes(), &mut out).unwrap();
        let s = said(out);
        assert_eq!(s[0]["call"]["op"], "host.layers");
        assert_eq!(s[1]["call"]["op"], "host.features");
        assert_eq!(s[2]["call"]["op"], "host.view");
        assert_eq!(s[3]["ok"]["parts"], 1, "{}", s[3]);
        assert_eq!(launched.get(), 1);
        // окно открыто - только пакет, окно перечитает его само
        p.window_open = |_| true;
        let mut out = Vec::new();
        rustgis_kit::serve(&p, input.as_bytes(), &mut out).unwrap();
        assert_eq!(said(out)[3]["ok"]["parts"], 1);
        assert_eq!(launched.get(), 1);
        // команда без слоёв - как прежде, без запросов к RustGIS
        let mut out = Vec::new();
        let line = format!("{}\n", json!({"id": "about", "lang": "en"}));
        rustgis_kit::serve(&p, line.as_bytes(), &mut out).unwrap();
        let s = said(out);
        assert_eq!(s.len(), 1);
        assert!(s[0]["ok"].is_object());
        // запрос не JSON - ответ-ошибка
        let mut out = Vec::new();
        rustgis_kit::serve(&p, &b"not json\n"[..], &mut out).unwrap();
        assert!(said(out)[0]["error"].is_string());
    }

    /// Сессия как у RustGIS: команда about по id, сцена из слоёв -
    /// расширение спрашивает host.layers и host.features, пишет пакет
    /// и отвечает тем же id.
    #[test]
    fn session_round_trip() {
        let _turn = SCENE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let fc = json!({"type": "FeatureCollection", "features": [
            {"type": "Feature", "geometry": {"type": "LineString", "coordinates": [[57.0, 56.0, -10.0], [57.01, 56.01, -20.0]]}}
        ]});
        let layers = json!([{"id": 3, "name": "Линии", "visible": true, "raster": false},
                            {"id": 4, "name": "Растр", "visible": true, "raster": true}]);
        let lines = [
            json!({"session": {"abi": 1, "plugin": "isoliner3d", "rustgis": "0.6.0", "lang": "ru"}}),
            json!({"command": {"abi": 1, "kind": "command", "id": "about", "params": {}}, "id": 1}),
            json!({"command": {"abi": 1, "kind": "command", "id": "scene", "params": {}}, "id": 2}),
            json!({"ok": layers, "id": "kit1"}),
            json!({"ok": fc, "id": "kit2"}),
            json!({"error": "no view", "id": "kit3"}),
        ];
        let input: String = lines.iter().map(|l| format!("{l}\n")).collect();
        let launched = Cell::new(0);
        let launch = |_: &Path| {
            launched.set(launched.get() + 1);
            Ok(())
        };
        let cat = catalog();
        let mut p = Isoliner::new(
            Context {
                catalog: &cat,
                launch: &launch,
            },
            crate::TOOLS_JSON,
        )
        .unwrap();
        p.window_open = |_| false;
        let mut out = Vec::new();
        rustgis_kit::serve(&p, input.as_bytes(), &mut out).unwrap();
        let s = said(out);
        assert_eq!(s[0]["id"], 1);
        assert_eq!(s[0]["ok"]["title"], "Isoliner Scene");
        assert_eq!(s[1]["call"]["op"], "host.layers");
        assert_eq!(s[2]["call"]["op"], "host.features");
        assert_eq!(s[2]["call"]["params"]["layer"], 3);
        // проекция карты не ответила - сцена по градусам в зоне UTM
        assert_eq!(s[3]["call"]["op"], "host.view");
        assert_eq!(s[4]["id"], 2);
        assert_eq!(s[4]["ok"]["parts"], 1, "{}", s[4]);
        assert_eq!(launched.get(), 1);
        let pkg = scene_format::read_package(&std::fs::read(scene_path()).unwrap()).unwrap();
        assert_eq!(pkg.parts[0].name, "Линии");
    }
}
