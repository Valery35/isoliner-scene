// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Геометрия Shapefile (`.shp`) в GeoJSON.
//!
//! Нужна сцене из слоёв: RustGIS отдаёт расширению объекты только
//! в градусах WGS 84, а в метрах своей системы слой выходит лишь выгрузкой
//! (`share.exportLayer`, Shapefile, система - та, в которой слой прочитан,
//! или заданная). Читаются точки, мультиточки, линии и полигоны, с Z и без.
//! Атрибуты не нужны: сцене хватает геометрии.
//!
//! Файл считается враждебным: длины записей, числа частей и точек
//! сверяются с размером файла до чтения.

use serde_json::{Value as J, json};

/// Не больше стольких точек в записи: защита от порченого счётчика.
const MAX_POINTS: usize = 50_000_000;

struct Reader<'a> {
    b: &'a [u8],
}

impl Reader<'_> {
    fn i32_le(&self, at: usize) -> Option<i32> {
        let s = self.b.get(at..at.checked_add(4)?)?;
        Some(i32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    fn i32_be(&self, at: usize) -> Option<i32> {
        let s = self.b.get(at..at.checked_add(4)?)?;
        Some(i32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }

    fn f64_le(&self, at: usize) -> Option<f64> {
        let s = self.b.get(at..at.checked_add(8)?)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(s);
        Some(f64::from_le_bytes(a))
    }

    fn count(&self, at: usize) -> Option<usize> {
        usize::try_from(self.i32_le(at)?)
            .ok()
            .filter(|&n| n <= MAX_POINTS)
    }
}

/// Точки записи с `at`: x, y, затем, у вида с Z, блок Z после всех точек.
fn points(r: &Reader<'_>, at: usize, n: usize, z_at: Option<usize>) -> Option<Vec<J>> {
    (0..n)
        .map(|i| {
            let p = at.checked_add(i.checked_mul(16)?)?;
            let (x, y) = (r.f64_le(p)?, r.f64_le(p + 8)?);
            let z = z_at.and_then(|z| r.f64_le(z.checked_add(i.checked_mul(8)?)?));
            Some(match z {
                Some(z) if z.is_finite() => json!([x, y, z]),
                _ => json!([x, y]),
            })
        })
        .collect()
}

/// Одна запись: содержимое с `at` длиной `len` байт. None - запись
/// порчена, Some(Null) - пустая фигура.
fn record(r: &Reader<'_>, at: usize, len: usize) -> Option<J> {
    let end = at.checked_add(len)?;
    if end > r.b.len() || len < 4 {
        return None;
    }
    let kind = r.i32_le(at)?;
    let has_z = matches!(kind, 11 | 13 | 15 | 18);
    Some(match kind {
        0 => J::Null,
        1 | 11 | 21 => {
            let mut p = vec![json!(r.f64_le(at + 4)?), json!(r.f64_le(at + 12)?)];
            if kind == 11 {
                p.push(json!(r.f64_le(at + 20)?));
            }
            json!({"type": "Point", "coordinates": p})
        }
        8 | 18 | 28 => {
            let n = r.count(at + 36)?;
            let pts_at = at + 40;
            let z_at = has_z.then(|| pts_at + n * 16 + 16);
            if pts_at.checked_add(n.checked_mul(16)?)? > end {
                return None;
            }
            json!({"type": "MultiPoint", "coordinates": points(r, pts_at, n, z_at)?})
        }
        3 | 13 | 23 | 5 | 15 | 25 => {
            let parts = r.count(at + 36)?;
            let n = r.count(at + 40)?;
            let starts_at = at + 44;
            let pts_at = starts_at.checked_add(parts.checked_mul(4)?)?;
            if pts_at.checked_add(n.checked_mul(16)?)? > end {
                return None;
            }
            let z_at = has_z.then(|| pts_at + n * 16 + 16);
            let all = points(r, pts_at, n, z_at)?;
            let mut starts: Vec<usize> = (0..parts)
                .map(|i| usize::try_from(r.i32_le(starts_at + i * 4)?).ok())
                .collect::<Option<_>>()?;
            starts.push(n);
            let mut rings = Vec::new();
            for w in starts.windows(2) {
                let (a, b) = (w[0], w[1]);
                if a > b || b > n {
                    return None;
                }
                rings.push(J::Array(all[a..b].to_vec()));
            }
            if matches!(kind, 5 | 15 | 25) {
                json!({"type": "Polygon", "coordinates": rings})
            } else {
                json!({"type": "MultiLineString", "coordinates": rings})
            }
        }
        _ => return None,
    })
}

/// Объекты `.shp` в FeatureCollection по порядку записей. Ошибка - файл
/// не Shapefile или порчен.
pub fn read(bytes: &[u8]) -> Result<J, String> {
    let r = Reader { b: bytes };
    if bytes.len() < 100 || r.i32_be(0) != Some(9994) {
        return Err("not a shapefile".to_string());
    }
    let mut feats = Vec::new();
    let mut at = 100usize;
    while at + 8 <= bytes.len() {
        let words = r
            .i32_be(at + 4)
            .and_then(|w| usize::try_from(w).ok())
            .ok_or("bad record length")?;
        let len = words.checked_mul(2).ok_or("bad record length")?;
        let g = record(&r, at + 8, len).ok_or("bad record")?;
        feats.push(json!({"type": "Feature", "properties": {}, "geometry": g}));
        at = at
            .checked_add(8)
            .and_then(|a| a.checked_add(len))
            .ok_or("bad record length")?;
    }
    Ok(json!({"type": "FeatureCollection", "features": feats}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(len_words: i32) -> Vec<u8> {
        let mut h = vec![0u8; 100];
        h[0..4].copy_from_slice(&9994i32.to_be_bytes());
        h[24..28].copy_from_slice(&len_words.to_be_bytes());
        h[28..32].copy_from_slice(&1000i32.to_le_bytes());
        h
    }

    fn rec(n: i32, content: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&n.to_be_bytes());
        v.extend_from_slice(&((content.len() / 2) as i32).to_be_bytes());
        v.extend_from_slice(content);
        v
    }

    fn polyline(pts: &[[f64; 2]]) -> Vec<u8> {
        let mut c = Vec::new();
        c.extend_from_slice(&3i32.to_le_bytes());
        c.extend_from_slice(&[0u8; 32]);
        c.extend_from_slice(&1i32.to_le_bytes());
        c.extend_from_slice(&(pts.len() as i32).to_le_bytes());
        c.extend_from_slice(&0i32.to_le_bytes());
        for p in pts {
            c.extend_from_slice(&p[0].to_le_bytes());
            c.extend_from_slice(&p[1].to_le_bytes());
        }
        c
    }

    /// Точка и линия читаются в метрах как есть, порченый файл - ошибка.
    #[test]
    fn reads_points_and_lines_and_refuses_junk() {
        let mut point = Vec::new();
        point.extend_from_slice(&1i32.to_le_bytes());
        point.extend_from_slice(&500_000.0f64.to_le_bytes());
        point.extend_from_slice(&6_000_000.0f64.to_le_bytes());
        let line = polyline(&[[500_000.0, 6_000_000.0], [500_100.0, 6_000_050.0]]);
        let mut f = header(0);
        f.extend(rec(1, &point));
        f.extend(rec(2, &line));
        let fc = read(&f).unwrap();
        let feats = fc["features"].as_array().unwrap();
        assert_eq!(
            feats[0]["geometry"]["coordinates"],
            json!([500_000.0, 6_000_000.0])
        );
        assert_eq!(
            feats[1]["geometry"]["coordinates"][0][1],
            json!([500_100.0, 6_000_050.0])
        );
        // порча: обрезанный файл, счётчик точек за краем, чужой файл
        assert!(read(&f[..f.len() - 5]).is_err());
        let mut bad = polyline(&[[0.0, 0.0], [1.0, 1.0]]);
        bad[40..44].copy_from_slice(&1_000_000i32.to_le_bytes());
        let mut g = header(0);
        g.extend(rec(1, &bad));
        assert!(read(&g).is_err());
        assert!(read(b"not a shapefile at all").is_err());
        assert!(read(&[0u8; 100]).is_err());
    }
}
