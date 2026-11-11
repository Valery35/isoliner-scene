// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Измерения по точкам сцены, по образцу линейки PlanetX Lite
//! (`ui::ruler`). Точки - в настоящих координатах, метры: план
//! в системе координат пакета, отметки без преувеличения.
//!
//! Ломаная даёт длину по наклону, проложение (длину в плане),
//! превышение конца над началом и уклон. Замкнутый контур - ещё
//! периметр в плане и площадь в плане.

/// Величины измерения. Нет величины - None.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Values {
    /// Длина по наклону, сумма отрезков в пространстве.
    pub length: Option<f64>,
    /// Проложение: длина в плане.
    pub plan: Option<f64>,
    /// Превышение последней точки над первой.
    pub rise: Option<f64>,
    /// Уклон прямой от первой точки к последней, тысячные (‰).
    pub slope: Option<f64>,
    /// Угол наклона прямой от первой точки к последней, градусы.
    pub angle: Option<f64>,
    /// Площадь контура в плане, у замкнутого контура.
    pub area: Option<f64>,
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt()
}

fn dist_plan(a: [f64; 3], b: [f64; 3]) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

/// Величины по точкам `pts`. `closed` - контур: последний отрезок
/// замыкает его к первой точке, есть площадь.
pub fn values(pts: &[[f64; 3]], closed: bool) -> Values {
    let mut v = Values::default();
    if pts.len() < 2 {
        return v;
    }
    let closed = closed && pts.len() >= 3;
    let mut edges: Vec<([f64; 3], [f64; 3])> = pts.windows(2).map(|w| (w[0], w[1])).collect();
    if closed && let (Some(&a), Some(&b)) = (pts.last(), pts.first()) {
        edges.push((a, b));
    }
    v.length = Some(edges.iter().map(|(a, b)| dist(*a, *b)).sum());
    v.plan = Some(edges.iter().map(|(a, b)| dist_plan(*a, *b)).sum());
    if closed {
        // площадь в плане по Гауссу, от первой точки: координаты
        // в миллионах метров теряли бы знаки
        let o = pts[0];
        let twice: f64 = pts
            .iter()
            .zip(pts.iter().cycle().skip(1))
            .map(|(a, b)| (a[0] - o[0]) * (b[1] - o[1]) - (b[0] - o[0]) * (a[1] - o[1]))
            .sum();
        v.area = Some(twice.abs() / 2.0);
    } else if let (Some(&a), Some(&b)) = (pts.first(), pts.last()) {
        let rise = b[2] - a[2];
        let run = dist_plan(a, b);
        v.rise = Some(rise);
        if run > 1e-9 {
            v.slope = Some(rise / run * 1000.0);
        }
        v.angle = Some(rise.atan2(run).to_degrees());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_along_a_drift() {
        // штрек 4 x 3 x 180 м модуля: подъём 3 м на 180 м
        let pts = [
            [512_000.0, 6_401_000.0, -300.0],
            [512_180.0, 6_401_000.0, -297.0],
        ];
        let v = values(&pts, false);
        assert!((v.plan.unwrap() - 180.0).abs() < 1e-9);
        assert!((v.length.unwrap() - (180.0f64.powi(2) + 9.0).sqrt()).abs() < 1e-9);
        assert_eq!(v.rise, Some(3.0));
        assert!((v.slope.unwrap() - 16.666_666_666_666_668).abs() < 1e-9);
        assert!((v.angle.unwrap() - (3.0f64 / 180.0).atan().to_degrees()).abs() < 1e-12);
        assert_eq!(v.area, None);
    }

    #[test]
    fn plumb_line_has_no_slope_in_permille() {
        let v = values(&[[0.0, 0.0, -100.0], [0.0, 0.0, -50.0]], false);
        assert_eq!(v.plan, Some(0.0));
        assert_eq!(v.slope, None);
        assert_eq!(v.angle, Some(90.0));
    }

    #[test]
    fn closed_contour_area_and_perimeter() {
        // квадрат 30 x 30 м в настоящих координатах, разные отметки
        let pts = [
            [512_000.0, 6_401_000.0, -10.0],
            [512_030.0, 6_401_000.0, -12.0],
            [512_030.0, 6_401_030.0, -11.0],
            [512_000.0, 6_401_030.0, -10.0],
        ];
        let v = values(&pts, true);
        assert!((v.area.unwrap() - 900.0).abs() < 1e-6);
        assert!((v.plan.unwrap() - 120.0).abs() < 1e-9);
        assert!(v.length.unwrap() > 120.0);
        assert_eq!(v.rise, None);
        // двух точек на контур мало - это линия
        assert_eq!(values(&pts[..2], true).area, None);
        assert_eq!(values(&pts[..1], false), Values::default());
    }
}
