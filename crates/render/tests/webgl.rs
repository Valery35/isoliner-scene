// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Шейдер сцены переводится в GLSL ES 3.00 - язык WebGL2, запасного
//! пути страницы в браузере без WebGPU. Перевод делает тот же naga,
//! что и wgpu на странице: не перевёлся здесь - сцена в таком браузере
//! не открылась бы.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use naga::back::glsl;
use naga::valid::{Capabilities, ValidationFlags, Validator};

#[test]
fn shader_translates_to_webgl2() {
    let src = include_str!("../src/shader.wgsl");
    let module = naga::front::wgsl::parse_str(src).expect("WGSL разбирается");
    // возможности WebGL2 у wgpu: без вычислений и хранилищ
    let info = Validator::new(ValidationFlags::all(), Capabilities::empty())
        .validate(&module)
        .expect("шейдер проходит проверку");
    let options = glsl::Options {
        version: glsl::Version::Embedded {
            version: 300,
            is_webgl: true,
        },
        ..glsl::Options::default()
    };
    let mut done = Vec::new();
    for ep in &module.entry_points {
        let pipeline = glsl::PipelineOptions {
            shader_stage: ep.stage,
            entry_point: ep.name.clone(),
            multiview: None,
        };
        let mut out = String::new();
        let mut w = glsl::Writer::new(
            &mut out,
            &module,
            &info,
            &options,
            &pipeline,
            naga::proc::BoundsCheckPolicies::default(),
        )
        .unwrap_or_else(|e| panic!("{}: {e}", ep.name));
        w.write().unwrap_or_else(|e| panic!("{}: {e}", ep.name));
        done.push(ep.name.clone());
    }
    for name in [
        "vs",
        "fs_mesh",
        "vs_shadow",
        "fs_line",
        "vs_point",
        "fs_point",
        "vs_bg",
        "fs_bg",
    ] {
        assert!(done.iter().any(|d| d == name), "нет {name}");
    }
}
