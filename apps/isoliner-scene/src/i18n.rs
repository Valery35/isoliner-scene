// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Тексты окна на двух языках. Ключ - русский текст.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ru,
    En,
}

impl Lang {
    /// Язык системы: русский для ru, иначе английский.
    pub fn system() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let locale = sys_locale::get_locale();
        // в браузере - язык браузера
        #[cfg(target_arch = "wasm32")]
        let locale = web_sys::window().and_then(|w| w.navigator().language());
        match locale {
            Some(l) if l.to_lowercase().starts_with("ru") => Lang::Ru,
            Some(_) => Lang::En,
            None => Lang::Ru,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::Ru => "ru",
            Lang::En => "en",
        }
    }

    pub fn from_code(s: &str) -> Self {
        if s == "en" { Lang::En } else { Lang::Ru }
    }
}

const TABLE: &[(&str, &str)] = &[
    ("Открыть", "Open"),
    ("Сохранить как", "Save as"),
    ("Пакет сцены Isoliner3D", "Isoliner3D scene package"),
    ("Вписать", "Fit"),
    ("Камера пакета", "Package camera"),
    ("Параллельная проекция", "Parallel projection"),
    ("Преувеличение", "Exaggeration"),
    ("Непрозрачность", "Opacity"),
    ("Части", "Parts"),
    ("Виды", "Views"),
    ("Добавить вид", "Add view"),
    ("Убрать вид", "Remove view"),
    ("Облёт по видам", "Fly through views"),
    ("Оборот", "Orbit"),
    ("Стоп", "Stop"),
    ("Вид %d", "View %d"),
    ("Сцена", "Scene"),
    ("Система координат", "Coordinate system"),
    ("без системы", "none"),
    ("Точка начала", "Origin"),
    ("Записал", "Written by"),
    ("тело", "body"),
    ("линии", "lines"),
    ("оформление", "decor"),
    ("объём %s м3", "volume %s m3"),
    ("не замкнуто", "not closed"),
    ("точки: %s", "points: %s"),
    ("значки", "marks"),
    (
        "Перетащите пакет .i3dscene в окно или нажмите «Открыть».",
        "Drop an .i3dscene package into the window or press Open.",
    ),
    ("Не открылось: %s", "Could not open: %s"),
    ("Не записалось: %s", "Could not save: %s"),
    ("Записано: %s", "Saved: %s"),
    (
        "Левая кнопка - вращение, Ctrl + левая или средняя - сдвиг, колесо - масштаб, Ctrl + колесо - угол обзора, щелчок правой - центр вращения, щелчок левой - опрос точки.",
        "Left button rotates, Ctrl + left or middle pans, the wheel zooms, Ctrl + wheel changes the field of view, a right click sets the rotation center, a left click probes a point.",
    ),
    ("Градиентный фон", "Gradient background"),
    ("Вид сверху", "Top view"),
    ("Сбросить вид", "Reset view"),
    ("Вращение", "Spin"),
    (
        "%s: X %s, Y %s, отметка %s м",
        "%s: X %s, Y %s, elevation %s m",
    ),
    (
        "Центр вращения: %s, %s, отметка %s м.",
        "Rotation center: %s, %s, elevation %s m.",
    ),
    (
        "Центр вращения - вся сцена.",
        "Rotation center: the whole scene.",
    ),
    (
        "Мимо объекта: центр не изменён. Щелчок за пределами площадки вернёт центр на всю сцену.",
        "Missed the objects: the center is unchanged. A click outside the site returns the center to the whole scene.",
    ),
    (
        "Азимут %.0f°, наклон %.0f°, удаление %s м",
        "Azimuth %.0f°, elevation %.0f°, distance %s m",
    ),
    (
        "Видеокарта недоступна: окно не может рисовать сцену.",
        "No graphics adapter: the window cannot draw the scene.",
    ),
];

/// Значок кнопки «Убрать вид» - знак умножения U+00D7. Знаков U+2715,
/// U+2716 и U+1F5D9 в шрифтах egui сборки нет, на их месте рисуется
/// пустой квадрат.
pub const ICON_REMOVE: &str = "×";

/// Перевод строки окна.
pub fn tr(lang: Lang, ru: &'static str) -> &'static str {
    match lang {
        Lang::Ru => ru,
        Lang::En => TABLE.iter().find(|(r, _)| *r == ru).map_or(ru, |(_, e)| e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Значки окна без перевода. Новый значок вписывается сюда.
    const ICONS: &[&str] = &[ICON_REMOVE];

    #[test]
    fn every_key_is_unique_and_translated() {
        for (i, (ru, en)) in TABLE.iter().enumerate() {
            assert!(!en.is_empty());
            assert!(TABLE.iter().skip(i + 1).all(|(r, _)| r != ru), "{ru}");
        }
        assert_eq!(tr(Lang::En, "Открыть"), "Open");
        assert_eq!(tr(Lang::En, "нет в таблице"), "нет в таблице");
    }

    /// Каждый знак текстов и значков окна есть в шрифтах egui.
    /// Знак без глифа рисуется пустым квадратом.
    #[test]
    fn every_char_has_a_glyph() {
        use eframe::egui::epaint::text::{Fonts, TextOptions};
        use eframe::egui::{FontDefinitions, FontId};
        let mut fonts = Fonts::new(TextOptions::default(), FontDefinitions::default());
        let font = FontId::proportional(14.0);
        let texts = TABLE
            .iter()
            .flat_map(|(ru, en)| [*ru, *en])
            .chain(ICONS.iter().copied());
        for text in texts {
            for c in text.chars() {
                assert!(
                    fonts.has_glyph(&font, c),
                    "нет глифа U+{:04X} в «{text}»",
                    u32::from(c)
                );
            }
        }
    }
}
