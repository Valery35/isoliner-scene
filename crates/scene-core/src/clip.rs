// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Обрезка тел и поверхностей сцены, как в окне модуля Isoliner3D
//! (`viewer_dialog._clip_tris`, `_cap_cut`, `viewer_core.z_cut`).
//!
//! Порядок для тела: резка в плане по области обрезки, сшивка,
//! вертикальная крышка на срезе, затем резка плоскостями отметок
//! и горизонтальная крышка. Поверхность режется без крышек.
//!
//! Область обрезки - объединение простых многоугольников в плане:
//! контур - сами кольца, коридор - буфер ломаной (прямоугольники
//! отрезков и круги по 12 отрезков на четверть, как `buffer(w, 12)`
//! QGIS), сторона - полоса по эту сторону ломаной, продлённой
//! за концы, шириной 100 км. Граница объединения считается здесь же:
//! отрезок - граница, если по разные стороны от него разная
//! принадлежность области.
//!
//! Грань, пересечённая границей, режется прямыми ближних отрезков
//! границы на выпуклые куски, куски отбираются по середине. Точка
//! на ребре грани считается от его концов в одном порядке, поэтому
//! соседние грани ставят её в одно место и вершины сшиваются.
//! Отвесная грань режется вдоль своего отрезка в плане. Крышки
//! заполняются триангуляцией с закреплёнными рёбрами (`spade`).
//!
//! [`clip_part`] режет часть сцены целиком: цвет новых вершин берётся
//! с ближней точки исходной сетки, линии и точки отбираются
//! по вершинам, как `_clip_run` модуля.

use std::collections::HashMap;

use scene_format::{Indices, Part};
use spade::{ConstrainedDelaunayTriangulation, Point2, Triangulation};

type P2 = [f64; 2];
type P3 = [f64; 3];
type Tri = [u32; 3];

/// Что оставить.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Внутри контура.
    In,
    /// Снаружи контура.
    Out,
    /// Полоса шириной `width` по обе стороны ломаной.
    Corridor,
    /// Слева по ходу ломаной.
    Left,
    /// Справа по ходу ломаной.
    Right,
}

impl Side {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "in" => Self::In,
            "out" => Self::Out,
            "corridor" => Self::Corridor,
            "left" => Self::Left,
            "right" => Self::Right,
            _ => return None,
        })
    }
}

/// Обрезка: область в плане и диапазон отметок.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    pub side: Side,
    /// Кольца контура (для In и Out).
    pub rings: Vec<Vec<P2>>,
    /// Ломаные (для коридора и сторон).
    pub lines: Vec<Vec<P2>>,
    /// Расстояние от ломаной коридора в каждую сторону, метры.
    pub width: f64,
    /// Оставить z >= zlo.
    pub zlo: Option<f64>,
    /// Оставить z <= zhi.
    pub zhi: Option<f64>,
}

/// Ширина полосы стороны, как `far` в `_side_region` модуля.
const FAR: f64 = 1e5;

// ------------------------------------------------------------- плоскость

fn cross(o: P2, a: P2, b: P2) -> f64 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

fn dist2(a: P2, b: P2) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)
}

/// Точка внутри простого многоугольника (чёт-нечет).
fn in_polygon(p: P2, ring: &[P2]) -> bool {
    let mut inside = false;
    let n = ring.len();
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if p[0] < x {
                inside = !inside;
            }
        }
    }
    inside
}

/// Параметры пересечения отрезков p-q и a-b: (t на p-q, u на a-b).
/// Параллельные - None.
fn seg_cross(p: P2, q: P2, a: P2, b: P2) -> Option<(f64, f64)> {
    let r = [q[0] - p[0], q[1] - p[1]];
    let s = [b[0] - a[0], b[1] - a[1]];
    let den = r[0] * s[1] - r[1] * s[0];
    if den.abs() <= 1e-15 * (r[0].hypot(r[1]) * s[0].hypot(s[1])).max(1e-300) {
        return None;
    }
    let w = [a[0] - p[0], a[1] - p[1]];
    let t = (w[0] * s[1] - w[1] * s[0]) / den;
    let u = (w[0] * r[1] - w[1] * r[0]) / den;
    Some((t, u))
}

/// Круг около точки многоугольником: 48 вершин, первая - по направлению
/// `start` (радианы), как дуги буфера QGIS с 12 отрезками на четверть.
fn circle(c: P2, r: f64, start: f64) -> Vec<P2> {
    (0..48)
        .map(|k| {
            let a = start + k as f64 * std::f64::consts::TAU / 48.0;
            [c[0] + r * a.cos(), c[1] + r * a.sin()]
        })
        .collect()
}

/// Ломаная без повторов подряд.
fn clean_line(pts: &[P2]) -> Vec<P2> {
    let mut out: Vec<P2> = Vec::new();
    for p in pts {
        if p.iter().all(|x| x.is_finite()) && out.last().is_none_or(|l| dist2(*l, *p) > 0.0) {
            out.push(*p);
        }
    }
    out
}

fn unit_normal(a: P2, b: P2) -> P2 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l = dx.hypot(dy).max(1e-300);
    [-dy / l, dx / l]
}

/// Область обрезки в плане: объединение простых многоугольников и его
/// граница кольцами.
#[derive(Debug, Clone)]
pub struct Region {
    parts: Vec<Vec<P2>>,
    /// Оставлять снаружи (контур, режим Out).
    outside: bool,
    /// Граница объединения отрезками.
    segs: Vec<(P2, P2)>,
    /// Граница объединения кольцами: по ним меряется путь вдоль среза.
    rings: Vec<Vec<P2>>,
    span: f64,
}

impl Region {
    /// Точка в объединении частей.
    pub fn inside(&self, p: P2) -> bool {
        self.parts.iter().any(|r| in_polygon(p, r))
    }

    /// Остаётся ли точка после обрезки.
    pub fn keep(&self, p: P2) -> bool {
        self.inside(p) != self.outside
    }

    /// Граница кольцами.
    pub fn rings(&self) -> &[Vec<P2>] {
        &self.rings
    }

    /// Граница отрезками.
    pub fn boundary(&self) -> &[(P2, P2)] {
        &self.segs
    }

