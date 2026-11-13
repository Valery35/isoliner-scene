// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Окно «О программе» по кнопке (i) плашки, как окно модуля:
//! название жирным с версией, абзацы текста, ссылки столбиком. Текст
//! и ссылки - из общего списка линейки (`scene_format::about`), версия -
//! из сборки. Ссылка открывается в браузере, на странице - новой
//! вкладкой.

use super::SceneApp;
use crate::i18n::Lang;
use eframe::egui;
use scene_format::about::About;

/// Версия программы из Cargo.
pub(super) const VERSION: &str = env!("CARGO_PKG_VERSION");

impl SceneApp {
    pub(super) fn about_window(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        let en = self.lang == Lang::En;
        let shown = About::load().map(|a| a.shown(en));
        let (window, title) = shown.as_ref().map_or_else(
            || (if en { "About" } else { "О программе" }, "Isoliner Scene"),
            |s| (s.window.as_str(), s.title.as_str()),
        );
        let mut open = true;
        egui::Window::new(window)
            .id(egui::Id::unique("about"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!("{title} {VERSION}"))
                        .strong()
                        .size(16.0),
                );
                let Some(s) = &shown else { return };
                for p in &s.text {
                    ui.add_space(4.0);
                    ui.label(p);
                }
                ui.add_space(6.0);
                for (label, url) in &s.links {
                    ui.add(egui::Hyperlink::from_label_and_url(label, url).open_in_new_tab(true));
                }
            });
        self.about_open = open;
    }
}
