// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Руководство Isoliner3D для RustGIS: своя глава «Isoliner3D в RustGIS»
//! (`doc/manual/rustgis.<язык>.md`) и общие с модулем QGIS страницы
//! инструментов (`fixtures/docs/tools/<код>.<язык>.md`, их собирает
//! из tools.json генератор модуля `tools/tool_pages.py`). Страницы
//! берутся для инструментов из раздела `tools` манифеста, в его порядке.
//!
//! Собранное руководство - `doc/manual/isoliner3d-rustgis.<язык>.md`.
//! Тест сверяет его со сборкой, `ISOLINER_MANUAL=1 cargo test -p
//! isoliner-rustgis` пересобирает.

#![cfg(test)]

use serde_json::Value as J;

fn root() -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{}/../..", env!("CARGO_MANIFEST_DIR")))
}

/// Заголовок главы инструментов и строка о главе сцены.
fn heads(lang: &str) -> (&'static str, &'static str) {
    if lang == "ru" {
        (
            "# Инструменты",
            "Окно сцены описано в главе «Окно просмотра» руководства Isoliner3D. Оно общее с модулем QGIS и программой Isoliner Scene.",
        )
    } else {
        (
            "# Tools",
            "The scene window is described in the View Window chapter of the Isoliner3D manual. It is common to the QGIS plugin and the Isoliner Scene program.",
        )
    }
}

/// Руководство на языке `lang`: глава RustGIS, затем страницы инструментов.
fn build(lang: &str) -> String {
    let root = root();
    let read = |p: std::path::PathBuf| {
        std::fs::read_to_string(&p)
            .map(|t| t.replace("\r\n", "\n"))
            .unwrap_or_else(|e| format!("<!-- {}: {e} -->", p.display()))
    };
    let chapter = read(root.join(format!("doc/manual/rustgis.{lang}.md")));
    // служебные комментарии главы - не для читателя
    let chapter: String = chapter
        .lines()
        .filter(|l| !l.trim_start().starts_with("<!--"))
        .collect::<Vec<_>>()
        .join("\n");
    let manifest: J = serde_json::from_str(&read(root.join("apps/isoliner-rustgis/plugin.json")))
        .unwrap_or(J::Null);
    let catalog: J =
        serde_json::from_str(&read(root.join("fixtures/tools.json"))).unwrap_or(J::Null);
    let (tools_head, scene_line) = heads(lang);
    let mut out = String::new();
    out.push_str(
        "<!-- Собрано тестом manual.rs расширения isoliner-rustgis. Правки - в doc/manual/rustgis.*.md и в tools.json модуля. -->\n\n",
    );
    out.push_str(chapter.trim());
    out.push_str("\n\n");
    out.push_str(scene_line);
    out.push_str("\n\n");
    out.push_str(tools_head);
    out.push_str("\n\n");
    for t in manifest["tools"].as_array().into_iter().flatten() {
        let name = t["id"].as_str().unwrap_or("").replace('-', "_");
        let code = catalog["tools"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|c| c["name"] == name.as_str())
            .and_then(|c| c["code"].as_str())
            .unwrap_or("?");
        let page = read(root.join(format!("fixtures/docs/tools/{code}.{lang}.md")));
        out.push_str(page.trim());
        out.push_str("\n\n");
    }
    format!("{}\n", out.trim_end())
}

#[test]
fn manual_is_built_from_the_chapter_and_the_tool_pages() {
    for lang in ["ru", "en"] {
        let got = build(lang);
        let path = root().join(format!("doc/manual/isoliner3d-rustgis.{lang}.md"));
        if std::env::var_os("ISOLINER_MANUAL").is_some() {
            std::fs::write(&path, &got).unwrap();
        }
        // глава и страница каждого инструмента RustGIS на месте
        assert!(
            !got.lines()
                .any(|l| l.starts_with("<!--") && l.contains("os error")),
            "{lang}: нет файла главы или страницы"
        );
        assert!(
            got.contains("## 2.24 ") && got.contains("## 2.17 "),
            "{lang}"
        );
        // правила текстов линейки: без точки с запятой и длинного тире
        let prose: String = got
            .lines()
            .filter(|l| {
                !l.trim_start().starts_with("--cmd") && !l.trim_start().starts_with("rustgis-cli")
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!prose.contains('—'), "{lang}: длинное тире");
        // git на Windows может отдать файл с \r\n (core.autocrlf)
        let saved = std::fs::read_to_string(&path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        assert_eq!(
            saved, got,
            "{lang}: руководство устарело, ISOLINER_MANUAL=1"
        );
    }
}
