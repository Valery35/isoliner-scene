// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Склейка кадров PNG в ролик MP4 внешней программой ffmpeg.
//! Программа не ставится и в сборку не входит: если её нет в PATH,
//! кадры остаются как есть. Только на диске, в браузере записи кадров нет.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Имя программы склейки, ищется в PATH.
pub const FFMPEG: &str = "ffmpeg";

/// Почему ролик не склеился.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinError {
    /// Программы нет в PATH.
    Missing,
    /// Программа есть, но ролик не получился: её сообщение.
    Failed(String),
}

/// Имя ролика по имени пакета: `шахта.i3dscene` -> `шахта.mp4`.
pub fn mp4_name(package: Option<&str>) -> String {
    let png = crate::shot::png_name(package);
    let stem = png.strip_suffix(".png").unwrap_or(&png);
    format!("{stem}.mp4")
}

/// Ключи ffmpeg: кадры `frame_00000.png` и дальше из `dir` с частотой
/// `fps` в H.264, ширина и высота дополняются до чётных, как требует
/// yuv420p. Ролик перезаписывается.
pub fn ffmpeg_args(dir: &Path, fps: u32, out: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["-y", "-hide_banner", "-loglevel", "error", "-framerate"]
        .iter()
        .map(OsString::from)
        .collect();
    args.push(fps.to_string().into());
    args.push("-i".into());
    args.push(dir.join("frame_%05d.png").into_os_string());
    for a in [
        "-vf",
        "pad=ceil(iw/2)*2:ceil(ih/2)*2",
        "-c:v",
        "libx264",
        "-pix_fmt",
        "yuv420p",
        "-movflags",
        "+faststart",
    ] {
        args.push(a.into());
    }
    args.push(out.as_os_str().to_owned());
    args
}

/// Склеить кадры из `dir` в ролик `out` программой `program`.
/// Ждёт её конца, поэтому зовётся из отдельного потока.
pub fn join(program: &str, dir: &Path, fps: u32, out: &Path) -> Result<PathBuf, JoinError> {
    let mut cmd = Command::new(program);
    cmd.args(ffmpeg_args(dir, fps, out));
    // без чёрного окна консоли поверх окна сцены
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let res = cmd.output().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => JoinError::Missing,
        _ => JoinError::Failed(e.to_string()),
    })?;
    if res.status.success() && out.is_file() {
        return Ok(out.to_path_buf());
    }
    let text = String::from_utf8_lossy(&res.stderr);
    let last = text.lines().rev().find(|l| !l.trim().is_empty());
    Err(JoinError::Failed(last.map_or_else(
        || res.status.to_string(),
        |l| l.trim().to_string(),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_ffmpeg_the_frames_stay() {
        let dir = std::env::temp_dir().join(format!("isoliner-video-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let frame = dir.join("frame_00000.png");
        std::fs::write(&frame, b"png").unwrap();
        let out = dir.join("scene.mp4");
        let got = join("isoliner-no-such-ffmpeg", &dir, 25, &out);
        assert_eq!(got, Err(JoinError::Missing));
        assert!(frame.is_file());
        assert!(!out.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn args_and_name() {
        let args = ffmpeg_args(Path::new("кадры"), 25, Path::new("кадры/шахта.mp4"));
        let s: Vec<String> = args.iter().map(|a| a.to_string_lossy().into()).collect();
        let at = |k: &str| s.iter().position(|a| a == k).unwrap();
        assert_eq!(s[at("-framerate") + 1], "25");
        assert!(s[at("-i") + 1].ends_with("frame_%05d.png"));
        assert_eq!(s[at("-pix_fmt") + 1], "yuv420p");
        assert!(s.last().unwrap().ends_with("шахта.mp4"));
        assert_eq!(mp4_name(Some("C:\\данные\\шахта.i3dscene")), "шахта.mp4");
        assert_eq!(mp4_name(None), "scene.mp4");
    }
}
