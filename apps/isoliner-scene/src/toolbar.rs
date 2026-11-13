// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Плашка значков поверх сцены - та же, что у окна модуля.
//!
//! Порядок кнопок, разделители, подсказки на двух языках и значки
//! задаёт модуль: список `fixtures/toolbar.json` и значки SVG
//! `fixtures/icons/*.svg` пишет `tools/make_fixtures.py` модуля из
//! `viewer3d._tool_icon`. Здесь значки разбираются в фигуры egui:
//! в SVG модуля только ломаные, многоугольники, прямоугольники, круги,
//! эллипсы и кривые Безье, и растеризатор ради них не нужен. В тёмной
//! теме чернила и бумага значка светлеют и темнеют, акценты те же.

use std::sync::OnceLock;

use eframe::egui;
use egui::{Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2};
use serde::Deserialize;

use crate::i18n::Lang;

/// Список кнопок модуля, вшитый при сборке.
const TOOLBAR_JSON: &str = include_str!("../../../fixtures/toolbar.json");

/// Значки модуля по имени из списка.
const ICONS: &[(&str, &str)] = &[
    ("params", include_str!("../../../fixtures/icons/params.svg")),
    ("top", include_str!("../../../fixtures/icons/top.svg")),
    ("home", include_str!("../../../fixtures/icons/home.svg")),
    (
        "section",
        include_str!("../../../fixtures/icons/section.svg"),
    ),
    ("draw", include_str!("../../../fixtures/icons/draw.svg")),
    ("undo", include_str!("../../../fixtures/icons/undo.svg")),
    ("done", include_str!("../../../fixtures/icons/done.svg")),
    ("line", include_str!("../../../fixtures/icons/line.svg")),
    ("clear", include_str!("../../../fixtures/icons/clear.svg")),
    ("save", include_str!("../../../fixtures/icons/save.svg")),
    ("shell", include_str!("../../../fixtures/icons/shell.svg")),
    ("export", include_str!("../../../fixtures/icons/export.svg")),
    ("scene", include_str!("../../../fixtures/icons/scene.svg")),
    ("spin", include_str!("../../../fixtures/icons/spin.svg")),
    ("views", include_str!("../../../fixtures/icons/views.svg")),
    ("frames", include_str!("../../../fixtures/icons/frames.svg")),
    ("copy", include_str!("../../../fixtures/icons/copy.svg")),
    ("png", include_str!("../../../fixtures/icons/png.svg")),
    (
        "rebuild",
        include_str!("../../../fixtures/icons/rebuild.svg"),
    ),
    ("about", include_str!("../../../fixtures/icons/about.svg")),
];

/// Сторона значка и кнопки, точки egui. Значок 15 точек на экране
/// выходит размером со значок окна модуля (QIcon 18 пикселей).
pub const ICON: f32 = 15.0;
pub const BUTTON: f32 = 24.0;

/// Кнопка или разделитель списка модуля.
#[derive(Debug, Clone, Deserialize)]
pub struct Item {
    #[serde(default)]
    pub sep: bool,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub ru: String,
    #[serde(default)]
    pub en: String,
    /// both, pro или lite.
    #[serde(default, rename = "where")]
    pub place: String,
    /// Кнопка видна только в этом режиме (`draw` - пока рисуется контур).
    #[serde(default)]
    pub mode: Option<String>,
}

impl Item {
    /// Кнопка есть в этой программе.
    pub fn shown(&self) -> bool {
        self.place == "both" || self.place == "lite"
    }

    pub fn tip(&self, lang: Lang) -> &str {
        match lang {
            Lang::Ru => &self.ru,
            Lang::En => &self.en,
        }
    }
}

#[derive(Debug, Deserialize)]
struct Toolbar {
    version: u32,
    items: Vec<Item>,
}

/// Кнопки модуля. Список не разобрался - пусто: плашка без кнопок
/// лучше падения окна.
pub fn items() -> &'static [Item] {
    static ITEMS: OnceLock<Vec<Item>> = OnceLock::new();
    ITEMS.get_or_init(|| {
        serde_json::from_str::<Toolbar>(TOOLBAR_JSON)
            .ok()
            .filter(|t| t.version == 1)
            .map(|t| t.items)
            .unwrap_or_default()
    })
}

