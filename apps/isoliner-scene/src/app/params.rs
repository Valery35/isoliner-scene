// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Окно «Параметры сцены» на одну кнопку плашки, общее с окном модуля.
//!
//! Состав, порядок вкладок, подписи и ключи `scene.json state` задаёт
//! модуль в `doc/SCENE_PARAMS.md`, копия лежит в
//! `fixtures/SCENE_PARAMS.md`. Тест ниже сверяет окно с копией: ключ
//! без привязки или подпись без перевода из списка роняют его.
//! Параметры с пометкой «Про» здесь не показываются.

use super::{SceneApp, Side, tr};
use eframe::egui;

/// Вкладки окна в порядке списка модуля: русское и английское имя.
pub(super) const TABS: [(&str, &str); 4] = [
    ("Вид", "View"),
    ("Оформление", "Look"),
    ("Короб", "Box"),
    ("Обрезка", "Clip"),
];

/// Ключи `state` из списка модуля, у которых в окне есть поле.
/// Камера идёт ключами `camera.*`. Нужны только тесту сверки.
#[cfg(test)]
pub(super) const KEYS: &[&str] = &[
    "vex",
    "opacity",
    "camera.azimuth",
    "camera.elevation",
    "camera.distance",
    "camera.fov",
    "camera.ortho",
    "light",
    "bg_grad",
    "smooth_edges",
    "sketch_on",
    "axes_on",
    "box_edges",
    "box_near",
    "grid_planes",
    "grid_step",
    "tick_xy",
    "tick_z",
    "mult_xy",
    "mult_z",
    "box_range_on",
    "box_range",
    "side",
    "width",
    "zlo",
    "zhi",
];

/// Отметка «нет» в `zlo`, `zhi` модуля.
pub(super) const NO_LEVEL: f64 = -1e7;

/// Рёбра короба (`box_edges`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Edges {
    #[default]
    All,
    Axes,
    None,
}

impl Edges {
    pub const ALL: [Edges; 3] = [Edges::All, Edges::Axes, Edges::None];

    pub fn key(self) -> &'static str {
        match self {
            Edges::All => "all",
            Edges::Axes => "axes",
            Edges::None => "none",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Edges::All => "все",
            Edges::Axes => "три оси",
            Edges::None => "без рёбер",
        }
    }

    fn from_key(k: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|e| e.key() == k)
            .unwrap_or_default()
    }
}

/// Сетка на плоскостях короба (`grid_planes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Grid {
    #[default]
    BoxOnly,
    Floor,
    FloorWalls,
    Walls,
}

impl Grid {
    pub const ALL: [Grid; 4] = [Grid::BoxOnly, Grid::Floor, Grid::FloorWalls, Grid::Walls];

    pub fn key(self) -> &'static str {
        match self {
            Grid::BoxOnly => "",
            Grid::Floor => "floor",
            Grid::FloorWalls => "floor,walls",
            Grid::Walls => "walls",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Grid::BoxOnly => "только короб",
            Grid::Floor => "пол",
            Grid::FloorWalls => "пол и стены",
            Grid::Walls => "стены",
        }
    }

    fn from_key(k: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|g| g.key() == k)
            .unwrap_or_default()
    }
}

/// Координатный короб, как `_box_opts` окна модуля.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct BoxOpts {
    /// Короб показан (`axes_on`).
    pub on: bool,
    pub edges: Edges,
    /// Подписи на рёбрах, ближних к камере (`box_near`), иначе у начала.
    pub near: bool,
    pub grid: Grid,
    /// Шаг сетки, ноль - круглый шаг от размаха (`grid_step`).
    pub grid_step: f64,
    /// Шаг делений в плане и по высоте (`tick_xy`, `tick_z`), ноль -
    /// круглый шаг от размаха.
    pub step_xy: f64,
    pub step_z: f64,
    /// Множитель подписей (`mult_xy`, `mult_z`): 0.001 - километры,
    /// -1 - глубина положительными числами.
    pub mult_xy: f64,
    pub mult_z: f64,
    /// Свой охват (`box_range_on`, `box_range`): x0, x1, y0, y1, z0, z1.
    pub range_on: bool,
    pub range: [f64; 6],
}

