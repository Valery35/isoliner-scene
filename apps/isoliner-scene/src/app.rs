// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Окно сцены: панель частей и видов слева, кадр в середине.
//!
//! Камера хранится в настоящих координатах, как в пакете. В координаты
//! просмотра она переводится на каждом кадре через [`Frame`]: так
//! смена преувеличения не сдвигает точку взгляда.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui;
use eframe::egui_wgpu;

use render::{GpuScene, Look, Renderer, Target};
use scene_core::camera::{self, fit};
use scene_core::{Camera, Frame, flight, mesh};
use scene_format::{Package, Part, PartInfo, read_package, write_package};

use crate::i18n::{Lang, tr};

/// Подставить значения в шаблон по порядку: `%s`, `%d`, `%.0f`.
fn fill(template: &str, args: &[String]) -> String {
    let mut out = String::new();
    let mut it = args.iter();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        while chars
            .peek()
            .is_some_and(|d| *d == '.' || d.is_ascii_digit())
        {
            chars.next();
        }
        chars.next();
        if let Some(a) = it.next() {
            out.push_str(a);
        }
    }
    out
}

/// Число с пробелами по тысячам: 12 345.6.
fn grouped(x: f64, decimals: usize) -> String {
    let s = format!("{:.*}", decimals, x.abs());
    let (int, frac) = s.split_once('.').unwrap_or((&s, ""));
    let mut g = String::new();
    for (i, ch) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            g.push(' ');
        }
        g.push(ch);
    }
    let sign = if x < 0.0 { "-" } else { "" };
    if frac.is_empty() {
        format!("{sign}{g}")
    } else {
        format!("{sign}{g}.{frac}")
    }
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Renderer,
    egui: Arc<egui::mutex::RwLock<egui_wgpu::Renderer>>,
}

/// Кадры анимации камеры в настоящих координатах.
struct Anim {
    frames: Vec<Camera>,
    start: f64,
    fps: u32,
}

/// Строка списка частей.
struct Row {
    info: Option<PartInfo>,
    volume: Option<f64>,
    swatch: [f32; 4],
}

pub struct SceneApp {
    lang: Lang,
    gpu: Option<Gpu>,
    path: Option<PathBuf>,
    pkg: Option<Package>,
    rows: Vec<Row>,
    scene: Option<GpuScene>,
    frame: Frame,
    /// Камера в настоящих координатах.
    cam: Camera,
    vex: f64,
    opacity: f32,
    views: Vec<Camera>,
    anim: Option<Anim>,
    target: Option<Target>,
    tex: Option<egui::TextureId>,
    dirty: bool,
    status: String,
}

impl SceneApp {
    pub fn new(cc: &eframe::CreationContext<'_>, path: Option<PathBuf>) -> Self {
        let lang = cc
            .storage
            .and_then(|s| s.get_string("lang"))
            .map_or_else(Lang::system, |c| Lang::from_code(&c));
        let gpu = cc.wgpu_render_state.as_ref().map(|rs| Gpu {
            device: rs.device.clone(),
            queue: rs.queue.clone(),
            renderer: Renderer::new(&rs.device),
            egui: rs.renderer.clone(),
        });
        let mut app = Self {
            lang,
            gpu,
            path: None,
            pkg: None,
            rows: Vec::new(),
            scene: None,
            frame: Frame::default(),
            cam: Camera::default(),
            vex: 1.0,
            opacity: 1.0,
            views: Vec::new(),
            anim: None,
            target: None,
            tex: None,
            dirty: true,
            status: String::new(),
        };
        if let Some(p) = path {
            app.open(&p);
        }
        app
    }

    fn t(&self, ru: &'static str) -> &'static str {
        tr(self.lang, ru)
    }

    fn open(&mut self, path: &Path) {
        let res = std::fs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|b| read_package(&b).map_err(|e| e.to_string()));
        match res {
            Ok(pkg) => self.load(pkg, path),
            Err(e) => self.status = fill(self.t("Не открылось: %s"), &[e]),
        }
    }

