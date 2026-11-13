// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Команды расширения RustGIS.
//!
//! Расширение-процесс без сессии, как RustPlanetX у RustGIS без
//! сессий (`planetx-lite`, `ui::rustgislink::plugin`): RustGIS
//! запускает `isoliner-rustgis rustgis-plugin` на каждую команду,
//! первая строка ввода - запрос `{"id", "params", "lang"}`, ответ -
//! одна строка JSON `{"ok": ...}` или `{"error": "..."}`.
//!
//! Команды (`plugin.json`):
//!
//! - `scene` - открыть пакет `.i3dscene` в окне Isoliner Scene,
//!   `params.path`;
//! - `tools` - описания инструментов из tools.json на языке RustGIS;
//! - `run` - расчёт инструмента `params.tool` (имя Processing) по
//!   значениям `params.values`, итог - выходы и сообщения отчёта;
//! - `about` - «О программе»: название, версия, текст и ссылки из общего
//!   списка линейки (`scene_format::about`), как окно (i) модуля.

use std::path::Path;

use serde_json::{Value as J, json};
use tool_spec::{Arg, Catalog, Lang, VecReport};

use crate::convert;
use crate::i18n::{lang_of, tr};

/// Окружение команды: каталог инструментов и запуск окна сцены.
pub struct Context<'a> {
    pub catalog: &'a Catalog,
    /// Запустить окно сцены с пакетом отдельным процессом.
    pub launch: &'a dyn Fn(&Path) -> Result<(), String>,
}

fn error(text: String) -> J {
    json!({ "error": text })
}

/// Параметры запроса: объект или строка JSON с объектом.
fn params_of(request: &J) -> J {
    match &request["params"] {
        J::String(s) => serde_json::from_str(s).unwrap_or(J::Null),
        p => p.clone(),
    }
}

/// Ответ на строку запроса RustGIS.
pub fn answer(line: &str, ctx: &Context<'_>) -> J {
    let request: J = match serde_json::from_str(line.trim_start_matches('\u{feff}').trim()) {
        Ok(r) => r,
        Err(e) => {
            return error(tr(
                Lang::En,
                "Запрос RustGIS не разобран: %s",
                &[Arg::Text(e.to_string())],
            ));
        }
    };
    let lang = lang_of(request["lang"].as_str());
    let params = params_of(&request);
    let id = request["id"].as_str().unwrap_or("");
    match id {
        "scene" => scene(&params, lang, ctx),
        "tools" => json!({ "ok": tools(ctx.catalog, lang) }),
        "run" => run(&params, lang, ctx.catalog),
        "about" => about(lang),
        _ => error(tr(
            lang,
            "Isoliner3D: команды «%s» нет.",
            &[Arg::Text(id.to_string())],
        )),
    }
}

/// «О программе»: то же, что окно (i) модуля и Isoliner Scene.
fn about(lang: Lang) -> J {
    let en = lang == Lang::En;
    let version = env!("CARGO_PKG_VERSION");
    match scene_format::about::About::load() {
        Some(a) => {
            let s = a.shown(en);
            json!({ "ok": {
                "window": s.window,
                "title": s.title,
                "version": version,
                "text": s.text,
                "links": s.links.iter().map(|(label, url)| json!({"label": label, "url": url})).collect::<Vec<_>>(),
            }})
        }
        None => json!({ "ok": { "title": "Isoliner Scene", "version": version } }),
    }
}

fn scene(params: &J, lang: Lang, ctx: &Context<'_>) -> J {
    let Some(path) = params["path"].as_str().filter(|p| !p.is_empty()) else {
        return error(tr(lang, "Не указан пакет сцены: поле path.", &[]));
    };
    let path = Path::new(path);
    // пакет читается до запуска окна: ошибка уходит в RustGIS,
    // а не в окно, которое никто не ждёт
    let pkg = match std::fs::read(path)
        .map_err(|e| e.to_string())
        .and_then(|b| scene_format::read_package(&b).map_err(|e| e.to_string()))
    {
        Ok(p) => p,
        Err(e) => return error(tr(lang, "Пакет сцены не открылся: %s", &[Arg::Text(e)])),
    };
    if let Err(e) = (ctx.launch)(path) {
        return error(tr(lang, "Окно сцены не запустилось: %s", &[Arg::Text(e)]));
    }
    let text = tr(
        lang,
        "Сцена «%s» открывается: частей %d.",
        &[
            Arg::Text(pkg.meta.name.clone()),
            Arg::Int(pkg.parts.len() as i64),
        ],
    );
    json!({ "ok": { "name": pkg.meta.name, "parts": pkg.parts.len(), "text": text } })
}

