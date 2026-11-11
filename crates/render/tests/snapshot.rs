// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Кадр сцены демо без окна. Без видеокарты (и без программной
//! отрисовки вроде lavapipe) тест пропускается с записью в журнал.
//! Кадр пишется в target/snapshots/scene_demo.ppm для просмотра.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use render::{BACKGROUND, GpuScene, Look, Renderer, Target, headless, read_rgba};
use scene_core::{Frame, mesh};
use scene_format::read_package;

fn background_share(rgba: &[u8]) -> f64 {
    let bg: Vec<u8> = BACKGROUND[..3]
        .iter()
        .map(|c| {
            // sRGB, как пишет текстура кадра
            let c = *c;
            let s = if c <= 0.0031308 {
                12.92 * c
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            };
            (s * 255.0).round() as u8
        })
        .collect();
    let n = rgba.len() / 4;
    let same = rgba
        .chunks_exact(4)
        .filter(|p| (0..3).all(|k| (i16::from(p[k]) - i16::from(bg[k])).abs() <= 2))
        .count();
    same as f64 / n as f64
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
    for p in rgba.chunks_exact(4) {
        ppm.extend_from_slice(&p[..3]);
    }
    std::fs::write(format!("{dir}/scene_demo.ppm"), ppm).unwrap();

    // камера на пустое место - один фон
    let mut away = look;
    away.camera.center[0] += 1.0e5;
    renderer.render(&device, &queue, &target, &scene, &away);
    let rgba = read_rgba(&device, &queue, &target).unwrap();
    assert!(background_share(&rgba) > 0.999);
}
