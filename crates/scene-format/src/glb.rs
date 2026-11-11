// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! GLB сцены: чтение и запись в том виде, как пишет `gltf.py` модуля.
//!
//! Узел - одна часть, у части один примитив: треугольники или линии,
//! позиции float32, номера вершин uint32, цвет вершин RGBA float32.
//! Оси glTF: x - восток, y - вверх, z - минус север. Позиция в файле -
//! сдвиг от точки начала `origin`, настоящая координата считается
//! в double: восток = x + origin[0], север = -z + origin[1],
//! отметка = y + origin[2].

use serde_json::{Value, json};

use crate::Error;

const MAGIC: u32 = 0x4654_6C67;
const CHUNK_JSON: u32 = 0x4E4F_534A;
const CHUNK_BIN: u32 = 0x004E_4942;
const FLOAT: u64 = 5126;
const UINT: u64 = 5125;
const USHORT: u64 = 5123;
const UBYTE: u64 = 5121;
const TRIANGLES: u64 = 4;
const LINES: u64 = 1;

/// Номера вершин части.
#[derive(Debug, Clone, PartialEq)]
pub enum Indices {
    Triangles(Vec<[u32; 3]>),
    Lines(Vec<[u32; 2]>),
}

impl Indices {
    /// Количество треугольников или отрезков.
    pub fn len(&self) -> usize {
        match self {
            Indices::Triangles(t) => t.len(),
            Indices::Lines(l) => l.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Часть сцены в настоящих координатах.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub name: String,
    /// Вершины: восток, север, отметка, метры, double.
    pub verts: Vec<[f64; 3]>,
    pub indices: Indices,
    /// Цвет вершин RGBA в долях единицы.
    pub colors: Option<Vec<[f32; 4]>>,
}

/// Привязка из `asset.extras.isoliner3d`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Georef {
    pub crs: String,
    pub crs_wkt: String,
    pub origin: [f64; 3],
    /// Преувеличение, применённое к высотам в файле. В пакете 1.
    pub vex: f64,
}

fn bad(msg: impl Into<String>) -> Error {
    Error::Glb(msg.into())
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, Error> {
    let s = b.get(at..at + 4).ok_or_else(|| bad("файл обрезан"))?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Разобранный документ glTF и двоичный кусок.
struct Doc<'a> {
    json: Value,
    bin: &'a [u8],
}

fn split(glb: &[u8]) -> Result<Doc<'_>, Error> {
    if u32_at(glb, 0)? != MAGIC {
        return Err(bad("не GLB"));
    }
    if u32_at(glb, 4)? != 2 {
        return Err(bad("поддерживается glTF 2"));
    }
    let n_js = u32_at(glb, 12)? as usize;
    if u32_at(glb, 16)? != CHUNK_JSON {
        return Err(bad("первый кусок не JSON"));
    }
    let js = glb
        .get(20..20usize.saturating_add(n_js))
        .ok_or_else(|| bad("кусок JSON обрезан"))?;
    let json: Value = serde_json::from_slice(js).map_err(|e| bad(format!("JSON: {e}")))?;
    let off = 20 + n_js;
    let bin: &[u8] = if off + 8 <= glb.len() {
        let n_bin = u32_at(glb, off)? as usize;
        if u32_at(glb, off + 4)? != CHUNK_BIN {
            return Err(bad("второй кусок не BIN"));
        }
        glb.get(off + 8..(off + 8).saturating_add(n_bin))
            .ok_or_else(|| bad("кусок BIN обрезан"))?
    } else {
        &[]
    };
    Ok(Doc { json, bin })
}

fn num(v: &Value, key: &str) -> Result<u64, Error> {
    v.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| bad(format!("нет поля {key}")))
}

impl Doc<'_> {
    /// Байты аксессора и число компонент: (байты, количество, компонент, тип).
    fn accessor(&self, i: u64) -> Result<(&[u8], usize, usize, u64), Error> {
        let acc = self
            .json
            .get("accessors")
            .and_then(|a| a.get(i as usize))
            .ok_or_else(|| bad(format!("нет аксессора {i}")))?;
        let comp = match acc.get("type").and_then(Value::as_str) {
            Some("SCALAR") => 1,
            Some("VEC2") => 2,
            Some("VEC3") => 3,
            Some("VEC4") => 4,
            other => return Err(bad(format!("тип аксессора {other:?}"))),
        };
        let ctype = num(acc, "componentType")?;
        let size = match ctype {
            FLOAT | UINT => 4,
            USHORT => 2,
            UBYTE => 1,
            _ => return Err(bad(format!("тип компоненты {ctype}"))),
        };
        let count = num(acc, "count")? as usize;
        let view = self
            .json
            .get("bufferViews")
            .and_then(|v| v.get(num(acc, "bufferView").unwrap_or(u64::MAX) as usize))
            .ok_or_else(|| bad("нет bufferView"))?;
        if view.get("byteStride").and_then(Value::as_u64).unwrap_or(0) as usize > comp * size {
            return Err(bad("чередование данных не поддерживается"));
        }
        let start = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize
            + acc.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
        let len = count
            .checked_mul(comp * size)
            .ok_or_else(|| bad("размер аксессора"))?;
        let bytes = self
            .bin
            .get(start..start.saturating_add(len))
            .ok_or_else(|| bad("аксессор за краем данных"))?;
        Ok((bytes, count, comp, ctype))
    }

    fn floats(&self, i: u64, want: usize) -> Result<Vec<f32>, Error> {
        let (b, _n, comp, ctype) = self.accessor(i)?;
        if ctype != FLOAT || comp != want {
            return Err(bad("ожидались числа float"));
        }
        Ok(b.as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect())
    }

    fn ints(&self, i: u64) -> Result<Vec<u32>, Error> {
        let (b, _n, comp, ctype) = self.accessor(i)?;
        if comp != 1 {
            return Err(bad("номера вершин не SCALAR"));
        }
        Ok(match ctype {
            UINT => b
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| u32::from_le_bytes(*c))
                .collect(),
            USHORT => b
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u32::from(u16::from_le_bytes(*c)))
                .collect(),
            UBYTE => b.iter().map(|&x| u32::from(x)).collect(),
            _ => return Err(bad("номера вершин не целые")),
        })
    }
}