    fn load(&mut self, mut pkg: Package, path: &Path) {
        // пустые части в видеокарту не идут: список строк и части
        // в видеокарте должны совпадать по номерам
        let infos = pkg.meta.parts.clone();
        let mut kept: Vec<Part> = Vec::new();
        self.rows.clear();
        for (node, part) in pkg.parts.drain(..).enumerate() {
            if part.verts.is_empty() || part.indices.is_empty() {
                continue;
            }
            self.rows.push(Row {
                info: infos.iter().find(|i| i.node == node).cloned(),
                volume: mesh::body_volume(&part),
                swatch: part
                    .colors
                    .as_ref()
                    .and_then(|c| c.first().copied())
                    .unwrap_or(render::DEFAULT_COLOR),
            });
            kept.push(part);
        }
        pkg.parts = kept;
        self.vex = if pkg.meta.vex > 0.0 {
            pkg.meta.vex
        } else {
            1.0
        };
        self.frame = match mesh::bounds(&pkg.parts) {
            Some((lo, hi)) => Frame::around(lo, hi, self.vex),
            None => Frame::default(),
        };
        self.views = pkg.meta.views.clone();
        self.scene = self
            .gpu
            .as_ref()
            .map(|g| GpuScene::upload(&g.device, &pkg.parts, self.frame));
        self.pkg = Some(pkg);
        self.path = Some(path.to_path_buf());
        self.reset_camera(1280.0, 800.0);
        self.anim = None;
        self.status = path.display().to_string();
        self.dirty = true;
    }

    fn reset_camera(&mut self, w: f64, h: f64) {
        let Some(pkg) = &self.pkg else { return };
        match pkg.meta.camera {
            Some(c) if c.distance > 0.0 => self.cam = c,
            _ => self.fit(w, h),
        }
    }

    fn fit(&mut self, w: f64, h: f64) {
        let Some(pkg) = &self.pkg else { return };
        let Some((lo, hi)) = mesh::bounds(&pkg.parts) else {
            return;
        };
        let view = self.view_camera();
        let fitted = fit(&view, self.frame.to_view(lo), self.frame.to_view(hi), w, h);
        self.cam = self.frame.camera_to_world(&fitted);
        self.dirty = true;
    }

    fn frame_now(&self) -> Frame {
        Frame {
            vex: self.vex,
            ..self.frame
        }
    }

    fn view_camera(&self) -> Camera {
        self.frame_now().camera_to_view(&self.cam)
    }

    fn set_view_camera(&mut self, view: Camera) {
        self.cam = self.frame_now().camera_to_world(&view);
        self.dirty = true;
    }

