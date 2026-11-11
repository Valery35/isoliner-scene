// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Облёт сцены для видео: позы камеры по кадрам.
//!
//! Перенос `flight.py` модуля, сверяется с `fixtures/flight.json`.
//! Оборот - камера идёт вокруг точки взгляда на заданный угол.
//! Облёт по видам - переход от вида к виду с паузой на каждом.
//! Переход плавный (smoothstep), азимут идёт короткой дугой,
//! удаление меняется в логарифме, проекция переключается на середине.

use scene_format::Camera;

/// Кадров в секунду по умолчанию.
pub const FPS: u32 = 25;

/// Количество кадров на отрезок `seconds`: не меньше одного.
///
/// Округление к чётному, как `round` в Python: 12.5 кадра - 12.
pub fn frame_count(seconds: f64, fps: u32) -> usize {
    let n = (seconds * f64::from(fps)).round_ties_even();
    if n.is_finite() && n >= 1.0 {
        n as usize
    } else {
        1
    }
}

/// Плавная доля пути: 0 и 1 с нулевой скоростью на концах.
pub fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Поворот от азимута `a` к `b` короткой дугой, градусы, -180..180.
pub fn turn(a: f64, b: f64) -> f64 {
    let d = (b - a).rem_euclid(360.0);
    if d > 180.0 { d - 360.0 } else { d }
}

fn clean(cam: &Camera) -> Camera {
    Camera {
        distance: cam.distance.max(1e-9),
        ..*cam
    }
}

/// Камера между видами `a` и `b` на доле `t` (уже сглаженной).
pub fn blend(a: &Camera, b: &Camera, t: f64) -> Camera {
    let (a, b) = (clean(a), clean(b));
    let t = t.clamp(0.0, 1.0);
    let mut center = [0.0; 3];
    for (k, c) in center.iter_mut().enumerate() {
        *c = a.center[k] + (b.center[k] - a.center[k]) * t;
    }
    let (la, lb) = (a.distance.ln(), b.distance.ln());
    Camera {
        center,
        distance: (la + (lb - la) * t).exp(),
        azimuth: (a.azimuth + turn(a.azimuth, b.azimuth) * t).rem_euclid(360.0),
        elevation: a.elevation + (b.elevation - a.elevation) * t,
        fov: a.fov + (b.fov - a.fov) * t,
        ortho: if t >= 0.5 { b.ortho } else { a.ortho },
    }
}

/// Кадры оборота вокруг точки взгляда.
///
/// Последний кадр оборота на 360° не повторяет первый: ролик,
/// пущенный по кругу, иначе запинается на стыке.
pub fn orbit(cam: &Camera, degrees: f64, seconds: f64, fps: u32) -> Vec<Camera> {
    let cam = clean(cam);
    let n = frame_count(seconds, fps);
    let full = (degrees.abs() - 360.0).abs() < 1e-9;
    let steps = if full { n } else { n.saturating_sub(1).max(1) };
    (0..n)
        .map(|i| Camera {
            azimuth: (cam.azimuth + degrees * i as f64 / steps as f64).rem_euclid(360.0),
            ..cam
        })
        .collect()
}

/// Кадры облёта по видам: пауза на виде, переход, пауза...
///
/// Один вид - только пауза на нём. Пустой список - пустой облёт.
pub fn tour(views: &[Camera], leg: f64, pause: f64, fps: u32) -> Vec<Camera> {
    let views: Vec<Camera> = views.iter().map(clean).collect();
    let Some(last) = views.last() else {
        return Vec::new();
    };
    let hold = if pause > 0.0 {
        frame_count(pause, fps)
    } else {
        0
    };
    let moves = frame_count(leg, fps);
    let mut out = Vec::new();
    for (i, v) in views.iter().enumerate() {
        out.extend(std::iter::repeat_n(*v, hold));
        if let Some(next) = views.get(i + 1) {
            for k in 1..=moves {
                out.push(blend(v, next, smoothstep(k as f64 / moves as f64)));
            }
        }
    }
    if out.is_empty() {
        out.push(*last);
    }
    out
}

/// Длительность ролика из `frames` кадров, секунды.
pub fn duration(frames: usize, fps: u32) -> f64 {
    frames as f64 / f64::from(fps.max(1))
}