/// Обводка: цвет и толщина в долях стороны.
type Line = Option<(Color32, f32)>;

/// Фигура значка в долях стороны: 0..1 по обеим осям.
#[derive(Debug, Clone, PartialEq)]
enum Prim {
    Line {
        pts: Vec<[f32; 2]>,
        closed: bool,
        fill: Option<Color32>,
        stroke: Line,
    },
    Ellipse {
        c: [f32; 2],
        r: [f32; 2],
        fill: Option<Color32>,
        stroke: Line,
    },
}

/// Стиль группы SVG: заливка и обводка, наследуется вглубь.
#[derive(Debug, Clone, Default)]
struct Style {
    fill: Option<Color32>,
    fill_opacity: f32,
    stroke: Option<Color32>,
    stroke_opacity: f32,
    width: f32,
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let i = tag.find(&key)? + key.len();
    let rest = tag.get(i..)?;
    rest.find('"').and_then(|j| rest.get(..j))
}

fn color(s: &str) -> Option<Color32> {
    match s {
        "none" => None,
        "black" => Some(Color32::BLACK),
        "white" => Some(Color32::WHITE),
        _ => {
            let h = s.strip_prefix('#').filter(|h| h.len() == 6)?;
            let v = u32::from_str_radix(h, 16).ok()?;
            let [_, r, g, b] = v.to_be_bytes();
            Some(Color32::from_rgb(r, g, b))
        }
    }
}

fn num(s: &str) -> Option<f32> {
    s.trim().parse::<f32>().ok().filter(|v| v.is_finite())
}

fn numbers(s: &str) -> Vec<f32> {
    s.split([' ', ',', '\n', '\t'])
        .filter(|t| !t.is_empty())
        .filter_map(num)
        .collect()
}

impl Style {
    /// Стиль тега поверх стиля группы.
    fn over(&self, tag: &str) -> Style {
        let mut s = self.clone();
        if let Some(v) = attr(tag, "fill") {
            s.fill = color(v);
        }
        if let Some(v) = attr(tag, "fill-opacity").and_then(num) {
            s.fill_opacity = v;
        }
        if let Some(v) = attr(tag, "stroke") {
            s.stroke = color(v);
        }
        if let Some(v) = attr(tag, "stroke-opacity").and_then(num) {
            s.stroke_opacity = v;
        }
        if let Some(v) = attr(tag, "stroke-width").and_then(num) {
            s.width = v;
        }
        s
    }

    fn fill(&self) -> Option<Color32> {
        self.fill.map(|c| c.gamma_multiply(self.fill_opacity))
    }

    fn stroke(&self, side: f32) -> Line {
        self.stroke
            .filter(|_| self.width > 0.0)
            .map(|c| (c.gamma_multiply(self.stroke_opacity), self.width / side))
    }
}

/// Добавить к точкам числа команды пути: M и L - точки, C - кривая
/// Безье двенадцатью отрезками.
fn flush(cmd: char, nums: &mut Vec<f32>, pts: &mut Vec<[f32; 2]>) {
    match cmd {
        'M' | 'L' => {
            for p in nums.as_chunks::<2>().0 {
                pts.push([p[0], p[1]]);
            }
        }
        'C' => {
            for c in nums.as_chunks::<6>().0 {
                let p0 = pts.last().copied().unwrap_or([c[0], c[1]]);
                for k in 1..=12 {
                    let t = k as f32 / 12.0;
                    let u = 1.0 - t;
                    let b = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                    pts.push([
                        b[0] * p0[0] + b[1] * c[0] + b[2] * c[2] + b[3] * c[4],
                        b[0] * p0[1] + b[1] * c[1] + b[2] * c[3] + b[3] * c[5],
                    ]);
                }
            }
        }
        _ => {}
    }
    nums.clear();
}

