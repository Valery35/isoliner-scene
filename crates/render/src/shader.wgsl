// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

// Тела и линии сцены. Вершины приходят сдвинутыми на центр сцены,
// без преувеличения: высота умножается на vex здесь, так ползунок
// преувеличения не требует новой выгрузки в видеокарту.
// Нормаль грани - по производным положения: тела модуля делят вершины
// между гранями под прямым углом, и сглаженная нормаль размывала бы
// рёбра штрека.
//
// Свет и фон - как в окне модуля (lights.py, viewer_dialog.py). Окно
// модуля считает цвет прямо в числах экрана, кадр здесь - текстура
// без sRGB (render::COLOR_FORMAT), и числа на выходе те же, что
// в окне QGIS.

struct U {
    view_proj: mat4x4<f32>,
    // три источника света в координатах просмотра, w - вес
    lights: array<vec4<f32>, 3>,
    // x - преувеличение, y - общая непрозрачность
    params: vec4<f32>,
    // фон: верх и низ кадра, sRGB
    bg_top: vec4<f32>,
    bg_bottom: vec4<f32>,
    // ширина и высота кадра в пикселях, пикселей на точку экрана
    screen: vec4<f32>,
    // вид и проекция со стороны Солнца
    light_vp: mat4x4<f32>,
    // направление на Солнце, w - сила тени (0 - теней нет)
    sun: vec4<f32>,
    // x - шаг карты теней в долях, y - сдвиг глубины
    shadow: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: U;
@group(1) @binding(0) var shadow_map: texture_depth_2d;
@group(1) @binding(1) var shadow_cmp: sampler_comparison;

// Карта теней: глубина тел со стороны Солнца.
@vertex
fn vs_shadow(@location(0) p: vec3<f32>, @location(1) c: vec4<f32>) -> @builtin(position) vec4<f32> {
    return u.light_vp * vec4<f32>(p.x, p.y, p.z * u.params.x, 1.0);
}

// Доля света в точке: 1 - освещена, 0 - в тени. Края мягкие: девять
// отборов вокруг точки. Отбор без уровней (Level) - он законен
// и в ветвлении, и на WebGL2.
fn lit(world: vec3<f32>) -> f32 {
    let lp = u.light_vp * vec4<f32>(world, 1.0);
    let uv = vec2<f32>(lp.x * 0.5 + 0.5, 0.5 - lp.y * 0.5);
    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || lp.z > 1.0 {
        return 1.0;
    }
    let d = lp.z - u.shadow.y;
    var sum = 0.0;
    for (var i = -1; i <= 1; i++) {
        for (var j = -1; j <= 1; j++) {
            let o = vec2<f32>(f32(i), f32(j)) * u.shadow.x;
            sum += textureSampleCompareLevel(shadow_map, shadow_cmp, uv + o, d);
        }
    }
    return sum / 9.0;
}

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

// Мягкий свет окна модуля (lights.soft_shader): три источника,
// освещённость по модулю косинуса - у поверхности нет изнанки, и грань,
// отвёрнутая от источника, не чернеет.
@fragment
fn fs_mesh(v: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(cross(dpdx(v.world), dpdy(v.world)));
    var p = 0.0;
    for (var i = 0; i < 3; i++) {
        p += u.lights[i].w * abs(dot(n, u.lights[i].xyz));
    }
    // params.z - сила отмывки: 0 - ровный цвет, 1 - свет окна модуля
    var k = mix(1.0, 0.32 + 0.78 * p, u.params.z);
    // тень от Солнца: в тени остаётся рассеянный свет
    if u.sun.w > 0.0 {
        k *= 1.0 - u.sun.w * 0.6 * (1.0 - lit(v.world));
    }
    let rgb = clamp(v.color.rgb * k, vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(rgb, v.color.a * u.params.y);
}

@fragment
fn fs_line(v: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(v.color.rgb, v.color.a * u.params.y);
}

// Точки-значки: круг постоянного размера в пикселях экрана, как спрайт
// pxMode окна модуля. Квадрат из шести вершин раздвигается от точки
// в координатах отсечения, лишнее срезается по кругу.
struct PtOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_point(
    @builtin(vertex_index) vi: u32,
    @location(0) p: vec3<f32>,
    @location(1) c: vec4<f32>,
    @location(2) size: f32,
) -> PtOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let k = corners[vi % 6u];
    var clip = u.view_proj * vec4<f32>(p.x, p.y, p.z * u.params.x, 1.0);
    // полразмера в пикселях кадра, в долях кадра от -1 до 1
    let half = 0.5 * size * u.screen.z;
    clip.x += k.x * half * 2.0 / max(u.screen.x, 1.0) * clip.w;
    clip.y += k.y * half * 2.0 / max(u.screen.y, 1.0) * clip.w;
    var out: PtOut;
    out.pos = clip;
    out.color = c;
    out.uv = k;
    return out;
}

@fragment
fn fs_point(v: PtOut) -> @location(0) vec4<f32> {
    if dot(v.uv, v.uv) > 1.0 {
        discard;
    }
    return v.color;
}

// Фон: треугольник на весь кадр, сверху холоднее, снизу светлее.
struct BgOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) up: f32,
};

@vertex
fn vs_bg(@builtin(vertex_index) i: u32) -> BgOut {
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    var out: BgOut;
    out.pos = vec4<f32>(x, y, 1.0, 1.0);
    out.up = (y + 1.0) / 2.0;
    return out;
}

@fragment
fn fs_bg(v: BgOut) -> @location(0) vec4<f32> {
    let c = mix(u.bg_bottom.rgb, u.bg_top.rgb, clamp(v.up, 0.0, 1.0));
    return vec4<f32>(c, 1.0);
}
