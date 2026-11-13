// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Тексты ответов RustGIS на двух языках. Ключ - русский текст.
//! Тексты инструментов - из tools.json.

use tool_spec::{Arg, Lang};

/// Язык запроса RustGIS: `ru...` - русский, иначе английский.
pub fn lang_of(code: Option<&str>) -> Lang {
    match code {
        Some(c) if c.to_lowercase().starts_with("ru") => Lang::Ru,
        _ => Lang::En,
    }
}

pub const TABLE: &[(&str, &str)] = &[
    (
        "Isoliner3D: команды «%s» нет.",
        "Isoliner3D: no command '%s'.",
    ),
    (
        "Запрос RustGIS не разобран: %s",
        "The RustGIS request was not read: %s",
    ),
    (
        "Не указан пакет сцены: поле path.",
        "No scene package given: the path field.",
    ),
    (
        "Пакет сцены не открылся: %s",
        "The scene package did not open: %s",
    ),
    (
        "Окно сцены не запустилось: %s",
        "The scene window did not start: %s",
    ),
    (
        "Сцена «%s» открывается: частей %d.",
        "Scene '%s' opens: %d parts.",
    ),
    ("Инструмента «%s» нет.", "No tool '%s'."),
    ("Значения не годятся: %s", "The values do not fit: %s"),
    ("%s: значение не того вида", "%s: wrong kind of value"),
    ("Расчёт не удался: %s", "The run failed: %s"),
    (
        "У таблицы устьев без точек нужен код системы координат X и Y, например EPSG:28410.",
        "A collar table without points needs the EPSG code of its X and Y, for example EPSG:28410.",
    ),
    (
        "%s: объёмный итог - в окне сцены.",
        "%s: the result in 3D is in the scene window.",
    ),
    (
        "Слой «%s» - не тела Isoliner3D. Тела с высотой дают инструменты 2.24 и 2.26, слой RustGIS их не хранит.",
        "Layer '%s' holds no Isoliner3D bodies. Bodies with heights come from tools 2.24 and 2.26, a RustGIS layer does not keep them.",
    ),
    (
        "%s: тела в плане, верхними гранями, метры зоны UTM %s.",
        "%s: bodies in plan by their top faces, metres of UTM zone %s.",
    ),
];

/// Сообщение расчёта на языке RustGIS: шаблон и строковые аргументы
/// переводятся по `messages` инструмента, как отчёт модуля в QGIS
/// (`Report` в `toolspec.py`: «точка» в 2.17 - «point»).
pub fn report_text(tool: &tool_spec::Tool, m: &tool_spec::Message, lang: Lang) -> String {
    let args: Vec<Arg> = m
        .args
        .iter()
        .map(|a| match a {
            Arg::Text(s) => Arg::Text(tool.translate(s, lang).to_string()),
            other => other.clone(),
        })
        .collect();
    tool_spec::format(tool.translate(m.template, lang), &args)
}

/// Перевод текста ответа с подстановкой.
pub fn tr(lang: Lang, ru: &str, args: &[Arg]) -> String {
    let template = match lang {
        Lang::Ru => ru,
        Lang::En => TABLE.iter().find(|(r, _)| *r == ru).map_or(ru, |(_, e)| *e),
    };
    tool_spec::format(template, args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_is_unique_and_translated() {
        for (i, (ru, en)) in TABLE.iter().enumerate() {
            assert!(!en.is_empty());
            assert_eq!(ru.matches('%').count(), en.matches('%').count(), "{ru}");
            assert!(TABLE.iter().skip(i + 1).all(|(r, _)| r != ru), "{ru}");
        }
        assert_eq!(
            tr(Lang::En, "Инструмента «%s» нет.", &[Arg::Text("x".into())]),
            "No tool 'x'."
        );
        assert_eq!(lang_of(Some("ru_RU")), Lang::Ru);
        assert_eq!(lang_of(None), Lang::En);
    }
}