fn georef_of(json: &Value) -> Georef {
    let ex = &json["asset"]["extras"]["isoliner3d"];
    let mut g = Georef {
        vex: ex["vertical_exaggeration"].as_f64().unwrap_or(1.0),
        crs: ex["crs"].as_str().unwrap_or("").to_string(),
        crs_wkt: ex["crs_wkt"].as_str().unwrap_or("").to_string(),
        origin: [0.0; 3],
    };
    if let Some(o) = ex["origin"].as_array() {
        for (k, v) in o.iter().take(3).enumerate() {
            g.origin[k] = v.as_f64().unwrap_or(0.0);
        }
    }
    g
}

/// Части и привязка из GLB.
pub fn read(glb: &[u8]) -> Result<(Vec<Part>, Georef), Error> {
    let doc = split(glb)?;
    let georef = georef_of(&doc.json);
    let o = georef.origin;
    let mut parts = Vec::new();
    let nodes = doc.json["nodes"].as_array().cloned().unwrap_or_default();
    for node in &nodes {
        let Some(mi) = node.get("mesh").and_then(Value::as_u64) else {
            continue;
        };
        let prim = &doc.json["meshes"][mi as usize]["primitives"][0];
        let attrs = &prim["attributes"];
        let pos = doc.floats(num(attrs, "POSITION")?, 3)?;
        let verts: Vec<[f64; 3]> = pos
            .as_chunks::<3>()
            .0
            .iter()
            .map(|p| {
                [
                    f64::from(p[0]) + o[0],
                    -f64::from(p[2]) + o[1],
                    f64::from(p[1]) + o[2],
                ]
            })
            .collect();
        let idx = doc.ints(num(prim, "indices")?)?;
        if idx.iter().any(|&i| i as usize >= verts.len()) {
            return Err(bad("номер вершины за краем"));
        }
        let mode = prim
            .get("mode")
            .and_then(Value::as_u64)
            .unwrap_or(TRIANGLES);
        let indices = match mode {
            TRIANGLES => Indices::Triangles(idx.as_chunks::<3>().0.to_vec()),
            LINES => Indices::Lines(idx.as_chunks::<2>().0.to_vec()),
            m => return Err(bad(format!("вид примитива {m}"))),
        };
        let colors = match attrs.get("COLOR_0").and_then(Value::as_u64) {
            Some(ci) => {
                let (_b, _n, comp, _t) = doc.accessor(ci)?;
                let c = doc.floats(ci, comp)?;
                let cols: Vec<[f32; 4]> = if comp == 4 {
                    c.as_chunks::<4>().0.to_vec()
                } else {
                    c.as_chunks::<3>()
                        .0
                        .iter()
                        .map(|&[r, g, b]| [r, g, b, 1.0])
                        .collect()
                };
                if cols.len() != verts.len() {
                    return Err(bad("цветов не столько, сколько вершин"));
                }
                Some(cols)
            }
            None => None,
        };
        parts.push(Part {
            name: node["name"].as_str().unwrap_or("").to_string(),
            verts,
            indices,
            colors,
        });
    }
    Ok((parts, georef))
}

/// Точка начала: середина охвата частей, круглая до `step` метров.
pub fn georef_origin(parts: &[Part], step: f64) -> [f64; 3] {
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for v in parts.iter().flat_map(|p| p.verts.iter()) {
        for k in 0..2 {
            lo[k] = lo[k].min(v[k]);
            hi[k] = hi[k].max(v[k]);
        }
    }
    if !lo[0].is_finite() {
        return [0.0; 3];
    }
    // round у Python - к чётному, у Rust - от нуля. Середина на ровной
    // половине шага попадается редко, но точка начала обязана совпасть
    // с модулем: округляем как Python.
    let r = |x: f64| (x / step).round_ties_even() * step;
    [r((lo[0] + hi[0]) / 2.0), r((lo[1] + hi[1]) / 2.0), 0.0]
}

