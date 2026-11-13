// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Окно «Параметры сцены» на одну кнопку плашки, как окно свойств
//! модуля: камера, вид, координатный короб, обрезка. Кнопок параметров
//! вида на плашке и в левой колонке нет, они все здесь. Окно низкое:
//! группы идут вкладками, а не столбиком.

use super::{SceneApp, Side, tr};
use eframe::egui;

/// Настройки координатного короба, как `_box_opts` окна модуля. При
/// открытии пакета берутся из состояния окна QGIS (`scene.json state`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct BoxOpts {
    /// Подписи на рёбрах, ближних к камере (`box_near`), иначе у начала.
    pub near: bool,
    /// Шаг делений в плане и по высоте (`tick_xy`, `tick_z`), ноль -
    /// круглый шаг от размаха.
    pub step_xy: f64,
    pub step_z: f64,
    /// Множитель подписи (`mult_xy`, `mult_z`): 0.001 - километры,
    /// -1 - глубина положительными числами.
    pub mult_xy: f64,
    pub mult_z: f64,
    /// Свои границы короба (`box_range_on`, `box_range`): xmin, xmax,
    /// ymin, ymax, zmin, zmax.
    pub range_on: bool,
    pub range: [f64; 6],
}

impl Default for BoxOpts {
    fn default() -> Self {
        Self {
            near: true,
            step_xy: 0.0,
            step_z: 0.0,
            mult_xy: 1.0,
            mult_z: 1.0,
            range_on: false,
            range: [0.0; 6],
        }
    }
}

impl BoxOpts {
    /// Настройки из состояния окна модуля, недостающие - по умолчанию.
    pub fn from_state(st: &serde_json::Value) -> Self {
        let d = Self::default();
        let num = |k: &str, v: f64| st.get(k).and_then(serde_json::Value::as_f64).unwrap_or(v);
        let flag = |k: &str, v: bool| st.get(k).and_then(serde_json::Value::as_bool).unwrap_or(v);
        let mut range = d.range;
        if let Some(a) = st.get("box_range").and_then(serde_json::Value::as_array) {
            for (r, v) in range.iter_mut().zip(a) {
                *r = v.as_f64().unwrap_or(0.0);
            }
        }
        Self {
            near: flag("box_near", d.near),
            step_xy: num("tick_xy", d.step_xy),
            step_z: num("tick_z", d.step_z),
            mult_xy: num("mult_xy", d.mult_xy),
            mult_z: num("mult_z", d.mult_z),
            range_on: flag("box_range_on", d.range_on),
            range,
        }
    }
}

/// Вкладка окна параметров.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Tab {
    #[default]
    Camera,
    Look,
    Box,
    Clip,
}

impl SceneApp {
    /// Окно «Параметры сцены». Открыто, пока нажата шестерёнка.
    pub(super) fn params_window(&mut self, ctx: &egui::Context) {
        if !self.params_open || self.pkg.is_none() {
            return;
        }
        let lang = self.lang;
        let t = |s: &'static str| tr(lang, s);
        let mut open = true;
        egui::Window::new(t("Параметры сцены"))
            .id(egui::Id::unique("params"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                ui.horizontal(|ui| {
                    for (tab, name) in [
                        (Tab::Camera, t("Камера")),
                        (Tab::Look, t("Вид")),
                        (Tab::Box, t("Короб")),
                        (Tab::Clip, t("Обрезка")),
                    ] {
                        ui.selectable_value(&mut self.params_tab, tab, name);
                    }
                });
                ui.separator();
                match self.params_tab {
                    Tab::Camera => self.camera_tab(ui),
                    Tab::Look => self.look_tab(ui),
                    Tab::Box => self.box_tab(ui),
                    Tab::Clip => self.clip_tab(ui),
                }
            });
        self.params_open = open;
    }

