// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Градусы WGS 84 в метры и обратно: поперечная проекция Меркатора
//! в зоне UTM (Снайдер, «Map Projections - A Working Manual», 1987,
//! формулы 8-9 - 8-25). Нужна расширению для RustGIS: его слои приходят
//! только в градусах WGS 84 (`host.features`), а сцене и инструментам
//! Isoliner3D нужны метры. Внутри зоны погрешность - доли миллиметра.

/// Большая полуось и сжатие WGS 84.
const A: f64 = 6_378_137.0;
const F: f64 = 1.0 / 298.257_223_563;
/// Масштаб на осевом меридиане и ложные сдвиги UTM.
const K0: f64 = 0.9996;
const FALSE_EAST: f64 = 500_000.0;
const FALSE_NORTH_SOUTH: f64 = 10_000_000.0;

/// Зона UTM: номер и полушарие.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Utm {
    pub zone: u8,
    pub north: bool,
}

impl Utm {
    /// Зона по точке: долгота и широта в градусах.
    pub fn at(lon: f64, lat: f64) -> Self {
        let z = ((lon + 180.0) / 6.0).floor().clamp(0.0, 59.0) as u8 + 1;
        Self {
            zone: z,
            north: lat >= 0.0,
        }
    }

    /// Код EPSG зоны: 326xx на севере, 327xx на юге.
    pub fn epsg(&self) -> u32 {
        (if self.north { 32600 } else { 32700 }) + u32::from(self.zone)
    }

    fn lon0(&self) -> f64 {
        (f64::from(self.zone) * 6.0 - 183.0).to_radians()
    }

    fn false_north(&self) -> f64 {
        if self.north { 0.0 } else { FALSE_NORTH_SOUTH }
    }

    /// Градусы в метры: восток, север.
    pub fn forward(&self, lon: f64, lat: f64) -> [f64; 2] {
        let e2 = F * (2.0 - F);
        let ep2 = e2 / (1.0 - e2);
        let (phi, lam) = (lat.to_radians(), lon.to_radians());
        let (s, c) = phi.sin_cos();
        let n = A / (1.0 - e2 * s * s).sqrt();
        let t = phi.tan().powi(2);
        let cc = ep2 * c * c;
        let a = (lam - self.lon0()) * c;
        let m = meridian(phi, e2);
        let x = K0
            * n
            * (a + (1.0 - t + cc) * a.powi(3) / 6.0
                + (5.0 - 18.0 * t + t * t + 72.0 * cc - 58.0 * ep2) * a.powi(5) / 120.0);
        let y = K0
            * (m + n
                * phi.tan()
                * (a * a / 2.0
                    + (5.0 - t + 9.0 * cc + 4.0 * cc * cc) * a.powi(4) / 24.0
                    + (61.0 - 58.0 * t + t * t + 600.0 * cc - 330.0 * ep2) * a.powi(6) / 720.0));
        [x + FALSE_EAST, y + self.false_north()]
    }

    /// Метры в градусы: долгота, широта.
    pub fn inverse(&self, x: f64, y: f64) -> [f64; 2] {
        let e2 = F * (2.0 - F);
        let ep2 = e2 / (1.0 - e2);
        let (x, y) = (x - FALSE_EAST, y - self.false_north());
        let m = y / K0;
        let mu = m / (A * (1.0 - e2 / 4.0 - 3.0 * e2 * e2 / 64.0 - 5.0 * e2.powi(3) / 256.0));
        let r = (1.0 - e2).sqrt();
        let e1 = (1.0 - r) / (1.0 + r);
        let phi1 = mu
            + (3.0 * e1 / 2.0 - 27.0 * e1.powi(3) / 32.0) * (2.0 * mu).sin()
            + (21.0 * e1 * e1 / 16.0 - 55.0 * e1.powi(4) / 32.0) * (4.0 * mu).sin()
            + (151.0 * e1.powi(3) / 96.0) * (6.0 * mu).sin()
            + (1097.0 * e1.powi(4) / 512.0) * (8.0 * mu).sin();
        let (s, c) = phi1.sin_cos();
        let c1 = ep2 * c * c;
        let t1 = phi1.tan().powi(2);
        let n1 = A / (1.0 - e2 * s * s).sqrt();
        let r1 = A * (1.0 - e2) / (1.0 - e2 * s * s).powf(1.5);
        let d = x / (n1 * K0);
        let phi = phi1
            - (n1 * phi1.tan() / r1)
                * (d * d / 2.0
                    - (5.0 + 3.0 * t1 + 10.0 * c1 - 4.0 * c1 * c1 - 9.0 * ep2) * d.powi(4) / 24.0
                    + (61.0 + 90.0 * t1 + 298.0 * c1 + 45.0 * t1 * t1
                        - 252.0 * ep2
                        - 3.0 * c1 * c1)
                        * d.powi(6)
                        / 720.0);
        let lam = self.lon0()
            + (d - (1.0 + 2.0 * t1 + c1) * d.powi(3) / 6.0
                + (5.0 - 2.0 * c1 + 28.0 * t1 - 3.0 * c1 * c1 + 8.0 * ep2 + 24.0 * t1 * t1)
                    * d.powi(5)
                    / 120.0)
                / c;
        [lam.to_degrees(), phi.to_degrees()]
    }
}

/// Длина дуги меридиана от экватора до широты `phi`.
fn meridian(phi: f64, e2: f64) -> f64 {
    let (e4, e6) = (e2 * e2, e2 * e2 * e2);
    A * ((1.0 - e2 / 4.0 - 3.0 * e4 / 64.0 - 5.0 * e6 / 256.0) * phi
        - (3.0 * e2 / 8.0 + 3.0 * e4 / 32.0 + 45.0 * e6 / 1024.0) * (2.0 * phi).sin()
        + (15.0 * e4 / 256.0 + 45.0 * e6 / 1024.0) * (4.0 * phi).sin()
        - (35.0 * e6 / 3072.0) * (6.0 * phi).sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Опорная точка: 10° в. д., 50° с. ш. в зоне 32N (EPSG:32632) -
    /// 571 666.45, 5 539 109.82. На осевом меридиане экватор - 500 000, 0.
    /// Туда и обратно - те же градусы до 1e-9.
    #[test]
    fn utm_like_proj() {
        let z = Utm::at(10.0, 50.0);
        assert_eq!((z.zone, z.north, z.epsg()), (32, true, 32632));
        let [x, y] = z.forward(10.0, 50.0);
        assert!((x - 571_666.45).abs() < 0.02, "{x}");
        assert!((y - 5_539_109.82).abs() < 0.02, "{y}");
        let [x, y] = Utm::at(3.0, 0.0).forward(3.0, 0.0);
        assert!((x - 500_000.0).abs() < 1e-6 && y.abs() < 1e-6);
        for (lon, lat) in [(57.3, 56.1), (59.9, 67.5), (-70.6, -33.4), (151.2, -33.9)] {
            let z = Utm::at(lon, lat);
            let [x, y] = z.forward(lon, lat);
            let [lo, la] = z.inverse(x, y);
            let [x2, y2] = z.forward(lo, la);
            // ряды Снайдера точны до миллиметра: сверка в метрах
            assert!(
                (x2 - x).abs() < 1e-3 && (y2 - y).abs() < 1e-3 && (la - lat).abs() < 1e-7,
                "{lon} {lat}"
            );
        }
        assert_eq!(Utm::at(-70.6, -33.4).epsg(), 32719);
    }
}