/// Точки пути SVG: M, L, C, Z. Других команд у значков модуля нет.
fn path_points(d: &str) -> (Vec<[f32; 2]>, bool) {
    let mut pts: Vec<[f32; 2]> = Vec::new();
    let mut closed = false;
    let mut cmd = 'M';
    let mut nums: Vec<f32> = Vec::new();
    let mut token = String::new();
    for ch in d.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_alphabetic() && ch != 'e' && ch != 'E' {
            if let Some(v) = num(&token) {
                nums.push(v);
            }
            token.clear();
            flush(cmd, &mut nums, &mut pts);
            cmd = ch.to_ascii_uppercase();
            if cmd == 'Z' {
                closed = true;
            }
        } else if ch == ' ' || ch == ',' {
            if let Some(v) = num(&token) {
                nums.push(v);
            }
            token.clear();
        } else {
            token.push(ch);
        }
    }
    flush(cmd, &mut nums, &mut pts);
    (pts, closed)
}

/// Разобрать значок модуля. Сторона - по `viewBox`.
fn parse(svg: &str) -> Vec<Prim> {
    let side = svg
        .find("viewBox=\"")
        .and_then(|i| svg.get(i + 9..))
        .and_then(|r| r.split('"').next())
        .map(numbers)
        .and_then(|v| v.get(2).copied())
        .filter(|s| *s > 0.0)
        .unwrap_or(18.0);
    let k = |v: f32| v / side;
    let mut stack = vec![Style {
        fill_opacity: 1.0,
        stroke_opacity: 1.0,
        width: 1.0,
        ..Style::default()
    }];
    let mut out = Vec::new();
    for raw in svg.split('<').skip(1) {
        let Some(tag) = raw.split('>').next() else {
            continue;
        };
        let name = tag.split_whitespace().next().unwrap_or("");
        let style = stack.last().cloned().unwrap_or_default().over(tag);
        let selfclose = tag.trim_end().ends_with('/');
        let g = |a: &str| attr(tag, a).and_then(num).unwrap_or(0.0);
        match name {
            "g" if !selfclose => stack.push(style),
            "/g" => {
                if stack.len() > 1 {
                    stack.pop();
                }
            }
            "polyline" | "polygon" => {
                let n = attr(tag, "points").map(numbers).unwrap_or_default();
                let pts = n
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|p| [k(p[0]), k(p[1])])
                    .collect();
                let closed = name == "polygon";
                out.push(Prim::Line {
                    pts,
                    closed,
                    fill: if closed { style.fill() } else { None },
                    stroke: style.stroke(side),
                });
            }
            "path" => {
                let (pts, closed) = attr(tag, "d").map(path_points).unwrap_or_default();
                out.push(Prim::Line {
                    pts: pts.iter().map(|p| [k(p[0]), k(p[1])]).collect(),
                    closed,
                    fill: if closed { style.fill() } else { None },
                    stroke: style.stroke(side),
                });
            }
            "rect" => {
                let (x, y, w, h) = (g("x"), g("y"), g("width"), g("height"));
                out.push(Prim::Line {
                    pts: vec![
                        [k(x), k(y)],
                        [k(x + w), k(y)],
                        [k(x + w), k(y + h)],
                        [k(x), k(y + h)],
                    ],
                    closed: true,
                    fill: style.fill(),
                    stroke: style.stroke(side),
                });
            }
            "circle" | "ellipse" => {
                let (rx, ry) = if name == "circle" {
                    (g("r"), g("r"))
                } else {
                    (g("rx"), g("ry"))
                };
                out.push(Prim::Ellipse {
                    c: [k(g("cx")), k(g("cy"))],
                    r: [k(rx), k(ry)],
                    fill: style.fill(),
                    stroke: style.stroke(side),
                });
            }
            _ => {}
        }
    }
    out
}

/// Значки, разобранные один раз.
fn parsed(icon: &str) -> &'static [Prim] {
    static ALL: OnceLock<Vec<(&'static str, Vec<Prim>)>> = OnceLock::new();
    ALL.get_or_init(|| ICONS.iter().map(|(n, s)| (*n, parse(s))).collect())
        .iter()
        .find(|(n, _)| *n == icon)
        .map_or(&[], |(_, p)| p.as_slice())
}