    fn save_as(&mut self) {
        let Some(pkg) = &self.pkg else { return };
        let name = self.path.as_ref().and_then(|p| p.file_name()).map_or_else(
            || "scene.i3dscene".to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let Some(dest) = rfd::FileDialog::new()
            .add_filter(self.t("Пакет сцены Isoliner3D"), &["i3dscene"])
            .set_file_name(name)
            .save_file()
        else {
            return;
        };
        let mut out = pkg.clone();
        out.meta.camera = Some(self.cam);
        out.meta.views = self.views.clone();
        out.meta.vex = self.vex;
        let res = write_package(&out)
            .map_err(|e| e.to_string())
            .and_then(|b| std::fs::write(&dest, b).map_err(|e| e.to_string()));
        self.status = match res {
            Ok(()) => fill(self.t("Записано: %s"), &[dest.display().to_string()]),
            Err(e) => fill(self.t("Не записалось: %s"), &[e]),
        };
    }

    fn start(&mut self, frames: Vec<Camera>, now: f64) {
        if !frames.is_empty() {
            self.anim = Some(Anim {
                frames,
                start: now,
                fps: flight::FPS,
            });
        }
    }

    fn fly_to(&mut self, to: Camera, now: f64) {
        let frames = flight::tour(&[self.cam, to], 1.2, 0.0, flight::FPS);
        self.start(frames, now);
    }

    fn panel(&mut self, ui: &mut egui::Ui, now: f64, size: egui::Vec2) {
        let (w, h) = (f64::from(size.x.max(1.0)), f64::from(size.y.max(1.0)));
        ui.horizontal(|ui| {
            if ui.button(self.t("Открыть")).clicked()
                && let Some(p) = rfd::FileDialog::new()
                    .add_filter(self.t("Пакет сцены Isoliner3D"), &["i3dscene"])
                    .pick_file()
            {
                self.open(&p);
            }
            if ui
                .add_enabled(
                    self.pkg.is_some(),
                    egui::Button::new(self.t("Сохранить как")),
                )
                .clicked()
            {
                self.save_as();
            }
            let other = if self.lang == Lang::Ru { "EN" } else { "RU" };
            if ui.button(other).clicked() {
                self.lang = if self.lang == Lang::Ru {
                    Lang::En
                } else {
                    Lang::Ru
                };
            }
        });
        if self.pkg.is_none() {
            ui.separator();
            ui.label(self.t("Перетащите пакет .i3dscene в окно или нажмите «Открыть»."));
            return;
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(self.t("Вписать")).clicked() {
                self.fit(w, h);
            }
            if ui.button(self.t("Камера пакета")).clicked()
                && let Some(c) = self.pkg.as_ref().and_then(|p| p.meta.camera)
            {
                self.fly_to(c, now);
            }
        });
        let mut ortho = self.cam.ortho;
        if ui
            .checkbox(&mut ortho, self.t("Параллельная проекция"))
            .changed()
        {
            self.cam.ortho = ortho;
            self.dirty = true;
        }
        let mut vex = self.vex;
        if ui
            .add(
                egui::Slider::new(&mut vex, 0.1..=50.0)
                    .logarithmic(true)
                    .text(self.t("Преувеличение")),
            )
            .changed()
        {
            // точка взгляда остаётся на месте в настоящих координатах
            self.vex = vex;
            self.dirty = true;
        }
        let label = self.t("Непрозрачность");
        if ui
            .add(egui::Slider::new(&mut self.opacity, 0.05..=1.0).text(label))
            .changed()
        {
            self.dirty = true;
        }

        ui.separator();
        ui.heading(self.t("Части"));
        let lang = self.lang;
        egui::ScrollArea::vertical()
            .max_height(ui.available_height() * 0.45)
            .id_salt("parts")
            .show(ui, |ui| {
                if let Some(scene) = self.scene.as_mut() {
                    for (p, row) in scene.parts.iter_mut().zip(&self.rows) {
                        ui.horizontal(|ui| {
                            let [r, g, b, a] = row.swatch;
                            let col = egui::Color32::from_rgba_unmultiplied(
                                (r * 255.0) as u8,
                                (g * 255.0) as u8,
                                (b * 255.0) as u8,
                                (a * 255.0) as u8,
                            );
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 2.0, col);
                            if ui.checkbox(&mut p.visible, &p.name).changed() {
                                self.dirty = true;
                            }
                        });
                        let mut note = if p.lines {
                            tr(lang, "линии").to_string()
                        } else {
                            tr(lang, "тело").to_string()
                        };
                        if row.info.as_ref().is_some_and(|i| i.decor) {
                            note = tr(lang, "оформление").to_string();
                        } else if !p.lines {
                            note = match row.volume {
                                Some(v) => fill(tr(lang, "объём %s м3"), &[grouped(v, 1)]),
                                None => tr(lang, "не замкнуто").to_string(),
                            };
                        }
                        ui.weak(format!("    {note}"));
                    }
                }
            });

