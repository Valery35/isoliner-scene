// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Сессия расширения RustGIS (`"session": true`, docs/plugins.md RustGIS,
//! PR opengeos/RustGIS#1553): программа живёт, пока открыт RustGIS,
//! связь - строки JSON на вводе и выводе.
//!
//! - Первая строка - `{"session": {"abi", "plugin", "rustgis", "lang"}}`.
//! - Команды - `{"command": запрос, "id": n}`, ответ -
//!   `{"ok": ..., "id": n}` или `{"error": "...", "id": n}`.
//! - Запрос к RustGIS - `{"call": {"op", "params"}, "id": "is1"}`, ответ
//!   несёт тот же id. Нужны `host.layers` и `host.features`.
//! - События - `{"event": "layers"}` и другие: по `layers` пакет сцены
//!   из слоёв проекта пишется заново, окно перечитывает его само.
//!
//! Ввод закрылся - RustGIS закрыт, сессия кончается.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use serde_json::{Value as J, json};

use crate::layers;
use crate::plugin::{self, Context};

/// Связь с RustGIS: ввод, вывод, счётчик запросов.
pub struct Host<R: BufRead, W: Write> {
    input: R,
    output: W,
    next: u64,
    /// Команды и события, пришедшие, пока ждали ответа RustGIS.
    queued: Vec<J>,
}

impl<R: BufRead, W: Write> Host<R, W> {
    pub fn new(input: R, output: W) -> Self {
        Self {
            input,
            output,
            next: 0,
            queued: Vec::new(),
        }
    }

    fn send(&mut self, v: &J) -> Result<(), String> {
        writeln!(self.output, "{v}")
            .and_then(|()| self.output.flush())
            .map_err(|e| e.to_string())
    }

    /// Следующая строка JSON ввода. Ввод закрыт - None.
    fn read(&mut self) -> Option<J> {
        loop {
            let mut line = String::new();
            match self.input.read_line(&mut line) {
                Ok(0) | Err(_) => return None,
                Ok(_) => {}
            }
            let line = line.trim_start_matches('\u{feff}').trim();
            if line.is_empty() {
                continue;
            }
            // строка не JSON - чужой вывод, пропускается
            if let Ok(v) = serde_json::from_str::<J>(line) {
                return Some(v);
            }
        }
    }

    /// Запрос к RustGIS и его ответ. Пришедшее до ответа запоминается.
    pub fn call(&mut self, op: &str, params: J) -> Result<J, String> {
        self.next += 1;
        let id = format!("is{}", self.next);
        self.send(&json!({"call": {"op": op, "params": params}, "id": id}))?;
        loop {
            let Some(v) = self.read() else {
                return Err("RustGIS closed the plugin session".to_string());
            };
            // RustGIS без сессий отвечает на запрос одной строкой без id
            let bare = v.get("id").is_none() && (v.get("ok").is_some() || v.get("error").is_some());
            if bare || v["id"].as_str() == Some(id.as_str()) {
                return match v.get("error") {
                    Some(e) => Err(e.as_str().unwrap_or("error").to_string()),
                    None => Ok(v["ok"].clone()),
                };
            }
            self.queued.push(v);
        }
    }

    /// Следующее сообщение RustGIS: сначала отложенные.
    fn next_message(&mut self) -> Option<J> {
        if self.queued.is_empty() {
            self.read()
        } else {
            Some(self.queued.remove(0))
        }
    }
}

/// Слои проекта для сцены и их система: `crs` - код системы, в метрах
/// которой лежат объекты, None - объекты в градусах WGS 84.
pub struct Layers {
    pub list: Vec<(String, J)>,
    pub crs: Option<String>,
}

/// Файл выбора системы сцены: код EPSG, его пишет окно сцены
/// (поле `scene_crs` окна «Параметры сцены», ключ `--crs-file`).
pub fn crs_path() -> PathBuf {
    scene_path().with_extension("crs")
}

/// Система сцены, как у карты (решение автора от 13 ноября 2026 года):
/// выбор в окне сцены, иначе проекция карты RustGIS (`host.view`,
/// `"EPSG:n"`), иначе None - зона UTM по середине слоёв. Проекции карты
/// «mercator» и «globe» - не система для метров сцены.
fn scene_crs<R: BufRead, W: Write>(host: &mut Host<R, W>, chosen: &Path) -> Option<String> {
    if let Some(c) = std::fs::read_to_string(chosen)
        .ok()
        .and_then(|t| crate::rgtools::epsg(&t))
    {
        return Some(c);
    }
    let view = host.call("host.view", json!({})).ok()?;
    view["projection"].as_str().and_then(crate::rgtools::epsg)
}

