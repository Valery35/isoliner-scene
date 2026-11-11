// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Камера окна Isoliner3D.
//!
//! Окно модуля рисует сцену в pyqtgraph. Сцена там сдвинута на центр
//! (cx, cy, cz), высоты умножены на преувеличение vex. Это координаты
//! просмотра. Камера окна - поза в них: точка взгляда, удаление,
//! азимут, наклон, раствор. В пакете сцены та же камера записана
//! в настоящих координатах.
//!
//! Матрица вида повторяет `GLViewWidget.viewMatrix`: отъезд на удаление,
//! поворот на наклон минус 90° вокруг x, поворот на азимут плюс 90°
//! вокруг -z, сдвиг на точку взгляда. Перспектива - раствор
//! по горизонтали, как `GLViewWidget.projectionMatrix`, ближняя плоскость
//! на тысячной доле удаления. Параллельная проекция - раствор
//! по вертикали, ближняя плоскость позади камеры, как `_PickView`
//! окна модуля. Глубина в матрицах - от 0 до 1, как у wgpu.

use glam::{DMat4, DVec3};

use scene_format::Camera;

/// Сдвиг и преувеличение координат просмотра.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub cx: f64,
    pub cy: f64,
    pub cz: f64,
    pub vex: f64,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            cx: 0.0,
            cy: 0.0,
            cz: 0.0,
            vex: 1.0,
        }
    }
}

impl Frame {
    /// Центр по охвату сцены, как окно модуля: середина плана
    /// и середина отметок.
    pub fn around(lo: [f64; 3], hi: [f64; 3], vex: f64) -> Self {
        Self {
            cx: (lo[0] + hi[0]) / 2.0,
            cy: (lo[1] + hi[1]) / 2.0,
            cz: (lo[2] + hi[2]) / 2.0,
            vex: if vex > 0.0 { vex } else { 1.0 },
        }
    }

    /// Настоящая точка в координатах просмотра.
    pub fn to_view(&self, p: [f64; 3]) -> [f64; 3] {
        [p[0] - self.cx, p[1] - self.cy, (p[2] - self.cz) * self.vex]
    }

    /// Точка просмотра в настоящих координатах.
    pub fn to_world(&self, p: [f64; 3]) -> [f64; 3] {
        [p[0] + self.cx, p[1] + self.cy, p[2] / self.vex + self.cz]
    }

    /// Камера пакета (настоящие координаты) в координатах просмотра.
    pub fn camera_to_view(&self, cam: &Camera) -> Camera {
        Camera {
            center: self.to_view(cam.center),
            ..*cam
        }
    }

    /// Камера просмотра в настоящих координатах.
    pub fn camera_to_world(&self, cam: &Camera) -> Camera {
        Camera {
            center: self.to_world(cam.center),
            ..*cam
        }
    }
}

/// Матрицы кадра для камеры в координатах просмотра.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub view: DMat4,
    pub proj: DMat4,
}

impl View {
    /// Матрица вида и проекции на кадр `width` x `height` пикселей.
    pub fn new(cam: &Camera, width: f64, height: f64) -> Self {
        Self {
            view: view_matrix(cam),
            proj: projection(cam, width, height),
        }
    }

    pub fn view_proj(&self) -> DMat4 {
        self.proj * self.view
    }
}

/// Матрица вида, как `GLViewWidget.viewMatrix`.
pub fn view_matrix(cam: &Camera) -> DMat4 {
    let c = DVec3::from(cam.center);
    DMat4::from_translation(DVec3::new(0.0, 0.0, -cam.distance))
        * DMat4::from_rotation_x((cam.elevation - 90.0).to_radians())
        * DMat4::from_rotation_z(-(cam.azimuth + 90.0).to_radians())
        * DMat4::from_translation(-c)
}

