// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Ссылка «Поделиться» и параметры адреса страницы, по образцу
//! PlanetX Lite (`ui::share`):
//!
//! - `?scene=адрес` - открыть пакет `.i3dscene` по ссылке;
//! - `?pose=x,y,z,удаление,азимут,наклон,раствор,проекция` - камера
//!   в настоящих координатах, проекция 1 - параллельная;
//! - `?check` - самопроверка страницы, отчёт в консоль и поверх.
//!
//! Разбор без браузера, чтобы его проверяли тесты.

use scene_format::Camera;

/// Что сказано в адресе страницы. Адрес есть только в браузере,
/// на диске разбор служит тестам.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Query {
    pub scene: Option<String>,
    pub pose: Option<Camera>,
    pub check: bool,
}

/// Разобрать `?a=b&c=d` из адреса. Неизвестное и испорченное
/// пропускается: страница открывается с тем, что понятно.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn parse_query(search: &str) -> Query {
    let mut q = Query::default();
    for item in search.trim_start_matches('?').split('&') {
        let (key, value) = item.split_once('=').unwrap_or((item, ""));
        match key {
            "scene" if !value.is_empty() => q.scene = decode(value),
            "pose" => q.pose = parse_pose(&decode(value).unwrap_or_default()),
            "check" => q.check = true,
            _ => {}
        }
    }
    q
}

/// Камера из `x,y,z,удаление,азимут,наклон[,раствор[,проекция]]`.
pub fn parse_pose(s: &str) -> Option<Camera> {
    let v: Vec<f64> = s
        .split(',')
        .map(|p| p.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .ok()?;
    if v.len() < 6 || v.iter().any(|x| !x.is_finite()) || v[3] <= 0.0 {
        return None;
    }
    let fov = v.get(6).copied().filter(|f| *f > 0.0 && *f < 180.0);
    Some(Camera {
        center: [v[0], v[1], v[2]],
        distance: v[3],
        azimuth: v[4],
        elevation: v[5].clamp(-90.0, 90.0),
        fov: fov.unwrap_or(60.0),
        ortho: v.get(7).is_some_and(|o| *o != 0.0),
    })
}

/// Камера строкой для `?pose=`: сантиметры, десятые доли градуса.
pub fn format_pose(c: &Camera) -> String {
    format!(
        "{:.2},{:.2},{:.2},{:.1},{:.1},{:.1},{:.1},{}",
        c.center[0],
        c.center[1],
        c.center[2],
        c.distance,
        c.azimuth.rem_euclid(360.0),
        c.elevation,
        c.fov,
        u8::from(c.ortho)
    )
}

/// Ссылка на страницу `base` с пакетом по адресу `scene` и камерой.
pub fn link(base: &str, scene: Option<&str>, cam: &Camera) -> String {
    let mut out = format!("{base}?");
    if let Some(s) = scene {
        out.push_str("scene=");
        out.push_str(&encode(s));
        out.push('&');
    }
    out.push_str("pose=");
    out.push_str(&format_pose(cam));
    out
}

/// Знаки, которые в значении параметра можно оставить как есть.
fn plain(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"-_.~/:".contains(&b)
}

/// Закодировать значение параметра адреса, как encodeURIComponent,
/// но `/` и `:` оставить: ссылка читается глазами.
pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if plain(b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Раскодировать `%XX` и `+`. Испорченная запись - None.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' => {
                let hex = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam() -> Camera {
        Camera {
            center: [512405.0, 6401269.0, -235.0],
            distance: 650.0,
            azimuth: -60.0,
            elevation: 25.0,
            fov: 60.0,
            ortho: false,
        }
    }

    #[test]
    fn link_round_trip() {
        let url = "https://example.org/сцены/шахта 1.i3dscene?v=2&x=1";
        let s = link(
            "https://valery35.github.io/isoliner-scene/app/",
            Some(url),
            &cam(),
        );
        assert!(s.starts_with("https://valery35.github.io/isoliner-scene/app/?scene=https://"));
        assert!(!s[s.find('?').unwrap() + 1..].contains(' '));
        let q = parse_query(&s[s.find('?').unwrap()..]);
        assert_eq!(q.scene.as_deref(), Some(url));
        let p = q.pose.unwrap();
        assert_eq!(p.center, cam().center);
        assert_eq!(p.distance, 650.0);
        // азимут в 0..360
        assert_eq!(p.azimuth, 300.0);
        assert_eq!((p.elevation, p.fov, p.ortho), (25.0, 60.0, false));
        assert!(!q.check);
    }

    #[test]
    fn short_and_broken_pose() {
        let p = parse_pose("1,2,3,100,10,95").unwrap();
        assert_eq!((p.fov, p.ortho, p.elevation), (60.0, false, 90.0));
        assert!(parse_pose("1,2,3,100,10,20,60,1").unwrap().ortho);
        assert!(parse_pose("1,2,3").is_none());
        assert!(parse_pose("1,2,3,0,10,20").is_none());
        assert!(parse_pose("1,2,x,100,10,20").is_none());
        assert!(parse_pose("1,2,NaN,100,10,20").is_none());
    }

    #[test]
    fn query_keeps_what_it_understands() {
        let q = parse_query("?check&foo=bar&pose=%ZZ&scene=demo.i3dscene");
        assert!(q.check);
        assert_eq!(q.scene.as_deref(), Some("demo.i3dscene"));
        assert!(q.pose.is_none());
        assert_eq!(parse_query(""), Query::default());
        assert_eq!(decode("a%2"), None);
        assert_eq!(decode("a+b%20c").as_deref(), Some("a b c"));
    }
}