/// Слои проекта для сцены: видимые векторные, сверху вниз, с объектами.
///
/// Сцена строится в метрах системы [`scene_crs`]: RustGIS выгружает
/// слои Shapefile в эту систему сам. Системы нет, она в градусах, итог
/// тел Isoliner3D в слоях или выгрузка не удалась - сцена в метрах зоны
/// UTM по градусам `host.features`.
pub fn project_layers<R: BufRead, W: Write>(host: &mut Host<R, W>) -> Result<Layers, String> {
    let list = host.call("host.layers", json!({}))?;
    let mut out = Vec::new();
    let mut ids = Vec::new();
    let mut bodies = false;
    for l in list.as_array().into_iter().flatten() {
        let visible = l["visible"].as_bool().unwrap_or(true);
        let raster = l["raster"].as_bool().unwrap_or(false);
        let Some(id) = l["id"].as_i64() else { continue };
        if !visible || raster {
            continue;
        }
        let fc = host.call("host.features", json!({"layer": id}))?;
        let name = l["name"].as_str().unwrap_or("").to_string();
        // итог тел Isoliner3D - телами с высотой, а не контуром плана
        let with = crate::rgtools::with_bodies(&name, fc.clone());
        bodies |= with != fc;
        ids.push(id);
        out.push((name, with));
    }
    let target = if bodies || ids.is_empty() {
        None
    } else {
        scene_crs(host, &crs_path())
    };
    if let Some(target) = target
        && let Some((list, crs)) = native(host, &ids, &out, &target)
    {
        return Ok(Layers {
            list,
            crs: Some(crs),
        });
    }
    Ok(Layers {
        list: out,
        crs: None,
    })
}

/// Слои в метрах системы `target` выгрузкой RustGIS. None - система
/// в градусах или выгрузка не удалась.
fn native<R: BufRead, W: Write>(
    host: &mut Host<R, W>,
    ids: &[i64],
    named: &[(String, J)],
    target: &str,
) -> Option<(Vec<(String, J)>, String)> {
    let dir = scene_path().with_extension("export");
    std::fs::create_dir_all(&dir).ok()?;
    let mut out = Vec::new();
    for (id, (name, _)) in ids.iter().zip(named) {
        let shp = dir.join(format!("layer-{id}.shp"));
        for ext in ["shp", "shx", "dbf", "prj", "cpg"] {
            let _ = std::fs::remove_file(shp.with_extension(ext));
        }
        let path = shp.display().to_string().replace('\\', "/");
        host.call(
            "share.exportLayer",
            json!({"layer": id, "path": path, "crs": target}),
        )
        .ok()?;
        let fc = std::fs::read(&shp)
            .ok()
            .and_then(|b| crate::shp::read(&b).ok())?;
        // система в градусах - не метры сцены
        if out.is_empty() && degrees(&fc) {
            return None;
        }
        out.push((name.clone(), fc));
    }
    Some((out, target.to_string()))
}

/// Все координаты в пределах градусов: слой в долготе и широте.
fn degrees(fc: &J) -> bool {
    let mut all = true;
    let mut any = false;
    fn walk(v: &J, f: &mut dyn FnMut(f64, f64)) {
        if let Some(a) = v.as_array() {
            if let (Some(x), Some(y)) =
                (a.first().and_then(J::as_f64), a.get(1).and_then(J::as_f64))
            {
                f(x, y);
            } else {
                for x in a {
                    walk(x, f);
                }
            }
        }
    }
    for feat in fc["features"].as_array().into_iter().flatten() {
        walk(&feat["geometry"]["coordinates"], &mut |x, y| {
            any = true;
            all &= x.abs() <= 180.0 && y.abs() <= 90.0;
        });
    }
    any && all
}

/// Куда пишется сцена из слоёв проекта: временная папка, как пакет
/// кнопки модуля (`<temp>/isoliner3d`).
pub fn scene_path() -> PathBuf {
    // тесты пишут свой пакет: живая сцена автора и чужие прогоны не задеты
    let dir = if cfg!(test) {
        format!("isoliner3d-test-{}", std::process::id())
    } else {
        "isoliner3d".to_string()
    };
    std::env::temp_dir().join(dir).join("RustGIS.i3dscene")
}