    fn new(parts: Vec<Vec<P2>>, outside: bool) -> Option<Self> {
        let parts: Vec<Vec<P2>> = parts.into_iter().filter(|r| r.len() >= 3).collect();
        if parts.is_empty() {
            return None;
        }
        let mut lo = [f64::INFINITY; 2];
        let mut hi = [f64::NEG_INFINITY; 2];
        for p in parts.iter().flatten() {
            for k in 0..2 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let span = (hi[0] - lo[0]).max(hi[1] - lo[1]);
        let mut reg = Self {
            parts,
            outside,
            segs: Vec::new(),
            rings: Vec::new(),
            span,
        };
        reg.segs = reg.union_boundary();
        reg.rings = chain_rings(&reg.segs, span);
        Some(reg)
    }

    /// Граница объединения: рёбра частей, разбитые в точках пересечения
    /// с рёбрами других частей; кусок - граница, если по разные стороны
    /// от его середины принадлежность разная.
    fn union_boundary(&self) -> Vec<(P2, P2)> {
        let edges: Vec<(usize, P2, P2)> = self
            .parts
            .iter()
            .enumerate()
            .flat_map(|(i, r)| (0..r.len()).map(move |k| (i, r[k], r[(k + 1) % r.len()])))
            .filter(|(_, a, b)| dist2(*a, *b) > 0.0)
            .collect();
        if self.parts.len() == 1 {
            return edges.iter().map(|(_, a, b)| (*a, *b)).collect();
        }
        // точки разбиения каждого ребра: параметр и сама точка, одна
        // на пару рёбер, чтобы куски сходились в одной точке
        let mut cuts: Vec<Vec<(f64, P2)>> = edges
            .iter()
            .map(|(_, a, b)| vec![(0.0, *a), (1.0, *b)])
            .collect();
        for i in 0..edges.len() {
            for j in i + 1..edges.len() {
                let (pi, a, b) = edges[i];
                let (pj, c, d) = edges[j];
                if pi == pj {
                    continue;
                }
                if a[0].max(b[0]) < c[0].min(d[0])
                    || c[0].max(d[0]) < a[0].min(b[0])
                    || a[1].max(b[1]) < c[1].min(d[1])
                    || c[1].max(d[1]) < a[1].min(b[1])
                {
                    continue;
                }
                if let Some((t, u)) = seg_cross(a, b, c, d)
                    && (-1e-12..=1.0 + 1e-12).contains(&t)
                    && (-1e-12..=1.0 + 1e-12).contains(&u)
                {
                    let p = [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])];
                    cuts[i].push((t, p));
                    cuts[j].push((u, p));
                }
            }
        }
        let mut out = Vec::new();
        for cs in cuts.iter_mut() {
            cs.sort_by(|x, y| x.0.total_cmp(&y.0));
            for w in cs.windows(2) {
                let (a, b) = (w[0].1, w[1].1);
                let len = dist2(a, b).sqrt();
                if len <= 1e-12 {
                    continue;
                }
                let m = [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])];
                let eps = (len * 1e-6).max(1e-9);
                let n = unit_normal(a, b);
                let l = [m[0] + n[0] * eps, m[1] + n[1] * eps];
                let r = [m[0] - n[0] * eps, m[1] - n[1] * eps];
                if self.inside(l) != self.inside(r) {
                    out.push((a, b));
                }
            }
        }
        // совпадающие рёбра соседних частей дают один кусок границы дважды
        let snap = (self.span * 1e-12).max(1e-9);
        let key = |p: P2| ((p[0] / snap).round() as i64, (p[1] / snap).round() as i64);
        let mut seen = std::collections::HashSet::new();
        out.retain(|(a, b)| {
            let (ka, kb) = (key(*a), key(*b));
            seen.insert(if ka <= kb { (ka, kb) } else { (kb, ka) })
        });
        out
    }

    /// Ближайшее кольцо границы и путь вдоль него: (кольцо, путь,
    /// расстояние).
    fn locate(&self, p: P2) -> Option<(usize, f64, f64)> {
        let mut best: Option<(usize, f64, f64)> = None;
        for (ri, ring) in self.rings.iter().enumerate() {
            let mut s = 0.0;
            let n = ring.len();
            for k in 0..n {
                let (a, b) = (ring[k], ring[(k + 1) % n]);
                let l2 = dist2(a, b);
                let len = l2.sqrt();
                let t = if l2 > 0.0 {
                    (((p[0] - a[0]) * (b[0] - a[0]) + (p[1] - a[1]) * (b[1] - a[1])) / l2)
                        .clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let q = [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])];
                let d = dist2(p, q).sqrt();
                if best.is_none_or(|x| d < x.2) {
                    best = Some((ri, s + t * len, d));
                }
                s += len;
            }
        }
        best
    }

    /// Точка кольца на пути `s`.
    fn point_at(&self, ring: usize, s: f64) -> Option<P2> {
        let r = self.rings.get(ring)?;
        let n = r.len();
        let mut acc = 0.0;
        for k in 0..n {
            let (a, b) = (r[k], r[(k + 1) % n]);
            let len = dist2(a, b).sqrt();
            if s <= acc + len || k + 1 == n {
                let t = if len > 0.0 {
                    ((s - acc) / len).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                return Some([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]);
            }
            acc += len;
        }
        None
    }

    fn ring_len(&self, ring: usize) -> f64 {
        self.rings.get(ring).map_or(0.0, |r| {
            (0..r.len())
                .map(|k| dist2(r[k], r[(k + 1) % r.len()]).sqrt())
                .sum()
        })
    }
}

/// Отрезки границы в замкнутые кольца по общим концам.
fn chain_rings(segs: &[(P2, P2)], span: f64) -> Vec<Vec<P2>> {
    let snap = (span * 1e-9).max(1e-9);
    let key = |p: P2| ((p[0] / snap).round() as i64, (p[1] / snap).round() as i64);
    let mut adj: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, (a, b)) in segs.iter().enumerate() {
        adj.entry(key(*a)).or_default().push(i);
        adj.entry(key(*b)).or_default().push(i);
    }
    let mut used = vec![false; segs.len()];
    let mut rings = Vec::new();
    for start in 0..segs.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let (first, mut cur) = segs[start];
        let mut ring = vec![first];
        let mut guard = 0;
        while key(cur) != key(first) && guard <= segs.len() {
            guard += 1;
            ring.push(cur);
            let next = adj
                .get(&key(cur))
                .and_then(|v| v.iter().copied().find(|&i| !used[i]));
            let Some(i) = next else { break };
            used[i] = true;
            let (a, b) = segs[i];
            cur = if key(a) == key(cur) { b } else { a };
        }
        if key(cur) == key(first) && ring.len() >= 3 {
            rings.push(ring);
        }
    }
    rings
}

