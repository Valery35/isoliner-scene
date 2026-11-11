// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Пакет .i3dscene: zip из scene.json и scene.glb.

use std::io::{Cursor, Read, Write};

use crate::glb::{self, Georef, Part};
use crate::meta::{FORMAT, FORMAT_POINTS, PartInfo, PartKind, SceneMeta};
use crate::{Error, MAX_ENTRY};

pub const GLB_NAME: &str = "scene.glb";
pub const JSON_NAME: &str = "scene.json";

/// Сцена целиком: описание, части в настоящих координатах, привязка.
#[derive(Debug, Clone, PartialEq)]
pub struct Package {
    pub meta: SceneMeta,
    pub parts: Vec<Part>,
    pub georef: Georef,
}

fn entry(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Vec<u8>, Error> {
    let file = zip
        .by_name(name)
        .map_err(|e| Error::Zip(format!("{name}: {e}")))?;
    if file.size() > MAX_ENTRY {
        return Err(Error::TooLarge("часть пакета", file.size()));
    }
    let mut out = Vec::new();
    // Объявленному размеру не верим: читаем не больше предела.
    file.take(MAX_ENTRY + 1)
        .read_to_end(&mut out)
        .map_err(|e| Error::Zip(format!("{name}: {e}")))?;
    if out.len() as u64 > MAX_ENTRY {
        return Err(Error::TooLarge("часть пакета", out.len() as u64));
    }
    Ok(out)
}

/// Прочитать пакет из байтов.
pub fn read_package(data: &[u8]) -> Result<Package, Error> {
    let mut zip = zip::ZipArchive::new(Cursor::new(data)).map_err(|e| Error::Zip(e.to_string()))?;
    let js = entry(&mut zip, JSON_NAME)?;
    let meta: SceneMeta = serde_json::from_slice(&js).map_err(|e| Error::Json(e.to_string()))?;
    if meta.format > FORMAT {
        return Err(Error::TooNew(meta.format));
    }
    let glb_name = if meta.glb.is_empty() {
        GLB_NAME
    } else {
        meta.glb.as_str()
    };
    let raw = entry(&mut zip, glb_name)?;
    let (parts, georef) = glb::read(&raw)?;
    Ok(Package {
        meta,
        parts,
        georef,
    })
}

/// Описание частей для scene.json по самим частям. Признаки показа
/// (decor, layer, opacity) берутся из `old` по номеру узла, если есть.
pub fn describe(parts: &[Part], old: &[PartInfo]) -> Vec<PartInfo> {
    parts
        .iter()
        .filter(|p| !p.verts.is_empty() && !p.indices.is_empty())
        .enumerate()
        .map(|(node, p)| {
            let prev = old.iter().find(|o| o.node == node);
            PartInfo {
                node,
                name: p.name.clone(),
                kind: match p.indices {
                    glb::Indices::Triangles(_) => PartKind::Mesh,
                    glb::Indices::Lines(_) => PartKind::Lines,
                    glb::Indices::Points(_) => PartKind::Points,
                },
                verts: p.verts.len() as u64,
                elements: p.indices.len() as u64,
                decor: prev.is_some_and(|o| o.decor),
                layer: prev.and_then(|o| o.layer.clone()),
                opacity: prev.and_then(|o| o.opacity),
                shape: prev.and_then(|o| o.shape.clone()),
            }
        })
        .collect()
}

/// Записать пакет в байты. Описание частей пересчитывается по частям,
/// точка начала - по охвату, как в модуле.
pub fn write_package(pkg: &Package) -> Result<Vec<u8>, Error> {
    let mut meta = pkg.meta.clone();
    meta.origin = glb::georef_origin(&pkg.parts, 1000.0);
    meta.parts = describe(&pkg.parts, &pkg.meta.parts);
    // наименьший формат, которым пакет читается, как в модуле
    meta.format = if meta.parts.iter().any(|p| p.kind == PartKind::Points) {
        FORMAT_POINTS
    } else {
        1
    };
    meta.glb = GLB_NAME.to_string();
    let georef = Georef {
        crs: meta.crs.authid.clone(),
        crs_wkt: meta.crs.wkt.clone(),
        origin: meta.origin,
        vex: 1.0,
    };
    let name = if meta.name.is_empty() {
        "Isoliner3D"
    } else {
        meta.name.as_str()
    };
    let glb_bytes = glb::write(&pkg.parts, name, &georef);
    let js = serde_json::to_vec_pretty(&meta).map_err(|e| Error::Json(e.to_string()))?;
    let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in [(JSON_NAME, &js), (GLB_NAME, &glb_bytes)] {
        zw.start_file(name, opts)
            .map_err(|e| Error::Zip(e.to_string()))?;
        zw.write_all(data).map_err(|e| Error::Zip(e.to_string()))?;
    }
    let cur = zw.finish().map_err(|e| Error::Zip(e.to_string()))?;
    Ok(cur.into_inner())
}