impl Default for BoxOpts {
    fn default() -> Self {
        Self {
            on: true,
            edges: Edges::All,
            near: true,
            grid: Grid::BoxOnly,
            grid_step: 0.0,
            step_xy: 0.0,
            step_z: 0.0,
            mult_xy: 1.0,
            mult_z: 1.0,
            range_on: false,
            range: [0.0; 6],
        }
    }
}

fn num(st: &serde_json::Value, k: &str, d: f64) -> f64 {
    st.get(k).and_then(serde_json::Value::as_f64).unwrap_or(d)
}

fn flag(st: &serde_json::Value, k: &str, d: bool) -> bool {
    st.get(k).and_then(serde_json::Value::as_bool).unwrap_or(d)
}

fn text<'a>(st: &'a serde_json::Value, k: &str) -> Option<&'a str> {
    st.get(k).and_then(serde_json::Value::as_str)
}

impl BoxOpts {
    /// Настройки из состояния окна модуля, недостающие - по умолчанию.
    pub fn from_state(st: &serde_json::Value) -> Self {
        let d = Self::default();
        let mut range = d.range;
        if let Some(a) = st.get("box_range").and_then(serde_json::Value::as_array) {
            for (r, v) in range.iter_mut().zip(a) {
                *r = v.as_f64().unwrap_or(0.0);
            }
        }
        Self {
            on: flag(st, "axes_on", d.on),
            edges: text(st, "box_edges").map_or(d.edges, Edges::from_key),
            near: flag(st, "box_near", d.near),
            grid: text(st, "grid_planes").map_or(d.grid, Grid::from_key),
            grid_step: num(st, "grid_step", d.grid_step),
            step_xy: num(st, "tick_xy", d.step_xy),
            step_z: num(st, "tick_z", d.step_z),
            mult_xy: num(st, "mult_xy", d.mult_xy),
            mult_z: num(st, "mult_z", d.mult_z),
            range_on: flag(st, "box_range_on", d.range_on),
            range,
        }
    }
}

/// Оформление, которого нет у других групп: отмывка и сглаживание.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct LookOpts {
    /// Отмывка, проценты (`light`), у модуля по умолчанию 55.
    pub light: f64,
    /// Сглаживать края (`smooth_edges`).
    pub smooth: bool,
}

impl Default for LookOpts {
    fn default() -> Self {
        Self {
            light: 55.0,
            smooth: true,
        }
    }
}

impl LookOpts {
    pub fn from_state(st: &serde_json::Value) -> Self {
        let d = Self::default();
        Self {
            light: num(st, "light", d.light).clamp(0.0, 100.0),
            smooth: flag(st, "smooth_edges", d.smooth),
        }
    }

    /// Сила отмывки для шейдера: 55 процентов - свет окна модуля.
    pub fn strength(&self) -> f32 {
        (self.light / 55.0).clamp(0.0, 1.4) as f32
    }
}

/// Непрозрачность из `opacity` модуля: там это прозрачность в процентах.
pub(super) fn opacity_from_state(st: &serde_json::Value) -> f32 {
    let t = num(st, "opacity", 0.0).clamp(0.0, 95.0);
    (1.0 - t / 100.0) as f32
}

/// Сторона обрезки по ключу `side` модуля.
pub(super) fn side_from_key(k: &str) -> Option<Side> {
    match k {
        "in" => Some(Side::In),
        "out" => Some(Side::Out),
        "left" => Some(Side::Left),
        "right" => Some(Side::Right),
        "corridor" => Some(Side::Corridor),
        _ => None,
    }
}

