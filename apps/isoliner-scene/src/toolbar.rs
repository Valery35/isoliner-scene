// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Плашка значков поверх сцены, как у окна модуля: значки нарисованы
//! кодом по `viewer3d._tool_icon` модуля, те же фигуры в том же
//! квадрате, полупрозрачная плашка в левом верхнем углу сцены.
//! Файлов нет: значки одинаково выглядят в светлой и тёмной теме.

use eframe::egui;
use egui::{Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2};

/// Значок кнопки.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Reload,
    Top,
    Home,
    Section,
    Draw,
    Undo,
    Done,
    Line,
    Eye,
    Clear,
    Export,
    Spin,
    Views,
    Frames,
    Copy,
    Png,
    Params,
}

/// Все значки: для проверки, что каждый рисуется.
#[cfg(test)]
pub const ALL: [Icon; 17] = [
    Icon::Reload,
    Icon::Top,
    Icon::Home,
    Icon::Section,
    Icon::Draw,
    Icon::Undo,
    Icon::Done,
    Icon::Line,
    Icon::Eye,
    Icon::Clear,
    Icon::Export,
    Icon::Spin,
    Icon::Views,
    Icon::Frames,
    Icon::Copy,
    Icon::Png,
    Icon::Params,
];

/// Краски значков модуля.
const ORANGE: Color32 = Color32::from_rgb(0xC2, 0x62, 0x2C);
const TEAL: Color32 = Color32::from_rgb(0x0E, 0x7C, 0x66);
const PALE: Color32 = Color32::from_rgb(0xCF, 0xE3, 0xF2);

/// Сторона значка и кнопки, точки egui.
pub const ICON: f32 = 18.0;
pub const BUTTON: f32 = 24.0;

/// Перо значка: квадрат значка и краски темы.
struct Pen<'a> {
    shapes: &'a mut Vec<Shape>,
    o: Pos2,
    s: f32,
    ink: Color32,
    paper: Color32,
    teal: Color32,
}

impl Pen<'_> {
    fn pt(&self, x: f32, y: f32) -> Pos2 {
        pos2(self.o.x + x * self.s, self.o.y + y * self.s)
    }

    fn stroke(&self, c: Color32, w: f32) -> Stroke {
        Stroke::new(w, c)
    }

    fn line(&mut self, a: (f32, f32), b: (f32, f32), c: Color32, w: f32) {
        let (p, q) = (self.pt(a.0, a.1), self.pt(b.0, b.1));
        self.shapes
            .push(Shape::line_segment([p, q], self.stroke(c, w)));
    }

    fn ink_line(&mut self, a: (f32, f32), b: (f32, f32)) {
        self.line(a, b, self.ink, 1.6);
    }

    /// Ломаная, замкнутая или нет.
    fn path(&mut self, pts: &[(f32, f32)], closed: bool, c: Color32, w: f32) {
        let mut v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.pt(x, y)).collect();
        if closed && let Some(&first) = v.first() {
            v.push(first);
        }
        self.shapes.push(Shape::line(v, self.stroke(c, w)));
    }

    /// Прямоугольник по углу и сторонам в долях значка, обвод чернилами.
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, fill: Option<Color32>) {
        let r = Rect::from_min_size(self.pt(x, y), vec2(w * self.s, h * self.s));
        if let Some(f) = fill {
            self.shapes
                .push(Shape::rect_filled(r, egui::CornerRadius::ZERO, f));
        }
        self.shapes.push(Shape::rect_stroke(
            r,
            egui::CornerRadius::ZERO,
            self.stroke(self.ink, 1.6),
            egui::StrokeKind::Middle,
        ));
    }

    fn ellipse(&mut self, c: (f32, f32), rx: f32, ry: f32, fill: Option<Color32>) {
        let (p, r) = (self.pt(c.0, c.1), vec2(rx * self.s, ry * self.s));
        if let Some(f) = fill {
            self.shapes.push(Shape::ellipse_filled(p, r, f));
        }
        self.shapes
            .push(Shape::ellipse_stroke(p, r, self.stroke(self.ink, 1.2)));
    }

    /// Дуга эллипса в прямоугольнике, углы как у Qt: от трёх часов
    /// против часовой стрелки, градусы.
    fn arc(&mut self, b: [f32; 4], start: f32, span: f32, c: Color32) {
        let [x, y, w, h] = b;
        let (cx, cy, rx, ry) = (x + w / 2.0, y + h / 2.0, w / 2.0, h / 2.0);
        let n = 24;
        let pts: Vec<(f32, f32)> = (0..=n)
            .map(|k| {
                let a = (start + span * k as f32 / n as f32).to_radians();
                (cx + rx * a.cos(), cy - ry * a.sin())
            })
            .collect();
        self.path(&pts, false, c, 1.6);
    }

    fn polygon(&mut self, pts: &[(f32, f32)], fill: Color32, outline: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.pt(x, y)).collect();
        self.shapes
            .push(Shape::convex_polygon(v, fill, self.stroke(outline, 1.0)));
    }
}

