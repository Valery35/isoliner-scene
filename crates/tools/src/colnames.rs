// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Поля по имени: как называют одно и то же Micromine, Datamine,
//! Surpac. Перенос `colnames.py` модуля.
//!
//! Сравнение без учёта регистра и без пробелов и точек по краям.
//! Порядок кандидатов - порядок предпочтения.

/// Кандидаты имени для роли, как `ROLES` модуля.
fn role(name: &str) -> &'static [&'static str] {
    match name {
        "hole" => &[
            "hole",
            "holeid",
            "hole_id",
            "bhid",
            "dhid",
            "well",
            "well_id",
            "wellid",
            "скважина",
            "скв",
            "номер",
        ],
        "collar_x" => &["east", "x", "xcollar", "easting", "x_collar"],
        "collar_y" => &["north", "y", "ycollar", "northing", "y_collar"],
        "collar_z" => &["rl", "z", "zcollar", "elevation", "elev", "z_collar"],
        "eoh" => &[
            "eoh",
            "depth",
            "max_depth",
            "maxdepth",
            "totaldepth",
            "total_depth",
            "length",
            "td",
            "глубина",
        ],
        "s_depth" => &["depth", "at", "md", "dist", "distance", "глубина"],
        "s_azi" => &["azimuth", "azi", "az", "brg", "bearing", "азимут"],
        "s_angle" => &[
            "dip",
            "inclination",
            "incl",
            "inc",
            "angle",
            "zenith",
            "угол",
        ],
        "from" => &[
            "from",
            "from_m",
            "depth_from",
            "fromdepth",
            "samp_from",
            "от",
        ],
        "to" => &["to", "to_m", "depth_to", "todepth", "samp_to", "до"],
        "unit" => &[
            "unit",
            "lith",
            "litho",
            "lithology",
            "soil_type",
            "soil",
            "rock",
            "rocktype",
            "rock_type",
            "formation",
            "geology",
            "geol",
            "code",
            "layer",
            "толща",
            "слой",
            "порода",
            "литология",
            "грунт",
        ],
        _ => &[],
    }
}

/// Имя для сравнения: нижний регистр, без пробелов и точек по краям.
/// Подчёркивание в начале значимо и не снимается.
fn norm(name: &str) -> String {
    name.trim().trim_matches('.').to_lowercase()
}

/// Имя поля из `names` для роли или None. `taken` - уже занятые имена.
pub fn guess(names: &[String], role_name: &str, taken: &[&str]) -> Option<String> {
    let mut by_norm: Vec<(String, &String)> = Vec::new();
    for nm in names {
        let k = norm(nm);
        if !by_norm.iter().any(|(n, _)| *n == k) {
            by_norm.push((k, nm));
        }
    }
    let busy: Vec<String> = taken
        .iter()
        .filter(|t| !t.is_empty())
        .map(|t| norm(t))
        .collect();
    for cand in role(role_name) {
        if let Some((_, nm)) = by_norm.iter().find(|(n, _)| n == cand)
            && !busy.iter().any(|b| b == cand)
        {
            return Some((*nm).clone());
        }
    }
    None
}

/// Выбранное поле, если оно есть в слое, иначе угаданное по имени.
pub fn resolve(chosen: &str, names: &[String], role_name: &str, taken: &[&str]) -> Option<String> {
    if !chosen.is_empty() && names.iter().any(|n| n == chosen) {
        return Some(chosen.to_string());
    }
    guess(names, role_name, taken)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_of_mining_software() {
        let cn: Vec<String> = ["HOLEID", " East ", "NORTH", "RL", "DEPTH"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(guess(&cn, "hole", &[]).as_deref(), Some("HOLEID"));
        assert_eq!(guess(&cn, "collar_x", &[]).as_deref(), Some(" East "));
        assert_eq!(guess(&cn, "eoh", &[]).as_deref(), Some("DEPTH"));
        // занятое поле глубины - не глубина замера
        assert_eq!(guess(&cn, "s_depth", &["DEPTH"]), None);
        assert_eq!(resolve("RL", &cn, "collar_x", &[]).as_deref(), Some("RL"));
        assert_eq!(resolve("нет", &cn, "collar_z", &[]).as_deref(), Some("RL"));
    }
}