impl SceneApp {
    /// Параметры окна из состояния окна QGIS в пакете.
    pub(super) fn params_from_state(&mut self, st: &serde_json::Value) {
        self.box_opts = BoxOpts::from_state(st);
        self.look_opts = LookOpts::from_state(st);
        self.opacity = opacity_from_state(st);
        self.marks = flag(st, "sketch_on", true);
        let c = &mut self.clip;
        if let Some(s) = text(st, "side").and_then(side_from_key) {
            c.side = Some(s);
        }
        c.width = num(st, "width", c.width);
        let zlo = num(st, "zlo", NO_LEVEL);
        let zhi = num(st, "zhi", NO_LEVEL);
        c.use_lo = zlo > NO_LEVEL;
        c.use_hi = zhi > NO_LEVEL;
        if c.use_lo {
            c.zlo = zlo;
        }
        if c.use_hi {
            c.zhi = zhi;
        }
    }

    /// Окно «Параметры сцены». Открыто, пока нажата кнопка плашки.
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
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                ui.horizontal(|ui| {
                    for (i, (ru, _)) in TABS.iter().enumerate() {
                        ui.selectable_value(&mut self.params_tab, i, t(ru));
                    }
                });
                ui.separator();
                match self.params_tab {
                    0 => self.view_tab(ui),
                    1 => self.look_tab(ui),
                    2 => self.box_tab(ui),
                    _ => self.clip_tab(ui),
                }
            });
        self.params_open = open;
    }

    fn view_tab(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let t = |s: &'static str| tr(lang, s);
        let mut c = self.cam;
        let mut vex = self.vex;
        let mut transp = f64::from(1.0 - self.opacity) * 100.0;
        egui::Grid::new("p_view").num_columns(2).show(ui, |ui| {
            ui.label(t("Вертикальное преувеличение"));
            ui.add(
                egui::DragValue::new(&mut vex)
                    .range(0.01..=10000.0)
                    .speed(0.05)
                    .max_decimals(2),
            );
            ui.end_row();
            ui.label(t("Прозрачность, %"));
            ui.add(egui::Slider::new(&mut transp, 0.0..=95.0).max_decimals(0));
            ui.end_row();
            // азимут 0-360, как в строке состояния
            ui.label(t("Азимут, °"));
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
        });
        ui.checkbox(&mut c.ortho, t("Параллельная проекция"));
        if c != self.cam {
            self.cam = c;
            self.anim = None;
            self.dirty = true;
        }
        if vex != self.vex {
            self.vex = vex;
            self.dirty = true;
        }
        let opacity = (1.0 - transp / 100.0) as f32;
        if (opacity - self.opacity).abs() > 1e-6 {
            self.opacity = opacity;
            self.dirty = true;
        }
    }

    fn look_tab(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let t = |s: &'static str| tr(lang, s);
        let before = (self.look_opts, self.gradient, self.marks, self.dark);
        let l = &mut self.look_opts;
        egui::Grid::new("p_look").num_columns(2).show(ui, |ui| {
            ui.label(t("Отмывка, %"));
            ui.add(egui::Slider::new(&mut l.light, 0.0..=100.0).max_decimals(0));
            ui.end_row();
            ui.label(t("Тема"));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.dark, false, t("Светлая"));
                ui.selectable_value(&mut self.dark, true, t("Тёмная"));
            });
            ui.end_row();
        });
        ui.checkbox(&mut self.gradient, t("Градиентный фон"));
        ui.checkbox(&mut l.smooth, t("Сглаживать края"));
        ui.checkbox(&mut self.marks, t("Показывать разметку"))
            .on_hover_text(t(
                "Нарисованный контур и линия разреза поверх сцены. Обрезка по ним остаётся и при снятой галке.",
            ));
        if before.3 != self.dark {
            ui.ctx().set_theme(if self.dark {
                egui::Theme::Dark
            } else {
                egui::Theme::Light
            });
        }
        if before != (self.look_opts, self.gradient, self.marks, self.dark) {
            self.dirty = true;
        }
    }

    fn box_tab(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let t = |s: &'static str| tr(lang, s);
        let before = self.box_opts;
        let b = &mut self.box_opts;
        ui.checkbox(&mut b.on, t("Координатный короб"));
        ui.add_enabled_ui(b.on, |ui| {
            egui::Grid::new("p_box").num_columns(3).show(ui, |ui| {
                ui.label(t("Рёбра"));
                egui::ComboBox::from_id_salt("box_edges")
                    .selected_text(t(b.edges.name()))
                    .show_ui(ui, |ui| {
                        for e in Edges::ALL {
                            ui.selectable_value(&mut b.edges, e, t(e.name()));
                        }
                    });
                ui.end_row();
                ui.label(t("Сетка"));
                egui::ComboBox::from_id_salt("grid_planes")
                    .selected_text(t(b.grid.name()))
                    .show_ui(ui, |ui| {
                        for g in Grid::ALL {
                            ui.selectable_value(&mut b.grid, g, t(g.name()));
                        }
                    });
                ui.end_row();
                ui.label(t("Шаг сетки, м"));
                ui.add(
                    egui::DragValue::new(&mut b.grid_step)
                        .range(0.0..=1e9)
                        .speed(1.0),
                );
                ui.end_row();
                ui.label("");
                ui.label(t("план"));
                ui.label("Z");
                ui.end_row();
                ui.label(t("Шаг делений"));
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
                ui.label(t("Множитель подписей"))
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
            ui.checkbox(&mut b.near, t("Подписи к ближним"));
            ui.checkbox(&mut b.range_on, t("Свой охват"));
            ui.add_enabled_ui(b.range_on, |ui| {
                ui.weak(t("X, Y, Z от и до"));
                egui::Grid::new("p_range").num_columns(3).show(ui, |ui| {
                    for (k, axis) in ["X", "Y", "Z"].into_iter().enumerate() {
                        ui.label(axis);
                        ui.add(egui::DragValue::new(&mut b.range[2 * k]).speed(1.0));
                        ui.add(egui::DragValue::new(&mut b.range[2 * k + 1]).speed(1.0));
                        ui.end_row();
                    }
                });
            });
        });
        if before != self.box_opts {
            self.dirty = true;
        }
    }

    fn clip_tab(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let t = |s: &'static str| tr(lang, s);
        ui.weak(t(
            "Контур и линия берутся из измерения: щелчками поставьте точки, двойной щелчок замыкает контур.",
        ));
        let c = &mut self.clip;
        let name = |s: Side| match s {
            Side::In => t("внутри"),
            Side::Out => t("снаружи"),
            Side::Left => t("слева"),
            Side::Right => t("справа"),
            Side::Corridor => t("коридор"),
        };
        egui::Grid::new("clip").num_columns(3).show(ui, |ui| {
            ui.label(t("Что оставить"));
            let mut side = c.side.unwrap_or(Side::In);
            egui::ComboBox::from_id_salt("clip_side")
                .selected_text(name(side))
                .show_ui(ui, |ui| {
                    for s in [Side::In, Side::Out, Side::Left, Side::Right, Side::Corridor] {
                        ui.selectable_value(&mut side, s, name(s));
                    }
                });
            c.side = Some(side);
            ui.end_row();
            ui.label(t("Полуширина коридора, м"));
            ui.add_enabled(
                side == Side::Corridor,
                egui::DragValue::new(&mut c.width)
                    .range(0.0..=1e9)
                    .speed(1.0),
            );
            ui.end_row();
            ui.label(t("По отметке"));
            ui.horizontal(|ui| {
                ui.checkbox(&mut c.use_lo, t("от"));
                ui.add_enabled(c.use_lo, egui::DragValue::new(&mut c.zlo).speed(1.0));
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut c.use_hi, t("до"));
                ui.add_enabled(c.use_hi, egui::DragValue::new(&mut c.zhi).speed(1.0));
            });
            ui.end_row();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{Lang, english};

    #[test]
    fn opts_from_module_state() {
        let st = serde_json::json!({
            "axes_on": false, "box_edges": "axes", "grid_planes": "floor,walls",
            "grid_step": 25.0, "box_near": false, "tick_xy": 50.0, "tick_z": 10.0,
            "mult_xy": 0.001, "mult_z": -1.0, "box_range_on": true,
            "box_range": [0.0, 100.0, 0.0, 50.0, -30.0, 0.0],
            "light": 30, "smooth_edges": false, "opacity": 40.0
        });
        let b = BoxOpts::from_state(&st);
        assert!(!b.on && !b.near && b.range_on);
        assert_eq!(
            (b.edges, b.grid, b.grid_step),
            (Edges::Axes, Grid::FloorWalls, 25.0)
        );
        assert_eq!(
            (b.step_xy, b.step_z, b.mult_xy, b.mult_z),
            (50.0, 10.0, 0.001, -1.0)
        );
        assert_eq!(b.range, [0.0, 100.0, 0.0, 50.0, -30.0, 0.0]);
        assert_eq!(
            BoxOpts::from_state(&serde_json::Value::Null),
            BoxOpts::default()
        );
        let l = LookOpts::from_state(&st);
        assert_eq!((l.light, l.smooth), (30.0, false));
        assert!((LookOpts::default().strength() - 1.0).abs() < 1e-6);
        assert!((opacity_from_state(&st) - 0.6).abs() < 1e-6);
        assert_eq!(side_from_key("corridor"), Some(Side::Corridor));
        for e in Edges::ALL {
            assert_eq!(Edges::from_key(e.key()), e);
        }
        for g in Grid::ALL {
            assert_eq!(Grid::from_key(g.key()), g);
        }
    }

    /// Строка таблицы списка: подпись, перевод, ключи, где.
    fn rows(md: &str) -> Vec<(String, String, Vec<String>, String)> {
        md.lines()
            .filter(|l| l.starts_with('|') && !l.starts_with("|---") && !l.starts_with("| Поле"))
            .filter_map(|l| {
                let c: Vec<&str> = l.trim_matches('|').split('|').map(str::trim).collect();
                // ключи до скобки: в скобках значения (`all`, `floor`)
                let keys = c
                    .get(2)?
                    .split('(')
                    .next()?
                    .split('`')
                    .skip(1)
                    .step_by(2)
                    .map(str::to_string)
                    .collect();
                Some((
                    c[0].to_string(),
                    c.get(1)?.to_string(),
                    keys,
                    c.get(3)?.to_string(),
                ))
            })
            .collect()
    }

    /// Подпись до скобки и до двоеточия, варианты после двоеточия.
    fn split(s: &str) -> (&str, Vec<&str>) {
        let s = s.split(" (").next().unwrap_or(s);
        match s.split_once(": ") {
            Some((head, tail)) => (head.trim(), tail.split(", ").map(str::trim).collect()),
            None => (s.trim(), Vec::new()),
        }
    }

    /// Окно повторяет список модуля: вкладки по порядку, у каждого
    /// параметра «оба» есть поле, подписи переведены так же, как
    /// у модуля, ключей сверх списка нет.
    #[test]
    fn window_follows_the_module_list() {
        let md = include_str!("../../../../fixtures/SCENE_PARAMS.md");
        let tabs: Vec<(&str, &str)> = md
            .lines()
            .filter_map(|l| l.strip_prefix("## "))
            .filter_map(|l| l.split_once(". "))
            .filter_map(|(_, name)| name.split_once(" / "))
            .collect();
        assert_eq!(tabs, TABS.to_vec());
        for (ru, en) in TABS {
            assert_eq!(tr(Lang::En, ru), en);
        }
        let mut listed = Vec::new();
        for (ru, en, keys, place) in rows(md) {
            if !(place == "оба" || place.starts_with("Лайт")) {
                continue;
            }
            let ((ru_head, ru_opts), (en_head, en_opts)) = (split(&ru), split(&en));
            assert_eq!(english(ru_head), Some(en_head), "{ru}");
            assert_eq!(ru_opts.len(), en_opts.len(), "{ru}");
            for (r, e) in ru_opts.iter().zip(&en_opts) {
                if r != e {
                    assert_eq!(english(r), Some(*e), "{ru}");
                }
            }
            for k in keys {
                assert!(KEYS.contains(&k.as_str()), "нет поля для {k}");
                listed.push(k);
            }
        }
        for k in KEYS {
            assert!(listed.iter().any(|l| l == k), "{k} нет в списке модуля");
        }
    }
}