fn pad4(buf: &mut Vec<u8>, fill: u8) {
    while !buf.len().is_multiple_of(4) {
        buf.push(fill);
    }
}

/// Записать части в GLB с привязкой, как `gltf.build_glb` модуля.
pub fn write(parts: &[Part], name: &str, georef: &Georef) -> Vec<u8> {
    let shift = georef.origin;
    let mut buf: Vec<u8> = Vec::new();
    let mut views = Vec::new();
    let mut accessors = Vec::new();
    let mut meshes = Vec::new();
    let mut nodes = Vec::new();
    let mut need_blend = false;
    let mut add_view = |buf: &mut Vec<u8>, data: &[u8], target: u64| {
        let offset = buf.len();
        buf.extend_from_slice(data);
        pad4(buf, 0);
        views.push(json!({"buffer": 0, "byteOffset": offset,
                          "byteLength": data.len(), "target": target}));
        views.len() - 1
    };
    for part in parts {
        if part.verts.is_empty() || part.indices.is_empty() {
            continue;
        }
        let pos: Vec<[f32; 3]> = part
            .verts
            .iter()
            .map(|v| {
                let e = (v[0] - shift[0]) as f32;
                let n = (v[1] - shift[1]) as f32;
                let z = (v[2] - shift[2]) as f32;
                [e, z, -n]
            })
            .collect();
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        let mut bytes = Vec::with_capacity(pos.len() * 12);
        for p in &pos {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
                bytes.extend_from_slice(&p[k].to_le_bytes());
            }
        }
        let vi = add_view(&mut buf, &bytes, 34962);
        accessors.push(json!({"bufferView": vi, "componentType": FLOAT,
            "count": pos.len(), "type": "VEC3", "min": lo, "max": hi}));
        let p_acc = accessors.len() - 1;
        let (flat, mode): (Vec<u32>, u64) = match &part.indices {
            Indices::Triangles(t) => (t.iter().flatten().copied().collect(), TRIANGLES),
            Indices::Lines(l) => (l.iter().flatten().copied().collect(), LINES),
        };
        let ib: Vec<u8> = flat.iter().flat_map(|i| i.to_le_bytes()).collect();
        let fi = add_view(&mut buf, &ib, 34963);
        accessors.push(json!({"bufferView": fi, "componentType": UINT,
            "count": flat.len(), "type": "SCALAR"}));
        let i_acc = accessors.len() - 1;
        let mut attrs = json!({"POSITION": p_acc});
        let mut soft = false;
        if let Some(cols) = part.colors.as_ref().filter(|c| !c.is_empty()) {
            let cb: Vec<u8> = cols
                .iter()
                .flatten()
                .flat_map(|c| c.to_le_bytes())
                .collect();
            let ci = add_view(&mut buf, &cb, 34962);
            accessors.push(json!({"bufferView": ci, "componentType": FLOAT,
                "count": cols.len(), "type": "VEC4"}));
            attrs["COLOR_0"] = json!(accessors.len() - 1);
            soft = cols.iter().any(|c| c[3] < 0.999);
            need_blend |= soft;
        }
        meshes.push(json!({"name": part.name, "primitives": [{
            "attributes": attrs, "indices": i_acc,
            "material": if soft { 1 } else { 0 }, "mode": mode}]}));
        nodes.push(json!({"mesh": meshes.len() - 1, "name": part.name}));
    }
    let base = json!({"name": "vertex", "pbrMetallicRoughness": {
        "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
        "metallicFactor": 0.0, "roughnessFactor": 0.9}, "doubleSided": true});
    let mut materials = vec![base];
    if need_blend {
        materials.push(json!({"name": "vertex_blend", "pbrMetallicRoughness": {
            "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
            "metallicFactor": 0.0, "roughnessFactor": 0.9},
            "doubleSided": true, "alphaMode": "BLEND"}));
    }
    let extras = json!({"isoliner3d": {
        "crs": georef.crs, "crs_wkt": georef.crs_wkt,
        "origin": shift,
        "axes": "gltf x = east, gltf y = up, gltf z = -north",
        "units": "m",
        "real_xyz": "east = x + origin[0], north = -z + origin[1], elevation = y + origin[2]",
        "vertical_exaggeration": georef.vex}});
    let doc = json!({
        "asset": {"version": "2.0", "generator": name, "extras": extras},
        "scene": 0,
        "scenes": [{"nodes": (0..nodes.len()).collect::<Vec<_>>()}],
        "nodes": nodes, "meshes": meshes, "accessors": accessors,
        "bufferViews": views, "buffers": [{"byteLength": buf.len()}],
        "materials": materials,
    });
    let mut js = doc.to_string().into_bytes();
    pad4(&mut js, b' ');
    pad4(&mut buf, 0);
    let total = 12 + 8 + js.len() + 8 + buf.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(js.len() as u32).to_le_bytes());
    out.extend_from_slice(&CHUNK_JSON.to_le_bytes());
    out.extend_from_slice(&js);
    out.extend_from_slice(&(buf.len() as u32).to_le_bytes());
    out.extend_from_slice(&CHUNK_BIN.to_le_bytes());
    out.extend_from_slice(&buf);
    out
}