/// Положение глаза, как `GLViewWidget.cameraPosition`.
pub fn eye(cam: &Camera) -> [f64; 3] {
    let (e, a) = (cam.elevation.to_radians(), cam.azimuth.to_radians());
    [
        cam.center[0] + cam.distance * e.cos() * a.cos(),
        cam.center[1] + cam.distance * e.cos() * a.sin(),
        cam.center[2] + cam.distance * e.sin(),
    ]
}

/// Проекция с глубиной от 0 до 1.
pub fn projection(cam: &Camera, width: f64, height: f64) -> DMat4 {
    let w = width.max(1.0);
    let h = height.max(1.0);
    let dist = if cam.distance > 0.0 {
        cam.distance
    } else {
        10.0
    };
    let fov = if cam.fov > 0.0 { cam.fov } else { 60.0 };
    let half = (fov.to_radians() / 2.0).tan();
    if cam.ortho {
        let hh = dist * half;
        let hw = hh * w / h;
        DMat4::orthographic_rh(-hw, hw, -hh, hh, -dist * 100.0, dist * 1000.0)
    } else {
        // раствор pyqtgraph - по горизонтали
        let fov_y = 2.0 * (half * h / w).atan();
        DMat4::perspective_rh(fov_y, w / h, dist * 0.001, dist * 1000.0)
    }
}

/// Вращение мышью: `dx`, `dy` в пикселях, как `GLViewWidget.orbit`
/// при движении с левой кнопкой (градус на пиксель).
pub fn orbit(cam: &mut Camera, dx: f64, dy: f64) {
    cam.azimuth -= dx;
    cam.elevation = (cam.elevation + dy).clamp(-90.0, 90.0);
}

/// Метров просмотра на пиксель мыши, как у pyqtgraph: ширина раствора
/// у точки взгляда, делённая на ширину кадра.
fn mouse_scale(cam: &Camera, width: f64) -> f64 {
    let fov = if cam.fov > 0.0 { cam.fov } else { 60.0 };
    cam.distance * 2.0 * (fov.to_radians() / 2.0).tan() / width.max(1.0)
}

/// Сдвиг в плоскости экрана, как `GLViewWidget.pan(relative="view")`:
/// Ctrl + левая кнопка в окне модуля. `dx`, `dy` - пиксели мыши,
/// `width` - ширина кадра в тех же пикселях.
pub fn pan(cam: &mut Camera, dx: f64, dy: f64, width: f64) {
    let s = mouse_scale(cam, width);
    let e = cam.elevation.to_radians();
    let a = cam.azimuth.to_radians();
    let z = s * e.cos() * dy;
    let x = s * (a.sin() * dx - e.sin() * a.cos() * dy);
    let y = s * (a.cos() * dx + e.sin() * a.sin() * dy);
    cam.center[0] += x;
    cam.center[1] -= y;
    cam.center[2] += z;
}

/// Сдвиг по плану, как `GLViewWidget.pan(relative="view-upright")`:
/// средняя кнопка в окне модуля. `dz` - подъём, Ctrl + средняя.
pub fn pan_upright(cam: &mut Camera, dx: f64, dy: f64, dz: f64, width: f64) {
    let s = mouse_scale(cam, width);
    let c = DVec3::from(cam.center) - DVec3::from(eye(cam));
    let x = DVec3::Z.cross(c).normalize_or_zero();
    let y = x.cross(DVec3::Z).normalize_or_zero();
    let d = (x * dx + y * dy + DVec3::Z * dz) * s;
    for k in 0..3 {
        cam.center[k] += d[k];
    }
}

/// Колесо: `notches` щелчков, как pyqtgraph (0.999 в степени 120
/// на щелчок).
pub fn zoom(cam: &mut Camera, notches: f64) {
    cam.distance *= 0.999f64.powf(notches * 120.0);
}

/// Ctrl + колесо: раствор, как `GLViewWidget.wheelEvent`. Раствор
/// держится в 1..179 градусах, иначе проекция вырождается.
pub fn zoom_fov(cam: &mut Camera, notches: f64) {
    let fov = if cam.fov > 0.0 { cam.fov } else { 60.0 };
    cam.fov = (fov * 0.999f64.powf(notches * 120.0)).clamp(1.0, 179.0);
}