impl Clip {
    /// Область обрезки в плане или None, если в плане не режется.
    pub fn region(&self) -> Option<Region> {
        match self.side {
            Side::In | Side::Out => {
                let parts: Vec<Vec<P2>> = self.rings.iter().map(|r| clean_ring(r)).collect();
                Region::new(parts, self.side == Side::Out)
            }
            Side::Corridor => {
                let w = self.width;
                if !(w.is_finite() && w > 0.0) {
                    return None;
                }
                let mut parts = Vec::new();
                for line in &self.lines {
                    let p = clean_line(line);
                    for s in p.windows(2) {
                        let n = unit_normal(s[0], s[1]);
                        parts.push(vec![
                            [s[0][0] - n[0] * w, s[0][1] - n[1] * w],
                            [s[1][0] - n[0] * w, s[1][1] - n[1] * w],
                            [s[1][0] + n[0] * w, s[1][1] + n[1] * w],
                            [s[0][0] + n[0] * w, s[0][1] + n[1] * w],
                        ]);
                    }
                    for (k, c) in p.iter().enumerate() {
                        // первая вершина круга - на нормали отрезка: так
                        // углы прямоугольника ложатся на круг
                        let (a, b) = if k + 1 < p.len() {
                            (p[k], p[k + 1])
                        } else if k > 0 {
                            (p[k - 1], p[k])
                        } else {
                            continue;
                        };
                        let n = unit_normal(a, b);
                        parts.push(circle(*c, w, n[1].atan2(n[0])));
                    }
                    if p.len() == 1 {
                        parts.push(circle(p[0], w, 0.0));
                    }
                }
                Region::new(parts, false)
            }
            Side::Left | Side::Right => {
                let sign = if self.side == Side::Left { 1.0 } else { -1.0 };
                let mut parts = Vec::new();
                for line in &self.lines {
                    let p = clean_line(line);
                    if p.len() < 2 {
                        continue;
                    }
                    // концы продлены вдоль крайних отрезков
                    let ext = |a: P2, b: P2| {
                        let (dx, dy) = (a[0] - b[0], a[1] - b[1]);
                        let l = dx.hypot(dy).max(1e-300);
                        [a[0] + dx / l * 2.0 * FAR, a[1] + dy / l * 2.0 * FAR]
                    };
                    let mut q = vec![ext(p[0], p[1])];
                    q.extend_from_slice(&p);
                    q.push(ext(p[p.len() - 1], p[p.len() - 2]));
                    for s in q.windows(2) {
                        let n = unit_normal(s[0], s[1]);
                        let o = [n[0] * FAR * sign, n[1] * FAR * sign];
                        let mut quad = vec![
                            s[0],
                            s[1],
                            [s[1][0] + o[0], s[1][1] + o[1]],
                            [s[0][0] + o[0], s[0][1] + o[1]],
                        ];
                        if sign < 0.0 {
                            quad.reverse();
                        }
                        parts.push(quad);
                    }
                    // скругление на внешней стороне излома
                    for k in 1..q.len() - 1 {
                        let turn = cross(q[k - 1], q[k], q[k + 1]);
                        if turn * sign >= 0.0 {
                            continue;
                        }
                        let n1 = unit_normal(q[k - 1], q[k]);
                        let n2 = unit_normal(q[k], q[k + 1]);
                        let a1 = (n1[1] * sign).atan2(n1[0] * sign);
                        let mut a2 = (n2[1] * sign).atan2(n2[0] * sign);
                        // короткая дуга от a1 к a2
                        while a2 - a1 > std::f64::consts::PI {
                            a2 -= std::f64::consts::TAU;
                        }
                        while a1 - a2 > std::f64::consts::PI {
                            a2 += std::f64::consts::TAU;
                        }
                        let steps = (((a2 - a1).abs() / (std::f64::consts::FRAC_PI_2 / 8.0)).ceil()
                            as usize)
                            .max(1);
                        let mut fan = vec![q[k]];
                        for i in 0..=steps {
                            let a = a1 + (a2 - a1) * i as f64 / steps as f64;
                            fan.push([q[k][0] + FAR * a.cos(), q[k][1] + FAR * a.sin()]);
                        }
                        parts.push(fan);
                    }
                }
                Region::new(parts, false)
            }
        }
    }
}

fn clean_ring(r: &[P2]) -> Vec<P2> {
    let mut p = clean_line(r);
    if p.len() > 1 && p.first() == p.last() {
        p.pop();
    }
    p
}

// ------------------------------------------------------------- резка в плане

/// Точка строго внутри треугольника в плане.
fn strictly_inside(t: [P3; 3], p: P2) -> bool {
    let q = |v: P3| [v[0], v[1]];
    let d0 = cross(q(t[0]), q(t[1]), p);
    let d1 = cross(q(t[1]), q(t[2]), p);
    let d2 = cross(q(t[2]), q(t[0]), p);
    (d0 > 0.0 && d1 > 0.0 && d2 > 0.0) || (d0 < 0.0 && d1 < 0.0 && d2 < 0.0)
}

/// Сетка: вершины и треугольники.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    pub verts: Vec<P3>,
    pub faces: Vec<Tri>,
}

impl Mesh {
    fn push_tri(&mut self, a: P3, b: P3, c: P3) {
        let n = self.verts.len() as u32;
        self.verts.extend_from_slice(&[a, b, c]);
        self.faces.push([n, n + 1, n + 2]);
    }

    fn append(&mut self, other: Mesh) {
        let n = self.verts.len() as u32;
        self.verts.extend(other.verts);
        self.faces
            .extend(other.faces.into_iter().map(|f| f.map(|i| i + n)));
    }
}

/// Разрезать выпуклый многоугольник прямой a-b: две части, точка
/// на ребре - от его концов в одном порядке.
fn split_by_line(poly: &[P3], a: P2, b: P2) -> (Vec<P3>, Vec<P3>) {
    let f = |p: &P3| cross(a, b, [p[0], p[1]]);
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let n = poly.len();
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let (fp, fq) = (f(&p), f(&q));
        if fp >= 0.0 {
            left.push(p);
        }
        if fp <= 0.0 {
            right.push(p);
        }
        if (fp > 0.0 && fq < 0.0) || (fp < 0.0 && fq > 0.0) {
            let (c, d, fc, fd) = if (p[0], p[1], p[2]) <= (q[0], q[1], q[2]) {
                (p, q, fp, fq)
            } else {
                (q, p, fq, fp)
            };
            let s = fc / (fc - fd);
            let x = [
                c[0] + s * (d[0] - c[0]),
                c[1] + s * (d[1] - c[1]),
                c[2] + s * (d[2] - c[2]),
            ];
            left.push(x);
            right.push(x);
        }
    }
    (left, right)
}

/// Резка грани, через которую проходит граница: прямыми через отрезки
/// границы на выпуклые куски. Кусок целиком по одну сторону каждого
/// отрезка и отбирается по середине.
fn cut_face(t: [P3; 3], reg: &Region, segs: &[(P2, P2)], out: &mut Mesh) {
    let mut pieces: Vec<Vec<P3>> = vec![t.to_vec()];
    for (a, b) in segs {
        let mut next = Vec::with_capacity(pieces.len() + 1);
        for p in pieces {
            let (l, r) = split_by_line(&p, *a, *b);
            for q in [l, r] {
                if q.len() >= 3 {
                    next.push(q);
                }
            }
        }
        pieces = next;
    }
    for p in pieces {
        let n = p.len() as f64;
        let c = [
            p.iter().map(|v| v[0]).sum::<f64>() / n,
            p.iter().map(|v| v[1]).sum::<f64>() / n,
        ];
        if !reg.keep(c) {
            continue;
        }
        for i in 1..p.len() - 1 {
            out.push_tri(p[0], p[i], p[i + 1]);
        }
    }
}

/// Т-стыки: вершина соседа на краевом ребре грани. Грань делится в этих
/// точках, и ребро сшивается.
fn fix_t_junctions(m: &Mesh) -> Mesh {
    let mut cur = m.clone();
    for _ in 0..4 {
        let border = border_edges(&cur);
        if border.is_empty() {
            break;
        }
        let mut cand: Vec<u32> = border.iter().flat_map(|(a, b)| [*a, *b]).collect();
        cand.sort_unstable();
        cand.dedup();
        // ребро -> точки на нём
        let mut on: HashMap<(u32, u32), Vec<(f64, u32)>> = HashMap::new();
        for (a, b) in &border {
            let (pa, pb) = (cur.verts[*a as usize], cur.verts[*b as usize]);
            let d = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
            let l2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
            if l2 <= 0.0 {
                continue;
            }
            let tol2 = (l2 * 1e-14).max(1e-14);
            for &v in &cand {
                if v == *a || v == *b {
                    continue;
                }
                let p = cur.verts[v as usize];
                let s =
                    ((p[0] - pa[0]) * d[0] + (p[1] - pa[1]) * d[1] + (p[2] - pa[2]) * d[2]) / l2;
                if s <= 1e-9 || s >= 1.0 - 1e-9 {
                    continue;
                }
                let f = [
                    pa[0] + s * d[0] - p[0],
                    pa[1] + s * d[1] - p[1],
                    pa[2] + s * d[2] - p[2],
                ];
                if f[0] * f[0] + f[1] * f[1] + f[2] * f[2] <= tol2 {
                    on.entry((*a, *b)).or_default().push((s, v));
                }
            }
        }
        if on.is_empty() {
            break;
        }
        let mut faces = Vec::with_capacity(cur.faces.len());
        for f in &cur.faces {
            // ребро грани с точками: грань делится веером от
            // противоположной вершины
            let mut done = false;
            for k in 0..3 {
                let (a, b, c) = (f[k], f[(k + 1) % 3], f[(k + 2) % 3]);
                let pts = on.get(&(a.min(b), a.max(b)));
                let Some(pts) = pts.filter(|p| !p.is_empty()) else {
                    continue;
                };
                let mut chain: Vec<(f64, u32)> = pts.clone();
                chain.sort_by(|x, y| x.0.total_cmp(&y.0));
                let mut seq: Vec<u32> = vec![a.min(b)];
                seq.extend(chain.iter().map(|x| x.1));
                seq.push(a.max(b));
                if a > b {
                    seq.reverse();
                }
                for w in seq.windows(2) {
                    faces.push([w[0], w[1], c]);
                }
                done = true;
                break;
            }
            if !done {
                faces.push(*f);
            }
        }
        cur.faces = faces;
    }
    cur
}