/// Значок есть среди вшитых.
#[cfg(test)]
pub fn has_icon(icon: &str) -> bool {
    ICONS.iter().any(|(n, _)| *n == icon)
}

/// Краска значка в тёмной теме: чернила светлые, белая бумага
/// тёмная, бирюза светлее. Остальные акценты те же.
fn tone(c: Color32, dark: bool) -> Color32 {
    if !dark {
        return c;
    }
    let to = match [c.r(), c.g(), c.b()] {
        [0x1d, 0x2b, 0x28] | [0, 0, 0] => [0xdd, 0xe6, 0xe3],
        [0xff, 0xff, 0xff] => [0x2b, 0x31, 0x30],
        [0x0e, 0x7c, 0x66] => [0x3f, 0xb8, 0x9c],
        other => other,
    };
    Color32::from_rgba_unmultiplied(to[0], to[1], to[2], c.a())
}

/// Линии значка тоньше, чем в SVG: Qt рисует их без сглаживающей
/// каймы, egui - с ней, и та же толщина выходит жирнее.
const THIN: f32 = 0.8;

/// Нарисовать значок `icon` в квадрате `rect`.
pub fn paint(icon: &str, rect: Rect, dark: bool) -> Vec<Shape> {
    let s = rect.width().min(rect.height());
    let at = |p: [f32; 2]| pos2(rect.min.x + p[0] * s, rect.min.y + p[1] * s);
    let mut out = Vec::new();
    for prim in parsed(icon) {
        match prim {
            Prim::Line {
                pts,
                closed,
                fill,
                stroke,
            } => {
                let v: Vec<Pos2> = pts.iter().map(|p| at(*p)).collect();
                if let Some(f) = fill
                    && v.len() >= 3
                {
                    out.push(Shape::convex_polygon(
                        v.clone(),
                        tone(*f, dark),
                        Stroke::NONE,
                    ));
                }
                if let Some((c, w)) = stroke {
                    let st = Stroke::new((w * s * THIN).max(0.6), tone(*c, dark));
                    out.push(if *closed {
                        Shape::closed_line(v, st)
                    } else {
                        Shape::line(v, st)
                    });
                }
            }
            Prim::Ellipse { c, r, fill, stroke } => {
                let (p, r) = (at(*c), vec2(r[0] * s, r[1] * s));
                if let Some(f) = fill {
                    out.push(Shape::ellipse_filled(p, r, tone(*f, dark)));
                }
                if let Some((col, w)) = stroke {
                    out.push(Shape::ellipse_stroke(
                        p,
                        r,
                        Stroke::new((w * s * THIN).max(0.6), tone(*col, dark)),
                    ));
                }
            }
        }
    }
    out
}

/// Плашка: полупрозрачный фон и рамка, как `#isoliner3dTools` модуля.
pub fn frame(dark: bool) -> egui::Frame {
    let (fill, line) = if dark {
        (
            Color32::from_rgba_unmultiplied(32, 36, 35, 215),
            Color32::from_rgba_unmultiplied(255, 255, 255, 40),
        )
    } else {
        (
            Color32::from_rgba_unmultiplied(255, 255, 255, 205),
            Color32::from_rgba_unmultiplied(0, 0, 0, 45),
        )
    };
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, line))
        .corner_radius(4)
        .inner_margin(3)
}

/// Кнопка-значок плашки. `checked` - нажата (вид по разрезу, вращение),
/// подсветка как у модуля: наведение, нажатие, включено.
pub fn button(ui: &mut Ui, icon: &str, tip: &str, checked: bool, enabled: bool) -> Response {
    let dark = ui.visuals().dark_mode;
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(BUTTON), sense);
    let tint = |a: u8| Color32::from_rgba_unmultiplied(14, 124, 102, a);
    let (bg, border) = if !enabled {
        (Color32::TRANSPARENT, Color32::TRANSPARENT)
    } else if resp.is_pointer_button_down_on() {
        (tint(110), tint(180))
    } else if checked {
        (tint(90), tint(180))
    } else if resp.hovered() {
        (tint(45), tint(120))
    } else {
        (Color32::TRANSPARENT, Color32::TRANSPARENT)
    };
    let painter = ui.painter();
    painter.rect(
        rect,
        3,
        bg,
        Stroke::new(1.0, border),
        egui::StrokeKind::Inside,
    );
    let inner = Rect::from_center_size(rect.center(), Vec2::splat(ICON));
    let mut shapes = paint(icon, inner, dark);
    if !enabled {
        for s in &mut shapes {
            fade(s);
        }
    }
    painter.extend(shapes);
    resp.on_hover_text(tip)
}