        ui.separator();
        ui.heading(self.t("Виды"));
        let mut go: Option<Camera> = None;
        let mut drop: Option<usize> = None;
        for (i, v) in self.views.iter().enumerate() {
            ui.horizontal(|ui| {
                if ui
                    .button(fill(tr(lang, "Вид %d"), &[(i + 1).to_string()]))
                    .clicked()
                {
                    go = Some(*v);
                }
                if ui
                    .small_button(crate::i18n::ICON_REMOVE)
                    .on_hover_text(tr(lang, "Убрать вид"))
                    .clicked()
                {
                    drop = Some(i);
                }
            });
        }
        if let Some(v) = go {
            self.fly_to(v, now);
        }
        if let Some(i) = drop {
            self.views.remove(i);
        }
        ui.horizontal(|ui| {
            if ui.button(self.t("Добавить вид")).clicked() {
                self.views.push(self.cam);
            }
            if ui
                .add_enabled(
                    self.views.len() > 1,
                    egui::Button::new(self.t("Облёт по видам")),
                )
                .clicked()
            {
                let frames = flight::tour(&self.views, 4.0, 1.0, flight::FPS);
                self.start(frames, now);
            }
            if ui.button(self.t("Оборот")).clicked() {
                let frames = flight::orbit(&self.cam, 360.0, 12.0, flight::FPS);
                self.start(frames, now);
            }
            if self.anim.is_some() && ui.button(self.t("Стоп")).clicked() {
                self.anim = None;
            }
        });

        ui.separator();
        if let Some(pkg) = &self.pkg {
            let m = &pkg.meta;
            egui::Grid::new("info").num_columns(2).show(ui, |ui| {
                ui.label(tr(lang, "Сцена"));
                ui.label(&m.name);
                ui.end_row();
                ui.label(tr(lang, "Система координат"));
                ui.label(if m.crs.authid.is_empty() {
                    tr(lang, "без системы")
                } else {
                    &m.crs.authid
                });
                ui.end_row();
                ui.label(tr(lang, "Точка начала"));
                ui.label(format!(
                    "{}, {}",
                    grouped(m.origin[0], 0),
                    grouped(m.origin[1], 0)
                ));
                ui.end_row();
                ui.label(tr(lang, "Записал"));
                ui.label(format!("{} {}", m.module, m.version));
                ui.end_row();
            });
        }
    }

    /// Кадр: текстура по размеру области, отрисовка, мышь.
    fn view(&mut self, ui: &mut egui::Ui, now: f64) {
        let rect = ui.max_rect();
        let ppp = ui.ctx().pixels_per_point();
        let pw = (rect.width() * ppp).round().max(1.0) as u32;
        let ph = (rect.height() * ppp).round().max(1.0) as u32;
        let resp = ui.allocate_rect(rect, egui::Sense::click_and_drag());

        if self.gpu.is_none() {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                self.t("Видеокарта недоступна: окно не может рисовать сцену."),
                egui::FontId::proportional(16.0),
                ui.visuals().text_color(),
            );
            return;
        }
        if let Some(gpu) = &self.gpu
            && self
                .target
                .as_ref()
                .is_none_or(|t| t.width != pw || t.height != ph)
        {
            let target = Target::new(&gpu.device, pw, ph);
            let mut eg = gpu.egui.write();
            match self.tex {
                Some(id) => eg.update_egui_texture_from_wgpu_texture(
                    &gpu.device,
                    &target.color_view,
                    wgpu::FilterMode::Linear,
                    id,
                ),
                None => {
                    self.tex = Some(eg.register_native_texture(
                        &gpu.device,
                        &target.color_view,
                        wgpu::FilterMode::Linear,
                    ));
                }
            }
            self.target = Some(target);
            self.dirty = true;
        }

        // мышь: левая - вращать, правая и средняя - сдвигать, колесо - удаление
        let (w, h) = (f64::from(pw), f64::from(ph));
        let d = resp.drag_delta();
        if resp.dragged_by(egui::PointerButton::Primary) && d != egui::Vec2::ZERO {
            let mut v = self.view_camera();
            camera::orbit(&mut v, f64::from(d.x), f64::from(d.y));
            self.set_view_camera(v);
            self.anim = None;
        } else if (resp.dragged_by(egui::PointerButton::Secondary)
            || resp.dragged_by(egui::PointerButton::Middle))
            && d != egui::Vec2::ZERO
        {
            let mut v = self.view_camera();
            camera::pan(&mut v, f64::from(d.x * ppp), f64::from(d.y * ppp), h);
            self.set_view_camera(v);
            self.anim = None;
        }
        if resp.hovered() {
            let scroll = ui.ctx().input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let mut v = self.view_camera();
                camera::zoom(&mut v, f64::from(scroll) / 50.0);
                self.set_view_camera(v);
                self.anim = None;
            }
        }
        if resp.double_clicked() {
            self.fit(w, h);
        }

        if let Some(anim) = &self.anim {
            let k = ((now - anim.start) * f64::from(anim.fps)).floor().max(0.0) as usize;
            match anim.frames.get(k) {
                Some(c) => {
                    self.cam = *c;
                    self.dirty = true;
                }
                None => {
                    if let Some(c) = anim.frames.last() {
                        self.cam = *c;
                    }
                    self.anim = None;
                    self.dirty = true;
                }
            }
            ui.ctx().request_repaint();
        }

        if self.dirty {
            if let (Some(gpu), Some(scene), Some(target)) = (&self.gpu, &self.scene, &self.target) {
                let look = Look {
                    camera: self.view_camera(),
                    vex: self.vex,
                    opacity: self.opacity,
                };
                gpu.renderer
                    .render(&gpu.device, &gpu.queue, target, scene, &look);
            }
            self.dirty = false;
        }
        if let Some(id) = self.tex {
            ui.painter().image(
                id,
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        if self.pkg.is_none() {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                self.t("Перетащите пакет .i3dscene в окно или нажмите «Открыть»."),
                egui::FontId::proportional(16.0),
                egui::Color32::from_gray(90),
            );
        }
    }
}