/// Отвесная грань: режется вдоль своего отрезка в плане, как
/// `clip_wall` модуля.
fn cut_wall(t: [P3; 3], reg: &Region, segs: &[(P2, P2)], out: &mut Mesh) {
    let p0: [P2; 3] = t.map(|v| [v[0], v[1]]);
    let mut k = 0;
    let mut best = -1.0;
    for i in 0..3 {
        let l = dist2(p0[i], p0[(i + 1) % 3]);
        if l > best {
            best = l;
            k = i;
        }
    }
    if best <= 0.0 {
        // вырождена и в плане в точку: по самой точке
        if reg.keep(p0[0]) {
            out.push_tri(t[0], t[1], t[2]);
        }
        return;
    }
    let (a, b) = (p0[k], p0[(k + 1) % 3]);
    let d = [b[0] - a[0], b[1] - a[1]];
    let mut ts = vec![0.0, 1.0];
    for (c, e) in segs {
        if let Some((u, v)) = seg_cross(a, b, *c, *e)
            && u > 0.0
            && u < 1.0
            && (-1e-12..=1.0 + 1e-12).contains(&v)
        {
            ts.push(u);
        }
    }
    ts.sort_by(f64::total_cmp);
    let mut spans: Vec<(f64, f64)> = Vec::new();
    for w in ts.windows(2) {
        let m = 0.5 * (w[0] + w[1]);
        if w[1] - w[0] > 1e-12 && reg.keep([a[0] + m * d[0], a[1] + m * d[1]]) {
            match spans.last_mut() {
                Some(s) if (s.1 - w[0]).abs() < 1e-12 => s.1 = w[1],
                _ => spans.push((w[0], w[1])),
            }
        }
    }
    if spans.len() == 1 && spans[0].0 <= 1e-12 && spans[0].1 >= 1.0 - 1e-12 {
        out.push_tri(t[0], t[1], t[2]);
        return;
    }
    let l2 = d[0] * d[0] + d[1] * d[1];
    let poly: Vec<[f64; 2]> = t
        .iter()
        .map(|v| [((v[0] - a[0]) * d[0] + (v[1] - a[1]) * d[1]) / l2, v[2]])
        .collect();
    for (lo, hi) in spans {
        let mut cur = poly.clone();
        for (sign, bound) in [(1.0, lo), (-1.0, hi)] {
            let mut next = Vec::new();
            let n = cur.len();
            for i in 0..n {
                let (p, q) = (cur[i], cur[(i + 1) % n]);
                let ip = (p[0] - bound) * sign >= 0.0;
                let iq = (q[0] - bound) * sign >= 0.0;
                if ip {
                    next.push(p);
                }
                if ip != iq {
                    let s = (bound - p[0]) / (q[0] - p[0]);
                    next.push([bound, p[1] + s * (q[1] - p[1])]);
                }
            }
            cur = next;
            if cur.len() < 3 {
                break;
            }
        }
        if cur.len() < 3 {
            continue;
        }
        let back = |q: [f64; 2]| [a[0] + q[0] * d[0], a[1] + q[0] * d[1], q[1]];
        for i in 1..cur.len() - 1 {
            out.push_tri(back(cur[0]), back(cur[i]), back(cur[i + 1]));
        }
    }
}

/// Грани сетки, обрезанные областью в плане.
fn cut_plan(m: &Mesh, reg: &Region) -> Mesh {
    let mut out = Mesh::default();
    for f in &m.faces {
        let (Some(a), Some(b), Some(c)) = (
            m.verts.get(f[0] as usize),
            m.verts.get(f[1] as usize),
            m.verts.get(f[2] as usize),
        ) else {
            continue;
        };
        let t = [*a, *b, *c];
        let lo = [a[0].min(b[0]).min(c[0]), a[1].min(b[1]).min(c[1])];
        let hi = [a[0].max(b[0]).max(c[0]), a[1].max(b[1]).max(c[1])];
        let near: Vec<(P2, P2)> = reg
            .segs
            .iter()
            .filter(|(p, q)| {
                p[0].max(q[0]) >= lo[0]
                    && p[0].min(q[0]) <= hi[0]
                    && p[1].max(q[1]) >= lo[1]
                    && p[1].min(q[1]) <= hi[1]
            })
            .copied()
            .collect();
        let area = cross([a[0], a[1]], [b[0], b[1]], [c[0], c[1]]);
        if area.abs() < 2e-9 {
            cut_wall(t, reg, &near, &mut out);
            continue;
        }
        // граница задевает грань: режем, иначе грань целиком по середине
        let touches = near.iter().any(|(p, q)| {
            strictly_inside(t, *p)
                || strictly_inside(t, *q)
                || (0..3).any(|k| {
                    let (e0, e1) = (t[k], t[(k + 1) % 3]);
                    seg_cross([e0[0], e0[1]], [e1[0], e1[1]], *p, *q)
                        .is_some_and(|(s, u)| (0.0..=1.0).contains(&s) && (0.0..=1.0).contains(&u))
                })
        });
        if touches {
            cut_face(t, reg, &near, &mut out);
        } else {
            let cen = [(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0];
            if reg.keep(cen) {
                out.push_tri(*a, *b, *c);
            }
        }
    }
    out
}

// ------------------------------------------------------------- отметки

/// Грани, обрезанные плоскостями z = zlo и z = zhi, как `z_cut` модуля:
/// точка на ребре от концов в одном порядке, отметка плоскости ровно.
pub fn cut_z(m: &Mesh, zlo: Option<f64>, zhi: Option<f64>) -> Mesh {
    let lo = zlo.unwrap_or(f64::NEG_INFINITY);
    let hi = zhi.unwrap_or(f64::INFINITY);
    let meet = |a: P3, b: P3, plane: f64| {
        let (a, b) = if (a[0], a[1], a[2]) > (b[0], b[1], b[2]) {
            (b, a)
        } else {
            (a, b)
        };
        let t = (plane - a[2]) / (b[2] - a[2]);
        [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1]), plane]
    };
    let clip = |poly: Vec<P3>, plane: f64, above: bool| {
        let mut res = Vec::new();
        let n = poly.len();
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let ia = if above { a[2] >= plane } else { a[2] <= plane };
            let ib = if above { b[2] >= plane } else { b[2] <= plane };
            if ia {
                res.push(a);
            }
            if ia != ib && a[2] != plane && b[2] != plane {
                res.push(meet(a, b, plane));
            }
        }
        res
    };
    let mut out = Mesh {
        verts: m.verts.clone(),
        faces: Vec::new(),
    };
    for f in &m.faces {
        let (Some(a), Some(b), Some(c)) = (
            m.verts.get(f[0] as usize),
            m.verts.get(f[1] as usize),
            m.verts.get(f[2] as usize),
        ) else {
            continue;
        };
        let zs = [a[2], b[2], c[2]];
        if zs.iter().all(|z| *z >= lo && *z <= hi) {
            out.faces.push(*f);
            continue;
        }
        if zs.iter().all(|z| *z < lo) || zs.iter().all(|z| *z > hi) {
            continue;
        }
        let mut poly = vec![*a, *b, *c];
        if lo.is_finite() {
            poly = clip(poly, lo, true);
        }
        if poly.len() >= 3 && hi.is_finite() {
            poly = clip(poly, hi, false);
        }
        poly.dedup_by(|p, q| (0..3).all(|k| (p[k] - q[k]).abs() < 1e-12));
        if poly.len() > 1 && (0..3).all(|k| (poly[0][k] - poly[poly.len() - 1][k]).abs() < 1e-12) {
            poly.pop();
        }
        if poly.len() < 3 {
            continue;
        }
        for i in 1..poly.len() - 1 {
            out.push_tri(poly[0], poly[i], poly[i + 1]);
        }
    }
    out
}