/// Записать пакет из слоёв проекта: сначала во временный файл, потом
/// заменой, чтобы окно не прочло его наполовину.
pub fn write_scene(layers: &Layers, path: &Path, title: &str) -> Result<usize, String> {
    let pkg = match &layers.crs {
        Some(c) => layers::package_native(&layers.list, title, c),
        None => layers::package(&layers.list, title),
    }
    .ok_or_else(|| "no features in visible layers".to_string())?;
    let bytes = scene_format::write_package(&pkg).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let part = path.with_extension("i3dscene.part");
    std::fs::write(&part, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&part, path).map_err(|e| e.to_string())?;
    Ok(pkg.parts.len())
}

/// Сцена из слоёв проекта: пакет и окно.
fn scene_from_layers<R: BufRead, W: Write>(
    host: &mut Host<R, W>,
    request: &J,
    ctx: &Context<'_>,
) -> J {
    let path = scene_path();
    let layers = match project_layers(host) {
        Ok(l) => l,
        Err(e) => return json!({"error": e}),
    };
    if let Err(e) = write_scene(&layers, &path, "RustGIS") {
        return json!({"error": e});
    }
    let mut req = request.clone();
    req["params"] = json!({"path": path.display().to_string()});
    plugin::answer(&req.to_string(), ctx)
}

/// Команда одного запуска (RustGIS без сессий, сборки до PR #1553):
/// `scene` без пути строит сцену из слоёв проекта запросами
/// `host.layers` и `host.features` внутри этого же запуска, остальное -
/// как обычно. Пока окно сцены открыто, новая команда только
/// переписывает пакет, окно перечитывает его само.
pub fn once<R: BufRead, W: Write>(host: &mut Host<R, W>, line: &str, ctx: &Context<'_>) -> J {
    once_with(host, line, ctx, window_open)
}

/// [`once`] с проверкой «окно открыто» со стороны: тесту.
fn once_with<R: BufRead, W: Write>(
    host: &mut Host<R, W>,
    line: &str,
    ctx: &Context<'_>,
    window_open: fn() -> bool,
) -> J {
    let req: J =
        serde_json::from_str(line.trim_start_matches('\u{feff}').trim()).unwrap_or(J::Null);
    let params = match &req["params"] {
        J::String(s) => serde_json::from_str(s).unwrap_or(J::Null),
        p => p.clone(),
    };
    let from_layers = req["id"] == "scene" && params["path"].as_str().is_none_or(str::is_empty);
    if !from_layers {
        return plugin::answer(line, ctx);
    }
    if window_open() {
        let path = scene_path();
        return match project_layers(host).and_then(|l| write_scene(&l, &path, "RustGIS")) {
            Ok(n) => {
                json!({"ok": {"name": "RustGIS", "parts": n, "path": path.display().to_string()}})
            }
            Err(e) => json!({"error": e}),
        };
    }
    scene_from_layers(host, &req, ctx)
}

/// Замок окна сцены из слоёв: окно держит файл открытым без общего
/// доступа, пока живо.
pub fn lock_path() -> PathBuf {
    scene_path().with_extension("lock")
}

/// Открыто ли окно сцены из слоёв: замок занят.
#[cfg(windows)]
fn window_open() -> bool {
    use std::os::windows::fs::OpenOptionsExt;
    let path = lock_path();
    path.exists()
        && std::fs::OpenOptions::new()
            .write(true)
            .share_mode(0)
            .open(&path)
            .is_err()
}

#[cfg(not(windows))]
fn window_open() -> bool {
    false
}

/// Занять замок на время жизни окна. None - не вышло, окно живёт без
/// замка.
#[cfg(windows)]
pub fn hold_lock() -> Option<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    let path = lock_path();
    std::fs::create_dir_all(path.parent()?).ok()?;
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .share_mode(0)
        .open(path)
        .ok()
}

#[cfg(not(windows))]
pub fn hold_lock() -> Option<std::fs::File> {
    None
}

/// Обслужить сессию до закрытия ввода. `lang` - из первой строки.
pub fn serve<R: BufRead, W: Write>(host: &mut Host<R, W>, lang: &str, ctx: &Context<'_>) {
    // сцена из слоёв открыта: по событию `layers` пакет пишется заново
    let mut live = false;
    while let Some(msg) = host.next_message() {
        if let Some(cmd) = msg.get("command") {
            let id = msg["id"].clone();
            let mut req = cmd.clone();
            if req.get("lang").is_none() {
                req["lang"] = json!(lang);
            }
            let from_layers =
                req["id"] == "scene" && req["params"]["path"].as_str().is_none_or(str::is_empty);
            let mut answer = if from_layers {
                let a = scene_from_layers(host, &req, ctx);
                live = a.get("ok").is_some();
                a
            } else {
                plugin::answer(&req.to_string(), ctx)
            };
            answer["id"] = id;
            if host.send(&answer).is_err() {
                return;
            }
        } else if msg["event"] == "layers" && live {
            // окно перечитает пакет само; не вышло - сцена остаётся прежней
            if let Ok(l) = project_layers(host) {
                let _ = write_scene(&l, &scene_path(), "RustGIS");
            }
        }
    }
}