/// Охват для кадра, как окно модуля: наибольшее из ширины плана
/// и высоты с преувеличением, не меньше метра.
pub fn view_span(lo: [f64; 3], hi: [f64; 3], vex: f64) -> f64 {
    let plan = (hi[0] - lo[0]).max(hi[1] - lo[1]);
    plan.max((hi[2] - lo[2]) * vex).max(1.0)
}

/// «Сбросить вид» окна модуля (`_reset_view`): раствор 60, наклон 30,
/// азимут -60, точка взгляда в центре сцены, удаление 1,5 охвата.
/// Проекция остаётся.
pub fn reset(cam: &Camera, span: f64) -> Camera {
    Camera {
        center: [0.0; 3],
        distance: if span > 0.0 { span * 1.5 } else { cam.distance },
        azimuth: -60.0,
        elevation: 30.0,
        fov: 60.0,
        ortho: cam.ortho,
    }
}

/// «Вид сверху» окна модуля (`_set_view(90, -90, plan=True)`): план
/// всегда в параллельной проекции.
pub fn top(cam: &Camera) -> Camera {
    Camera {
        azimuth: -90.0,
        elevation: 90.0,
        ortho: true,
        ..*cam
    }
}

/// Перенести центр вращения в точку `p`, не сдвинув картинку
/// (`_center_keeping_view` окна модуля): глаз остаётся на месте,
/// удаление и углы считаются от нового центра.
pub fn center_keeping_view(cam: &Camera, p: [f64; 3]) -> Camera {
    let e = eye(cam);
    let d = DVec3::new(e[0] - p[0], e[1] - p[1], e[2] - p[2]);
    let dist = d.length();
    if dist < 1e-9 {
        return Camera { center: p, ..*cam };
    }
    Camera {
        center: p,
        distance: dist,
        elevation: (d.z / dist).clamp(-1.0, 1.0).asin().to_degrees(),
        azimuth: d.y.atan2(d.x).to_degrees(),
        ..*cam
    }
}

/// Луч из пикселя (`px`, `py` от левого верхнего угла кадра
/// `width` x `height`): начало на ближней плоскости и путь до дальней,
/// в координатах просмотра. Нет обратной матрицы - None.
pub fn ray(cam: &Camera, width: f64, height: f64, px: f64, py: f64) -> Option<(DVec3, DVec3)> {
    let m = View::new(cam, width, height).view_proj();
    if m.determinant().abs() < 1e-300 {
        return None;
    }
    let inv = m.inverse();
    let x = 2.0 * px / width.max(1.0) - 1.0;
    let y = 1.0 - 2.0 * py / height.max(1.0);
    let a = inv.project_point3(DVec3::new(x, y, 0.0));
    let b = inv.project_point3(DVec3::new(x, y, 1.0));
    (a.is_finite() && b.is_finite()).then_some((a, b - a))
}

/// Точка просмотра в пикселях кадра. За камерой - None.
pub fn to_screen(cam: &Camera, width: f64, height: f64, p: [f64; 3]) -> Option<[f64; 2]> {
    let clip = View::new(cam, width, height).view_proj() * DVec3::from(p).extend(1.0);
    if clip.w <= 0.0 {
        return None;
    }
    let n = clip.truncate() / clip.w;
    Some([(n.x + 1.0) / 2.0 * width, (1.0 - n.y) / 2.0 * height])
}

/// Размер пикселя в точке взгляда, метры просмотра.
pub fn pixel_size(cam: &Camera, height: f64) -> f64 {
    2.0 * cam.distance * (cam.fov.to_radians() / 2.0).tan() / height.max(1.0)
}