// ------------------------------------------------------------- сшивка

/// Склейка совпадающих вершин по округлению, как `iso3d.weld` модуля,
/// и снятие вырожденных граней.
pub fn weld(m: &Mesh, snap: f64) -> Mesh {
    if m.verts.is_empty() {
        return Mesh::default();
    }
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for v in &m.verts {
        for k in 0..3 {
            lo[k] = lo[k].min(v[k]);
            hi[k] = hi[k].max(v[k]);
        }
    }
    let span = (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f64::max);
    let step = snap.max(if span > 0.0 { span / 1.5e6 } else { snap });
    let key = |v: &P3| [0, 1, 2].map(|k| ((v[k] - lo[k]) / step).round_ties_even() as i64);
    let mut ids: HashMap<[i64; 3], u32> = HashMap::new();
    let mut verts = Vec::new();
    let remap: Vec<u32> = m
        .verts
        .iter()
        .map(|v| {
            *ids.entry(key(v)).or_insert_with(|| {
                verts.push(*v);
                (verts.len() - 1) as u32
            })
        })
        .collect();
    let faces = m
        .faces
        .iter()
        .filter_map(|f| {
            let g = [
                *remap.get(f[0] as usize)?,
                *remap.get(f[1] as usize)?,
                *remap.get(f[2] as usize)?,
            ];
            (g[0] != g[1] && g[1] != g[2] && g[0] != g[2]).then_some(g)
        })
        .collect();
    Mesh { verts, faces }
}

/// Рёбра с одной гранью: край отверстия.
fn border_edges(m: &Mesh) -> Vec<(u32, u32)> {
    let mut cnt: HashMap<(u32, u32), u32> = HashMap::new();
    for f in &m.faces {
        for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
            *cnt.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    let mut out: Vec<(u32, u32)> = cnt
        .into_iter()
        .filter(|(_, n)| *n == 1)
        .map(|(e, _)| e)
        .collect();
    out.sort_unstable();
    out
}

/// Число рёбер с одной гранью: 0 у замкнутого тела.
pub fn holes(m: &Mesh) -> usize {
    border_edges(&weld(m, 1e-6)).len()
}

// ------------------------------------------------------------- крышки

/// Многоугольники по отрезкам в плоскости (u, v), чёт-нечет:
/// треугольники внутри. `splits` - вертикальные разрезы u = const:
/// у вертикальной крышки это углы кольца, треугольник не должен
/// перешагивать угол, иначе в пространстве он срезал бы его хордой.
fn fill_even_odd(segs: &[(P2, P2)], splits: &[f64]) -> Vec<[P2; 3]> {
    // Триангуляция в местных координатах: у шахтных координат порядка
    // 1e6 близкие точки окружности почти на одной прямой, и проверки
    // пересечений в больших числах ломаются. Сдвиг вычитается здесь
    // и возвращается в ответе.
    let Some((p0, _)) = segs.first() else {
        return Vec::new();
    };
    let o = *p0;
    let local: Vec<(P2, P2)> = segs
        .iter()
        .map(|(a, b)| ([a[0] - o[0], a[1] - o[1]], [b[0] - o[0], b[1] - o[1]]))
        .collect();
    let splits: Vec<f64> = splits.iter().map(|u| u - o[0]).collect();
    fill_even_odd_local(&local, &splits)
        .into_iter()
        .map(|t| t.map(|p| [p[0] + o[0], p[1] + o[1]]))
        .collect()
}

fn fill_even_odd_local(segs: &[(P2, P2)], splits: &[f64]) -> Vec<[P2; 3]> {
    let mut cdt: ConstrainedDelaunayTriangulation<Point2<f64>> =
        ConstrainedDelaunayTriangulation::new();
    for (a, b) in segs {
        if dist2(*a, *b) <= 0.0 {
            continue;
        }
        let (Ok(ha), Ok(hb)) = (
            cdt.insert(Point2::new(a[0], a[1])),
            cdt.insert(Point2::new(b[0], b[1])),
        ) else {
            continue;
        };
        if ha != hb {
            cdt.add_constraint_and_split(ha, hb, |p| p);
        }
    }
    if !splits.is_empty() {
        let (lo, hi) = segs.iter().flat_map(|(a, b)| [a, b]).fold(
            ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]),
            |(l, h), p| {
                (
                    [l[0].min(p[0]), l[1].min(p[1])],
                    [h[0].max(p[0]), h[1].max(p[1])],
                )
            },
        );
        let pad = (hi[1] - lo[1]).max(1.0);
        for &u in splits {
            if u <= lo[0] || u >= hi[0] {
                continue;
            }
            let (Ok(ha), Ok(hb)) = (
                cdt.insert(Point2::new(u, lo[1] - pad)),
                cdt.insert(Point2::new(u, hi[1] + pad)),
            ) else {
                continue;
            };
            cdt.add_constraint_and_split(ha, hb, |p| p);
        }
    }
    let mut out = Vec::new();
    for f in cdt.inner_faces() {
        let p = f.positions().map(|q| [q.x, q.y]);
        let c = [
            (p[0][0] + p[1][0] + p[2][0]) / 3.0,
            (p[0][1] + p[1][1] + p[2][1]) / 3.0,
        ];
        let mut odd = false;
        for (a, b) in segs {
            if (a[1] > c[1]) != (b[1] > c[1]) {
                let x = a[0] + (c[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
                if c[0] < x {
                    odd = !odd;
                }
            }
        }
        if odd {
            out.push(p);
        }
    }
    out
}

/// Сдвиг начала отсчёта вдоль кольца в самый большой пустой промежуток
/// между точками среза, как `ring_start_shift` модуля. `spans` - рёбра
/// среза: (путь конца, путь конца, путь середины).
pub fn ring_start_shift(positions: &[f64], length: f64, spans: &[(f64, f64, f64)]) -> f64 {
    if length <= 0.0 {
        return 0.0;
    }
    let mut s: Vec<f64> = positions.iter().map(|p| p.rem_euclid(length)).collect();
    s.sort_by(f64::total_cmp);
    if s.len() < 2 {
        return 0.0;
    }
    let mut inner = Vec::new();
    let mut wrap_used = false;
    for (a, b, m) in spans {
        let (lo, hi) = (a.min(*b), a.max(*b));
        if lo - 1e-9 <= *m && *m <= hi + 1e-9 {
            inner.push((lo, hi));
        } else {
            wrap_used = true;
        }
    }
    let (mut best, mut s0) = (-1.0, 0.0);
    if !wrap_used {
        best = s[0] + length - s[s.len() - 1];
    }
    for k in 0..s.len() - 1 {
        let g = s[k + 1] - s[k];
        if g <= best {
            continue;
        }
        if inner
            .iter()
            .any(|(lo, hi)| *lo <= s[k] + 1e-9 && *hi >= s[k + 1] - 1e-9)
        {
            continue;
        }
        best = g;
        s0 = s[k] + 0.5 * g;
    }
    s0
}

/// Вертикальная крышка на срезе в плане: краевые рёбра на границе
/// области переводятся в плоскость «путь вдоль границы - отметка»,
/// заполняются чёт-нечет и возвращаются в пространство.
fn cap_plan(m: &Mesh, reg: &Region) -> Mesh {
    let border = border_edges(m);
    let mut out = Mesh::default();
    if border.is_empty() || m.verts.is_empty() {
        return out;
    }
    let ptp = |k: usize| {
        let (lo, hi) = m
            .verts
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), v| {
                (l.min(v[k]), h.max(v[k]))
            });
        hi - lo
    };
    let span_v = ptp(0).max(ptp(1));
    let tol = reg.span.min(span_v) * 1e-4 + 1e-6;
    let mut spos: HashMap<u32, (usize, f64)> = HashMap::new();
    let mut on_cut = |i: u32| -> Option<(usize, f64)> {
        if let Some(x) = spos.get(&i) {
            return Some(*x);
        }
        let v = m.verts.get(i as usize)?;
        let (r, s, d) = reg.locate([v[0], v[1]])?;
        if d > tol {
            return None;
        }
        spos.insert(i, (r, s));
        Some((r, s))
    };
    let mut by_ring: HashMap<usize, Vec<(u32, u32, f64, f64)>> = HashMap::new();
    for (a, b) in border {
        if let (Some((ra, sa)), Some((rb, sb))) = (on_cut(a), on_cut(b))
            && ra == rb
        {
            by_ring.entry(ra).or_default().push((a, b, sa, sb));
        }
    }
    for (ring, edges) in by_ring {
        let len = reg.ring_len(ring);
        if len <= 0.0 {
            continue;
        }
        let spans: Vec<(f64, f64, f64)> = edges
            .iter()
            .map(|(a, b, sa, sb)| {
                let (pa, pb) = (m.verts[*a as usize], m.verts[*b as usize]);
                let mid = [0.5 * (pa[0] + pb[0]), 0.5 * (pa[1] + pb[1])];
                let sm = reg.locate(mid).map_or(0.5 * (sa + sb), |x| x.1);
                (*sa, *sb, sm)
            })
            .collect();
        let positions: Vec<f64> = edges.iter().flat_map(|e| [e.2, e.3]).collect();
        let s0 = ring_start_shift(&positions, len, &spans);
        let sh = |s: f64| {
            if s0 != 0.0 {
                (s - s0).rem_euclid(len)
            } else {
                s
            }
        };
        let segs: Vec<(P2, P2)> = edges
            .iter()
            .map(|(a, b, sa, sb)| {
                (
                    [sh(*sa), m.verts[*a as usize][2]],
                    [sh(*sb), m.verts[*b as usize][2]],
                )
            })
            .filter(|(p, q)| dist2(*p, *q) > 1e-24)
            .collect();
        // углы кольца на сдвинутом пути
        let corners: Vec<f64> = {
            let r = &reg.rings[ring];
            let mut acc = 0.0;
            let mut out = Vec::with_capacity(r.len());
            for k in 0..r.len() {
                out.push(sh(acc));
                acc += dist2(r[k], r[(k + 1) % r.len()]).sqrt();
            }
            out
        };
        for tri in fill_even_odd(&segs, &corners) {
            let back = |q: P2| {
                let s = if s0 != 0.0 {
                    (q[0] + s0).rem_euclid(len)
                } else {
                    q[0]
                };
                reg.point_at(ring, s).map(|p| [p[0], p[1], q[1]])
            };
            if let (Some(a), Some(b), Some(c)) = (back(tri[0]), back(tri[1]), back(tri[2])) {
                out.push_tri(a, b, c);
            }
        }
    }
    out
}