    fn camera_tab(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let t = |s: &'static str| tr(lang, s);
        let mut c = self.cam;
        egui::Grid::new("p_cam").num_columns(2).show(ui, |ui| {
            ui.label(t("Азимут, °"));
            // азимут 0-360, как в строке состояния
            let mut az = c.azimuth.rem_euclid(360.0);
            if ui
                .add(egui::DragValue::new(&mut az).speed(1.0).max_decimals(1))
                .changed()
            {
                c.azimuth = az;
            }
            ui.end_row();
            ui.label(t("Наклон, °"));
            ui.add(
                egui::DragValue::new(&mut c.elevation)
                    .range(-90.0..=90.0)
                    .speed(1.0)
                    .max_decimals(1),
            );
            ui.end_row();
            ui.label(t("Удаление, м"));
            let speed = c.distance.abs() * 0.01 + 0.1;
            ui.add(
                egui::DragValue::new(&mut c.distance)
                    .range(0.01..=1e9)
                    .speed(speed)
                    .max_decimals(1),
            );
            ui.end_row();
            ui.label(t("Угол обзора, °"));
            ui.add(
                egui::DragValue::new(&mut c.fov)
                    .range(1.0..=120.0)
                    .speed(0.5)
                    .max_decimals(1),
            );
            ui.end_row();
            ui.label(t("Центр X, м"));
            ui.add(
                egui::DragValue::new(&mut c.center[0])
                    .speed(1.0)
                    .max_decimals(2),
            );
            ui.end_row();
            ui.label(t("Центр Y, м"));
            ui.add(
                egui::DragValue::new(&mut c.center[1])
                    .speed(1.0)
                    .max_decimals(2),
            );
            ui.end_row();
            ui.label(t("Центр Z, м"));
            ui.add(
                egui::DragValue::new(&mut c.center[2])
                    .speed(1.0)
                    .max_decimals(2),
            );
            ui.end_row();
        });
        ui.checkbox(&mut c.ortho, t("Параллельная проекция"));
        if c != self.cam {
            self.cam = c;
            self.anim = None;
            self.dirty = true;
        }
    }

    fn look_tab(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let t = |s: &'static str| tr(lang, s);
        egui::Grid::new("p_look").num_columns(2).show(ui, |ui| {
            ui.label(t("Вертикальный масштаб, Z ×")).on_hover_text(t(
                "Вертикальное преувеличение. Единица - натуральный масштаб.",
            ));
            if ui
                .add(
                    egui::DragValue::new(&mut self.vex)
                        .range(0.01..=10000.0)
                        .speed(0.05)
                        .max_decimals(2),
                )
                .changed()
            {
                self.dirty = true;
            }
            ui.end_row();
            ui.label(t("Непрозрачность"));
            if ui
                .add(egui::Slider::new(&mut self.opacity, 0.05..=1.0))
                .changed()
            {
                self.dirty = true;
            }
            ui.end_row();
            ui.label(t("Тема"));
            ui.horizontal(|ui| {
                let was = self.dark;
                ui.selectable_value(&mut self.dark, false, t("Светлая"));
                ui.selectable_value(&mut self.dark, true, t("Тёмная"));
                if was != self.dark {
                    ui.ctx().set_theme(if self.dark {
                        egui::Theme::Dark
                    } else {
                        egui::Theme::Light
                    });
                    self.dirty = true;
                }
            });
            ui.end_row();
        });
        if ui
            .checkbox(&mut self.gradient, t("Градиентный фон"))
            .changed()
        {
            self.dirty = true;
        }
    }

