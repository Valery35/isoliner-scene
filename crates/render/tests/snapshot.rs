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

/// Тесты кадра идут по одному: два устройства видеокарты, создаваемые
/// одновременно из параллельных тестов, изредка вешали весь прогон.
static GPU: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn gpu_turn() -> std::sync::MutexGuard<'static, ()> {
    GPU.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

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
    let _turn = gpu_turn();
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
        light: 1.0,
        sun: None,
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

    // кадр всей сцены с пробами - для сверки со снимком окна модуля
    for p in scene.parts.iter_mut() {
        p.visible = true;
    }
    look.background = BG_GRADIENT;
    renderer.render(&device, &queue, &target, &scene, &look);
    let rgba = read_rgba(&device, &queue, &target).unwrap();
    let dir = format!("{}/../../target/snapshots", env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    let mut ppm = format!("P6\n{} {}\n255\n", target.width, target.height).into_bytes();
    for p in rgba.as_chunks::<4>().0 {
        ppm.extend_from_slice(&p[..3]);
    }
    std::fs::write(format!("{dir}/scene_points.ppm"), ppm).unwrap();
}

/// Куб на плите под Солнцем: тень ложится на плиту, сила тени ноль -
/// кадр как без Солнца. Кадр пишется в target/snapshots/shadow.ppm.
#[test]
fn sun_casts_a_shadow() {
    use scene_format::{Indices, Part};
    let _turn = gpu_turn();
    let (device, queue) = match headless() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("видеокарты нет, кадр не проверяется: {e}");
            return;
        }
    };
    let quad = |verts: Vec<[f64; 3]>, faces: Vec<[u32; 3]>, c: [f32; 4]| Part {
        name: String::new(),
        colors: Some(vec![c; verts.len()]),
        verts,
        indices: Indices::Triangles(faces),
        sizes: None,
    };
    let plate = quad(
        vec![
            [-100.0, -100.0, 0.0],
            [100.0, -100.0, 0.0],
            [100.0, 100.0, 0.0],
            [-100.0, 100.0, 0.0],
        ],
        vec![[0, 1, 2], [0, 2, 3]],
        [0.8, 0.8, 0.8, 1.0],
    );
    let mut v = Vec::new();
    for z in [0.0, 40.0] {
        for (x, y) in [(-10.0, -10.0), (10.0, -10.0), (10.0, 10.0), (-10.0, 10.0)] {
            v.push([x, y, z]);
        }
    }
    let f = vec![
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    let cube = quad(v, f, [0.6, 0.3, 0.2, 1.0]);
    let parts = vec![plate, cube];
    let frame = Frame::default();
    let scene = GpuScene::upload(&device, &parts, frame);
    let renderer = Renderer::new(&device);
    let target = Target::new(&device, 400, 400);
    let mut look = Look {
        camera: scene_core::Camera {
            center: [0.0, 0.0, 0.0],
            distance: 400.0,
            azimuth: 0.0,
            elevation: 90.0,
            fov: 60.0,
            ortho: true,
        },
        vex: 1.0,
        opacity: 1.0,
        light: 0.0,
        sun: None,
        background: BG_PLAIN,
        pixels_per_point: 1.0,
    };
    let shot = |look: &Look| {
        renderer.render(&device, &queue, &target, &scene, look);
        read_rgba(&device, &queue, &target).unwrap()
    };
    let plain = shot(&look);
    let darker = |a: &[u8], b: &[u8]| {
        a.as_chunks::<4>()
            .0
            .iter()
            .zip(b.as_chunks::<4>().0)
            .filter(|(p, q)| i16::from(p[0]) - i16::from(q[0]) > 30)
            .count()
    };
    look.sun = Some(render::Sun {
        azimuth: 180.0,
        altitude: 45.0,
        strength: 0.0,
    });
    assert_eq!(darker(&plain, &shot(&look)), 0, "тень силы ноль");
    look.sun = Some(render::Sun {
        azimuth: 180.0,
        altitude: 45.0,
        strength: 1.0,
    });
    let lit = shot(&look);
    // тень куба 20 x 40 м на плите: сотни пикселей кадра
    let n = darker(&plain, &lit);
    assert!(n > 300 && n < 40_000, "в тени пикселей {n}");
    let dir = format!("{}/../../target/snapshots", env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    let mut ppm = format!("P6\n{} {}\n255\n", target.width, target.height).into_bytes();
    for p in lit.as_chunks::<4>().0 {
        ppm.extend_from_slice(&p[..3]);
    }
    std::fs::write(format!("{dir}/shadow.ppm"), ppm).unwrap();
}

/// Сцена модуля для сверки теней (рельеф, холмы, гребень, плита):
/// с тенями заметная доля рельефа темнее, чем без них. Кадр - в
/// target/snapshots/shadow_demo.ppm, рядом с ним снимок модуля
/// fixtures/shadow_demo_module.png той же камерой.
#[test]
fn module_shadow_scene() {
    let _turn = gpu_turn();
    let Ok((device, queue)) = headless() else {
        return;
    };
    let path = format!(
        "{}/../../fixtures/shadow_demo.i3dscene",
        env!("CARGO_MANIFEST_DIR")
    );
    let pkg = read_package(&std::fs::read(path).unwrap()).unwrap();
    let (lo, hi) = mesh::bounds(&pkg.parts).unwrap();
    let frame = Frame::around(lo, hi, pkg.meta.vex);
    let mut scene = GpuScene::upload(&device, &pkg.parts, frame);
    // оформление пакета окно рисует само
    for (p, info) in scene.parts.iter_mut().zip(&pkg.meta.parts) {
        p.visible = !info.decor;
    }
    let st = &pkg.meta.state;
    let num = |k: &str| st.get(k).and_then(serde_json::Value::as_f64).unwrap();
    let renderer = Renderer::new(&device);
    let target = Target::new(&device, 800, 600);
    let mut look = Look {
        camera: frame.camera_to_view(&pkg.meta.camera.unwrap()),
        vex: frame.vex,
        opacity: 1.0,
        light: (num("light") / 100.0) as f32,
        sun: Some(render::Sun {
            azimuth: num("sun_az"),
            altitude: num("sun_alt"),
            strength: 0.0,
        }),
        background: BG_PLAIN,
        pixels_per_point: 1.0,
    };
    renderer.render(&device, &queue, &target, &scene, &look);
    let plain = read_rgba(&device, &queue, &target).unwrap();
    look.sun = look.sun.map(|s| render::Sun {
        strength: (num("shadow_k") / 100.0) as f32,
        ..s
    });
    renderer.render(&device, &queue, &target, &scene, &look);
    let lit = read_rgba(&device, &queue, &target).unwrap();
    let darker = plain
        .as_chunks::<4>()
        .0
        .iter()
        .zip(lit.as_chunks::<4>().0)
        .filter(|(p, q)| i16::from(p[0]) - i16::from(q[0]) > 20)
        .count();
    let share = darker as f64 / (800.0 * 600.0);
    // у модуля в тени около пятой части кадра
    assert!(share > 0.05 && share < 0.4, "в тени {share}");
    let dir = format!("{}/../../target/snapshots", env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    let mut ppm = format!("P6\n{} {}\n255\n", target.width, target.height).into_bytes();
    for p in lit.as_chunks::<4>().0 {
        ppm.extend_from_slice(&p[..3]);
    }
    std::fs::write(format!("{dir}/shadow_demo.ppm"), ppm).unwrap();
}

/// Цена теней на большой сцене: рельеф 1000 x 1000 ячеек (два
/// миллиона треугольников), кадр 1600 x 900 с тенями и без. Замер,
/// а не проверка: `cargo test -p render --release -- --ignored
/// shadow_cost --nocapture`.
#[test]
#[ignore]
fn shadow_cost() {
    use scene_format::{Indices, Part};
    let _turn = gpu_turn();
    let Ok((device, queue)) = headless() else {
        return;
    };
    let n = 1000u32;
    let mut verts = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            let (x, y) = (f64::from(i), f64::from(j));
            verts.push([x, y, 40.0 * (x / 90.0).sin() * (y / 70.0).cos()]);
        }
    }
    let mut faces = Vec::new();
    for j in 0..n {
        for i in 0..n {
            let a = j * (n + 1) + i;
            faces.push([a, a + 1, a + n + 2]);
            faces.push([a, a + n + 2, a + n + 1]);
        }
    }
    let part = Part {
        name: String::new(),
        colors: None,
        verts,
        indices: Indices::Triangles(faces),
        sizes: None,
    };
    let frame = Frame {
        cx: 500.0,
        cy: 500.0,
        cz: 0.0,
        vex: 1.0,
    };
    let scene = GpuScene::upload(&device, &[part], frame);
    let renderer = Renderer::new(&device);
    let target = Target::new(&device, 1600, 900);
    let mut look = Look {
        camera: scene_core::Camera {
            center: [0.0, 0.0, 0.0],
            distance: 1500.0,
            azimuth: 30.0,
            elevation: 35.0,
            fov: 60.0,
            ortho: false,
        },
        vex: 1.0,
        opacity: 1.0,
        light: 0.55,
        sun: Some(render::Sun {
            azimuth: 218.0,
            altitude: 35.0,
            strength: 0.0,
        }),
        background: BG_PLAIN,
        pixels_per_point: 1.0,
    };
    let time = |look: &Look| {
        // первый кадр - прогрев
        renderer.render(&device, &queue, &target, &scene, look);
        read_rgba(&device, &queue, &target).unwrap();
        let t = std::time::Instant::now();
        for _ in 0..10 {
            renderer.render(&device, &queue, &target, &scene, look);
            read_rgba(&device, &queue, &target).unwrap();
        }
        t.elapsed().as_secs_f64() * 100.0
    };
    let plain = time(&look);
    look.sun = look.sun.map(|s| render::Sun { strength: 0.5, ..s });
    let shadow = time(&look);
    eprintln!("кадр: без теней {plain:.1} мс, с тенями {shadow:.1} мс");
}

#[test]
fn demo_scene_renders_with_its_camera() {
    let _turn = gpu_turn();
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
        light: 1.0,
        sun: None,
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
