// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Слежение за открытым пакетом: модуль QGIS пересобрал сцену
//! и переписал файл - окно перечитывает его само. Опрос времени
//! и длины файла раз в полсекунды, без своих потоков и крейтов.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Как часто смотреть на файл, секунды.
pub const PERIOD: f64 = 0.5;

/// Отпечаток файла: время записи и длина.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    modified: Option<SystemTime>,
    len: u64,
}

/// Отпечаток файла сейчас. Файла нет - None: модуль как раз
/// переименовывает новый пакет на место старого.
pub fn stamp(path: &Path) -> Option<Stamp> {
    let m = std::fs::metadata(path).ok()?;
    Some(Stamp {
        modified: m.modified().ok(),
        len: m.len(),
    })
}

/// Слежение за одним файлом.
#[derive(Debug, Clone)]
pub struct Watch {
    pub path: PathBuf,
    /// Отпечаток прочитанного пакета.
    seen: Option<Stamp>,
    /// Когда смотрели последний раз, секунды окна.
    last: f64,
}

impl Watch {
    /// Следить за файлом, который только что прочитан.
    pub fn new(path: PathBuf) -> Self {
        let seen = stamp(&path);
        Self {
            path,
            seen,
            last: f64::NEG_INFINITY,
        }
    }

    /// Пора ли перечитать: прошло полсекунды и отпечаток другой.
    /// Новый отпечаток не запоминается, пока пакет не прочитан
    /// ([`Watch::read_ok`]): недописанный файл перечитается
    /// в следующий раз.
    pub fn due(&mut self, now: f64) -> bool {
        if now - self.last < PERIOD {
            return false;
        }
        self.last = now;
        match stamp(&self.path) {
            Some(s) => Some(s) != self.seen,
            None => false,
        }
    }

    /// Перечитать при следующем опросе, даже если файл не менялся:
    /// кнопка «Перечитать пакет».
    pub fn force(&mut self) {
        self.seen = None;
        self.last = f64::NEG_INFINITY;
    }

    /// Пакет прочитан: его отпечаток теперь свой.
    pub fn read_ok(&mut self) {
        self.seen = stamp(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_is_seen_once_read() {
        let dir = std::env::temp_dir().join(format!("isoliner-watch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("scene.i3dscene");
        std::fs::write(&path, b"one").unwrap();
        let mut w = Watch::new(path.clone());
        assert!(!w.due(0.0));
        // длина другая - пакет переписан
        std::fs::write(&path, b"second").unwrap();
        assert!(!w.due(0.2), "раньше полсекунды не смотрим");
        assert!(w.due(0.6));
        // не прочитали - видим снова
        assert!(w.due(1.2));
        w.read_ok();
        assert!(!w.due(1.8));
        // файла нет, пока модуль кладёт новый, - не перечитываем
        std::fs::remove_file(&path).unwrap();
        assert!(!w.due(2.4));
        std::fs::write(&path, b"third one").unwrap();
        assert!(w.due(3.0));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
