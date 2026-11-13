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

use render::{GpuPart, GpuScene, Look, Renderer, Target};
use scene_core::camera::{self, fit};
use scene_core::clip::{self, Clip, Side};
use scene_core::{Camera, Frame, flight, mesh};
use scene_format::{Package, Part, PartInfo, read_package, write_package};

use crate::i18n::{Lang, tr};
use crate::{share, shot};

mod params;
use params::{BoxOpts, LookOpts};

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
/// Потолок подписей короба, как у окна модуля: больше забили бы сцену.
const LABEL_CAP: usize = 60;

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
    /// Адрес, если пакет пришёл по ссылке `?scene=`: его повторит
    /// ссылка «Поделиться».
    pub url: Option<String>,
}

/// Файл, выбранный или скачанный в браузере, ждёт здесь ближайшего
/// кадра. Ошибка - текст для строки состояния.
type Inbox = Arc<Mutex<Option<Result<Incoming, String>>>>;

/// Самопроверка страницы `?check`: кадры с начала и время начала.
struct Check {
    start: f64,
    frames: u32,
    done: bool,
}

/// Страница программы в браузере: туда ведёт ссылка «Поделиться»
/// из окна на диске.
pub const SITE_APP: &str = "https://valery35.github.io/isoliner-scene/app/";

/// Точка опроса в настоящих координатах.
struct Probe {
    point: [f64; 3],
}

/// Настройки записи кадров, как окно записи видео модуля: оборот
/// на угол за время или облёт по видам с переходом и паузой.
#[derive(Clone, Copy)]
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
struct Film {
    tour: bool,
    degrees: f64,
    seconds: f64,
    leg: f64,
    pause: f64,
    /// Склеить кадры в MP4, если в PATH есть ffmpeg.
    mp4: bool,
}

impl Default for Film {
    fn default() -> Self {
        Self {
            tour: false,
            degrees: 360.0,
            seconds: 12.0,
            leg: 4.0,
            pause: 1.0,
            mp4: true,
        }
    }
}

/// Идущая склейка кадров в MP4: ответ ffmpeg из своего потока.
#[cfg(not(target_arch = "wasm32"))]
struct Joining {
    rx: std::sync::mpsc::Receiver<Result<std::path::PathBuf, crate::video::JoinError>>,
    dir: std::path::PathBuf,
    made: usize,
}

/// Идущая запись кадров в папку.
#[cfg(not(target_arch = "wasm32"))]
struct Recording {
    /// Камеры кадров в настоящих координатах.
    frames: Vec<Camera>,
    next: usize,
    dir: std::path::PathBuf,
    /// Камера до записи: после неё возвращается.
    start: Camera,
    /// Кадр записи своего размера: окно в это время рисует своё.
    target: Target,
    error: Option<String>,
    /// Склеить в MP4 после записи.
    mp4: bool,
}

/// Точки измерения в настоящих координатах.
#[derive(Default)]
struct Ruler {
    pts: Vec<[f64; 3]>,
    /// Контур замкнут двойным щелчком.
    closed: bool,
}

/// Обрезка, как раздел «Обрезка» окна модуля. Контур и линия берутся
/// из измерения: замкнутый контур - для «внутри» и «снаружи»,
/// ломаная - для сторон и коридора.
struct ClipUi {
    /// Что оставить в плане. None - в плане не резать.
    side: Option<Side>,
    /// Полуширина коридора, метры. У модуля по умолчанию 250.
    width: f64,
    use_lo: bool,
    zlo: f64,
    use_hi: bool,
    zhi: f64,
}

impl Default for ClipUi {
    fn default() -> Self {
        Self {
            side: Some(Side::In),
            width: 250.0,
            use_lo: false,
            zlo: 0.0,
            use_hi: false,
            zhi: 0.0,
        }
    }
}

/// По чему построен короб: изменилось - строится заново.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BoxKey {
    opts: BoxOpts,
    sides: scene_core::axes::Sides,
    lo: [f64; 3],
    hi: [f64; 3],
    dark: bool,
}

/// Короб частью-линиями, как `_add_axes_box` окна модуля: рёбра,
/// сетка, штрихи делений, стрелка севера, каждое своим цветом. Цвета
/// модуля, в тёмной теме - светлые.
fn box_part(k: &BoxKey) -> Part {
    use scene_core::axes;
    let o = &k.opts;
    let shade = |c: [f32; 4]| {
        if k.dark {
            [1.0 - c[0] * 0.6, 1.0 - c[1] * 0.6, 1.0 - c[2] * 0.5, c[3]]
        } else {
            c
        }
    };
    let ticks: Vec<_> = axes::tick_marks(k.lo, k.hi, 5, [o.step_xy, o.step_xy, o.step_z], k.sides)
        .into_iter()
        .map(|t| (t.a, t.b))
        .collect();
    let groups = [
        (
            axes::box_edges(k.lo, k.hi, o.edges.key(), k.sides),
            [0.35, 0.38, 0.45, 0.65],
        ),
        (
            axes::grid_lines(k.lo, k.hi, o.grid_step, o.grid.key()),
            [0.45, 0.48, 0.55, 0.35],
        ),
        (ticks, [0.25, 0.28, 0.36, 0.9]),
        (
            axes::north_arrow(k.lo, k.hi).to_vec(),
            [0.20, 0.24, 0.38, 0.95],
        ),
    ];
    let mut verts = Vec::new();
    let mut colors = Vec::new();
    let mut lines = Vec::new();
    for (segs, c) in groups {
        for (a, b) in segs {
            let n = verts.len() as u32;
            verts.extend([a, b]);
            colors.extend([shade(c); 2]);
            lines.push([n, n + 1]);
        }
    }
    Part {
        name: "box".to_string(),
        verts,
        indices: scene_format::Indices::Lines(lines),
        colors: Some(colors),
        sizes: None,
    }
}

/// Строка списка частей.
struct Row {
    info: Option<PartInfo>,
    volume: Option<f64>,
    swatch: [f32; 4],
    /// Треугольников, отрезков или точек.
    count: usize,
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
    /// Последняя ошибка открытия: её видит самопроверка.
    error: Option<String>,
    /// Адрес пакета, открытого по ссылке.
    source: Option<String>,
    /// Камера из `?pose=`: ставится, когда пакет откроется.
    pending_pose: Option<Camera>,
    share_open: bool,
    check: Option<Check>,
    /// Снимок кадра, который ещё читается из видеокарты.
    shot: Option<render::Readback>,
    /// Измерение: включено, если есть.
    ruler: Option<Ruler>,
    clip: ClipUi,
    /// Части до обрезки, по номерам строк. Есть - сцена обрезана.
    full: Option<Vec<Part>>,
    /// Вид по разрезу: начало и конец линии в плане. Есть - поворот
    /// заперт, на кадре шкалы отметок и глубин.
    section: Option<([f64; 2], [f64; 2])>,
    /// Тёмная тема. По умолчанию светлая, выбор запоминается.
    dark: bool,
    /// Показывать разметку: линию и контур измерения.
    marks: bool,
    /// Снимок идёт в буфер обмена, а не в файл.
    shot_copy: bool,
    /// Окно «Параметры сцены» открыто и его вкладка.
    params_open: bool,
    params_tab: usize,
    /// Настройки координатного короба.
    box_opts: BoxOpts,
    /// Отмывка и сглаживание краёв.
    look_opts: LookOpts,
    /// Короб, построенный окном: номер в частях видеокарты и то,
    /// по чему он построен. Изменилось - строится заново.
    box_slot: Option<usize>,
    box_key: Option<BoxKey>,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    film: Film,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    film_open: bool,
    #[cfg(not(target_arch = "wasm32"))]
    recording: Option<Recording>,
    #[cfg(not(target_arch = "wasm32"))]
    joining: Option<Joining>,
    /// Открытый с диска пакет: переписан - перечитывается.
    #[cfg(not(target_arch = "wasm32"))]
    watch: Option<crate::watch::Watch>,
    /// Папка из `--frames`: запись начнётся сама, как только есть кадр.
    #[cfg(not(target_arch = "wasm32"))]
    film_dir: Option<std::path::PathBuf>,
    /// Закрыть окно после записи (`--frames`).
    #[cfg(not(target_arch = "wasm32"))]
    film_exit: bool,
}

