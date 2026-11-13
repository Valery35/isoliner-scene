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
            if v["id"].as_str() == Some(id.as_str()) {
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

/// Слои проекта для сцены: видимые векторные, сверху вниз, с объектами.
pub fn project_layers<R: BufRead, W: Write>(
    host: &mut Host<R, W>,
) -> Result<Vec<(String, J)>, String> {
    let list = host.call("host.layers", json!({}))?;
    let mut out = Vec::new();
    for l in list.as_array().into_iter().flatten() {
        let visible = l["visible"].as_bool().unwrap_or(true);
        let raster = l["raster"].as_bool().unwrap_or(false);
        let Some(id) = l["id"].as_i64() else { continue };
        if !visible || raster {
            continue;
        }
        let fc = host.call("host.features", json!({"layer": id}))?;
        let name = l["name"].as_str().unwrap_or("").to_string();
        out.push((name, fc));
    }
    Ok(out)
}

/// Куда пишется сцена из слоёв проекта: временная папка, как пакет
/// кнопки модуля (`<temp>/isoliner3d`).
pub fn scene_path() -> PathBuf {
    std::env::temp_dir()
        .join("isoliner3d")
        .join("RustGIS.i3dscene")
}

/// Записать пакет из слоёв проекта: сначала во временный файл, потом
/// заменой, чтобы окно не прочло его наполовину.
pub fn write_scene(layers: &[(String, J)], path: &Path, title: &str) -> Result<usize, String> {
    let pkg = layers::package(layers, title)
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

    #[test]
    fn hello_line() {
        assert_eq!(
            hello(r#"{"session":{"abi":1,"plugin":"isoliner3d","rustgis":"0.6.0","lang":"ru"}}"#),
            Some("ru".to_string())
        );
        assert_eq!(hello(r#"{"id":"tools"}"#), None);
        assert_eq!(hello("not json"), None);
    }

    /// Сессия как у RustGIS: команда about по id, сцена из слоёв -
    /// расширение спрашивает host.layers и host.features, пишет пакет
    /// и отвечает тем же id; событие layers переписывает пакет.
    #[test]
    fn session_round_trip() {
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
        assert_eq!(said[3]["id"], 2);
        assert_eq!(said[3]["ok"]["parts"], 1, "{}", said[3]);
        assert_eq!(launched.get(), 1);
        let pkg = scene_format::read_package(&std::fs::read(scene_path()).unwrap()).unwrap();
        assert_eq!(pkg.parts[0].name, "Линии");
    }
}
