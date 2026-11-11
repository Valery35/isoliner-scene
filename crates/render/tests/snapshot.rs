// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Кадр сцены демо без окна. Без видеокарты (и без программной
//! отрисовки вроде lavapipe) тест пропускается с записью в журнал.
//! Кадр пишется в target/snapshots/scene_demo.ppm для просмотра.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use render::{BG_GRADIENT, BG_PLAIN, GpuScene, Look, Renderer, Target, headless, read_rgba};
use scene_core::{Frame, mesh};
use scene_format::read_package;

/// Цвет sRGB в байтах, как его видит окно модуля.
fn bytes(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| (v * 255.0).round() as u8)
}

/// Доля пикселей ровного фона окна модуля, 250, 250, 248.
fn background_share(rgba: &[u8]) -> f64 {
    let bg = bytes(BG_PLAIN.bottom);
    let n = rgba.len() / 4;
    let same = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| (0..3).all(|k| (i16::from(p[k]) - i16::from(bg[k])).abs() <= 2))
        .count();
    same as f64 / n as f64
}

/// Пакет с пробами: круглые значки видны своим цветом и своим
/// размером в пикселях, вдвое крупнее при двух пикселях на точку.
#[test]
fn point_marks_render_in_screen_pixels() {
    let (device, queue) = match headless() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("видеокарты нет, кадр не проверяется: {e}");
            return;
        }
    };
    let path = format!(
        "{}/../../fixtures/scene_points.i3dscene",
        env!("CARGO_MANIFEST_DIR")
    );
    let pkg = read_package(&std::fs::read(path).unwrap()).unwrap();
    let (lo, hi) = mesh::bounds(&pkg.parts).unwrap();
    let frame = Frame::around(lo, hi, pkg.meta.vex);
    let mut scene = GpuScene::upload(&device, &pkg.parts, frame);
    assert_eq!(scene.parts.len(), pkg.parts.len());
    let probes = scene.parts.iter().position(|p| p.points).unwrap();
    // только пробы: их пиксели не спутать с другими частями
    for (i, p) in scene.parts.iter_mut().enumerate() {
        p.visible = i == probes;
    }
    let renderer = Renderer::new(&device);
    let target = Target::new(&device, 800, 600);
    let mut look = Look {
        camera: frame.camera_to_view(&pkg.meta.camera.unwrap()),
        vex: frame.vex,
        opacity: 1.0,
        background: BG_PLAIN,
        pixels_per_point: 1.0,
    };
    // сколько пикселей не фон
    let ink = |look: &Look| {
        renderer.render(&device, &queue, &target, &scene, look);
        let rgba = read_rgba(&device, &queue, &target).unwrap();
        let bg = bytes(BG_PLAIN.bottom);
        rgba.as_chunks::<4>()
            .0
            .iter()
            .filter(|p| (0..3).any(|k| p[k].abs_diff(bg[k]) > 8))
            .count()
    };
    let one = ink(&look);
    // 12 кругов 6, 9 и 12 пикселей: площадь около 12 * 75 пикселей,
    // часть может накрыть друг друга
    assert!(one > 200 && one < 2000, "пикселей {one}");
    look.pixels_per_point = 2.0;
    let two = ink(&look);
    assert!(
        two as f64 > one as f64 * 2.5 && (two as f64) < one as f64 * 5.0,
        "{one} -> {two}"
    );
    // цвет первой пробы на месте
    look.pixels_per_point = 1.0;
    renderer.render(&device, &queue, &target, &scene, &look);
    let rgba = read_rgba(&device, &queue, &target).unwrap();
    let c = pkg.parts[probes].colors.as_ref().unwrap()[0];
    let want = bytes([c[0], c[1], c[2]]);
    assert!(
        rgba.as_chunks::<4>()
            .0
            .iter()
            .any(|p| (0..3).all(|k| p[k].abs_diff(want[k]) <= 2)),
        "нет цвета {want:?}"
    );
}

#[test]
fn demo_scene_renders_with_its_camera() {
    let (device, queue) = match headless() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("видеокарты нет, кадр не проверяется: {e}");
            return;
        }
    };
    let path = format!(
        "{}/../../fixtures/scene_demo.i3dscene",
        env!("CARGO_MANIFEST_DIR")
    );
    let pkg = read_package(&std::fs::read(path).unwrap()).unwrap();
    let (lo, hi) = mesh::bounds(&pkg.parts).unwrap();
    let frame = Frame::around(lo, hi, pkg.meta.vex);
    let scene = GpuScene::upload(&device, &pkg.parts, frame);
    assert_eq!(scene.parts.len(), 5);
    assert!(scene.parts.iter().any(|p| p.transparent));
    let renderer = Renderer::new(&device);
    let target = Target::new(&device, 800, 600);
    let look = Look {
        camera: frame.camera_to_view(&pkg.meta.camera.unwrap()),
        vex: frame.vex,
        opacity: 1.0,
        background: BG_PLAIN,
        pixels_per_point: 1.0,
    };
    renderer.render(&device, &queue, &target, &scene, &look);
    let rgba = read_rgba(&device, &queue, &target).unwrap();
    assert_eq!(rgba.len(), 800 * 600 * 4);
    let share = background_share(&rgba);
    // сцена занимает кадр, но не весь
    assert!(share > 0.2 && share < 0.95, "фон {share}");
    // угол кадра - фон
    assert!(background_share(&rgba[..4 * 10]) > 0.99);

    let dir = format!("{}/../../target/snapshots", env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    let mut ppm = format!("P6\n{} {}\n255\n", target.width, target.height).into_bytes();
    for p in rgba.as_chunks::<4>().0 {
        ppm.extend_from_slice(&p[..3]);
    }
    std::fs::write(format!("{dir}/scene_demo.ppm"), ppm).unwrap();

    // камера на пустое место - один фон
    let mut away = look;
    away.camera.center[0] += 1.0e5;
    renderer.render(&device, &queue, &target, &scene, &away);
    let rgba = read_rgba(&device, &queue, &target).unwrap();
    assert!(background_share(&rgba) > 0.999);

    // градиент окна модуля: верх кадра холодный, низ светлый, числа
    // sRGB те же, что в QGIS
    away.background = BG_GRADIENT;
    renderer.render(&device, &queue, &target, &scene, &away);
    let rgba = read_rgba(&device, &queue, &target).unwrap();
    let px = |row: usize| {
        let i = (row * 800 + 400) * 4;
        [rgba[i], rgba[i + 1], rgba[i + 2]]
    };
    let near = |a: [u8; 3], b: [u8; 3]| (0..3).all(|k| a[k].abs_diff(b[k]) <= 2);
    assert!(near(px(0), bytes(BG_GRADIENT.top)), "верх {:?}", px(0));
    assert!(
        near(px(599), bytes(BG_GRADIENT.bottom)),
        "низ {:?}",
        px(599)
    );
    let mid = px(300);
    assert!(mid[0] > px(0)[0] && mid[0] < px(599)[0], "середина {mid:?}");

    // кадр с градиентом - для сверки со снимком окна модуля
    let mut look = look;
    look.background = BG_GRADIENT;
    renderer.render(&device, &queue, &target, &scene, &look);
    let rgba = read_rgba(&device, &queue, &target).unwrap();
    let mut ppm = format!("P6\n{} {}\n255\n", target.width, target.height).into_bytes();
    for p in rgba.as_chunks::<4>().0 {
        ppm.extend_from_slice(&p[..3]);
    }
    std::fs::write(format!("{dir}/scene_demo_gradient.ppm"), ppm).unwrap();
}