/// Горизонтальная крышка по краевым рёбрам на плоскости z = plane.
fn cap_z(m: &Mesh, plane: f64) -> Mesh {
    let tol = 1e-6 * plane.abs().max(1.0);
    let segs: Vec<(P2, P2)> = border_edges(m)
        .into_iter()
        .filter_map(|(a, b)| {
            let (pa, pb) = (m.verts.get(a as usize)?, m.verts.get(b as usize)?);
            ((pa[2] - plane).abs() <= tol && (pb[2] - plane).abs() <= tol)
                .then_some(([pa[0], pa[1]], [pb[0], pb[1]]))
        })
        .collect();
    let mut out = Mesh::default();
    for t in fill_even_odd(&segs, &[]) {
        out.push_tri(
            [t[0][0], t[0][1], plane],
            [t[1][0], t[1][1], plane],
            [t[2][0], t[2][1], plane],
        );
    }
    out
}

/// Согласовать обход граней по оболочке, как `orient_faces` модуля,
/// и развернуть наружу: объём со знаком плюс.
pub fn orient(m: &mut Mesh) {
    let mut edge_faces: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (i, f) in m.faces.iter().enumerate() {
        for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
            edge_faces.entry((a.min(b), a.max(b))).or_default().push(i);
        }
    }
    let has = |f: &Tri, a: u32, b: u32| (0..3).any(|k| f[k] == a && f[(k + 1) % 3] == b);
    let mut seen = vec![false; m.faces.len()];
    for start in 0..m.faces.len() {
        if seen[start] {
            continue;
        }
        seen[start] = true;
        let mut stack = vec![start];
        while let Some(i) = stack.pop() {
            let f = m.faces[i];
            for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
                let Some(nb) = edge_faces.get(&(a.min(b), a.max(b))) else {
                    continue;
                };
                if nb.len() != 2 {
                    continue;
                }
                for &j in nb {
                    if j == i || seen[j] {
                        continue;
                    }
                    // согласованные грани проходят общее ребро навстречу
                    if has(&m.faces[j], a, b) {
                        m.faces[j].swap(1, 2);
                    }
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
    }
    if signed_volume(m) < 0.0 {
        for f in &mut m.faces {
            f.swap(1, 2);
        }
    }
}

/// Объём со знаком от середины вершин.
pub fn signed_volume(m: &Mesh) -> f64 {
    let n = m.verts.len().max(1) as f64;
    let mut c = [0.0; 3];
    for v in &m.verts {
        for k in 0..3 {
            c[k] += v[k] / n;
        }
    }
    let p = |i: u32| {
        m.verts
            .get(i as usize)
            .map(|v| [v[0] - c[0], v[1] - c[1], v[2] - c[2]])
    };
    let mut sum = 0.0;
    for f in &m.faces {
        if let (Some(a), Some(b), Some(d)) = (p(f[0]), p(f[1]), p(f[2])) {
            sum += a[0] * (b[1] * d[2] - b[2] * d[1])
                + a[1] * (b[2] * d[0] - b[0] * d[2])
                + a[2] * (b[0] * d[1] - b[1] * d[0]);
        }
    }
    sum / 6.0
}

// ------------------------------------------------------------- целиком

/// Обрезать сетку. `body` - замкнутое тело: срез закрывается крышками.
/// Поверхность (`body` false) только режется.
pub fn clip_mesh(m: &Mesh, clip: &Clip, body: bool) -> Mesh {
    let mut cur = m.clone();
    if let Some(reg) = clip.region() {
        cur = fix_t_junctions(&weld(&cut_plan(&cur, &reg), 1e-6));
        if body {
            let cap = cap_plan(&cur, &reg);
            cur.append(cap);
            cur = weld(&cur, 1e-6);
        }
    }
    if clip.zlo.is_some() || clip.zhi.is_some() {
        cur = weld(&cut_z(&cur, clip.zlo, clip.zhi), 1e-6);
        if body {
            for plane in [clip.zlo, clip.zhi].into_iter().flatten() {
                let cap = cap_z(&cur, plane);
                cur.append(cap);
            }
            cur = fix_t_junctions(&weld(&cur, 1e-6));
        }
    }
    if body {
        orient(&mut cur);
    }
    cur
}

impl Clip {
    /// Остаётся ли точка: область в плане и полоса отметок.
    pub fn keeps(&self, region: Option<&Region>, p: P3) -> bool {
        region.is_none_or(|r| r.keep([p[0], p[1]]))
            && self.zlo.is_none_or(|z| p[2] >= z)
            && self.zhi.is_none_or(|z| p[2] <= z)
    }
}

/// Обрезать часть сцены. Тело (замкнутая сетка) закрывается крышками,
/// поверхность только режется, у линий и точек остаётся то, что
/// внутри: отрезок - если внутри оба конца. От части может ничего
/// не остаться - тогда номеров вершин нет.
pub fn clip_part(part: &Part, clip: &Clip) -> Part {
    let region = clip.region();
    let keeps = |i: u32| {
        part.verts
            .get(i as usize)
            .is_some_and(|p| clip.keeps(region.as_ref(), *p))
    };
    match &part.indices {
        Indices::Triangles(t) => {
            let src = Mesh {
                verts: part.verts.clone(),
                faces: t.clone(),
            };
            let body = !t.is_empty() && holes(&weld(&src, 1e-6)) == 0;
            let got = clip_mesh(&src, clip, body);
            let colors = part
                .colors
                .as_ref()
                .map(|c| transfer_colors(&src, c, &got.verts));
            Part {
                name: part.name.clone(),
                verts: got.verts,
                indices: Indices::Triangles(got.faces),
                colors,
                sizes: None,
            }
        }
        Indices::Lines(l) => {
            let kept: Vec<[u32; 2]> = l
                .iter()
                .copied()
                .filter(|s| keeps(s[0]) && keeps(s[1]))
                .collect();
            let used: Vec<u32> = kept.iter().flatten().copied().collect();
            let (map, out) = compact(part, &used);
            Part {
                indices: Indices::Lines(
                    kept.iter()
                        .filter_map(|s| Some([*map.get(&s[0])?, *map.get(&s[1])?]))
                        .collect(),
                ),
                ..out
            }
        }
        Indices::Points(p) => {
            let kept: Vec<u32> = p.iter().copied().filter(|&i| keeps(i)).collect();
            let (map, out) = compact(part, &kept);
            Part {
                indices: Indices::Points(kept.iter().filter_map(|i| map.get(i).copied()).collect()),
                ..out
            }
        }
    }
}

/// Оставить у части только вершины `used`: новые номера и часть
/// без номеров вершин.
fn compact(part: &Part, used: &[u32]) -> (HashMap<u32, u32>, Part) {
    let mut map: HashMap<u32, u32> = HashMap::new();
    let mut order: Vec<usize> = Vec::new();
    for &i in used {
        if (i as usize) < part.verts.len() && !map.contains_key(&i) {
            map.insert(i, order.len() as u32);
            order.push(i as usize);
        }
    }
    let pick = |v: &Vec<[f32; 4]>| {
        order
            .iter()
            .filter_map(|&i| v.get(i).copied())
            .collect::<Vec<_>>()
    };
    let out = Part {
        name: part.name.clone(),
        verts: order.iter().map(|&i| part.verts[i]).collect(),
        indices: Indices::Points(Vec::new()),
        colors: part
            .colors
            .as_ref()
            .map(pick)
            .filter(|c| c.len() == order.len()),
        sizes: part
            .sizes
            .as_ref()
            .map(|s| {
                order
                    .iter()
                    .filter_map(|&i| s.get(i).copied())
                    .collect::<Vec<_>>()
            })
            .filter(|s| s.len() == order.len()),
    };
    (map, out)
}

/// Цвет вершин обрезанной сетки с исходной: вершина на месте старой
/// берёт её цвет, новая - цвет ближней точки исходных граней
/// по долям вершин грани.
fn transfer_colors(src: &Mesh, cols: &[[f32; 4]], verts: &[P3]) -> Vec<[f32; 4]> {
    let fallback = cols.first().copied().unwrap_or([0.8, 0.8, 0.8, 1.0]);
    let key = |p: &P3| p.map(|x| (x * 1e6).round() as i64);
    let mut exact: HashMap<[i64; 3], usize> = HashMap::new();
    for (i, v) in src.verts.iter().enumerate() {
        exact.entry(key(v)).or_insert(i);
    }
    let grid = TriGrid::new(src);
    let col = |i: u32| cols.get(i as usize).copied().unwrap_or(fallback);
    verts
        .iter()
        .map(|p| {
            if let Some(&i) = exact.get(&key(p)) {
                return cols.get(i).copied().unwrap_or(fallback);
            }
            let Some((f, w)) = grid.nearest(src, *p) else {
                return fallback;
            };
            let (a, b, c) = (col(f[0]), col(f[1]), col(f[2]));
            let mut out = [0.0f32; 4];
            for k in 0..4 {
                out[k] = (w[0] * f64::from(a[k]) + w[1] * f64::from(b[k]) + w[2] * f64::from(c[k]))
                    as f32;
            }
            out
        })
        .collect()
}

/// Грани по клеткам плана: поиск ближней грани к точке.
struct TriGrid {
    lo: P2,
    cell: f64,
    nx: i64,
    ny: i64,
    cells: HashMap<(i64, i64), Vec<usize>>,
}

impl TriGrid {
    fn new(m: &Mesh) -> Self {
        let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for v in &m.verts {
            for k in 0..2 {
                lo[k] = lo[k].min(v[k]);
                hi[k] = hi[k].max(v[k]);
            }
        }
        let span = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1e-6);
        let n = (m.faces.len() as f64).sqrt().clamp(1.0, 512.0);
        let cell = span / n;
        let at = |x: f64, k: usize| ((x - lo[k]) / cell).floor() as i64;
        let mut cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (fi, f) in m.faces.iter().enumerate() {
            let p = f.map(|i| m.verts.get(i as usize).copied().unwrap_or([0.0; 3]));
            let (x0, x1) = (
                at(p[0][0].min(p[1][0]).min(p[2][0]), 0),
                at(p[0][0].max(p[1][0]).max(p[2][0]), 0),
            );
            let (y0, y1) = (
                at(p[0][1].min(p[1][1]).min(p[2][1]), 1),
                at(p[0][1].max(p[1][1]).max(p[2][1]), 1),
            );
            for x in x0..=x1 {
                for y in y0..=y1 {
                    cells.entry((x, y)).or_default().push(fi);
                }
            }
        }
        Self {
            lo,
            cell,
            nx: at(hi[0], 0) + 1,
            ny: at(hi[1], 1) + 1,
            cells,
        }
    }

    /// Ближняя грань и доли её вершин в ближней точке.
    fn nearest(&self, m: &Mesh, p: P3) -> Option<(Tri, [f64; 3])> {
        let cx = ((p[0] - self.lo[0]) / self.cell).floor() as i64;
        let cy = ((p[1] - self.lo[1]) / self.cell).floor() as i64;
        let max_r = self.nx.max(self.ny) + (cx.abs()).max(cy.abs()) + 1;
        let mut best: Option<(f64, Tri, [f64; 3])> = None;
        let mut seen = std::collections::HashSet::new();
        for r in 0..=max_r {
            for x in (cx - r)..=(cx + r) {
                for y in (cy - r)..=(cy + r) {
                    if (x - cx).abs() != r && (y - cy).abs() != r {
                        continue;
                    }
                    let Some(list) = self.cells.get(&(x, y)) else {
                        continue;
                    };
                    for &fi in list {
                        if !seen.insert(fi) {
                            continue;
                        }
                        let Some(f) = m.faces.get(fi) else { continue };
                        let t = f.map(|i| m.verts.get(i as usize).copied().unwrap_or(p));
                        let w = closest_bary(p, t);
                        let q = [0, 1, 2].map(|k| w[0] * t[0][k] + w[1] * t[1][k] + w[2] * t[2][k]);
                        let d = (0..3).map(|k| (q[k] - p[k]).powi(2)).sum::<f64>().sqrt();
                        if best.is_none_or(|b| d < b.0) {
                            best = Some((d, *f, w));
                        }
                    }
                }
            }
            // клетки следующего кольца не ближе r клеток
            if let Some(b) = best
                && b.0 <= r as f64 * self.cell
            {
                break;
            }
        }
        best.map(|b| (b.1, b.2))
    }
}

