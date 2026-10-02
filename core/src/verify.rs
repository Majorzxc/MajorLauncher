//! Проверка файлов по хешу и кэш уже проверенных файлов.
//!
//! Пересчитывать SHA-1 тысяч файлов при каждом запуске долго, поэтому после
//! успешной проверки запоминаем размер и время изменения файла. Пока они
//! не поменялись, файл считается целым (раздел 7 ТЗ). Полная проверка — по
//! запросу (`mlcli verify`) или когда размер или дата изменились.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::error::{IoContext, Result};

/// SHA-1 файла в нижнем регистре.
pub fn sha1_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).at(path)?;
    let mut hasher = Sha1::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).at(path)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

pub fn sha1_bytes(bytes: &[u8]) -> String {
    hex(&Sha1::digest(bytes))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// Без #[serde(flatten)] и u128: serde_json умеет такое записать, но не прочитать,
// и кэш молча загружался пустым.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct Stamp {
    size: u64,
    /// Наносекунды с 1970 года: в u64 хватит до 2554-го.
    mtime_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    stamp: Stamp,
    sha1: String,
}

fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime_ns = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos()
        .try_into()
        .ok()?;
    Some(Stamp {
        size: meta.len(),
        mtime_ns,
    })
}

/// Кэш проверенных файлов. Потокобезопасен: проверки идут параллельно.
pub struct VerifyCache {
    file: PathBuf,
    entries: Mutex<HashMap<PathBuf, Entry>>,
    dirty: Mutex<bool>,
}

impl VerifyCache {
    /// Загружает кэш; битый или отсутствующий файл — просто пустой кэш.
    pub fn load(file: PathBuf) -> Self {
        let entries = std::fs::read(&file)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            file,
            entries: Mutex::new(entries),
            dirty: Mutex::new(false),
        }
    }

    pub fn save(&self) -> Result<()> {
        if !*self.dirty.lock().unwrap() {
            return Ok(());
        }
        let bytes = serde_json::to_vec(&*self.entries.lock().unwrap()).expect("кэш сериализуется");
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir).at(dir)?;
        }
        // Через временный файл: оборванная запись не портит кэш.
        let tmp = self.file.with_extension("tmp");
        std::fs::write(&tmp, bytes).at(&tmp)?;
        std::fs::rename(&tmp, &self.file).at(&self.file)?;
        *self.dirty.lock().unwrap() = false;
        Ok(())
    }

    /// Запомнить, что файл только что проверен и совпал с `sha1`.
    pub fn record(&self, path: &Path, sha1: &str) {
        if let Some(stamp) = stamp(path) {
            self.entries.lock().unwrap().insert(
                path.to_path_buf(),
                Entry {
                    stamp,
                    sha1: sha1.to_string(),
                },
            );
            *self.dirty.lock().unwrap() = true;
        }
    }

    /// Файл на месте и цел? Сначала быстрая проверка по размеру и дате,
    /// при расхождении — полный пересчёт хеша. `sha1 = None` — хеш неизвестен,
    /// проверяем только наличие (и размер, если он задан).
    pub fn is_valid(&self, path: &Path, sha1: Option<&str>, size: Option<u64>) -> bool {
        let Some(current) = stamp(path) else {
            return false;
        };
        if size.is_some_and(|s| s != current.size) {
            return false;
        }
        let Some(expected) = sha1 else {
            return true;
        };
        if let Some(entry) = self.entries.lock().unwrap().get(path)
            && entry.stamp == current
            && entry.sha1.eq_ignore_ascii_case(expected)
        {
            return true;
        }
        self.is_valid_full(path, expected)
    }

    /// Полная проверка с пересчётом хеша, мимо кэша.
    pub fn is_valid_full(&self, path: &Path, expected: &str) -> bool {
        match sha1_file(path) {
            Ok(actual) if actual.eq_ignore_ascii_case(expected) => {
                self.record(path, &actual);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_of_known_string() {
        assert_eq!(
            sha1_bytes(b"abc"),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }

    #[test]
    fn cache_detects_changed_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"abc").unwrap();
        let hash = sha1_bytes(b"abc");

        let cache = VerifyCache::load(dir.path().join("cache.json"));
        assert!(cache.is_valid(&file, Some(&hash), Some(3)));
        assert!(
            !cache.is_valid(&file, Some(&hash), Some(4)),
            "другой размер"
        );

        // Тот же размер, но другое содержимое и время изменения.
        std::fs::write(&file, b"abd").unwrap();
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert!(
            !cache.is_valid(&file, Some(&hash), None),
            "содержимое изменилось"
        );

        cache.save().unwrap();
        assert!(dir.path().join("cache.json").exists());
    }

    #[test]
    fn cache_survives_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"abc").unwrap();
        let hash = sha1_bytes(b"abc");
        let cache_file = dir.path().join("cache.json");

        let cache = VerifyCache::load(cache_file.clone());
        cache.record(&file, &hash);
        cache.save().unwrap();

        let loaded = VerifyCache::load(cache_file);
        assert_eq!(
            loaded.entries.lock().unwrap().len(),
            1,
            "запись пережила перезагрузку"
        );
    }
}
