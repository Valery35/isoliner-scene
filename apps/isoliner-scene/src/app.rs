// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Окно сцены: панель частей и видов слева, кадр в середине.
//!
//! Камера хранится в настоящих координатах, как в пакете. В координаты
//! просмотра она переводится на каждом кадре через [`Frame`]: так
//! смена преувеличения не сдвигает точку взгляда.

use std::path::Path;
use std::sync::{Arc, Mutex};

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

/// Подсказка по мыши, как `CAMERA_HINT` окна модуля.
const HINT: &str = "Левая кнопка - вращение, Ctrl + левая или средняя - сдвиг, колесо - масштаб, Ctrl + колесо - угол обзора, щелчок правой - центр вращения, щелчок левой - опрос точки.";

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

/// Файл, пришедший в окно: имя и содержимое. В браузере пакет приходит
/// так из выбора файла, который отвечает позже, на одном из кадров.
pub struct Incoming {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Файл, выбранный в браузере, ждёт здесь ближайшего кадра.
type Inbox = Arc<Mutex<Option<Incoming>>>;

/// Точка опроса в настоящих координатах.
struct Probe {
    point: [f64; 3],
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
    /// Путь или имя открытого пакета.
    name: Option<String>,
    inbox: Inbox,
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
    /// Вращение кнопкой «Вращение».
    spin: bool,
    /// Градиентный фон, как `state.bg_grad` окна модуля.
    gradient: bool,
    probe: Option<Probe>,
    target: Option<Target>,
    tex: Option<egui::TextureId>,
    dirty: bool,
    status: String,
}

impl SceneApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
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
        Self {
            lang,
            gpu,
            name: None,
            inbox: Arc::new(Mutex::new(None)),
            pkg: None,
            rows: Vec::new(),
            scene: None,
            frame: Frame::default(),
            cam: Camera::default(),
            vex: 1.0,
            opacity: 1.0,
            views: Vec::new(),
            anim: None,
            spin: false,
            gradient: true,
            probe: None,
            target: None,
            tex: None,
            dirty: true,
            status: String::new(),
        }
    }

    fn t(&self, ru: &'static str) -> &'static str {
        tr(self.lang, ru)
    }

    /// Открыть пакет с диска.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(&mut self, path: &Path) {
        match std::fs::read(path) {
            Ok(b) => self.open_bytes(path.display().to_string(), &b),
            Err(e) => self.status = fill(self.t("Не открылось: %s"), &[e.to_string()]),
        }
    }

    /// Открыть пакет из байтов: перетаскивание и выбор файла в браузере.
    pub fn open_bytes(&mut self, name: String, bytes: &[u8]) {
        match read_package(bytes) {
            Ok(pkg) => self.load(pkg, name),
            Err(e) => self.status = fill(self.t("Не открылось: %s"), &[e.to_string()]),
        }
    }

    /// Выбор файла. В браузере ответ приходит позже, через `inbox`.
    fn pick(&mut self, ctx: &egui::Context) {
        let filter = self.t("Пакет сцены Isoliner3D");
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = ctx;
            if let Some(p) = rfd::FileDialog::new()
                .add_filter(filter, &["i3dscene"])
                .pick_file()
            {
                self.open(&p);
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let dialog = rfd::AsyncFileDialog::new().add_filter(filter, &["i3dscene"]);
            let inbox = self.inbox.clone();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(f) = dialog.pick_file().await {
                    let file = Incoming {
                        name: f.file_name(),
                        bytes: f.read().await,
                    };
                    if let Ok(mut slot) = inbox.lock() {
                        *slot = Some(file);
                    }
                    ctx.request_repaint();
                }
            });
        }
    }

    fn load(&mut self, mut pkg: Package, name: String) {
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
        // градиент в окне модуля включён по умолчанию
        self.gradient = pkg
            .meta
            .state
            .get("bg_grad")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        self.scene = self
            .gpu
            .as_ref()
            .map(|g| GpuScene::upload(&g.device, &pkg.parts, self.frame));
        // показ частей как в окне QGIS: короб выключен - оформление скрыто
        if let Some(scene) = self.scene.as_mut() {
            for (p, row) in scene.parts.iter_mut().zip(&self.rows) {
                p.visible = pkg.meta.part_shown(row.info.as_ref());
            }
        }
        self.pkg = Some(pkg);
        self.status.clone_from(&name);
        self.name = Some(name);
        self.reset_camera(1280.0, 800.0);
        self.anim = None;
        self.probe = None;
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
        let name = self
            .name
            .as_deref()
            .and_then(|n| Path::new(n).file_name())
            .map_or_else(
                || "scene.i3dscene".to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
        let mut out = pkg.clone();
        out.meta.camera = Some(self.cam);
        out.meta.views = self.views.clone();
        out.meta.vex = self.vex;
        if let Some(state) = out.meta.state.as_object_mut() {
            state.insert("bg_grad".into(), self.gradient.into());
        }
        let bytes = match write_package(&out) {
            Ok(b) => b,
            Err(e) => {
                self.status = fill(self.t("Не записалось: %s"), &[e.to_string()]);
                return;
            }
        };
        #[cfg(not(target_arch = "wasm32"))]
        let res = {
            let Some(dest) = rfd::FileDialog::new()
                .add_filter(self.t("Пакет сцены Isoliner3D"), &["i3dscene"])
                .set_file_name(name)
                .save_file()
            else {
                return;
            };
            std::fs::write(&dest, bytes)
                .map(|()| dest.display().to_string())
                .map_err(|e| e.to_string())
        };
        // в браузере пакет скачивается в папку загрузок
        #[cfg(target_arch = "wasm32")]
        let res = crate::web::download(&name, &bytes).map(|()| name);
        self.status = match res {
            Ok(dest) => fill(self.t("Записано: %s"), &[dest]),
            Err(e) => fill(self.t("Не записалось: %s"), &[e]),
        };
    }

    /// Охват для кадра, как окно модуля.
    fn span(&self) -> f64 {
        self.pkg
            .as_ref()
            .and_then(|p| mesh::bounds(&p.parts))
            .map_or(0.0, |(lo, hi)| camera::view_span(lo, hi, self.vex))
    }

    /// Попадание луча из точки кадра (точки egui от угла кадра
    /// `w` x `h`) в видимые тела.
    fn hit_at(&self, px: f64, py: f64, w: f64, h: f64) -> Option<mesh::Hit> {
        let pkg = self.pkg.as_ref()?;
        let scene = self.scene.as_ref()?;
        let (o, d) = camera::ray(&self.view_camera(), w, h, px, py)?;
        let visible = pkg
            .parts
            .iter()
            .enumerate()
            .filter(|(i, _)| scene.parts.get(*i).is_some_and(|p| p.visible));
        mesh::raycast(visible, &self.frame_now(), o, d)
    }

    /// Щелчок пришёлся за пределы площадки (`_outside_scene` окна
    /// модуля): луч до уровня середины сцены, запас в четверть охвата.
    fn outside(&self, px: f64, py: f64, w: f64, h: f64) -> bool {
        let Some((o, d)) = camera::ray(&self.view_camera(), w, h, px, py) else {
            return true;
        };
        if d.z.abs() < 1e-12 {
            return true;
        }
        let t = -o.z / d.z;
        if t < 0.0 {
            return true;
        }
        let p = o + d * t;
        let Some((lo, hi)) = self.pkg.as_ref().and_then(|p| mesh::bounds(&p.parts)) else {
            return true;
        };
        let edge = 0.5 * (hi[0] - lo[0]).max(hi[1] - lo[1]) * 1.25;
        p.x.abs() > edge || p.y.abs() > edge
    }

    /// Щелчок правой: центр вращения в точку сцены (`_pivot_at`).
    fn pivot_at(&mut self, px: f64, py: f64, w: f64, h: f64) {
        if self.pkg.is_none() {
            return;
        }
        let view = self.view_camera();
        match self.hit_at(px, py, w, h) {
            Some(hit) => {
                self.set_view_camera(camera::center_keeping_view(&view, hit.point));
                let p = self.frame_now().to_world(hit.point);
                self.status = fill(
                    self.t("Центр вращения: %s, %s, отметка %s м."),
                    &[grouped(p[0], 1), grouped(p[1], 1), format!("{:.2}", p[2])],
                );
            }
            None if self.outside(px, py, w, h) => {
                self.set_view_camera(camera::center_keeping_view(&view, [0.0; 3]));
                self.status = self.t("Центр вращения - вся сцена.").to_string();
            }
            None => {
                self.status = self
                    .t("Мимо объекта: центр не изменён. Щелчок за пределами площадки вернёт центр на всю сцену.")
                    .to_string();
            }
        }
        self.anim = None;
    }

    /// Щелчок левой: опрос точки. Мимо - опрос снимается.
    fn probe_at(&mut self, px: f64, py: f64, w: f64, h: f64) {
        let Some(hit) = self.hit_at(px, py, w, h) else {
            if self.probe.take().is_some() {
                self.status.clear();
            }
            return;
        };
        let p = self.frame_now().to_world(hit.point);
        let name = self
            .scene
            .as_ref()
            .and_then(|s| s.parts.get(hit.part))
            .map_or(String::new(), |q| q.name.clone());
        let mut text = fill(
            self.t("%s: X %s, Y %s, отметка %s м"),
            &[
                name,
                grouped(p[0], 1),
                grouped(p[1], 1),
                format!("{:.2}", p[2]),
            ],
        );
        if let Some(v) = self.rows.get(hit.part).and_then(|r| r.volume) {
            text.push_str(", ");
            text.push_str(&fill(self.t("объём %s м3"), &[grouped(v, 1)]));
        }
        self.status = text;
        self.probe = Some(Probe { point: p });
    }

    /// Стрелки вращают, как в окне модуля: 2 градуса на 16 мс.
    fn keys(&mut self, ui: &egui::Ui) {
        if ui.ctx().egui_wants_keyboard_input() {
            return;
        }
        let (dt, l, r, u, d) = ui.input(|i| {
            (
                f64::from(i.stable_dt.min(0.1)),
                i.key_down(egui::Key::ArrowLeft),
                i.key_down(egui::Key::ArrowRight),
                i.key_down(egui::Key::ArrowUp),
                i.key_down(egui::Key::ArrowDown),
            )
        });
        let speed = 125.0 * dt;
        let azim = (f64::from(u8::from(l)) - f64::from(u8::from(r))) * speed;
        let elev = (f64::from(u8::from(d)) - f64::from(u8::from(u))) * speed;
        if azim != 0.0 || elev != 0.0 {
            self.cam.azimuth += azim;
            self.cam.elevation = (self.cam.elevation + elev).clamp(-90.0, 90.0);
            self.anim = None;
            self.dirty = true;
            ui.ctx().request_repaint();
        }
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
            if ui.button(self.t("Открыть")).clicked() {
                self.pick(ui.ctx());
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
        // кнопки плашки окна модуля
        ui.horizontal(|ui| {
            if ui.button(self.t("Вид сверху")).clicked() {
                let v = camera::top(&self.view_camera());
                self.set_view_camera(v);
                self.anim = None;
            }
            if ui.button(self.t("Сбросить вид")).clicked() {
                let v = camera::reset(&self.view_camera(), self.span());
                self.set_view_camera(v);
                self.anim = None;
            }
            let label = self.t("Вращение");
            ui.toggle_value(&mut self.spin, label);
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
        let label = self.t("Градиентный фон");
        if ui.checkbox(&mut self.gradient, label).changed() {
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

        // Мышь как в окне модуля (`_PickView`, `GLViewWidget`): левая -
        // вращать, Ctrl + левая - сдвиг в плоскости экрана, средняя -
        // сдвиг по плану, Ctrl + средняя - вбок и вверх, колесо -
        // удаление, Ctrl + колесо - раствор. Правая с протяжкой тоже
        // сдвигает: в окне модуля она свободна. Пиксели мыши - точки
        // egui, как логические пиксели Qt.
        let (w, h) = (f64::from(pw), f64::from(ph));
        let (wpt, hpt) = (f64::from(rect.width()), f64::from(rect.height()));
        let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
        let d = resp.drag_delta();
        let (dx, dy) = (f64::from(d.x), f64::from(d.y));
        if d != egui::Vec2::ZERO {
            let mut v = self.view_camera();
            let mut moved = true;
            if resp.dragged_by(egui::PointerButton::Primary) {
                if ctrl {
                    camera::pan(&mut v, dx, dy, wpt);
                } else {
                    camera::orbit(&mut v, dx, dy);
                }
            } else if resp.dragged_by(egui::PointerButton::Middle) {
                if ctrl {
                    camera::pan_upright(&mut v, dx, 0.0, dy, wpt);
                } else {
                    camera::pan_upright(&mut v, dx, dy, 0.0, wpt);
                }
            } else if resp.dragged_by(egui::PointerButton::Secondary) {
                camera::pan(&mut v, dx, dy, wpt);
            } else {
                moved = false;
            }
            if moved {
                self.set_view_camera(v);
                self.anim = None;
            }
        }
        if resp.hovered() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            if scroll != 0.0 {
                let mut v = self.view_camera();
                camera::zoom(&mut v, f64::from(scroll) / 50.0);
                self.set_view_camera(v);
                self.anim = None;
            }
            // Ctrl + колесо egui отдаёт масштабом: раствор делится на него
            if zoom != 1.0 && zoom > 0.0 {
                let mut v = self.view_camera();
                camera::zoom_fov(&mut v, f64::from(zoom).ln() / (-120.0 * 0.999f64.ln()));
                self.set_view_camera(v);
                self.anim = None;
            }
        }
        self.keys(ui);
        if self.spin {
            // вращение окна модуля: градус на 40 мс
            let dt = f64::from(ui.input(|i| i.stable_dt).min(0.1));
            self.cam.azimuth = (self.cam.azimuth + 25.0 * dt).rem_euclid(360.0);
            self.dirty = true;
            ui.ctx().request_repaint();
        }
        if let Some(pos) = resp.interact_pointer_pos() {
            let (px, py) = (f64::from(pos.x - rect.min.x), f64::from(pos.y - rect.min.y));
            if resp.secondary_clicked() {
                self.pivot_at(px, py, wpt, hpt);
            } else if resp.clicked() {
                self.probe_at(px, py, wpt, hpt);
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
                    background: if self.gradient {
                        render::BG_GRADIENT
                    } else {
                        render::BG_PLAIN
                    },
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
        // маркер опроса - красная точка, как шарик окна модуля
        if let Some(p) = &self.probe {
            let v = self.frame_now().to_view(p.point);
            if let Some([sx, sy]) = camera::to_screen(&self.view_camera(), wpt, hpt, v) {
                let at = rect.min + egui::vec2(sx as f32, sy as f32);
                if rect.contains(at) {
                    let painter = ui.painter();
                    painter.circle_filled(at, 5.0, egui::Color32::from_rgb(217, 38, 38));
                    painter.circle_stroke(at, 5.0, egui::Stroke::new(1.5, egui::Color32::WHITE));
                }
            }
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
        // перетащенный файл: на диске - путь, в браузере - байты
        let dropped = ui.ctx().input(|i| i.raw.dropped_files.first().cloned());
        if let Some(f) = dropped {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let p = f.path();
                if !p.as_os_str().is_empty() {
                    let p = p.to_path_buf();
                    self.open(&p);
                }
            }
            // браузер читает файл не сразу: ответ приходит через inbox
            #[cfg(target_arch = "wasm32")]
            {
                let inbox = self.inbox.clone();
                let ctx = ui.ctx().clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let name = f.path().display().to_string();
                    let read = f.bytes_async().await;
                    let file = match read {
                        Ok(bytes) => Incoming { name, bytes },
                        // пустой файл - пакет не прочтётся, и окно скажет об этом
                        Err(_) => Incoming {
                            name,
                            bytes: Vec::new(),
                        },
                    };
                    if let Ok(mut slot) = inbox.lock() {
                        *slot = Some(file);
                    }
                    ctx.request_repaint();
                });
            }
        }
        let picked = self.inbox.lock().ok().and_then(|mut slot| slot.take());
        if let Some(file) = picked {
            self.open_bytes(file.name, &file.bytes);
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
                // ответ опроса важнее подсказки: он идёт первым
                if !self.status.is_empty() {
                    ui.separator();
                    ui.label(&self.status);
                }
                ui.separator();
                ui.weak(self.t(HINT));
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