/// Камера, в кадр `width` x `height` которой целиком входит охват
/// `lo`..`hi` в координатах просмотра. Азимут и наклон остаются.
pub fn fit(cam: &Camera, lo: [f64; 3], hi: [f64; 3], width: f64, height: f64) -> Camera {
    let c = [
        (lo[0] + hi[0]) / 2.0,
        (lo[1] + hi[1]) / 2.0,
        (lo[2] + hi[2]) / 2.0,
    ];
    let r =
        0.5 * ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt();
    let t = (cam.fov.max(1.0).to_radians() / 2.0).tan();
    let aspect = width.max(1.0) / height.max(1.0);
    // перспектива: раствор по горизонтали, параллельная - по вертикали
    let (hor, ver) = if cam.ortho {
        (t * aspect, t)
    } else {
        (t, t / aspect)
    };
    let half = hor.min(ver);
    Camera {
        center: c,
        distance: (r / half).max(1e-6) * 1.05,
        ..*cam
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam() -> Camera {
        Camera {
            center: [10.0, -20.0, 5.0],
            distance: 300.0,
            azimuth: -60.0,
            elevation: 25.0,
            fov: 60.0,
            ortho: false,
        }
    }

    #[test]
    fn eye_goes_to_origin_and_center_ahead() {
        let c = cam();
        let v = view_matrix(&c);
        let e = v.transform_point3(DVec3::from(eye(&c)));
        assert!(e.length() < 1e-9, "{e}");
        let m = v.transform_point3(DVec3::from(c.center));
        assert!((m - DVec3::new(0.0, 0.0, -300.0)).length() < 1e-9);
    }

    #[test]
    fn world_up_is_screen_up() {
        let c = cam();
        let v = view_matrix(&c);
        let a = v.transform_point3(DVec3::from(c.center));
        let b = v.transform_point3(DVec3::from(c.center) + DVec3::Z);
        assert!(b.y > a.y);
        assert!((b.x - a.x).abs() < 1e-9);
    }

    #[test]
    fn horizontal_fov_like_pyqtgraph() {
        let c = cam();
        let vw = View::new(&c, 800.0, 400.0);
        // точка на краю горизонтального раствора у точки взгляда
        let edge = 300.0 * (30f64).to_radians().tan();
        let p = vw.proj.project_point3(DVec3::new(edge, 0.0, -300.0));
        assert!((p.x - 1.0).abs() < 1e-9, "{p}");
        let q = vw.proj.project_point3(DVec3::new(0.0, 0.0, -300.0));
        assert!(q.z > 0.0 && q.z < 1.0);
    }

    #[test]
    fn ortho_keeps_the_back_visible() {
        let c = Camera {
            ortho: true,
            ..cam()
        };
        let vw = View::new(&c, 400.0, 400.0);
        let half = 300.0 * (30f64).to_radians().tan();
        let p = vw.proj.project_point3(DVec3::new(0.0, half, 100.0));
        assert!((p.y - 1.0).abs() < 1e-9);
        assert!(p.z >= 0.0 && p.z <= 1.0);
    }

    #[test]
    fn frame_round_trip() {
        let f = Frame {
            cx: 512405.0,
            cy: 6401269.0,
            cz: -160.0,
            vex: 2.0,
        };
        let w = [512405.0, 6401269.0, -235.0];
        let v = f.to_view(w);
        assert_eq!(v, [0.0, 0.0, -150.0]);
        assert_eq!(f.to_world(v), w);
    }

    #[test]
    fn pan_moves_center_in_screen_plane() {
        let mut c = cam();
        let before = c.center;
        pan(&mut c, 10.0, 0.0, 500.0);
        let v = view_matrix(&cam());
        let a = v.transform_point3(DVec3::from(before));
        let b = v.transform_point3(DVec3::from(c.center));
        // сдвиг вправо мышью - точка взгляда уходит влево, на ширину
        // раствора на 50 пикселей из 500
        let want = 300.0 * 2.0 * (30f64).to_radians().tan() / 50.0;
        assert!((a.x - b.x - want).abs() < 1e-9, "{a} {b}");
        assert!((b.y - a.y).abs() < 1e-9 && (b.z - a.z).abs() < 1e-9);
        // мышь вниз - точка взгляда вверх по экрану
        let mut d = cam();
        pan(&mut d, 0.0, 10.0, 500.0);
        let b = v.transform_point3(DVec3::from(d.center));
        assert!(b.y > a.y && (b.x - a.x).abs() < 1e-9);
    }

    #[test]
    fn pan_upright_keeps_height() {
        let mut c = cam();
        pan_upright(&mut c, 10.0, 7.0, 0.0, 500.0);
        assert!((c.center[2] - 5.0).abs() < 1e-9);
        assert!(c.center[0] != 10.0 || c.center[1] != -20.0);
        let mut d = cam();
        pan_upright(&mut d, 0.0, 0.0, 10.0, 500.0);
        assert!(d.center[2] > 5.0 && (d.center[0] - 10.0).abs() < 1e-9);
    }

    #[test]
    fn fov_wheel_like_pyqtgraph() {
        let mut c = cam();
        zoom_fov(&mut c, 1.0);
        assert!((c.fov - 60.0 * 0.999f64.powi(120)).abs() < 1e-9);
        zoom_fov(&mut c, -1000.0);
        assert_eq!(c.fov, 179.0);
    }

    #[test]
    fn reset_and_top_like_the_module() {
        let r = reset(&cam(), 200.0);
        assert_eq!(
            (r.fov, r.elevation, r.azimuth, r.distance),
            (60.0, 30.0, -60.0, 300.0)
        );
        assert_eq!(r.center, [0.0; 3]);
        let t = top(&cam());
        assert_eq!((t.elevation, t.azimuth, t.ortho), (90.0, -90.0, true));
        assert_eq!(view_span([0.0, 0.0, 0.0], [100.0, 40.0, 30.0], 5.0), 150.0);
    }

    #[test]
    fn new_center_keeps_the_eye() {
        let c = cam();
        let n = center_keeping_view(&c, [40.0, 10.0, -20.0]);
        let (a, b) = (DVec3::from(eye(&c)), DVec3::from(eye(&n)));
        assert!((a - b).length() < 1e-9, "{a} {b}");
        assert_eq!(n.center, [40.0, 10.0, -20.0]);
    }

    #[test]
    fn ray_through_the_middle_hits_the_center() {
        for ortho in [false, true] {
            let c = Camera { ortho, ..cam() };
            let (o, d) = ray(&c, 640.0, 480.0, 320.0, 240.0).unwrap();
            // ближайшая к центру точка луча - сам центр
            let t = (DVec3::from(c.center) - o).dot(d) / d.length_squared();
            let p = o + d * t;
            assert!((p - DVec3::from(c.center)).length() < 1e-6, "{p}");
            let s = to_screen(&c, 640.0, 480.0, c.center).unwrap();
            assert!((s[0] - 320.0).abs() < 1e-6 && (s[1] - 240.0).abs() < 1e-6);
        }
    }

    #[test]
    fn zoom_and_orbit() {
        let mut c = cam();
        zoom(&mut c, 1.0);
        assert!((c.distance - 300.0 * 0.999f64.powi(120)).abs() < 1e-9);
        orbit(&mut c, 10.0, 100.0);
        assert_eq!(c.azimuth, -70.0);
        assert_eq!(c.elevation, 90.0);
    }

    #[test]
    fn fit_contains_the_box() {
        let c = fit(
            &cam(),
            [-50.0, -50.0, -10.0],
            [50.0, 50.0, 10.0],
            640.0,
            480.0,
        );
        let vw = View::new(&c, 640.0, 480.0);
        for x in [-50.0, 50.0] {
            for y in [-50.0, 50.0] {
                let p = vw.view_proj().project_point3(DVec3::new(x, y, 10.0));
                assert!(p.x.abs() <= 1.0 && p.y.abs() <= 1.0, "{p}");
            }
        }
    }
}