/// Описания инструментов на языке показа: по ним RustGIS строит окно.
pub fn tools(catalog: &Catalog, lang: Lang) -> J {
    let list: Vec<J> = catalog
        .tools
        .iter()
        .filter(|t| tools::run_fn(&t.name).is_some())
        .map(|t| {
            let params: Vec<J> = t
                .params
                .iter()
                .map(|p| {
                    json!({
                        "key": p.key,
                        "kind": p.kind,
                        "label": p.label.get(lang),
                        "hint": p.hint.get(lang),
                        "default": p.default,
                        "options": p.options.iter().map(|o| o.get(lang)).collect::<Vec<_>>(),
                        "optional": p.optional,
                        "parent": p.parent,
                        "minimum": p.minimum,
                        "maximum": p.maximum,
                        "geometry": p.geometry,
                    })
                })
                .collect();
            json!({
                "name": t.name,
                "code": t.code,
                "title": t.display_name(lang),
                "help": t.help.get(lang),
                "params": params,
            })
        })
        .collect();
    json!({ "version": catalog.version, "tools": list })
}

fn run(params: &J, lang: Lang, catalog: &Catalog) -> J {
    let name = params["tool"].as_str().unwrap_or("");
    let (Some(tool), Some(run)) = (catalog.tool(name), tools::run_fn(name)) else {
        return error(tr(
            lang,
            "Инструмента «%s» нет.",
            &[Arg::Text(name.to_string())],
        ));
    };
    let values = match convert::values(tool, &params["values"]) {
        Ok(v) => v,
        Err(key) => {
            let label = tool
                .param(&key)
                .map_or(key.clone(), |p| p.label.get(lang).to_string());
            let why = tr(lang, "%s: значение не того вида", &[Arg::Text(label)]);
            return error(tr(lang, "Значения не годятся: %s", &[Arg::Text(why)]));
        }
    };
    let bad = tool.check(&values, lang);
    if !bad.is_empty() {
        return error(tr(
            lang,
            "Значения не годятся: %s",
            &[Arg::Text(bad.join(", "))],
        ));
    }
    let mut report = VecReport::default();
    let result = run(&values, &mut report);
    // сообщения отчёта - на языке RustGIS, шаблоны из tools.json
    let lines: Vec<J> = report
        .lines
        .iter()
        .map(|(level, m)| {
            json!([
                level,
                tool_spec::format(tool.translate(m.template, lang), &m.args)
            ])
        })
        .collect();
    match result {
        Ok(out) => json!({ "ok": {
            "tool": tool.name,
            "outputs": convert::outputs(tool, &out, lang),
            "report": lines,
        }}),
        Err(e) => {
            let text = tool_spec::format(tool.translate(e.0.template, lang), &e.0.args);
            json!({
                "error": tr(lang, "Расчёт не удался: %s", &[Arg::Text(text)]),
                "report": lines,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::path::PathBuf;

    /// Команды, которые разбирает [`answer`], как в `plugin.json`.
    const COMMANDS: &[&str] = &["scene", "tools", "run", "about"];

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(format!(
            "{}/../../fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
    }

    fn catalog() -> Catalog {
        tool_spec::parse_catalog(&std::fs::read(fixture("tools.json")).unwrap()).unwrap()
    }

    fn ask(cat: &Catalog, request: J) -> J {
        let no_window = |_: &Path| -> Result<(), String> { Ok(()) };
        let ctx = Context {
            catalog: cat,
            launch: &no_window,
        };
        answer(&request.to_string(), &ctx)
    }

    /// Команды и переводы plugin.json совпадают с кодом.
    #[test]
    fn manifest_matches_the_commands() {
        let path = format!("{}/plugin.json", env!("CARGO_MANIFEST_DIR"));
        let m: J = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let ids: Vec<&str> = m["commands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, COMMANDS);
        let ru = &m["i18n"]["ru"];
        let mut texts = vec![m["description"].as_str().unwrap()];
        for c in m["commands"].as_array().unwrap() {
            texts.push(c["label"].as_str().unwrap());
            texts.push(c["help"].as_str().unwrap());
        }
        for t in texts {
            assert!(ru[t].is_string(), "нет перевода: {t}");
        }
        assert_eq!(m["version"].as_str(), Some(env!("CARGO_PKG_VERSION")));
        // каждая команда отвечает, а не «команды нет»
        let cat = catalog();
        for id in COMMANDS {
            let r = ask(&cat, json!({"id": id, "lang": "en"}));
            let e = r["error"].as_str().unwrap_or("");
            assert!(!e.contains("no command"), "{id}: {e}");
        }
    }

    #[test]
    fn unknown_and_broken_requests_get_an_error() {
        let cat = catalog();
        let r = ask(&cat, json!({"id": "nope", "lang": "ru"}));
        assert_eq!(r["error"], "Isoliner3D: команды «nope» нет.");
        let ctx = Context {
            catalog: &cat,
            launch: &|_| Ok(()),
        };
        assert!(answer("not json", &ctx)["error"].is_string());
        assert!(answer("", &ctx)["error"].is_string());
    }

    #[test]
    fn tools_are_listed_in_the_language_of_rustgis() {
        let cat = catalog();
        let r = ask(&cat, json!({"id": "tools", "lang": "ru"}));
        let list = r["ok"]["tools"].as_array().unwrap();
        let names: Vec<&str> = list.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"sweep_body") && names.contains(&"fill_to_volume"));
        for t in list {
            let title = t["title"].as_str().unwrap();
            assert!(title.starts_with(t["code"].as_str().unwrap()));
            assert!(!t["params"].as_array().unwrap().is_empty());
        }
        let en = ask(&cat, json!({"id": "tools", "lang": "en"}));
        assert_ne!(en["ok"]["tools"][0]["title"], list[0]["title"]);
    }

    /// Все прогоны модуля из tool_runs.json через JSON расширения:
    /// те же объекты, значения и сообщения, что у модуля QGIS.
    #[test]
    fn runs_through_json_like_the_module() {
        let cat = catalog();
        let runs: J =
            serde_json::from_slice(&std::fs::read(fixture("tool_runs.json")).unwrap()).unwrap();
        let mut n = 0;
        let mut want_n = 0;
        for r in runs["runs"].as_array().unwrap() {
            let name = r["tool"].as_str().unwrap();
            let tool = cat.tool(name).unwrap();
            // инструмент без расчёта на Rust - его прогоны позже
            if tools::run_fn(name).is_none() {
                continue;
            }
            want_n += 1;
            let mut values = r["values"].clone();
            // вход - по ключам полей, или один слой первого поля-слоя
            let input = &r["input"];
            let by_key = input
                .as_object()
                .is_some_and(|o| o.keys().all(|k| tool.param(k).is_some()));
            if by_key {
                for (k, v) in input.as_object().unwrap() {
                    values[k] = v.clone();
                }
            } else {
                let layer = tool
                    .params
                    .iter()
                    .find(|p| {
                        matches!(
                            p.kind,
                            tool_spec::ParamKind::Lines | tool_spec::ParamKind::Mesh
                        )
                    })
                    .unwrap();
                values[&layer.key] = input.clone();
            }
            let got = ask(
                &cat,
                json!({"id": "run", "lang": "ru", "params": {"tool": name, "values": values}}),
            );
            let ok = &got["ok"];
            assert!(ok.is_object(), "{name}: {got}");
            // выход один - он и есть OUTPUT, иначе по ключам
            let pairs: Vec<(&J, &J)> = if r["output"].get("features").is_some() {
                vec![(&ok["outputs"]["OUTPUT"], &r["output"])]
            } else {
                r["output"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(k, w)| (&ok["outputs"][k], w))
                    .collect()
            };
            for (got_out, want_out) in pairs {
                check_output(name, r, got_out, want_out);
            }
            let report = ok["report"].as_array().unwrap();
            assert_eq!(
                report.len(),
                r["report"].as_array().unwrap().len(),
                "{name}"
            );
            if name != "fill_to_volume" {
                assert_eq!(report, r["report"].as_array().unwrap(), "{name}");
            }
            n += 1;
        }
        assert_eq!(n, want_n);
        assert!(n >= 10);
    }

    /// Выход расширения против выхода модуля: число объектов, тела,
    /// строки слово в слово, числа - в допуске прогона.
    fn check_output(name: &str, r: &J, got: &J, want: &J) {
        let out = &got["features"];
        let want = want["features"].as_array().unwrap();
        assert_eq!(out.as_array().unwrap().len(), want.len(), "{name}");
        for (g, w) in out.as_array().unwrap().iter().zip(want) {
            assert_eq!(g["source"], w["source"], "{name}");
            if let Some(nv) = w["n_verts"].as_u64() {
                let geo = &g["geometry"];
                assert_eq!(geo["verts"].as_array().unwrap().len() as u64, nv);
                assert_eq!(
                    geo["faces"].as_array().unwrap().len() as u64,
                    w["n_faces"].as_u64().unwrap()
                );
            }
            // объект источника без своих полей - значений в образце нет
            if let Some(want_values) = w["values"].as_array() {
                assert_eq!(
                    g["values"].as_array().map(Vec::len),
                    Some(want_values.len()),
                    "{name}"
                );
                // строки слово в слово, числа - в допуске прогона
                let tol = r["tolerance"]["volume_rel"].as_f64().unwrap_or(1e-9);
                let lvl = r["tolerance"]["level"].as_f64();
                for (i, (a, b)) in g["values"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .zip(w["values"].as_array().unwrap())
                    .enumerate()
                {
                    match (a.as_f64(), b.as_f64()) {
                        (Some(x), Some(y)) => {
                            let d = if i == 1 && lvl.is_some() {
                                (x - y).abs() / lvl.unwrap_or(1.0)
                            } else {
                                (x - y).abs() / y.abs().max(1.0) / tol
                            };
                            assert!(d < 1.0, "{name}: {x} {y}");
                        }
                        _ => assert_eq!(a, b, "{name}"),
                    }
                }
            }
        }
    }

    #[test]
    fn bad_values_get_an_error_not_a_panic() {
        let cat = catalog();
        // номер вершины за краем тела
        let r = ask(
            &cat,
            json!({"id": "run", "lang": "ru", "params": {"tool": "fill_to_volume",
                "values": {"SHELL": {"verts": [[0, 0, 0]], "faces": [[0, 0, 5]]}}}}),
        );
        assert!(
            r["error"]
                .as_str()
                .unwrap()
                .starts_with("Значения не годятся")
        );
        // нет обязательного слоя
        let r = ask(
            &cat,
            json!({"id": "run", "lang": "en", "params": {"tool": "sweep_body", "values": {}}}),
        );
        assert!(r["error"].is_string());
        let r = ask(&cat, json!({"id": "run", "params": {"tool": "nope"}}));
        assert_eq!(r["error"], "No tool 'nope'.");
    }

    #[test]
    fn scene_is_read_before_the_window_starts() {
        let cat = catalog();
        let started = RefCell::new(Vec::<PathBuf>::new());
        let launch = |p: &Path| -> Result<(), String> {
            started.borrow_mut().push(p.to_path_buf());
            Ok(())
        };
        let ctx = Context {
            catalog: &cat,
            launch: &launch,
        };
        let demo = fixture("scene_demo.i3dscene");
        let req = json!({"id": "scene", "lang": "ru", "params": {"path": demo}});
        let r = answer(&req.to_string(), &ctx);
        assert_eq!(r["ok"]["parts"], 5, "{r}");
        assert_eq!(
            r["ok"]["text"],
            "Сцена «Демо: выработки» открывается: частей 5."
        );
        assert_eq!(started.borrow().as_slice(), std::slice::from_ref(&demo));

        // пакета нет - ошибка, окно не запускается
        let req = json!({"id": "scene", "params": {"path": fixture("none.i3dscene")}});
        assert!(answer(&req.to_string(), &ctx)["error"].is_string());
        let req = json!({"id": "scene", "params": "{}"});
        assert!(answer(&req.to_string(), &ctx)["error"].is_string());
        assert_eq!(started.borrow().len(), 1);

        // окно не запустилось - ошибка в RustGIS
        let fail = |_: &Path| -> Result<(), String> { Err("x".into()) };
        let ctx = Context {
            catalog: &cat,
            launch: &fail,
        };
        let req = json!({"id": "scene", "lang": "en", "params": {"path": demo}});
        assert_eq!(
            answer(&req.to_string(), &ctx)["error"],
            "The scene window did not start: x"
        );
    }
}