impl eframe::App for SceneApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let now = ui.ctx().input(|i| i.time);
        let dropped: Option<PathBuf> = ui.ctx().input(|i| {
            i.raw.dropped_files.iter().find_map(|f| {
                let p = f.path();
                (!p.as_os_str().is_empty()).then(|| p.to_path_buf())
            })
        });
        if let Some(p) = dropped {
            self.open(&p);
        }
        let view_size = self.target.as_ref().map_or(egui::vec2(1280.0, 800.0), |t| {
            egui::vec2(t.width as f32, t.height as f32)
        });
        egui::Panel::left("panel")
            .resizable(true)
            .default_size(300.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.panel(ui, now, view_size));
            });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                let c = self.cam;
                ui.label(fill(
                    self.t("Азимут %.0f°, наклон %.0f°, удаление %s м"),
                    &[
                        format!("{:.0}", c.azimuth.rem_euclid(360.0)),
                        format!("{:.0}", c.elevation),
                        grouped(c.distance, 0),
                    ],
                ));
                ui.separator();
                ui.weak(
                    self.t("Левая кнопка - вращать, правая - сдвигать, колесо - ближе и дальше."),
                );
                ui.separator();
                ui.weak(&self.status);
            });
        });
        egui::CentralPanel::no_frame().show(ui, |ui| self.view(ui, now));
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("lang", self.lang.code().to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_and_numbers() {
        assert_eq!(fill("Вид %d", &["3".into()]), "Вид 3");
        assert_eq!(
            fill("%.0f° и %s м", &["10".into(), "5".into()]),
            "10° и 5 м"
        );
        assert_eq!(grouped(1234567.25, 1), "1 234 567.2");
        assert_eq!(grouped(-512000.0, 0), "-512 000");
        assert_eq!(grouped(12.0, 0), "12");
    }
}
