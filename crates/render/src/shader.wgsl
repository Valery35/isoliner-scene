// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

// Тела и линии сцены. Вершины приходят сдвинутыми на центр сцены,
// без преувеличения: высота умножается на vex здесь, так ползунок
// преувеличения не требует новой выгрузки в видеокарту.
// Нормаль грани - по производным положения: тела модуля делят вершины
// между гранями под прямым углом, и сглаженная нормаль размывала бы
// рёбра штрека.

struct U {
    view_proj: mat4x4<f32>,
    // xyz - направление на свет в координатах просмотра
    light: vec4<f32>,
    // x - преувеличение, y - общая непрозрачность
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: U;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) world: vec3<f32>,
};

@vertex
fn vs(@location(0) p: vec3<f32>, @location(1) c: vec4<f32>) -> VsOut {
    let w = vec3<f32>(p.x, p.y, p.z * u.params.x);
    var out: VsOut;
    out.pos = u.view_proj * vec4<f32>(w, 1.0);
    out.color = c;
    out.world = w;
    return out;
}

@fragment
fn fs_mesh(v: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(cross(dpdx(v.world), dpdy(v.world)));
    let l = normalize(u.light.xyz);
    // свет с двух сторон: у поверхности пласта нет «изнанки»
    let d = abs(dot(n, l));
    let shade = 0.38 + 0.62 * d;
    return vec4<f32>(v.color.rgb * shade, v.color.a * u.params.y);
}

@fragment
fn fs_line(v: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(v.color.rgb, v.color.a * u.params.y);
}