/// Погасить фигуру недоступной кнопки.
fn fade(shape: &mut Shape) {
    let dim = |c: Color32| c.gamma_multiply(0.35);
    match shape {
        Shape::LineSegment { stroke, .. } => stroke.color = dim(stroke.color),
        Shape::Path(p) => {
            p.fill = dim(p.fill);
            if let egui::epaint::ColorMode::Solid(c) = &mut p.stroke.color {
                *c = dim(*c);
            }
        }
        Shape::Ellipse(e) => {
            e.fill = dim(e.fill);
            e.stroke.color = dim(e.stroke.color);
        }
        _ => {}
    }
}

/// Черта между группами кнопок, как разделитель плашки модуля.
pub fn separator(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(5.0, BUTTON), Sense::hover());
    let c = ui.visuals().widgets.noninteractive.bg_stroke.color;
    ui.painter().line_segment(
        [
            pos2(rect.center().x, rect.min.y + 3.0),
            pos2(rect.center().x, rect.max.y - 3.0),
        ],
        Stroke::new(1.0, c),
    );
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    /// Список модуля разбирается, у каждой кнопки есть значок
    /// и подсказки, каждый значок рисуется в своём квадрате в обеих
    /// темах.
    #[test]
    fn module_icons_paint_inside() {
        let items = items();
        assert!(items.iter().filter(|i| !i.sep).count() > 10);
        let r = Rect::from_min_size(pos2(10.0, 20.0), Vec2::splat(ICON));
        for it in items.iter().filter(|i| !i.sep) {
            assert!(has_icon(&it.icon), "нет значка {}", it.icon);
            assert!(!it.ru.is_empty() && !it.en.is_empty(), "{}", it.key);
        }
        for (name, _) in ICONS {
            assert!(!parsed(name).is_empty(), "{name}: пусто");
            for dark in [false, true] {
                for s in paint(name, r, dark) {
                    let b = s.visual_bounding_rect();
                    assert!(r.expand(3.0).contains_rect(b), "{name}: {b:?}");
                }
            }
        }
    }

    #[test]
    fn svg_subset() {
        let (pts, closed) = path_points("M0,0 L10,0 C10,5 5,10 0,10 Z");
        assert!(closed);
        assert_eq!(pts[0], [0.0, 0.0]);
        assert_eq!(pts.len(), 2 + 12);
        assert_eq!(pts.last().copied(), Some([0.0, 10.0]));
        let p = parse(
            r##"<svg viewBox="0 0 18 18"><g fill="#0e7c66" stroke="#1d2b28" stroke-width="1.8"><circle cx="9" cy="9" r="4.5"/><polyline fill="none" points="0,0 18,18 "/></g></svg>"##,
        );
        assert_eq!(p.len(), 2);
        let Prim::Ellipse { c, r, fill, stroke } = &p[0] else {
            panic!("не эллипс: {:?}", p[0]);
        };
        assert_eq!((*c, *r), ([0.5, 0.5], [0.25, 0.25]));
        assert_eq!(*fill, Some(Color32::from_rgb(0x0e, 0x7c, 0x66)));
        let (sc, sw) = stroke.unwrap_or_default();
        assert_eq!(sc, Color32::from_rgb(0x1d, 0x2b, 0x28));
        assert!((sw - 0.1).abs() < 1e-6, "{sw}");
        assert_eq!(tone(Color32::from_rgb(0x1d, 0x2b, 0x28), true).r(), 0xdd);
    }
}