/// Нарисовать значок в квадрате `rect`. `dark` - тёмная тема:
/// чернила светлые, бумага тёмная, акценты те же.
pub fn paint(icon: Icon, rect: Rect, dark: bool) -> Vec<Shape> {
    let mut shapes = Vec::new();
    let ink = if dark {
        Color32::from_rgb(0xDD, 0xE6, 0xE3)
    } else {
        Color32::from_rgb(0x1D, 0x2B, 0x28)
    };
    let paper = if dark {
        Color32::from_rgb(0x2B, 0x31, 0x30)
    } else {
        Color32::WHITE
    };
    let teal = if dark {
        Color32::from_rgb(0x3F, 0xB8, 0x9C)
    } else {
        TEAL
    };
    let mut p = Pen {
        shapes: &mut shapes,
        o: rect.min,
        s: rect.width().min(rect.height()),
        ink,
        paper,
        teal,
    };
    match icon {
        Icon::Draw => {
            let pts = [
                (0.20, 0.62),
                (0.38, 0.26),
                (0.72, 0.32),
                (0.82, 0.68),
                (0.46, 0.82),
            ];
            p.path(&pts, true, p.ink, 1.6);
            for q in pts {
                let r = 1.7 / p.s;
                p.ellipse(q, r, r, Some(ORANGE));
            }
        }
        Icon::Export => {
            let front = [(0.14, 0.42), (0.52, 0.42), (0.52, 0.80), (0.14, 0.80)];
            p.path(&front, true, p.ink, 1.6);
            p.line((0.58, 0.46), (0.88, 0.18), p.teal, 2.0);
            p.polygon(&[(0.90, 0.10), (0.92, 0.40), (0.62, 0.34)], p.teal, p.teal);
        }
        Icon::Copy => {
            p.rect(0.16, 0.16, 0.50, 0.56, None);
            p.rect(0.34, 0.30, 0.50, 0.56, Some(p.paper));
        }
        Icon::Png => {
            p.rect(0.12, 0.30, 0.76, 0.48, None);
            p.ink_line((0.34, 0.30), (0.42, 0.20));
            p.ink_line((0.42, 0.20), (0.62, 0.20));
            p.ellipse((0.50, 0.55), 0.16, 0.16, Some(PALE));
        }
        Icon::Spin => {
            p.arc([0.18, 0.24, 0.64, 0.52], 30.0, 260.0, p.ink);
            p.line((0.74, 0.30), (0.84, 0.40), ORANGE, 1.6);
            p.line((0.84, 0.40), (0.70, 0.46), ORANGE, 1.6);
        }
        Icon::Frames => {
            for off in [0.10, 0.20, 0.30] {
                p.rect(off, 0.22 + off * 0.5, 0.52, 0.36, None);
            }
            p.ellipse((0.56, 0.58), 0.07, 0.07, Some(ORANGE));
        }
        Icon::Views => {
            p.arc([0.08, 0.30, 0.84, 0.62], 20.0, 140.0, p.ink);
            for q in [(0.20, 0.44), (0.50, 0.30), (0.80, 0.44)] {
                p.ellipse(q, 0.08, 0.08, Some(p.teal));
            }
            p.ellipse((0.50, 0.74), 0.11, 0.11, Some(ORANGE));
        }
        Icon::Top => {
            p.rect(0.18, 0.18, 0.64, 0.64, None);
            p.line((0.50, 0.10), (0.50, 0.90), p.teal, 1.4);
            p.line((0.10, 0.50), (0.90, 0.50), p.teal, 1.4);
        }
        Icon::Eye => {
            p.arc([0.08, 0.24, 0.84, 0.52], 20.0, 140.0, p.ink);
            p.arc([0.08, 0.24, 0.84, 0.52], 200.0, 140.0, p.ink);
            p.ellipse((0.50, 0.50), 0.13, 0.13, Some(ORANGE));
        }
        Icon::Clear => {
            p.path(
                &[(0.22, 0.30), (0.62, 0.22), (0.78, 0.58), (0.36, 0.74)],
                true,
                p.ink,
                1.6,
            );
            p.line((0.20, 0.80), (0.82, 0.18), ORANGE, 2.2);
        }
        Icon::Line => {
            p.ink_line((0.14, 0.78), (0.86, 0.22));
            for t in [0.0, 0.5, 1.0] {
                let x = 0.14 + (0.86 - 0.14) * t;
                let y = 0.78 + (0.22 - 0.78) * t;
                p.line((x - 0.06, y - 0.08), (x + 0.06, y + 0.08), ORANGE, 1.6);
            }
        }
        Icon::Undo => {
            p.ink_line((0.24, 0.50), (0.82, 0.50));
            p.polygon(&[(0.16, 0.50), (0.40, 0.34), (0.40, 0.66)], p.ink, p.ink);
        }
        Icon::Done => {
            p.path(
                &[(0.18, 0.30), (0.52, 0.18), (0.72, 0.44), (0.44, 0.62)],
                true,
                p.ink,
                1.6,
            );
            p.line((0.42, 0.74), (0.56, 0.88), p.teal, 2.2);
            p.line((0.56, 0.88), (0.88, 0.56), p.teal, 2.2);
        }
        Icon::Section => {
            p.rect(0.30, 0.14, 0.40, 0.72, None);
            for k in [0.36, 0.52, 0.68] {
                p.line((0.30, k), (0.70, k), p.teal, 1.4);
            }
            p.line((0.06, 0.50), (0.22, 0.50), p.teal, 1.4);
            p.line((0.16, 0.42), (0.22, 0.50), p.teal, 1.4);
            p.line((0.16, 0.58), (0.22, 0.50), p.teal, 1.4);
        }
        Icon::Home => {
            p.path(
                &[(0.14, 0.50), (0.50, 0.16), (0.86, 0.50)],
                false,
                p.ink,
                1.6,
            );
            p.path(
                &[(0.26, 0.42), (0.26, 0.84), (0.74, 0.84), (0.74, 0.42)],
                false,
                p.ink,
                1.6,
            );
            p.path(
                &[(0.43, 0.84), (0.43, 0.60), (0.57, 0.60), (0.57, 0.84)],
                false,
                p.teal,
                1.4,
            );
        }
        Icon::Reload => {
            p.arc([0.18, 0.18, 0.64, 0.64], 40.0, 280.0, p.ink);
            p.polygon(&[(0.78, 0.20), (0.90, 0.44), (0.64, 0.42)], p.ink, p.ink);
        }
        // три ползунка: параметры сцены, как значок «params» модуля
        Icon::Params => {
            for (y, x) in [(0.26, 0.34), (0.50, 0.64), (0.74, 0.44)] {
                p.ink_line((0.12, y), (0.88, y));
                p.ellipse((x, y), 0.09, 0.09, Some(p.teal));
            }
        }
    }
    shapes
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
pub fn button(ui: &mut Ui, icon: Icon, tip: &str, checked: bool, enabled: bool) -> Response {
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
        Shape::Rect(r) => {
            r.fill = dim(r.fill);
            r.stroke.color = dim(r.stroke.color);
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
mod tests {
    use super::*;

    /// Каждый значок рисуется в своём квадрате, в обеих темах.
    #[test]
    fn every_icon_paints_inside() {
        let r = Rect::from_min_size(pos2(10.0, 20.0), Vec2::splat(ICON));
        for dark in [false, true] {
            for icon in ALL {
                let shapes = paint(icon, r, dark);
                assert!(!shapes.is_empty(), "{icon:?}");
                for s in &shapes {
                    let b = s.visual_bounding_rect();
                    assert!(r.expand(3.0).contains_rect(b), "{icon:?}: {b:?}");
                }
            }
        }
    }
}
