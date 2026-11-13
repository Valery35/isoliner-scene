// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Сведения «О программе» - общий список линейки Isoliner3D.
//!
//! Список ведёт модуль QGIS и кладёт его в `fixtures/about.json`
//! (тот же, что у его окна «О программе»): заголовок, абзацы текста,
//! ссылки с подписями на двух языках и пометкой, где запись видна
//! (`both`, `pro` - только модуль, `lite` - только эта программа).
//! Здесь он вшит при сборке и читается окном программы и расширением
//! для RustGIS.

use serde::Deserialize;

const ABOUT_JSON: &str = include_str!("../../../fixtures/about.json");

/// Строка на двух языках с пометкой, где она видна.
#[derive(Debug, Clone, Deserialize)]
pub struct Entry {
    pub ru: String,
    pub en: String,
    #[serde(rename = "where", default)]
    pub place: String,
}

/// Ссылка с подписью.
#[derive(Debug, Clone, Deserialize)]
pub struct Link {
    pub url: String,
    pub ru: String,
    pub en: String,
    #[serde(rename = "where", default)]
    pub place: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Window {
    ru: String,
    en: String,
}

/// Список целиком, как он лежит в `about.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct About {
    pub version: u32,
    window: Window,
    pub title: Vec<Entry>,
    pub text: Vec<Entry>,
    pub links: Vec<Link>,
}

/// Запись видна в этой программе: общая или только её.
fn ours(place: &str) -> bool {
    place == "both" || place == "lite"
}

fn pick<'a>(en: bool, ru: &'a str, eng: &'a str) -> &'a str {
    if en { eng } else { ru }
}

/// Сведения для этой программы на одном языке.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    /// Заголовок окна: «О программе».
    pub window: String,
    /// Название программы.
    pub title: String,
    /// Абзацы текста по порядку.
    pub text: Vec<String>,
    /// Подпись и адрес ссылок по порядку.
    pub links: Vec<(String, String)>,
}

impl About {
    /// Разобрать вшитый список. Не разобрался или версия другая - None:
    /// окно тогда пишет только название и версию.
    pub fn load() -> Option<About> {
        serde_json::from_str::<About>(ABOUT_JSON)
            .ok()
            .filter(|a| a.version == 1)
    }

    /// Записи этой программы (`both` и `lite`) на английском (`en`)
    /// или русском.
    pub fn shown(&self, en: bool) -> Shown {
        Shown {
            window: pick(en, &self.window.ru, &self.window.en).to_string(),
            title: self
                .title
                .iter()
                .find(|t| ours(&t.place))
                .map_or_else(String::new, |t| pick(en, &t.ru, &t.en).to_string()),
            text: self
                .text
                .iter()
                .filter(|t| ours(&t.place))
                .map(|t| pick(en, &t.ru, &t.en).to_string())
                .collect(),
            links: self
                .links
                .iter()
                .filter(|l| ours(&l.place))
                .map(|l| (pick(en, &l.ru, &l.en).to_string(), l.url.clone()))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Список модуля разбирается, программе достаются её записи:
    /// название Isoliner Scene, лицензия Apache, своя страница и issues,
    /// без записей модуля. Все адреса - https.
    #[test]
    fn about_list_for_the_program() {
        let a = About::load().unwrap();
        let ru = a.shown(false);
        assert_eq!(ru.window, "О программе");
        assert_eq!(ru.title, "Isoliner Scene");
        assert!(ru.text.iter().any(|t| t.contains("Apache-2.0")));
        assert!(!ru.text.iter().any(|t| t.contains("GPL")));
        let urls: Vec<&str> = ru.links.iter().map(|l| l.1.as_str()).collect();
        assert!(urls.contains(&"https://github.com/Valery35/isoliner-scene/issues"));
        assert!(!urls.contains(&"https://github.com/Valery35/qgis-isoliner3d/issues"));
        assert!(urls.iter().all(|u| u.starts_with("https://")));
        let lite = a.links.iter().filter(|l| ours(&l.place)).count();
        assert_eq!(ru.links.len(), lite);
        let en = a.shown(true);
        assert_eq!(en.window, "About");
        assert_eq!(en.links.len(), ru.links.len());
    }
}