    fn box_tab(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let t = |s: &'static str| tr(lang, s);
        let mut shown = self.box_shown_now();
        if ui
            .checkbox(
                &mut shown,
                t("Координатный короб: деления и подписи по осям"),
            )
            .changed()
        {
            self.show_box(shown);
        }
        let b = &mut self.box_opts;
        ui.checkbox(&mut b.near, t("Подписи на ближних к камере рёбрах"));
        egui::Grid::new("p_box").num_columns(3).show(ui, |ui| {
            ui.label("");
            ui.label(t("в плане"));
            ui.label(t("по высоте"));
            ui.end_row();
            ui.label(t("Шаг делений, м (0 - сам)"));
            ui.add(
                egui::DragValue::new(&mut b.step_xy)
                    .range(0.0..=1e9)
                    .speed(1.0),
            );
            ui.add(
                egui::DragValue::new(&mut b.step_z)
                    .range(0.0..=1e9)
                    .speed(1.0),
            );
            ui.end_row();
            ui.label(t("Множитель подписи"))
                .on_hover_text(t("0.001 - километры, -1 - глубина положительными числами."));
            ui.add(
                egui::DragValue::new(&mut b.mult_xy)
                    .speed(0.01)
                    .max_decimals(4),
            );
            ui.add(
                egui::DragValue::new(&mut b.mult_z)
                    .speed(0.01)
                    .max_decimals(4),
            );
            ui.end_row();
        });
        ui.checkbox(&mut b.range_on, t("Свои границы короба"));
        ui.add_enabled_ui(b.range_on, |ui| {
            egui::Grid::new("p_range").num_columns(3).show(ui, |ui| {
                for (k, axis) in ["X", "Y", "Z"].into_iter().enumerate() {
                    ui.label(axis);
                    ui.add(egui::DragValue::new(&mut b.range[2 * k]).speed(1.0));
                    ui.add(egui::DragValue::new(&mut b.range[2 * k + 1]).speed(1.0));
                    ui.end_row();
                }
            });
        });
    }

    fn clip_tab(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let t = |s: &'static str| tr(lang, s);
        ui.weak(t(
            "Контур и линия берутся из измерения: щелчками поставьте точки, двойной щелчок замыкает контур.",
        ));
        let c = &mut self.clip;
        let name = |s: Option<Side>| match s {
            None => t("(нет)"),
            Some(Side::In) => t("Оставить внутри"),
            Some(Side::Out) => t("Убрать внутри"),
            Some(Side::Left) => t("Слева от линии"),
            Some(Side::Right) => t("Справа от линии"),
            Some(Side::Corridor) => t("Коридор вдоль линии"),
        };
        egui::Grid::new("clip").num_columns(2).show(ui, |ui| {
            ui.label(t("Что оставить"));
            egui::ComboBox::from_id_salt("clip_side")
                .selected_text(name(c.side))
                .show_ui(ui, |ui| {
                    for s in [
                        None,
                        Some(Side::In),
                        Some(Side::Out),
                        Some(Side::Left),
                        Some(Side::Right),
                        Some(Side::Corridor),
                    ] {
                        ui.selectable_value(&mut c.side, s, name(s));
                    }
                });
            ui.end_row();
            ui.label(t("Полуширина коридора, м"));
            ui.add_enabled(
                c.side == Some(Side::Corridor),
                egui::DragValue::new(&mut c.width)
                    .range(0.0..=1e9)
                    .speed(1.0),
            );
            ui.end_row();
            ui.checkbox(&mut c.use_lo, t("Отметка не ниже, м"));
            ui.add_enabled(c.use_lo, egui::DragValue::new(&mut c.zlo).speed(1.0));
            ui.end_row();
            ui.checkbox(&mut c.use_hi, t("Отметка не выше, м"));
            ui.add_enabled(c.use_hi, egui::DragValue::new(&mut c.zhi).speed(1.0));
            ui.end_row();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_opts_from_module_state() {
        let st = serde_json::json!({
            "box_near": false, "tick_xy": 50.0, "tick_z": 10.0,
            "mult_xy": 0.001, "mult_z": -1.0, "box_range_on": true,
            "box_range": [0.0, 100.0, 0.0, 50.0, -30.0, 0.0]
        });
        let b = BoxOpts::from_state(&st);
        assert!(!b.near && b.range_on);
        assert_eq!(
            (b.step_xy, b.step_z, b.mult_xy, b.mult_z),
            (50.0, 10.0, 0.001, -1.0)
        );
        assert_eq!(b.range, [0.0, 100.0, 0.0, 50.0, -30.0, 0.0]);
        assert_eq!(
            BoxOpts::from_state(&serde_json::Value::Null),
            BoxOpts::default()
        );
    }
}
