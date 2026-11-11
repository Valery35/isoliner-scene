// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Тело вдоль оси: выработка, тоннель, канава. Перенос `sweep.py`.
//!
//! Сечение ставится поперёк оси в каждой её вершине и протягивается
//! по ней, торцы закрываются веером. Поперечная ось сечения всегда
//! горизонтальна. На изломе сечение стоит по биссектрисе, ширина
//! растягивается на 1/cos половины угла поворота.

/// Виды сечения, как в модуле.
pub const PROFILES: [&str; 3] = ["circle", "rect", "arch"];
/// Излом круче этого, градусы, даёт самопересечение тела.
pub const MAX_TURN: f64 = 100.0;

type V3 = [f64; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: V3) -> f64 {
    dot(a, a).sqrt()
}
/// Единичный вектор. Нулевой остаётся нулевым, как `_unit` модуля.
fn unit(a: V3) -> V3 {
    let n = norm(a);
    if n > 0.0 { scale(a, 1.0 / n) } else { a }
}

/// Контур сечения против часовой стрелки, низ на v = 0.
///
/// circle - круг диаметра `width`. rect - прямоугольник. arch -
/// прямоугольник со сводом подъёмом `rise` (None - полуциркульный).
pub fn profile(
    kind: &str,
    width: f64,
    height: Option<f64>,
    rise: Option<f64>,
    segments: i64,
) -> Result<Vec<[f64; 2]>, String> {
    let w = width;
    if w.is_nan() || w <= 0.0 {
        return Err("width must be positive".into());
    }
    let n = segments.max(6) as usize;
    if kind == "circle" {
        let r = w / 2.0;
        return Ok((0..n)
            .map(|i| {
                let a =
                    -std::f64::consts::FRAC_PI_2 + 2.0 * std::f64::consts::PI * i as f64 / n as f64;
                [r * a.cos(), r + r * a.sin()]
            })
            .collect());
    }
    let h = height.unwrap_or(w);
    if h.is_nan() || h <= 0.0 {
        return Err("height must be positive".into());
    }
    match kind {
        "rect" => Ok(vec![
            [-w / 2.0, 0.0],
            [w / 2.0, 0.0],
            [w / 2.0, h],
            [-w / 2.0, h],
        ]),
        "arch" => {
            let f = match rise {
                Some(r) if r != 0.0 => r,
                _ => w / 2.0,
            };
            let f = f.max(1e-6 * w).min(w / 2.0);
            let rad = (w * w / 4.0 + f * f) / (2.0 * f);
            let cy = h + f - rad;
            let a0 = (h - cy).atan2(w / 2.0);
            let a1 = std::f64::consts::PI - a0;
            let k = (n / 2).max(3);
            let mut out = vec![[-w / 2.0, 0.0], [w / 2.0, 0.0], [w / 2.0, h]];
            for i in 1..k {
                let ang = a0 + (a1 - a0) * i as f64 / k as f64;
                out.push([rad * ang.cos(), cy + rad * ang.sin()]);
            }
            out.push([-w / 2.0, h]);
            Ok(out)
        }
        other => Err(format!("unknown profile {other:?}")),
    }
}

/// Площадь сечения по формуле шнурования.
pub fn section_area(prof: &[[f64; 2]]) -> f64 {
    let n = prof.len();
    let mut a = 0.0;
    let mut b = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        a += prof[i][0] * prof[j][1];
        b += prof[i][1] * prof[j][0];
    }
    0.5 * (a - b).abs()
}

fn clean_axis(pts: &[V3]) -> Vec<V3> {
    let mut out: Vec<V3> = Vec::new();
    for &p in pts {
        match out.last() {
            Some(&q) if norm(sub(p, q)) <= 1e-9 => {}
            _ => out.push(p),
        }
    }
    out
}

/// Рамки сечений по оси: точки, поперечник u, верх v, растяжение,
/// излом в градусах.
#[derive(Debug, Clone)]
pub struct Frames {
    pub points: Vec<V3>,
    pub u: Vec<V3>,
    pub v: Vec<V3>,
    pub stretch: Vec<f64>,
    pub turn: Vec<f64>,
}