impl SceneApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let lang = cc
            .storage
            .and_then(|s| s.get_string("lang"))
            .map_or_else(Lang::system, |c| Lang::from_code(&c));
        // светлая тема по умолчанию, тёмная - если выбрана раньше
        let dark = cc
            .storage
            .and_then(|s| s.get_string("theme"))
            .is_some_and(|t| t == "dark");
        cc.egui_ctx.set_theme(if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        });
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
            error: None,
            source: None,
            pending_pose: None,
            share_open: false,
            check: None,
            shot: None,
            ruler: None,
            clip: ClipUi::default(),
            full: None,
            section: None,
            dark,
            marks: true,
            shot_copy: false,
            params_open: false,
            params_tab: 0,
            box_opts: BoxOpts::default(),
            look_opts: LookOpts::default(),
            box_slot: None,
            box_key: None,
            film: Film::default(),
            film_open: false,
            #[cfg(not(target_arch = "wasm32"))]
            recording: None,
            #[cfg(not(target_arch = "wasm32"))]
            joining: None,
            #[cfg(not(target_arch = "wasm32"))]
            watch: None,
            #[cfg(not(target_arch = "wasm32"))]
            film_dir: None,
            #[cfg(not(target_arch = "wasm32"))]
            film_exit: false,
        }
    }

    /// Точка измерения под курсором. Контур замкнут - новый щелчок
    /// начинает новое измерение.
    fn ruler_add(&mut self, px: f64, py: f64, w: f64, h: f64) {
        let Some(hit) = self.hit_at(px, py, w, h) else {
            self.status = self.t("Мимо объекта: точка не поставлена.").to_string();
            return;
        };
        let p = self.frame_now().to_world(hit.point);
        if let Some(r) = self.ruler.as_mut() {
            if r.closed {
                *r = Ruler::default();
            }
            r.pts.push(p);
        }
        self.ruler_status();
    }

    /// Снять последнюю точку. Замкнутый контур сначала размыкается.
    fn ruler_undo(&mut self) {
        if let Some(r) = self.ruler.as_mut() {
            if r.closed {
                r.closed = false;
            } else {
                r.pts.pop();
            }
        }
        self.ruler_status();
    }

    /// Двойной щелчок: второй щелчок уже поставил точку там же,
    /// она снимается, контур замыкается.
    fn ruler_close(&mut self) {
        if let Some(r) = self.ruler.as_mut() {
            let n = r.pts.len();
            if n >= 2 {
                let (a, b) = (r.pts[n - 2], r.pts[n - 1]);
                if (0..3).all(|k| (a[k] - b[k]).abs() < 1e-6) {
                    r.pts.pop();
                }
            }
            r.closed = r.pts.len() >= 3;
        }
        self.ruler_status();
    }

    /// Backspace снимает точку, Esc начинает измерение заново.
    fn ruler_keys(&mut self, ui: &egui::Ui) {
        if self.ruler.is_none() || ui.ctx().egui_wants_keyboard_input() {
            return;
        }
        let (back, esc) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::Backspace),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if back {
            self.ruler_undo();
        }
        if esc && let Some(r) = self.ruler.as_mut() {
            *r = Ruler::default();
            self.ruler_status();
        }
    }

    /// Итог измерения строкой состояния.
    fn ruler_status(&mut self) {
        let Some(r) = &self.ruler else { return };
        let v = scene_core::measure::values(&r.pts, r.closed);
        self.status = match (v.area, v.length) {
            (Some(a), Some(_)) => fill(
                self.t("Площадь в плане %s м2, периметр %s м"),
                &[grouped(a, 1), grouped(v.plan.unwrap_or(0.0), 2)],
            ),
            (None, Some(l)) => fill(
                self.t("Длина %s м, проложение %s м, превышение %s м"),
                &[
                    grouped(l, 2),
                    grouped(v.plan.unwrap_or(0.0), 2),
                    format!("{:+.2}", v.rise.unwrap_or(0.0)),
                ],
            ),
            _ => fill(self.t("Точек: %s"), &[r.pts.len().to_string()]),
        };
    }

    /// Величины измерения в панели.
    fn ruler_panel(&self, ui: &mut egui::Ui) {
        let Some(r) = &self.ruler else { return };
        let v = scene_core::measure::values(&r.pts, r.closed);
        ui.weak(self.t(
            "Щелчок - точка, двойной щелчок - замкнуть контур, правая кнопка или Backspace - убрать точку, Esc - начать заново.",
        ));
        let m = |x: f64| format!("{} м", grouped(x, 2));
        let mut rows: Vec<(&str, String)> = vec![(self.t("Точек"), r.pts.len().to_string())];
        if let Some(a) = v.area {
            rows.push((self.t("Площадь в плане"), format!("{} м2", grouped(a, 1))));
            if let Some(p) = v.plan {
                rows.push((self.t("Периметр в плане"), m(p)));
            }
            if let Some(l) = v.length {
                rows.push((self.t("Периметр по наклону"), m(l)));
            }
        } else {
            if let Some(l) = v.length {
                rows.push((self.t("Длина по наклону"), m(l)));
            }
            if let Some(p) = v.plan {
                rows.push((self.t("Проложение"), m(p)));
            }
            if let Some(d) = v.rise {
                rows.push((self.t("Превышение"), format!("{d:+.2} м")));
            }
            if let Some(a) = v.angle {
                let slope = v.slope.map_or(String::new(), |s| format!("{s:+.1} ‰, "));
                rows.push((self.t("Уклон"), format!("{slope}{a:+.2}°")));
            }
        }
        egui::Grid::new("ruler").num_columns(2).show(ui, |ui| {
            for (k, val) in rows {
                ui.label(k);
                ui.label(val);
                ui.end_row();
            }
        });
    }

    /// Настройки кадра для камеры в координатах просмотра.
    fn look(&self, camera: Camera, ppp: f32) -> Look {
        Look {
            camera,
            vex: self.vex,
            opacity: self.opacity,
            light: self.look_opts.strength(),
            background: match (self.gradient, self.dark) {
                (true, false) => render::BG_GRADIENT,
                (false, false) => render::BG_PLAIN,
                (true, true) => render::BG_GRADIENT_DARK,
                (false, true) => render::BG_PLAIN_DARK,
            },
            pixels_per_point: ppp,
        }
    }

    /// Окно «Кадры PNG»: путь камеры, углы и длительности, как окно
    /// записи видео модуля (`_spin_capture`).
    #[cfg(not(target_arch = "wasm32"))]
    fn film_window(&mut self, ctx: &egui::Context, now: f64) {
        if !self.film_open || self.pkg.is_none() {
            return;
        }
        let _ = now;
        let mut open = true;
        let mut go = false;
        let lang = self.lang;
        let t = |s: &'static str| tr(lang, s);
        let has_views = self.views.len() > 1;
        if !has_views {
            self.film.tour = false;
        }
        let f = &mut self.film;
        egui::Window::new(t("Кадры PNG"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.radio_value(&mut f.tour, false, t("Оборот вокруг сцены"));
                    if has_views {
                        ui.radio_value(&mut f.tour, true, t("Облёт по видам"));
                    }
                });
                egui::Grid::new("film").num_columns(2).show(ui, |ui| {
                    if f.tour {
                        ui.label(t("Переход между видами"));
                        ui.add(egui::DragValue::new(&mut f.leg).range(0.5..=120.0).suffix(" s"));
                        ui.end_row();
                        ui.label(t("Пауза на виде"));
                        ui.add(egui::DragValue::new(&mut f.pause).range(0.0..=60.0).suffix(" s"));
                        ui.end_row();
                    } else {
                        ui.label(t("Угол оборота"));
                        ui.add(egui::DragValue::new(&mut f.degrees).range(-720.0..=720.0).suffix("°"));
                        ui.end_row();
                        ui.label(t("Длительность оборота"));
                        ui.add(egui::DragValue::new(&mut f.seconds).range(1.0..=600.0).suffix(" s"));
                        ui.end_row();
                    }
                });
                ui.weak(t(
                    "Кадры 25 в секунду, размер окна сцены, имена frame_00000.png и дальше. Склеивают их программой монтажа или ffmpeg.",
                ));
                ui.checkbox(&mut f.mp4, t("Склеить в MP4 (ffmpeg из PATH)"));
                if ui.button(t("Выбрать папку и записать")).clicked() {
                    go = true;
                }
            });
        self.film_open = open;
        if go {
            self.film_open = false;
            self.start_film();
        }
    }

    /// Записать кадры оборота в папку и закрыть окно (`--frames`).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn film_to(&mut self, dir: std::path::PathBuf) {
        self.film_dir = Some(dir);
    }

    /// Начать запись: кадры пути камеры в папку.
    #[cfg(not(target_arch = "wasm32"))]
    fn start_film(&mut self) {
        let f = self.film;
        let frames = if f.tour {
            flight::tour(&self.views, f.leg, f.pause, flight::FPS)
        } else {
            flight::orbit(&self.cam, f.degrees, f.seconds, flight::FPS)
        };
        if frames.is_empty() {
            return;
        }
        let dir = match self.film_dir.take() {
            Some(d) => {
                self.film_exit = true;
                d
            }
            None => {
                let Some(d) = rfd::FileDialog::new()
                    .set_title(self.t("Куда складывать кадры"))
                    .pick_folder()
                else {
                    return;
                };
                d
            }
        };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.status = fill(
                self.t("Запись прервана: %s. Записано кадров: %s."),
                &[e.to_string(), "0".into()],
            );
            return;
        }
        let (Some(gpu), Some(view)) = (&self.gpu, &self.target) else {
            return;
        };
        let target =
            Target::with_samples(&gpu.device, view.width, view.height, gpu.renderer.samples());
        self.spin = false;
        self.anim = None;
        self.recording = Some(Recording {
            frames,
            next: 0,
            dir,
            start: self.cam,
            target,
            error: None,
            mp4: f.mp4,
        });
    }

    /// Снять очередной кадр записи. Кадр на обновление окна: так видно
    /// ход и запись можно прервать.
    #[cfg(not(target_arch = "wasm32"))]
    fn film_step(&mut self, ctx: &egui::Context) {
        let Some(rec) = self.recording.as_ref() else {
            return;
        };
        let k = rec.next;
        let n = rec.frames.len();
        let mut error = rec.error.clone();
        if error.is_none()
            && k < n
            && let (Some(gpu), Some(scene)) = (&self.gpu, &self.scene)
        {
            let view = self.frame_now().camera_to_view(&rec.frames[k]);
            let look = self.look(view, ctx.pixels_per_point());
            gpu.renderer
                .render(&gpu.device, &gpu.queue, &rec.target, scene, &look);
            let (w, h) = (rec.target.width, rec.target.height);
            let res = render::read_rgba(&gpu.device, &gpu.queue, &rec.target)
                .and_then(|rgba| shot::encode_png(w, h, &rgba))
                .and_then(|png| {
                    std::fs::write(rec.dir.join(format!("frame_{k:05}.png")), png)
                        .map_err(|e| e.to_string())
                });
            if let Err(e) = res {
                error = Some(e);
            }
        }
        let template = self.t("Запись кадров: %s из %s");
        let Some(rec) = self.recording.as_mut() else {
            return;
        };
        rec.error.clone_from(&error);
        if error.is_none() && k < n {
            rec.next += 1;
            let done = rec.next;
            self.status = fill(template, &[done.to_string(), n.to_string()]);
            ctx.request_repaint();
            if done < n {
                return;
            }
        }
        self.finish_film(error, ctx);
    }

    /// Закончить или прервать запись: камера возвращается на место.
    #[cfg(not(target_arch = "wasm32"))]
    fn finish_film(&mut self, error: Option<String>, ctx: &egui::Context) {
        let Some(rec) = self.recording.take() else {
            return;
        };
        self.cam = rec.start;
        self.dirty = true;
        let (made, n) = (rec.next, rec.frames.len());
        self.status = if let Some(e) = error {
            fill(
                self.t("Запись прервана: %s. Записано кадров: %s."),
                &[e, made.to_string()],
            )
        } else if made < n {
            fill(
                self.t("Запись прервана на кадре %s из %s."),
                &[made.to_string(), n.to_string()],
            )
        } else if rec.mp4 {
            // ffmpeg ждётся в своём потоке, окно в это время живёт
            let out = rec.dir.join(crate::video::mp4_name(self.name.as_deref()));
            let dir = rec.dir.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let res = crate::video::join(crate::video::FFMPEG, &dir, flight::FPS, &out);
                let _ = tx.send(res);
            });
            self.joining = Some(Joining {
                rx,
                dir: rec.dir,
                made,
            });
            self.status = fill(self.t("Склейка MP4 из %s кадров…"), &[made.to_string()]);
            ctx.request_repaint();
            return;
        } else {
            fill(
                self.t("Записано кадров: %s, %s с ролика: %s"),
                &[
                    made.to_string(),
                    format!("{:.1}", flight::duration(made, flight::FPS)),
                    rec.dir.display().to_string(),
                ],
            )
        };
        self.film_done(ctx);
    }

    /// Ответ ffmpeg: ролик есть, программы нет или она не справилась.
    /// Кадры PNG в папке остаются в любом случае.
    #[cfg(not(target_arch = "wasm32"))]
    fn poll_join(&mut self, ctx: &egui::Context) {
        let Some(job) = self.joining.as_ref() else {
            return;
        };
        let res = match job.rx.try_recv() {
            Ok(r) => r,
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
                return;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err(
                crate::video::JoinError::Failed(self.t("поток склейки оборвался").into()),
            ),
        };
        let Some(job) = self.joining.take() else {
            return;
        };
        let dir = job.dir.display().to_string();
        self.status = match res {
            Ok(path) => fill(
                self.t("Записано кадров: %s, ролик MP4: %s"),
                &[job.made.to_string(), path.display().to_string()],
            ),
            Err(crate::video::JoinError::Missing) => fill(
                self.t("ffmpeg не найден в PATH. Кадры PNG остались в папке %s"),
                &[dir],
            ),
            Err(crate::video::JoinError::Failed(e)) => fill(
                self.t("ffmpeg не склеил ролик: %s. Кадры PNG остались в папке %s"),
                &[e, dir],
            ),
        };
        self.film_done(ctx);
    }

    /// Запись из командной строки: итог в вывод, окно закрывается.
    #[cfg(not(target_arch = "wasm32"))]
    fn film_done(&mut self, ctx: &egui::Context) {
        if self.film_exit {
            println!("{}", self.status);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// Начать снимок кадра: текстура кадра уже держит последний кадр.
    fn take_shot(&mut self, ctx: &egui::Context) {
        if let (Some(gpu), Some(target)) = (&self.gpu, &self.target) {
            self.shot = Some(render::start_readback(&gpu.device, &gpu.queue, target));
            ctx.request_repaint();
        }
    }

    /// Снимок готов - в PNG и в файл: на диске через выбор файла,
    /// в браузере - скачиванием.
    fn finish_shot(&mut self, ctx: &egui::Context) {
        let (Some(gpu), Some(rb)) = (&self.gpu, &self.shot) else {
            return;
        };
        let Some(got) = rb.poll(&gpu.device) else {
            ctx.request_repaint();
            return;
        };
        let (w, h) = rb.size();
        self.shot = None;
        // кадр в буфер обмена, как Ctrl+C окна модуля
        if std::mem::take(&mut self.shot_copy) {
            self.status = match got {
                Ok(rgba) => {
                    ctx.copy_image(egui::ColorImage::from_rgba_unmultiplied(
                        [w as usize, h as usize],
                        &rgba,
                    ));
                    fill(
                        self.t("Кадр %s x %s в буфере обмена."),
                        &[w.to_string(), h.to_string()],
                    )
                }
                Err(e) => fill(self.t("Не записалось: %s"), &[e]),
            };
            return;
        }
        let png = got.and_then(|rgba| shot::encode_png(w, h, &rgba));
        let bytes = match png {
            Ok(b) => b,
            Err(e) => {
                self.status = fill(self.t("Не записалось: %s"), &[e]);
                return;
            }
        };
        let name = shot::png_name(self.name.as_deref());
        #[cfg(not(target_arch = "wasm32"))]
        let res = {
            let Some(dest) = rfd::FileDialog::new()
                .add_filter("PNG", &["png"])
                .set_file_name(name)
                .save_file()
            else {
                return;
            };
            std::fs::write(&dest, bytes)
                .map(|()| dest.display().to_string())
                .map_err(|e| e.to_string())
        };
        #[cfg(target_arch = "wasm32")]
        let res = crate::web::download(&name, &bytes, "image/png").map(|()| name);
        self.status = match res {
            Ok(dest) => fill(
                self.t("Снимок %s x %s: %s"),
                &[w.to_string(), h.to_string(), dest],
            ),
            Err(e) => fill(self.t("Не записалось: %s"), &[e]),
        };
    }

    /// Камера, которую поставить при открытии пакета (`?pose=`).
    pub fn set_pose(&mut self, cam: Camera) {
        self.pending_pose = Some(cam);
    }

    /// Включить самопроверку страницы (`?check`).
    pub fn start_check(&mut self) {
        self.check = Some(Check {
            start: -1.0,
            frames: 0,
            done: false,
        });
    }

    /// Скачать пакет по ссылке (`?scene=`). Ответ приходит через inbox.
    #[cfg(target_arch = "wasm32")]
    pub fn fetch(&mut self, url: String, ctx: &egui::Context) {
        self.status = fill(self.t("Загружается: %s"), std::slice::from_ref(&url));
        let inbox = self.inbox.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let got = crate::web::fetch_bytes(&url).await.map(|bytes| Incoming {
                name: url.rsplit('/').next().unwrap_or(&url).to_string(),
                bytes,
                url: Some(url.clone()),
            });
            if let Ok(mut slot) = inbox.lock() {
                *slot = Some(got.map_err(|e| format!("{url}: {e}")));
            }
            ctx.request_repaint();
        });
    }

    fn t(&self, ru: &'static str) -> &'static str {
        tr(self.lang, ru)
    }

    /// Открыть пакет с диска.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(&mut self, path: &Path) {
        match std::fs::read(path) {
            Ok(b) => {
                self.open_bytes(path.display().to_string(), &b);
                // открытый пакет перечитывается, когда модуль QGIS
                // его перепишет
                if self.error.is_none() {
                    self.watch = Some(crate::watch::Watch::new(path.to_path_buf()));
                }
            }
            Err(e) => self.status = fill(self.t("Не открылось: %s"), &[e.to_string()]),
        }
    }

    /// Пакет на диске переписан - перечитать. Камера остаётся, где
    /// была: сцену в QGIS пересобрали, а смотрят на неё отсюда.
    /// Недописанный пакет не читается, попытка повторится. Во время
    /// записи кадров сцена не меняется.
    #[cfg(not(target_arch = "wasm32"))]
    fn poll_watch(&mut self, now: f64, ctx: &egui::Context) {
        let busy = self.recording.is_some() || self.joining.is_some();
        let Some(w) = self.watch.as_mut() else {
            return;
        };
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(crate::watch::PERIOD));
        if busy || !w.due(now) {
            return;
        }
        let path = w.path.clone();
        let Ok(pkg) = std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|b| read_package(&b).map_err(|e| e.to_string()))
        else {
            return;
        };
        w.read_ok();
        if self.pkg.is_some() {
            self.pending_pose = Some(self.cam);
        }
        let name = path.display().to_string();
        self.load(pkg, name.clone());
        self.error = None;
        self.status = fill(self.t("Пакет обновился, камера на месте: %s"), &[name]);
    }

    /// Открыть пакет из байтов: перетаскивание и выбор файла в браузере.
    pub fn open_bytes(&mut self, name: String, bytes: &[u8]) {
        match read_package(bytes) {
            Ok(pkg) => {
                self.load(pkg, name);
                self.source = None;
                self.error = None;
                // пакет не с диска - следить не за чем, с диска
                // слежение ставит `open`
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.watch = None;
                }
            }
            Err(e) => {
                self.status = fill(self.t("Не открылось: %s"), &[e.to_string()]);
                self.error = Some(self.status.clone());
            }
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
                        url: None,
                    };
                    if let Ok(mut slot) = inbox.lock() {
                        *slot = Some(Ok(file));
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
                count: part.indices.len(),
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
        // Оформление из пакета (короб линиями, подписи буквами из
        // отрезков) пакет несёт для других просмотрщиков. Окно строит
        // короб само по параметрам сцены, как окно модуля, поэтому
        // готовое оформление скрыто, а в списке частей его нет.
        if let Some(scene) = self.scene.as_mut() {
            for (p, row) in scene.parts.iter_mut().zip(&self.rows) {
                p.visible = !row.info.as_ref().is_some_and(|i| i.decor);
            }
        }
        self.box_key = None;
        self.box_slot = None;
        self.params_from_state(&pkg.meta.state);
        self.pkg = Some(pkg);
        self.status.clone_from(&name);
        self.name = Some(name);
        self.reset_camera(1280.0, 800.0);
        if let Some(pose) = self.pending_pose.take() {
            self.cam = pose;
        }
        self.anim = None;
        self.probe = None;
        self.full = None;
        self.section = None;
        self.dirty = true;
    }

    /// Вид по разрезу, как `_section_view` окна модуля: камера поперёк
    /// линии измерения (линия идёт слева направо), горизонтально,
    /// в параллельной проекции, кадр по длине линии. Поворот заперт.
    fn section_view(&mut self, w: f64, h: f64) {
        let pts: Vec<[f64; 2]> = self
            .ruler
            .as_ref()
            .map(|r| r.pts.iter().map(|p| [p[0], p[1]]).collect())
            .unwrap_or_default();
        let (a, b) = match (pts.first(), pts.last()) {
            (Some(&a), Some(&b)) if pts.len() >= 2 => (a, b),
            _ => {
                self.section = None;
                self.status = self
                    .t("Вид по разрезу смотрит поперёк линии: поставьте измерением две точки и больше.")
                    .to_string();
                return;
            }
        };
        let mut d = [b[0] - a[0], b[1] - a[1]];
        let mut length = d[0].hypot(d[1]);
        if length <= 0.0 {
            length = pts
                .windows(2)
                .map(|s| (s[1][0] - s[0][0]).hypot(s[1][1] - s[0][1]))
                .sum();
            d = [pts[1][0] - pts[0][0], pts[1][1] - pts[0][1]];
        }
        if d == [0.0, 0.0] {
            d = [1.0, 0.0];
        }
        let (lo, hi) =
            pts.iter()
                .fold(([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]), |(l, h), p| {
                    (
                        [l[0].min(p[0]), l[1].min(p[1])],
                        [h[0].max(p[0]), h[1].max(p[1])],
                    )
                });
        self.cam.center[0] = 0.5 * (lo[0] + hi[0]);
        self.cam.center[1] = 0.5 * (lo[1] + hi[1]);
        if let Some((zlo, zhi)) = self.section_z() {
            self.cam.center[2] = 0.5 * (zlo + zhi);
        }
        // камера с той стороны, откуда линия идёт слева направо
        self.cam.azimuth = (-d[0]).atan2(d[1]).to_degrees();
        self.cam.elevation = 0.0;
        self.cam.fov = 60.0;
        self.cam.ortho = true;
        // запас по краям: подписи отметок стоят за концами линии
        let half_w = 0.66 * length.max(1.0);
        self.cam.distance = (half_w * h / w.max(1.0)) / 30f64.to_radians().tan();
        self.spin = false;
        self.anim = None;
        self.section = Some((a, b));
        self.dirty = true;
        self.status = fill(
            self.t("Вид по разрезу: длина линии %s м. Поворот заперт, левая кнопка сдвигает, колёсико приближает. Отжать кнопку или «Сбросить вид» - поворот снова свободен."),
            &[grouped(length, 0)],
        );
    }

    /// Шкалы вида по разрезу поверх кадра: отметки у начала линии,
    /// глубины от верхней кромки у конца, стрелка с азимутом.
    fn section_marks(&self, ui: &egui::Ui, rect: egui::Rect) {
        let (Some((a, b)), Some((zlo, zhi))) = (self.section, self.section_z()) else {
            return;
        };
        let sc = scene_core::axes::section_scales(a, b, zlo, zhi, 6, None, self.vex);
        let view = self.view_camera();
        let frame = self.frame_now();
        let (w, h) = (f64::from(rect.width()), f64::from(rect.height()));
        let at = |p: [f64; 3]| {
            camera::to_screen(&view, w, h, frame.to_view(p))
                .map(|[x, y]| rect.min + egui::vec2(x as f32, y as f32))
        };
        let painter = ui.painter_at(rect);
        let ink = egui::Color32::from_rgba_unmultiplied(51, 61, 97, 242);
        for (p, q) in &sc.segs {
            if let (Some(p), Some(q)) = (at(*p), at(*q)) {
                painter.line_segment([p, q], egui::Stroke::new(1.4, ink));
            }
        }
        let font = egui::FontId::proportional(11.0);
        let labels = sc
            .left
            .iter()
            .map(|l| (l, egui::Align2::RIGHT_CENTER))
            .chain(sc.right.iter().map(|l| (l, egui::Align2::LEFT_CENTER)))
            .chain(std::iter::once((
                &sc.arrow_label,
                egui::Align2::CENTER_BOTTOM,
            )));
        for ((p, text), align) in labels {
            if let Some(s) = at(*p) {
                for o in [
                    egui::vec2(-1.0, 0.0),
                    egui::vec2(1.0, 0.0),
                    egui::vec2(0.0, -1.0),
                    egui::vec2(0.0, 1.0),
                ] {
                    painter.text(s + o, align, text, font.clone(), egui::Color32::WHITE);
                }
                painter.text(s, align, text, font.clone(), egui::Color32::from_gray(30));
            }
        }
    }

    /// Стороны короба для подписей и рёбер «три оси»: ближние к камере
    /// или у начала, как выбрано в окне параметров.
    fn box_sides(&self, lo: [f64; 3], hi: [f64; 3]) -> scene_core::axes::Sides {
        if self.box_opts.near {
            let eye = camera::eye(&self.view_camera());
            scene_core::axes::near_sides(lo, hi, self.frame_now().to_world(eye))
        } else {
            scene_core::axes::ORIGIN_SIDES
        }
    }

    /// Построить короб заново, если изменились его параметры, охват,
    /// стороны подписей или тема. Короб - последняя часть в видеокарте,
    /// после частей пакета: строки списка частей его не касаются.
    fn update_box(&mut self) {
        let Some((lo, hi)) = self.box_range() else {
            return;
        };
        let key = BoxKey {
            opts: self.box_opts,
            sides: self.box_sides(lo, hi),
            lo,
            hi,
            dark: self.dark,
        };
        if self.box_key == Some(key) {
            return;
        }
        let (Some(gpu), Some(scene)) = (&self.gpu, self.scene.as_mut()) else {
            return;
        };
        let part = box_part(&key);
        let slot = self.box_slot.filter(|&i| i < scene.parts.len());
        match (GpuPart::upload(&gpu.device, &part, scene.frame), slot) {
            (Some(mut p), Some(i)) => {
                p.visible = key.opts.on;
                scene.parts[i] = p;
            }
            (Some(mut p), None) => {
                p.visible = key.opts.on;
                self.box_slot = Some(scene.parts.len());
                scene.parts.push(p);
            }
            (None, Some(i)) => scene.parts[i].visible = false,
            (None, None) => {}
        }
        self.box_key = Some(key);
        self.dirty = true;
    }
    /// Охват координатного короба в настоящих координатах, как
    /// `_box_scene` модуля: тела сцены целиком, до обрезки, и свои
    /// границы короба из окна QGIS, если они были заданы.
    fn box_range(&self) -> Option<([f64; 3], [f64; 3])> {
        let pkg = self.pkg.as_ref()?;
        let src = self.full.as_ref().unwrap_or(&pkg.parts);
        let body: Vec<Part> = src
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.is_decor(*i))
            .map(|(_, p)| p.clone())
            .collect();
        let (lo, hi) = mesh::bounds(&body)?;
        let b = &self.box_opts;
        let rng = b.range_on.then_some(&b.range[..]);
        Some(scene_core::axes::custom_range(lo, hi, rng))
    }

    /// Подписи координатного короба поверх кадра, как в окне модуля:
    /// числа делений на рёбрах, ближних к камере, буква у стрелки севера.
    /// Шаг, множитель подписи и выбор рёбер - из окна параметров,
    /// при открытии пакета они берутся из состояния окна QGIS. Подписи
    /// оси, смотрящей на камеру торцом, и налезающие на соседние
    /// не ставятся.
    fn box_marks(&self, ui: &egui::Ui, rect: egui::Rect) {
        use scene_core::axes;
        if !self.box_opts.on {
            return;
        }
        let Some((lo, hi)) = self.box_range() else {
            return;
        };
        let b = self.box_opts;
        let (sxy, sz, mxy, mz) = (b.step_xy, b.step_z, b.mult_xy, b.mult_z);
        let view = self.view_camera();
        let frame = self.frame_now();
        let sides = self.box_sides(lo, hi);
        let (w, h) = (f64::from(rect.width()), f64::from(rect.height()));
        let at = |p: [f64; 3]| {
            camera::to_screen(&view, w, h, frame.to_view(p))
                .map(|[x, y]| rect.min + egui::vec2(x as f32, y as f32))
        };
        let painter = ui.painter_at(rect);
        let (text, halo) = if self.dark {
            (egui::Color32::from_gray(235), egui::Color32::from_gray(20))
        } else {
            (egui::Color32::from_gray(30), egui::Color32::WHITE)
        };
        let ticks = axes::tick_marks(lo, hi, 5, [sxy, sxy, sz], sides);
        // штрихи и стрелку рисует видеокарта частью короба (`box_part`),
        // здесь только текст
        let arrow = axes::north_arrow(lo, hi);
        let font = egui::FontId::proportional(11.0);
        let put = |pos: egui::Pos2, align: egui::Align2, s: &str| {
            for o in [
                egui::vec2(-1.0, 0.0),
                egui::vec2(1.0, 0.0),
                egui::vec2(0.0, -1.0),
                egui::vec2(0.0, 1.0),
            ] {
                painter.text(pos + o, align, s, font.clone(), halo);
            }
            painter.text(pos, align, s, font.clone(), text)
        };
        if let Some(tip) = at(arrow[0].1) {
            put(tip, egui::Align2::CENTER_BOTTOM, self.t("С"));
        }
        // подпись уходит от штриха в его же сторону на экране
        let mut taken: Vec<egui::Rect> = Vec::new();
        for t in ticks.iter().take(LABEL_CAP) {
            if axes::edge_on(t.axis, view.azimuth, view.elevation) {
                continue;
            }
            let (Some(a), Some(b)) = (at(t.a), at(t.b)) else {
                continue;
            };
            let d = (b - a).normalized();
            let d = if d.x.is_finite() {
                d
            } else {
                egui::vec2(-1.0, 0.0)
            };
            let align = if d.x > 0.4 {
                egui::Align2::LEFT_CENTER
            } else if d.x < -0.4 {
                egui::Align2::RIGHT_CENTER
            } else if d.y > 0.0 {
                egui::Align2::CENTER_TOP
            } else {
                egui::Align2::CENTER_BOTTOM
            };
            let mult = if t.axis == 2 { mz } else { mxy };
            let s = axes::tick_label(t.value, mult);
            let pos = b + d * 3.0;
            let galley = painter.layout_no_wrap(s.clone(), font.clone(), text);
            let r = align.anchor_size(pos, galley.size()).expand(1.0);
            if taken.iter().any(|o| o.intersects(r)) {
                continue;
            }
            taken.push(r);
            put(pos, align, &s);
        }
    }

    /// Обрезка из настроек и точек измерения.
    fn clip_spec(&self) -> Result<Clip, &'static str> {
        let (pts, closed): (Vec<[f64; 2]>, bool) =
            self.ruler.as_ref().map_or((Vec::new(), false), |r| {
                (r.pts.iter().map(|p| [p[0], p[1]]).collect(), r.closed)
            });
        let c = &self.clip;
        let mut spec = Clip {
            side: c.side.unwrap_or(Side::In),
            rings: Vec::new(),
            lines: Vec::new(),
            width: c.width,
            zlo: c.use_lo.then_some(c.zlo),
            zhi: c.use_hi.then_some(c.zhi),
        };
        // без точек измерения в плане не режется: остаются отметки
        let plan = if pts.is_empty() { None } else { c.side };
        match plan {
            Some(Side::In | Side::Out) => {
                if !closed || pts.len() < 3 {
                    return Err(
                        "Нужен замкнутый контур: поставьте точки измерением и замкните двойным щелчком.",
                    );
                }
                spec.rings = vec![pts];
            }
            Some(side) => {
                if pts.len() < 2 {
                    return Err("Нужна линия: поставьте измерением две точки и больше.");
                }
                if side == Side::Corridor && c.width <= 0.0 {
                    return Err("Полуширина коридора должна быть больше нуля.");
                }
                spec.lines = vec![pts];
            }
            None => {}
        }
        if plan.is_none() && spec.zlo.is_none() && spec.zhi.is_none() {
            return Err("Нечего обрезать: выберите, что оставить, или отметки.");
        }
        Ok(spec)
    }

    /// Обрезать сцену. Режутся части до обрезки: новая обрезка
    /// заменяет прежнюю, а не режет уже обрезанное.
    fn apply_clip(&mut self) {
        let spec = match self.clip_spec() {
            Ok(s) => s,
            Err(e) => {
                self.status = self.t(e).to_string();
                return;
            }
        };
        let Some(pkg) = self.pkg.as_ref() else { return };
        let src = self.full.clone().unwrap_or_else(|| pkg.parts.clone());
        // оформление (короб, подписи) не режется: оно о всей сцене
        let cut: Vec<Part> = src
            .iter()
            .enumerate()
            .map(|(i, p)| {
                if self.is_decor(i) {
                    p.clone()
                } else {
                    clip::clip_part(p, &spec)
                }
            })
            .collect();
        let tris = |parts: &[Part]| {
            parts
                .iter()
                .filter(|p| matches!(p.indices, scene_format::Indices::Triangles(_)))
                .map(|p| p.indices.len())
                .sum::<usize>()
        };
        self.status = fill(
            self.t("Обрезка: осталось %s граней из %s."),
            &[grouped(tris(&cut) as f64, 0), grouped(tris(&src) as f64, 0)],
        );
        self.full = Some(src);
        self.set_parts(cut);
    }

    /// Часть - оформление сцены: короб, подписи.
    fn is_decor(&self, i: usize) -> bool {
        self.rows
            .get(i)
            .and_then(|r| r.info.as_ref())
            .is_some_and(|info| info.decor)
    }

    /// Отметки низа и верха для шкал разреза: охват частей сцены без
    /// оформления после обрезки, как `_box_scene` модуля.
    fn section_z(&self) -> Option<(f64, f64)> {
        let pkg = self.pkg.as_ref()?;
        let body: Vec<Part> = pkg
            .parts
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.is_decor(*i))
            .map(|(_, p)| p.clone())
            .collect();
        let (lo, hi) = mesh::bounds(&body).or_else(|| mesh::bounds(&pkg.parts))?;
        Some((lo[2], hi[2]))
    }

    /// Снять обрезку: вернуть части целиком.
    fn clear_clip(&mut self) {
        if let Some(full) = self.full.take() {
            self.set_parts(full);
            self.status = self.t("Обрезка снята, сцена показана целиком.").to_string();
        }
    }

    /// Заменить части сцены, номера строк те же. Показ частей
    /// и кадр не меняются.
    fn set_parts(&mut self, parts: Vec<Part>) {
        if let (Some(gpu), Some(scene)) = (&self.gpu, self.scene.as_mut()) {
            for (old, part) in scene.parts.iter_mut().zip(&parts) {
                match GpuPart::upload(&gpu.device, part, scene.frame) {
                    Some(mut p) => {
                        p.visible = old.visible;
                        *old = p;
                    }
                    None => old.clear(),
                }
            }
        }
        for (row, part) in self.rows.iter_mut().zip(&parts) {
            row.volume = mesh::body_volume(part);
            row.count = part.indices.len();
        }
        if let Some(pkg) = self.pkg.as_mut() {
            pkg.parts = parts;
        }
        self.probe = None;
        self.dirty = true;
    }

    /// Кнопки обрезки в панели. Что оставить и отметки - во вкладке
    /// «Обрезка» окна параметров.
    fn clip_buttons(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let t = |s: &'static str| tr(lang, s);
        ui.horizontal_wrapped(|ui| {
            if ui.button(t("Обрезать")).clicked() {
                self.apply_clip();
            }
            if ui
                .add_enabled(self.full.is_some(), egui::Button::new(t("Снять обрезку")))
                .clicked()
            {
                self.clear_clip();
            }
        });
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
        let res = crate::web::download(&name, &bytes, "application/zip").map(|()| name);
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
        let view = self.view_camera();
        let frame = self.frame_now();
        let (o, d) = camera::ray(&view, w, h, px, py)?;
        let visible = || {
            pkg.parts
                .iter()
                .enumerate()
                .filter(|(i, _)| scene.parts.get(*i).is_some_and(|p| p.visible))
        };
        let body = mesh::raycast(visible(), &frame, o, d);
        // значок пробы: под курсором, если не закрыт телом ближе к глазу
        let mark = mesh::pick_point(visible(), &frame, &view, w, h, px, py, 4.0);
        let eye = glam::DVec3::from(camera::eye(&view));
        let dist = |p: [f64; 3]| (glam::DVec3::from(p) - eye).length();
        match (body, mark) {
            (Some(b), Some(m)) if dist(b.point) < m.t => Some(b),
            (_, Some(m)) => Some(m),
            (b, None) => b,
        }
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

    /// Адрес страницы для ссылки: в браузере - своя страница, на диске -
    /// страница программы на сайте.
    fn page_url() -> String {
        #[cfg(target_arch = "wasm32")]
        if let Some(loc) = web_sys::window().map(|w| w.location())
            && let (Ok(origin), Ok(path)) = (loc.origin(), loc.pathname())
        {
            return format!("{origin}{path}");
        }
        SITE_APP.to_string()
    }

    /// Окно «Поделиться»: ссылка с камерой и адресом пакета.
    fn share_window(&mut self, ctx: &egui::Context) {
        if !self.share_open || self.pkg.is_none() {
            return;
        }
        let link = share::link(&Self::page_url(), self.source.as_deref(), &self.cam);
        let mut open = true;
        let (title, copy) = (self.t("Поделиться"), self.t("Копировать"));
        let note = self.source.is_none().then(|| {
            self.t("Пакет открыт с диска: по ссылке откроется сцена демо с этой камерой. Пакет по ссылке - адрес файла в параметре ?scene=.")
        });
        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .default_width(480.0)
            .show(ctx, |ui| {
                let mut shown = link.as_str();
                ui.add(egui::TextEdit::multiline(&mut shown).desired_width(f32::INFINITY));
                if ui.button(copy).clicked() {
                    ui.ctx().copy_text(link.clone());
                }
                if let Some(n) = note {
                    ui.weak(n);
                }
            });
        self.share_open = open;
    }

    /// Самопроверка `?check`: пакет открыт, видеокарта есть, кадры
    /// рисуются. Отчёт - строкой, в браузере в консоль и поверх страницы.
    fn run_check(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let Some(check) = self.check.as_mut() else {
            return;
        };
        if check.done {
            return;
        }
        if check.start < 0.0 {
            check.start = now;
        }
        if self.target.is_some() && self.pkg.is_some() {
            check.frames += 1;
        }
        let elapsed = now - check.start;
        // ждём 30 кадров сцены, но не дольше 20 секунд
        if check.frames < 30 && elapsed < 20.0 && self.error.is_none() {
            ctx.request_repaint();
            return;
        }
        check.done = true;
        let frames = check.frames;
        let mut problems = Vec::new();
        if self.gpu.is_none() {
            problems.push("нет видеокарты".to_string());
        }
        if let Some(e) = &self.error {
            problems.push(e.clone());
        }
        if self.pkg.is_none() {
            problems.push("пакет не открыт".to_string());
        }
        if frames < 30 {
            problems.push(format!("кадров {frames} за {elapsed:.1} с"));
        }
        let parts = self.rows.len();
        let name = self.name.clone().unwrap_or_default();
        let report = if problems.is_empty() {
            format!(
                "Isoliner Scene: проверка пройдена. {name}, частей {parts}, кадров {frames} за {elapsed:.1} с"
            )
        } else {
            format!(
                "Isoliner Scene: проверка не пройдена: {}",
                problems.join("; ")
            )
        };
        #[cfg(target_arch = "wasm32")]
        crate::web::show(&report);
        #[cfg(not(target_arch = "wasm32"))]
        println!("{report}");
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
        if (azim != 0.0 || elev != 0.0) && self.section.is_none() {
            self.cam.azimuth += azim;
            self.cam.elevation = (self.cam.elevation + elev).clamp(-90.0, 90.0);
            self.anim = None;
            self.dirty = true;
            ui.ctx().request_repaint();
        }
    }

    fn start(&mut self, frames: Vec<Camera>, now: f64) {
        if !frames.is_empty() {
            self.section = None;
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
        ui.horizontal_wrapped(|ui| {
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
        ui.horizontal_wrapped(|ui| {
            if ui.button(self.t("Вписать")).clicked() {
                self.fit(w, h);
            }
            if ui.button(self.t("Камера пакета")).clicked()
                && let Some(c) = self.pkg.as_ref().and_then(|p| p.meta.camera)
            {
                self.fly_to(c, now);
            }
            let label = self.t("Поделиться");
            ui.toggle_value(&mut self.share_open, label);
        });
        // кнопки окна модуля - на плашке поверх сцены (`toolbar`),
        // здесь только то, чего на плашке нет
        // параметры вида - в окне «Параметры сцены» (шестерёнка),
        // здесь только действия
        self.ruler_panel(ui);
        self.clip_buttons(ui);

        ui.separator();
        ui.heading(self.t("Части"));
        let lang = self.lang;
        egui::ScrollArea::vertical()
            .max_height(ui.available_height() * 0.45)
            .id_salt("parts")
            .show(ui, |ui| {
                if let Some(scene) = self.scene.as_mut() {
                    // оформление пакета не показывается: короб строит
                    // окно, он - в окне параметров
                    for (p, row) in scene
                        .parts
                        .iter_mut()
                        .zip(&self.rows)
                        .filter(|(_, r)| !r.info.as_ref().is_some_and(|i| i.decor))
                    {
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
                        if p.points {
                            note = fill(tr(lang, "точки: %s"), &[row.count.to_string()]);
                        } else if row.info.as_ref().is_some_and(|i| i.shape.is_some()) {
                            // значки скважин и реперов - плоские фигуры
                            note = tr(lang, "значки").to_string();
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
        ui.horizontal_wrapped(|ui| {
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
        // запись кадров - значком плашки; здесь видно, что идёт
        // запись или склейка, и её можно прервать
        #[cfg(not(target_arch = "wasm32"))]
        ui.horizontal_wrapped(|ui| {
            if self.recording.is_some() {
                if ui.button(self.t("Прервать")).clicked() {
                    self.finish_film(None, ui.ctx());
                }
            } else if self.joining.is_some() {
                ui.spinner();
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
        // «Сглаживать края»: конвейеры и кадр с другим числом отсчётов
        let samples = if self.look_opts.smooth {
            render::SAMPLES
        } else {
            1
        };
        if let Some(gpu) = self.gpu.as_mut()
            && gpu.renderer.samples() != samples
        {
            gpu.renderer = Renderer::with_samples(&gpu.device, samples);
            self.target = None;
        }
        if let Some(gpu) = &self.gpu
            && self
                .target
                .as_ref()
                .is_none_or(|t| t.width != pw || t.height != ph)
        {
            let target = Target::with_samples(&gpu.device, pw, ph, gpu.renderer.samples());
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
        // два пальца: щипок, поворот и сдвиг; масштаб egui здесь - щипок,
        // а не Ctrl + колесо
        let touch = ui
            .input(|i| i.multi_touch())
            .filter(|t| t.num_touches >= 2 && resp.contains_pointer());
        if let Some(t) = touch {
            let mut v = self.view_camera();
            // в виде по разрезу пальцы не поворачивают
            let turn = if self.section.is_some() {
                0.0
            } else {
                f64::from(t.rotation_delta)
            };
            camera::touch(
                &mut v,
                f64::from(t.zoom_delta),
                turn,
                f64::from(t.translation_delta.x),
                f64::from(t.translation_delta.y),
                wpt,
            );
            self.set_view_camera(v);
            self.anim = None;
        } else if d != egui::Vec2::ZERO {
            let mut v = self.view_camera();
            let mut moved = true;
            if resp.dragged_by(egui::PointerButton::Primary) {
                // в виде по разрезу поворот заперт: левая кнопка сдвигает
                if ctrl || self.section.is_some() {
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
        if resp.hovered() && touch.is_none() {
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
            let measuring = self.ruler.is_some();
            if resp.secondary_clicked() {
                // в измерении правая кнопка снимает точку, как в режиме
                // рисования окна модуля
                if measuring {
                    self.ruler_undo();
                } else {
                    self.pivot_at(px, py, wpt, hpt);
                }
            } else if resp.double_clicked() && measuring {
                self.ruler_close();
            } else if resp.clicked() {
                if measuring {
                    self.ruler_add(px, py, wpt, hpt);
                } else {
                    self.probe_at(px, py, wpt, hpt);
                }
            }
        }
        if resp.double_clicked() && self.ruler.is_none() {
            self.fit(w, h);
        }
        self.ruler_keys(ui);

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

        self.update_box();
        if self.dirty {
            if let (Some(gpu), Some(scene), Some(target)) = (&self.gpu, &self.scene, &self.target) {
                let look = self.look(self.view_camera(), ppp);
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
        self.box_marks(ui, rect);
        self.section_marks(ui, rect);
        // линия измерения: отрезки и точки поверх кадра, если разметка
        // не скрыта значком «глаз»
        if let Some(r) = self.ruler.as_ref().filter(|_| self.marks) {
            let view = self.view_camera();
            let frame = self.frame_now();
            let screen: Vec<Option<egui::Pos2>> = r
                .pts
                .iter()
                .map(|p| {
                    camera::to_screen(&view, wpt, hpt, frame.to_view(*p))
                        .map(|[x, y]| rect.min + egui::vec2(x as f32, y as f32))
                })
                .collect();
            let painter = ui.painter_at(rect);
            let ink = egui::Color32::from_rgb(230, 120, 20);
            let mut pairs: Vec<(usize, usize)> = (1..screen.len()).map(|i| (i - 1, i)).collect();
            if r.closed && screen.len() >= 3 {
                pairs.push((screen.len() - 1, 0));
            }
            for (a, b) in pairs {
                if let (Some(a), Some(b)) = (screen[a], screen[b]) {
                    painter.line_segment([a, b], egui::Stroke::new(2.5, egui::Color32::WHITE));
                    painter.line_segment([a, b], egui::Stroke::new(1.5, ink));
                }
            }
            for p in screen.iter().flatten() {
                painter.circle_filled(*p, 4.0, ink);
                painter.circle_stroke(*p, 4.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
            }
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
            // сцены нет - кадр не рисуется: фон темы вместо чёрного
            let bg = self.look(self.view_camera(), ppp).background;
            let c = |k: usize| (bg.bottom[k] * 255.0) as u8;
            ui.painter()
                .rect_filled(rect, 0.0, egui::Color32::from_rgb(c(0), c(1), c(2)));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                self.t("Перетащите пакет .i3dscene в окно или нажмите «Открыть»."),
                egui::FontId::proportional(16.0),
                ui.visuals().weak_text_color(),
            );
        }
        let ctx = ui.ctx().clone();
        self.toolbar(&ctx, rect, w, h, now);
    }

    /// Плашка значков в левом верхнем углу сцены, в порядке плашки
    /// окна модуля: перечитать | вид сверху, сброс, вид по разрезу,
    /// параллельная проекция, короб, рисование (и его снять, замкнуть,
    /// резать по линии), разметка, снять обрезку, сохранить, вращение,
    /// виды, видео, буфер, PNG, преувеличение. В конце - тема.
    fn toolbar(&mut self, ctx: &egui::Context, rect: egui::Rect, w: f64, h: f64, now: f64) {
        use crate::toolbar::{Icon, button, separator};
        let lang = self.lang;
        let t = |s: &'static str| tr(lang, s);
        let loaded = self.pkg.is_some();
        // Ctrl+C - кадр в буфер, как в окне модуля; поле ввода
        // в фокусе забирает его себе
        let copy_key = loaded
            && !ctx.egui_wants_keyboard_input()
            && ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy)));
        // одна плашка на окно: id глобально единственный
        egui::Area::new(egui::Id::unique("isoliner-tools"))
            .fixed_pos(rect.min + egui::vec2(8.0, 8.0))
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                crate::toolbar::frame(self.dark).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
                    ui.set_max_width((rect.width() - 16.0).max(120.0));
                    ui.horizontal_wrapped(|ui| {
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            let can = self.watch.is_some();
                            if button(ui, Icon::Reload, t("Перечитать пакет с диска"), false, can)
                                .clicked()
                                && let Some(wt) = self.watch.as_mut()
                            {
                                wt.force();
                            }
                            separator(ui);
                        }
                        if button(ui, Icon::Top, t("Вид сверху, план"), false, loaded).clicked() {
                            self.section = None;
                            let v = camera::top(&self.view_camera());
                            self.set_view_camera(v);
                            self.anim = None;
                        }
                        if button(
                            ui,
                            Icon::Home,
                            t("Сбросить вид: угол обзора, ракурс и кадр по сцене"),
                            false,
                            loaded,
                        )
                        .clicked()
                        {
                            self.section = None;
                            let v = camera::reset(&self.view_camera(), self.span());
                            self.set_view_camera(v);
                            self.anim = None;
                        }
                        if button(
                            ui,
                            Icon::Section,
                            t("Вид по разрезу: камера поперёк линии обрезки, поворот заперт, остаются сдвиг и масштаб"),
                            self.section.is_some(),
                            loaded,
                        )
                        .clicked()
                        {
                            if self.section.is_some() {
                                self.section = None;
                                self.status.clear();
                            } else {
                                self.section_view(w, h);
                            }
                        }
                        let drawing = self.ruler.is_some();
                        if button(
                            ui,
                            Icon::Draw,
                            t("Рисовать контур по поверхности: щелчок ставит вершину"),
                            drawing,
                            loaded,
                        )
                        .clicked()
                        {
                            self.ruler = if drawing {
                                None
                            } else {
                                Some(Ruler::default())
                            };
                            self.probe = None;
                            self.status.clear();
                        }
                        if drawing {
                            if button(ui, Icon::Undo, t("Снять последнюю вершину"), false, true)
                                .clicked()
                            {
                                self.ruler_undo();
                            }
                            if button(ui, Icon::Done, t("Замкнуть контур и обрезать сцену"), false, true)
                                .clicked()
                            {
                                self.ruler_close();
                                if !matches!(self.clip.side, Some(Side::In | Side::Out)) {
                                    self.clip.side = Some(Side::In);
                                }
                                self.apply_clip();
                            }
                            if button(ui, Icon::Line, t("Завершить линию и резать по ней"), false, true)
                                .clicked()
                            {
                                if !matches!(
                                    self.clip.side,
                                    Some(Side::Left | Side::Right | Side::Corridor)
                                ) {
                                    self.clip.side = Some(Side::Corridor);
                                }
                                self.apply_clip();
                            }
                        }
                        if button(
                            ui,
                            Icon::Clear,
                            t("Снять обрезку, наброски и точку опроса"),
                            false,
                            loaded,
                        )
                        .clicked()
                        {
                            self.clear_clip();
                            if let Some(r) = self.ruler.as_mut() {
                                r.pts.clear();
                                r.closed = false;
                            }
                            self.probe = None;
                        }
                        if button(
                            ui,
                            Icon::Export,
                            t("Сохранить пакет сцены (.i3dscene)"),
                            false,
                            loaded,
                        )
                        .clicked()
                        {
                            self.save_as();
                        }
                        if button(
                            ui,
                            Icon::Spin,
                            t("Вращать сцену (ещё раз - остановить)"),
                            self.spin,
                            loaded,
                        )
                        .clicked()
                        {
                            self.spin = !self.spin;
                            if self.spin {
                                self.section = None;
                            }
                        }
                        let views = button(
                            ui,
                            Icon::Views,
                            t("Виды облёта: запомнить текущий вид, убрать, перейти к виду"),
                            false,
                            loaded,
                        );
                        egui::Popup::menu(&views).show(|ui| self.views_menu(ui, now));
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            let rec = self.recording.is_some();
                            if button(
                                ui,
                                Icon::Frames,
                                t("Записать видео: оборот вокруг сцены или облёт по видам, файлом MP4 или кадрами PNG"),
                                rec,
                                loaded && self.joining.is_none(),
                            )
                            .clicked()
                            {
                                if rec {
                                    self.finish_film(None, ui.ctx());
                                } else {
                                    self.film_open = true;
                                }
                            }
                        }
                        let can_shot = self.target.is_some() && self.shot.is_none() && loaded;
                        let copy = button(
                            ui,
                            Icon::Copy,
                            t("Положить кадр сцены в буфер обмена (Ctrl+C)"),
                            false,
                            can_shot,
                        )
                        .clicked();
                        if copy || (copy_key && can_shot) {
                            self.shot_copy = true;
                            self.take_shot(ui.ctx());
                        }
                        if button(ui, Icon::Png, t("Сохранить кадр сцены в файл PNG"), false, can_shot)
                            .clicked()
                        {
                            self.shot_copy = false;
                            self.take_shot(ui.ctx());
                        }
                        separator(ui);
                        // все параметры вида - одно окно, как у модуля
                        if button(
                            ui,
                            Icon::Params,
                            t("Параметры сцены: камера, вертикальное преувеличение, короб, оформление, обрезка"),
                            self.params_open,
                            loaded,
                        )
                        .clicked()
                        {
                            self.params_open = !self.params_open;
                            // окно egui меряет себя первым кадром и видно
                            // со второго: второй нужен сразу
                            ui.ctx().request_repaint();
                        }
                    });
                });
            });
    }

    /// Меню видов облёта: запомнить, перейти, убрать, облететь.
    fn views_menu(&mut self, ui: &mut egui::Ui, now: f64) {
        let lang = self.lang;
        if ui.button(tr(lang, "Запомнить текущий вид")).clicked() {
            self.views.push(self.cam);
        }
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
        ui.separator();
        if ui
            .add_enabled(
                self.views.len() > 1,
                egui::Button::new(tr(lang, "Облёт по видам")),
            )
            .clicked()
        {
            let frames = flight::tour(&self.views, 4.0, 1.0, flight::FPS);
            self.start(frames, now);
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
                    let file = f
                        .bytes_async()
                        .await
                        .map(|bytes| Incoming {
                            name: name.clone(),
                            bytes,
                            url: None,
                        })
                        .map_err(|e| format!("{name}: {e}"));
                    if let Ok(mut slot) = inbox.lock() {
                        *slot = Some(file);
                    }
                    ctx.request_repaint();
                });
            }
        }
        let picked = self.inbox.lock().ok().and_then(|mut slot| slot.take());
        match picked {
            Some(Ok(file)) => {
                self.open_bytes(file.name, &file.bytes);
                if self.error.is_none() {
                    self.source = file.url;
                }
            }
            Some(Err(e)) => {
                self.status = fill(self.t("Не загрузилось: %s"), &[e]);
                self.error = Some(self.status.clone());
            }
            None => {}
        }
        self.share_window(ui.ctx());
        self.params_window(ui.ctx());
        self.run_check(ui.ctx());
        self.finish_shot(ui.ctx());
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.film_window(ui.ctx(), now);
            // запись из командной строки ждёт открытый пакет и кадр окна
            if self.film_dir.is_some() && self.pkg.is_some() && self.target.is_some() {
                self.start_film();
            }
            self.film_step(ui.ctx());
            self.poll_join(ui.ctx());
            self.poll_watch(now, ui.ctx());
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
        storage.set_string("theme", if self.dark { "dark" } else { "light" }.into());
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
