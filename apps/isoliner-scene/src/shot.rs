// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Снимок кадра в PNG, как «Кадр в PNG» окна модуля (`_save_png`).

/// RGBA `width` x `height`, строки сверху вниз, в байты PNG.
/// Прозрачности в кадре нет: альфа пишется как есть, она всюду 255.
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let need = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or("размер снимка")?;
    if rgba.len() != need || width == 0 || height == 0 {
        return Err(format!(
            "снимок {width} x {height}: {} байт вместо {need}",
            rgba.len()
        ));
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().map_err(|e| e.to_string())?;
        w.write_image_data(rgba).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// Имя файла снимка по имени пакета: `шахта.i3dscene` -> `шахта.png`.
pub fn png_name(package: Option<&str>) -> String {
    let stem = package
        .and_then(|n| std::path::Path::new(n).file_stem())
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "scene".to_string());
    format!("{stem}.png")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_round_trip() {
        let (w, h) = (3, 2);
        let rgba: Vec<u8> = (0..w * h * 4).map(|i| (i * 7) as u8).collect();
        let bytes = encode_png(w, h, &rgba).unwrap();
        let dec = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut r = dec.read_info().unwrap();
        let mut buf = vec![0; r.output_buffer_size().unwrap()];
        let info = r.next_frame(&mut buf).unwrap();
        assert_eq!((info.width, info.height), (w, h));
        assert_eq!(&buf[..info.buffer_size()], &rgba[..]);
        // размер не сходится - ошибка, а не паника
        assert!(encode_png(w, h, &rgba[..5]).is_err());
        assert!(encode_png(0, 0, &[]).is_err());
    }

    #[test]
    fn png_name_from_package() {
        assert_eq!(png_name(Some("C:\\данные\\шахта.i3dscene")), "шахта.png");
        assert_eq!(png_name(Some("scene_demo.i3dscene")), "scene_demo.png");
        assert_eq!(png_name(None), "scene.png");
    }
}