pub fn frames(axis: &[V3]) -> Result<Frames, String> {
    let p = clean_axis(axis);
    if p.len() < 2 {
        return Err("axis needs two distinct points".into());
    }
    let seg: Vec<V3> = p.windows(2).map(|w| unit(sub(w[1], w[0]))).collect();
    let n = p.len();
    let up = [0.0, 0.0, 1.0];
    let mut fr = Frames {
        points: p.clone(),
        u: Vec::with_capacity(n),
        v: Vec::with_capacity(n),
        stretch: Vec::with_capacity(n),
        turn: Vec::with_capacity(n),
    };
    for i in 0..n {
        let (t, k, ang) = if i == 0 {
            (seg[0], 1.0, 0.0)
        } else if i == n - 1 {
            (seg[n - 2], 1.0, 0.0)
        } else {
            let (a, b) = (seg[i - 1], seg[i]);
            let mut t = unit(add(a, b));
            if norm(t) < 1e-9 {
                t = a; // разворот назад
            }
            let c = dot(a, b).clamp(-1.0, 1.0);
            let ang = c.acos().to_degrees();
            (t, 1.0 / (ang.to_radians() / 2.0).cos().max(0.2), ang)
        };
        let mut u = cross(t, up);
        if norm(u) < 1e-9 {
            u = [1.0, 0.0, 0.0]; // вертикальная ось, ствол
        }
        u = unit(u);
        let mut v = unit(cross(u, t));
        if v[2] < 0.0 {
            v = scale(v, -1.0);
            u = scale(u, -1.0);
        }
        fr.u.push(u);
        fr.v.push(v);
        fr.stretch.push(k);
        fr.turn.push(ang);
    }
    Ok(fr)
}

/// Замкнутое тело вдоль оси.
#[derive(Debug, Clone)]
pub struct Swept {
    pub verts: Vec<V3>,
    pub faces: Vec<[u32; 3]>,
    pub length: f64,
    pub max_turn: f64,
}

/// Протянуть сечение `prof` по оси. `centre` - ось по центру сечения,
/// иначе по почве.
pub fn sweep(axis: &[V3], prof: &[[f64; 2]], centre: bool) -> Result<Swept, String> {
    let mut prof = prof.to_vec();
    if centre {
        let lo = prof.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
        let hi = prof.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
        for p in &mut prof {
            p[1] -= (lo + hi) / 2.0;
        }
    }
    let fr = frames(axis)?;
    let m = prof.len();
    let np = fr.points.len();
    let mut verts = Vec::with_capacity(np * m + 2);
    for i in 0..np {
        for q in &prof {
            verts.push(add(
                add(fr.points[i], scale(fr.u[i], q[0] * fr.stretch[i])),
                scale(fr.v[i], q[1]),
            ));
        }
    }
    let mut faces: Vec<[u32; 3]> = Vec::new();
    let idx = |x: usize| x as u32;
    for i in 0..np - 1 {
        let (a0, b0) = (i * m, (i + 1) * m);
        for j in 0..m {
            let j1 = (j + 1) % m;
            faces.push([idx(a0 + j), idx(b0 + j), idx(b0 + j1)]);
            faces.push([idx(a0 + j), idx(b0 + j1), idx(a0 + j1)]);
        }
    }
    let mean = |r: &[V3]| {
        let mut c = [0.0; 3];
        for v in r {
            c = add(c, *v);
        }
        scale(c, 1.0 / r.len() as f64)
    };
    let c0 = verts.len();
    let first = mean(&verts[0..m]);
    let last_ring = mean(&verts[(np - 1) * m..np * m]);
    verts.push(first);
    verts.push(last_ring);
    let last = (np - 1) * m;
    for j in 0..m {
        let j1 = (j + 1) % m;
        faces.push([idx(c0), idx(j), idx(j1)]);
        faces.push([idx(c0 + 1), idx(last + j1), idx(last + j)]);
    }
    if signed_volume(&verts, &faces) < 0.0 {
        for f in &mut faces {
            f.reverse();
        }
    }
    let length = fr.points.windows(2).map(|w| norm(sub(w[1], w[0]))).sum();
    let max_turn = fr.turn.iter().copied().fold(0.0, f64::max);
    Ok(Swept {
        verts,
        faces,
        length,
        max_turn,
    })
}

/// Объём замкнутой сетки со знаком: плюс у наружных нормалей.
/// Как в модуле, от начала координат.
pub fn signed_volume(verts: &[V3], faces: &[[u32; 3]]) -> f64 {
    faces
        .iter()
        .map(|f| {
            let a = verts[f[0] as usize];
            let b = verts[f[1] as usize];
            let c = verts[f[2] as usize];
            dot(a, cross(b, c))
        })
        .sum::<f64>()
        / 6.0
}