/// Доли вершин треугольника в его ближней к `p` точке (Эриксон,
/// «Real-Time Collision Detection», 5.1.5).
fn closest_bary(p: P3, t: [P3; 3]) -> [f64; 3] {
    let sub = |u: P3, v: P3| [u[0] - v[0], u[1] - v[1], u[2] - v[2]];
    let dot = |u: P3, v: P3| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
    let (a, b, c) = (t[0], t[1], t[2]);
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return [1.0, 0.0, 0.0];
    }
    let bp = sub(p, b);
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return [0.0, 1.0, 0.0];
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 && d1 - d3 > 0.0 {
        let v = d1 / (d1 - d3);
        return [1.0 - v, v, 0.0];
    }
    let cp = sub(p, c);
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return [0.0, 0.0, 1.0];
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 && d2 - d6 > 0.0 {
        let w = d2 / (d2 - d6);
        return [1.0 - w, 0.0, w];
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 && (d4 - d3) + (d5 - d6) > 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return [0.0, 1.0 - w, w];
    }
    let s = va + vb + vc;
    if s.abs() <= f64::MIN_POSITIVE {
        return [1.0, 0.0, 0.0];
    }
    let (v, w) = (vb / s, vc / s);
    [1.0 - v - w, v, w]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Куб 10 x 10 x 10 с углом в начале координат, грани наружу.
    fn cube() -> Mesh {
        let mut verts = Vec::new();
        for i in 0..8u32 {
            let s = |b: u32| if i & b == 0 { 0.0 } else { 10.0 };
            verts.push([s(1), s(2), s(4)]);
        }
        let quads = [
            [0, 2, 3, 1],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [2, 6, 7, 3],
            [0, 4, 6, 2],
            [1, 3, 7, 5],
        ];
        let faces = quads
            .iter()
            .flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]])
            .collect();
        Mesh { verts, faces }
    }

    fn clip(side: Side) -> Clip {
        Clip {
            side,
            rings: vec![vec![[-5.0, -5.0], [5.0, -5.0], [5.0, 5.0], [-5.0, 5.0]]],
            lines: Vec::new(),
            width: 0.0,
            zlo: None,
            zhi: None,
        }
    }

    #[test]
    fn part_keeps_colors_and_points() {
        let c = cube();
        let colors: Vec<[f32; 4]> = c
            .verts
            .iter()
            .map(|v| [(v[2] / 10.0) as f32, 0.0, 0.0, 1.0])
            .collect();
        let part = Part {
            name: "куб".into(),
            verts: c.verts.clone(),
            indices: Indices::Triangles(c.faces.clone()),
            colors: Some(colors),
            sizes: None,
        };
        let mut band = clip(Side::In);
        band.side = Side::Out;
        band.rings.clear();
        band.zhi = Some(5.0);
        let got = clip_part(&part, &band);
        let faces = match &got.indices {
            Indices::Triangles(t) => t.clone(),
            _ => Vec::new(),
        };
        let m = Mesh {
            verts: got.verts.clone(),
            faces,
        };
        assert_eq!(holes(&m), 0);
        assert!((signed_volume(&m) - 500.0).abs() < 1e-6);
        let cols = got.colors.unwrap();
        for (v, col) in got.verts.iter().zip(&cols) {
            assert!(
                (f64::from(col[0]) - v[2] / 10.0).abs() < 1e-4,
                "{v:?} {col:?}"
            );
        }
        let pts = Part {
            name: "точки".into(),
            verts: vec![[0.0, 0.0, 1.0], [0.0, 0.0, 9.0], [1.0, 1.0, 2.0]],
            indices: Indices::Points(vec![0, 1, 2]),
            colors: None,
            sizes: Some(vec![4.0, 5.0, 6.0]),
        };
        let got = clip_part(&pts, &band);
        assert_eq!(got.verts, vec![[0.0, 0.0, 1.0], [1.0, 1.0, 2.0]]);
        assert_eq!(got.indices, Indices::Points(vec![0, 1]));
        assert_eq!(got.sizes, Some(vec![4.0, 6.0]));
    }

    #[test]
    fn contour_cuts_a_quarter_and_closes_it() {
        let got = clip_mesh(&cube(), &clip(Side::In), true);
        assert_eq!(holes(&got), 0);
        assert!(
            (signed_volume(&got) - 250.0).abs() < 1e-6,
            "{}",
            signed_volume(&got)
        );
        let out = clip_mesh(&cube(), &clip(Side::Out), true);
        assert_eq!(holes(&out), 0);
        assert!(
            (signed_volume(&out) - 750.0).abs() < 1e-6,
            "{}",
            signed_volume(&out)
        );
    }

    #[test]
    fn planes_cut_and_close() {
        let mut c = clip(Side::In);
        c.rings.clear();
        c.zlo = Some(2.5);
        c.zhi = Some(7.0);
        let got = clip_mesh(&cube(), &c, true);
        assert_eq!(holes(&got), 0);
        assert!((signed_volume(&got) - 450.0).abs() < 1e-6);
    }

    #[test]
    fn side_of_a_line_and_corridor() {
        let mut c = clip(Side::Left);
        c.rings.clear();
        c.lines = vec![vec![[-1.0, 4.0], [20.0, 4.0]]];
        let got = clip_mesh(&cube(), &c, true);
        assert_eq!(holes(&got), 0);
        // слева по ходу на восток - север: y от 4 до 10
        assert!(
            (signed_volume(&got) - 600.0).abs() < 1e-6,
            "{}",
            signed_volume(&got)
        );
        c.side = Side::Corridor;
        c.width = 1.0;
        let got = clip_mesh(&cube(), &c, true);
        assert_eq!(holes(&got), 0);
        assert!(
            (signed_volume(&got) - 200.0).abs() < 1e-6,
            "{}",
            signed_volume(&got)
        );
    }
}
