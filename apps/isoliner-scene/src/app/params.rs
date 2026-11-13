// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Окно «Параметры сцены» на одну кнопку плашки, общее с окном модуля.
//!
//! Окно строится по списку модуля `fixtures/scene_params.json`: его
//! пишет `tools/make_fixtures.py` модуля из `isoliner3d/scene_params.py`,
//! там же описание схемы (`doc/SCENE_PARAMS.md` модуля). Вкладки,
//! секции, порядок, подписи на двух языках и варианты выбора - из
//! списка, здесь только привязка ключей `state` к полям программы
//! (`get`, `set`). Поля «pro» (только модуль) не показываются. Тест
//! ниже падает, если у поля «both» или «lite» нет привязки.

use std::sync::OnceLock;

use super::{SceneApp, Side};
use crate::i18n::Lang;
use eframe::egui;
use serde::Deserialize;

/// Список модуля, вшитый в программу при сборке.
const SPEC_JSON: &str = include_str!("../../../../fixtures/scene_params.json");

/// Отметка «нет» в `zlo`, `zhi` модуля.
pub(super) const NO_LEVEL: f64 = -1e7;

/// Подпись на двух языках.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct Text {
    pub ru: String,
    pub en: String,
}

impl Text {
    fn get(&self, lang: Lang) -> &str {
        match lang {
            Lang::Ru => &self.ru,
            Lang::En => &self.en,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct Choice {
    pub value: String,
    #[serde(flatten)]
    pub text: Text,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct Param {
    pub key: String,
    #[serde(flatten)]
    pub text: Text,
    pub kind: String,
    #[serde(rename = "where")]
    pub place: String,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub step: Option<f64>,
    #[serde(default)]
    pub choices: Vec<Choice>,
}

impl Param {
    /// Поле есть в этом окне: общее или только программы.
    pub fn shown(&self) -> bool {
        self.place == "both" || self.place == "lite"
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct Section {
    #[serde(flatten)]
    pub text: Text,
    /// Ключ галки в заголовке секции: снята - поля секции недоступны.
    #[serde(default)]
    pub toggle: Option<String>,
    pub params: Vec<Param>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct Group {
    pub id: String,
    #[serde(flatten)]
    pub text: Text,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct Spec {
    pub version: u32,
    pub window: Text,
    pub groups: Vec<Group>,
}

/// Список модуля, разобранный один раз. Не разобрался - None, окно
/// пишет об этом вместо полей.
pub(super) fn spec() -> Option<&'static Spec> {
    static SPEC: OnceLock<Option<Spec>> = OnceLock::new();
    // схема другой версии - окно её не понимает
    SPEC.get_or_init(|| {
        serde_json::from_str::<Spec>(SPEC_JSON)
            .ok()
            .filter(|s| s.version == 1)
    })
    .as_ref()
}

/// Значение поля окна.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Val {
    Num(f64),
    Flag(bool),
    Text(String),
    Six([f64; 6]),
    /// Отметка обрезки: None - не задана (-1e7 у модуля).
    Level(Option<f64>),
}

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
    /// Короб без ключа `axes_on` показан, как и в пакетах до него.
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

/// Отмывка и сглаживание краёв.
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

/// Сторона обрезки по ключу `side` модуля и обратно.
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

fn side_key(s: Side) -> &'static str {
    match s {
        Side::In => "in",
        Side::Out => "out",
        Side::Left => "left",
        Side::Right => "right",
        Side::Corridor => "corridor",
    }
}

/// Поле числа по пределам и шагу из списка.
fn drag_value<'a>(x: &'a mut f64, p: &Param) -> egui::DragValue<'a> {
    let decimals = if p.kind == "int" { 0 } else { 4 };
    egui::DragValue::new(x)
        .range(p.min.unwrap_or(f64::MIN)..=p.max.unwrap_or(f64::MAX))
        .speed(p.step.unwrap_or(1.0) * 0.2)
        .max_decimals(decimals)
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

    /// Значение поля по ключу списка. None - у ключа нет привязки.
    pub(super) fn get(&self, key: &str) -> Option<Val> {
        let b = &self.box_opts;
        let c = &self.clip;
        Some(match key {
            "vex" => Val::Num(self.vex),
            "opacity" => Val::Num(f64::from(1.0 - self.opacity) * 100.0),
            // азимут 0-360, как в строке состояния
            "camera.azimuth" => Val::Num(self.cam.azimuth.rem_euclid(360.0)),
            "camera.elevation" => Val::Num(self.cam.elevation),
            "camera.distance" => Val::Num(self.cam.distance),
            "camera.fov" => Val::Num(self.cam.fov),
            "camera.ortho" => Val::Flag(self.cam.ortho),
            "light" => Val::Num(self.look_opts.light),
            "bg_grad" => Val::Flag(self.gradient),
            "smooth_edges" => Val::Flag(self.look_opts.smooth),
            "sketch_on" => Val::Flag(self.marks),
            "theme" => Val::Text(if self.dark { "dark" } else { "light" }.into()),
            "axes_on" => Val::Flag(b.on),
            "box_edges" => Val::Text(b.edges.key().into()),
            "box_near" => Val::Flag(b.near),
            "grid_planes" => Val::Text(b.grid.key().into()),
            "grid_step" => Val::Num(b.grid_step),
            "tick_xy" => Val::Num(b.step_xy),
            "tick_z" => Val::Num(b.step_z),
            "mult_xy" => Val::Num(b.mult_xy),
            "mult_z" => Val::Num(b.mult_z),
            "box_range_on" => Val::Flag(b.range_on),
            "box_range" => Val::Six(b.range),
            "side" => Val::Text(side_key(c.side.unwrap_or(Side::In)).into()),
            "width" => Val::Num(c.width),
            "zlo" => Val::Level(c.use_lo.then_some(c.zlo)),
            "zhi" => Val::Level(c.use_hi.then_some(c.zhi)),
            _ => return None,
        })
    }

    /// Записать поле по ключу списка. Чужой ключ или чужой вид
    /// значения пропускаются.
    pub(super) fn set(&mut self, key: &str, v: Val) {
        let b = &mut self.box_opts;
        let c = &mut self.clip;
        match (key, v) {
            ("vex", Val::Num(x)) => self.vex = x.max(0.01),
            ("opacity", Val::Num(x)) => self.opacity = (1.0 - x.clamp(0.0, 95.0) / 100.0) as f32,
            ("camera.azimuth", Val::Num(x)) => self.cam.azimuth = x,
            ("camera.elevation", Val::Num(x)) => self.cam.elevation = x,
            ("camera.distance", Val::Num(x)) => self.cam.distance = x.max(0.01),
            ("camera.fov", Val::Num(x)) => self.cam.fov = x,
            ("camera.ortho", Val::Flag(x)) => self.cam.ortho = x,
            ("light", Val::Num(x)) => self.look_opts.light = x.clamp(0.0, 100.0),
            ("bg_grad", Val::Flag(x)) => self.gradient = x,
            ("smooth_edges", Val::Flag(x)) => self.look_opts.smooth = x,
            ("sketch_on", Val::Flag(x)) => self.marks = x,
            ("theme", Val::Text(x)) => self.dark = x == "dark",
            ("axes_on", Val::Flag(x)) => b.on = x,
            ("box_edges", Val::Text(x)) => b.edges = Edges::from_key(&x),
            ("box_near", Val::Flag(x)) => b.near = x,
            ("grid_planes", Val::Text(x)) => b.grid = Grid::from_key(&x),
            ("grid_step", Val::Num(x)) => b.grid_step = x.max(0.0),
            ("tick_xy", Val::Num(x)) => b.step_xy = x.max(0.0),
            ("tick_z", Val::Num(x)) => b.step_z = x.max(0.0),
            ("mult_xy", Val::Num(x)) => b.mult_xy = x,
            ("mult_z", Val::Num(x)) => b.mult_z = x,
            ("box_range_on", Val::Flag(x)) => b.range_on = x,
            ("box_range", Val::Six(x)) => b.range = x,
            ("side", Val::Text(x)) => {
                if let Some(s) = side_from_key(&x) {
                    c.side = Some(s);
                }
            }
            ("width", Val::Num(x)) => c.width = x.max(0.0),
            ("zlo", Val::Level(x)) => {
                c.use_lo = x.is_some();
                c.zlo = x.unwrap_or(c.zlo);
            }
            ("zhi", Val::Level(x)) => {
                c.use_hi = x.is_some();
                c.zhi = x.unwrap_or(c.zhi);
            }
            _ => {}
        }
    }

    /// Поле недоступно: коридор без коридора, охват без своей галки.
    fn idle(&self, key: &str) -> bool {
        match key {
            "width" => self.clip.side != Some(Side::Corridor),
            "box_range" => !self.box_opts.range_on,
            _ => false,
        }
    }

    /// Окно «Параметры сцены». Открыто, пока нажата кнопка плашки.
    pub(super) fn params_window(&mut self, ctx: &egui::Context) {
        if !self.params_open || self.pkg.is_none() {
            return;
        }
        let lang = self.lang;
        let mut open = true;
        let Some(spec) = spec() else {
            self.status = "scene_params.json".to_string();
            self.params_open = false;
            return;
        };
        let before = (self.cam, self.vex, self.dark);
        egui::Window::new(spec.window.get(lang))
            .id(egui::Id::unique("params"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(330.0)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                ui.horizontal(|ui| {
                    for (i, g) in spec.groups.iter().enumerate() {
                        ui.selectable_value(&mut self.params_tab, i, g.text.get(lang));
                    }
                });
                ui.separator();
                if let Some(g) = spec.groups.get(self.params_tab) {
                    if g.id == "clip" {
                        ui.weak(crate::i18n::tr(
                            lang,
                            "Контур и линия берутся из измерения: щелчками поставьте точки, двойной щелчок замыкает контур.",
                        ));
                    }
                    for s in &g.sections {
                        self.section_ui(ui, s, lang);
                    }
                }
            });
        self.params_open = open;
        if (self.cam, self.vex, self.dark) != before {
            self.anim = None;
        }
        if self.dark != before.2 {
            ctx.set_theme(if self.dark {
                egui::Theme::Dark
            } else {
                egui::Theme::Light
            });
        }
    }

    /// Секция вкладки: заголовок (с галкой, если она есть) и поля.
    /// Соседние поля с одной подписью идут одной строкой.
    fn section_ui(&mut self, ui: &mut egui::Ui, s: &Section, lang: Lang) {
        let params: Vec<&Param> = s
            .params
            .iter()
            .filter(|p| p.shown() && Some(&p.key) != s.toggle.as_ref())
            .collect();
        let toggle = s.toggle.as_deref();
        if params.is_empty() && toggle.is_none() {
            return;
        }
        let mut on = true;
        if let Some(k) = toggle {
            if let Some(Val::Flag(mut x)) = self.get(k) {
                if ui
                    .checkbox(&mut x, egui::RichText::new(s.text.get(lang)).strong())
                    .changed()
                {
                    self.set(k, Val::Flag(x));
                    self.dirty = true;
                }
                on = x;
            }
        } else if !s.text.ru.is_empty() {
            ui.label(egui::RichText::new(s.text.get(lang)).strong());
        }
        // флажки отдельно, под таблицей: так строки таблицы ровные
        let (flags, rows): (Vec<&Param>, Vec<&Param>) =
            params.into_iter().partition(|p| p.kind == "bool");
        ui.add_enabled_ui(on, |ui| {
            egui::Grid::new(("p", s.text.ru.as_str()))
                .num_columns(2)
                .show(ui, |ui| {
                    let mut i = 0;
                    while i < rows.len() {
                        let head = rows[i];
                        let mut same = vec![head];
                        while let Some(next) = rows.get(i + same.len()) {
                            if next.text.ru != head.text.ru {
                                break;
                            }
                            same.push(next);
                        }
                        i += same.len();
                        ui.label(head.text.get(lang));
                        ui.horizontal(|ui| {
                            for p in same {
                                self.field(ui, p, lang);
                            }
                        });
                        ui.end_row();
                    }
                });
            for p in flags {
                if let Some(Val::Flag(mut x)) = self.get(&p.key)
                    && ui.checkbox(&mut x, p.text.get(lang)).changed()
                {
                    self.set(&p.key, Val::Flag(x));
                    self.dirty = true;
                }
            }
        });
    }

    /// Поле значения по виду из списка.
    fn field(&mut self, ui: &mut egui::Ui, p: &Param, lang: Lang) {
        let Some(v) = self.get(&p.key) else {
            return;
        };
        let idle = self.idle(&p.key);
        let new = match v {
            Val::Num(mut x) => ui
                .add_enabled(!idle, drag_value(&mut x, p))
                .changed()
                .then_some(Val::Num(x)),
            Val::Level(x) => {
                let mut on = x.is_some();
                let mut z = x.unwrap_or(0.0);
                let a = ui.checkbox(&mut on, "").changed();
                let b = ui.add_enabled(on, drag_value(&mut z, p)).changed();
                (a || b).then_some(Val::Level(on.then_some(z)))
            }
            Val::Six(mut r) => {
                let mut changed = false;
                ui.add_enabled_ui(!idle, |ui| {
                    egui::Grid::new(("six", p.key.as_str())).show(ui, |ui| {
                        for (k, axis) in ["X", "Y", "Z"].into_iter().enumerate() {
                            ui.label(axis);
                            changed |= ui.add(drag_value(&mut r[2 * k], p)).changed();
                            changed |= ui.add(drag_value(&mut r[2 * k + 1], p)).changed();
                            ui.end_row();
                        }
                    });
                });
                changed.then_some(Val::Six(r))
            }
            Val::Text(cur) => {
                let shown = p
                    .choices
                    .iter()
                    .find(|c| c.value == cur)
                    .map_or(cur.as_str(), |c| c.text.get(lang));
                let mut pick = cur.clone();
                egui::ComboBox::from_id_salt(("c", p.key.as_str()))
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        for c in &p.choices {
                            ui.selectable_value(&mut pick, c.value.clone(), c.text.get(lang));
                        }
                    });
                (pick != cur).then_some(Val::Text(pick))
            }
            Val::Flag(mut x) => ui.checkbox(&mut x, "").changed().then_some(Val::Flag(x)),
        };
        if let Some(v) = new {
            self.set(&p.key, v);
            self.dirty = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        for s in [Side::In, Side::Out, Side::Left, Side::Right, Side::Corridor] {
            assert_eq!(side_from_key(side_key(s)), Some(s));
        }
        for e in Edges::ALL {
            assert_eq!(Edges::from_key(e.key()), e);
        }
        for g in Grid::ALL {
            assert_eq!(Grid::from_key(g.key()), g);
        }
    }

    /// Окно повторяет список модуля: список разбирается, вкладок
    /// четыре, у каждого поля «both» и «lite» есть привязка, варианты
    /// выбора совпадают с теми, что понимает программа.
    #[test]
    fn window_follows_the_module_list() {
        let s = spec().expect("scene_params.json разбирается");
        assert_eq!(s.version, 1);
        let ids: Vec<&str> = s.groups.iter().map(|g| g.id.as_str()).collect();
        assert_eq!(ids, ["view", "look", "box", "clip"]);
        let mut keys = Vec::new();
        for p in s
            .groups
            .iter()
            .flat_map(|g| &g.sections)
            .flat_map(|s| &s.params)
            .filter(|p| p.shown())
        {
            keys.push(p.key.as_str());
            for c in &p.choices {
                let known = match p.key.as_str() {
                    "box_edges" => Edges::from_key(&c.value).key() == c.value,
                    "grid_planes" => Grid::from_key(&c.value).key() == c.value,
                    "side" => side_from_key(&c.value).is_some(),
                    "theme" => c.value == "light" || c.value == "dark",
                    _ => false,
                };
                assert!(known, "{}: {}", p.key, c.value);
            }
        }
        // привязки проверяются на настоящем окне программы без видеокарты
        let st = serde_json::Value::Null;
        let mut app = super::super::tests::bare_app();
        app.params_from_state(&st);
        for k in &keys {
            let v = app.get(k);
            assert!(v.is_some(), "нет привязки для {k}");
            // запись того же значения ничего не ломает
            if let Some(v) = v {
                app.set(k, v.clone());
                assert_eq!(app.get(k), Some(v), "{k}");
            }
        }
        for k in ["vex", "camera.azimuth", "axes_on", "side", "zlo", "theme"] {
            assert!(keys.contains(&k), "{k} нет в списке модуля");
        }
    }
}