/// Первая строка - начало сессии? Тогда её язык.
pub fn hello(line: &str) -> Option<String> {
    let v: J = serde_json::from_str(line.trim_start_matches('\u{feff}').trim()).ok()?;
    let s = v.get("session")?.as_object()?;
    Some(
        s.get("lang")
            .and_then(J::as_str)
            .unwrap_or("en")
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Тесты сцены пишут один пакет во временной папке: по очереди.
    static SCENE: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn hello_line() {
        assert_eq!(
            hello(r#"{"session":{"abi":1,"plugin":"isoliner3d","rustgis":"0.6.0","lang":"ru"}}"#),
            Some("ru".to_string())
        );
        assert_eq!(hello(r#"{"id":"tools"}"#), None);
        assert_eq!(hello("not json"), None);
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
        let replies = [
            json!({"ok": [{"id": 1, "name": "axes", "visible": true, "raster": false}]}),
            json!({"ok": fc}),
        ];
        let input: String = replies.iter().map(|l| format!("{l}\n")).collect();
        let mut out = Vec::new();
        let launched = std::cell::Cell::new(0);
        let launch = |_: &Path| {
            launched.set(launched.get() + 1);
            Ok(())
        };
        let catalog =
            tool_spec::parse_catalog(include_bytes!("../../../fixtures/tools.json")).unwrap();
        let ctx = Context {
            catalog: &catalog,
            launch: &launch,
        };
        let mut host = Host::new(input.as_bytes(), &mut out);
        let line = json!({"abi": 1, "kind": "command", "id": "scene", "params": {}, "lang": "ru"});
        let answer = once_with(&mut host, &line.to_string(), &ctx, || false);
        assert_eq!(answer["ok"]["parts"], 1, "{answer}");
        assert_eq!(launched.get(), 1);
        let said = String::from_utf8(out).unwrap();
        assert!(said.contains("host.layers") && said.contains("host.features"));
        // команда с путём - как прежде, без запросов к RustGIS
        let mut out = Vec::new();
        let mut host = Host::new(&b""[..], &mut out);
        let line = json!({"id": "about", "lang": "en"});
        assert!(once(&mut host, &line.to_string(), &ctx)["ok"].is_object());
        assert!(out.is_empty());
    }

    /// Сессия как у RustGIS: команда about по id, сцена из слоёв -
    /// расширение спрашивает host.layers и host.features, пишет пакет
    /// и отвечает тем же id; событие layers переписывает пакет.
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
            json!({"command": {"abi": 1, "kind": "command", "id": "about", "params": {}}, "id": 1}),
            json!({"command": {"abi": 1, "kind": "command", "id": "scene", "params": {}}, "id": 2}),
            json!({"ok": layers, "id": "is1"}),
            json!({"ok": fc, "id": "is2"}),
        ];
        let input: String = lines.iter().map(|l| format!("{l}\n")).collect();
        let mut out = Vec::new();
        let launched = std::cell::Cell::new(0);
        let launch = |_: &Path| {
            launched.set(launched.get() + 1);
            Ok(())
        };
        let catalog =
            tool_spec::parse_catalog(include_bytes!("../../../fixtures/tools.json")).unwrap();
        let ctx = Context {
            catalog: &catalog,
            launch: &launch,
        };
        let mut host = Host::new(input.as_bytes(), &mut out);
        serve(&mut host, "ru", &ctx);
        let said: Vec<J> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(said[0]["id"], 1);
        assert_eq!(said[0]["ok"]["title"], "Isoliner Scene");
        assert_eq!(said[1]["call"]["op"], "host.layers");
        assert_eq!(said[2]["call"]["op"], "host.features");
        assert_eq!(said[2]["call"]["params"]["layer"], 3);
        // проекция карты не ответила - сцена по градусам в зоне UTM
        assert_eq!(said[3]["call"]["op"], "host.view");
        assert_eq!(said[4]["id"], 2);
        assert_eq!(said[4]["ok"]["parts"], 1, "{}", said[4]);
        assert_eq!(launched.get(), 1);
        let pkg = scene_format::read_package(&std::fs::read(scene_path()).unwrap()).unwrap();
        assert_eq!(pkg.parts[0].name, "Линии");
    }
}
