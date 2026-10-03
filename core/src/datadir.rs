//! Выбор папки данных при первом запуске (раздел 5.15 ТЗ): подсказать, где её
//! создать, и проверить выбранную — можно ли писать, хватит ли места, нет ли
//! известных ловушек (OneDrive, кириллица в пути).

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Меньше этого свободного места — предупреждаем: одна сборка с модами
/// и её версия игры легко занимают несколько гигабайт.
const LOW_SPACE: u64 = 5 * 1024 * 1024 * 1024;
const FOLDER: &str = "MajorLauncherData";

#[derive(Debug, Clone, Serialize)]
pub struct DirCheck {
    pub path: String,
    /// Свободно на диске, байт; `None` — не удалось узнать.
    pub free_bytes: Option<u64>,
    /// В папке уже лежат данные MajorLauncher — лаунчер продолжит с ними.
    pub has_data: bool,
    /// Можно ли создать папку и писать в неё.
    pub writable: bool,
    pub warnings: Vec<String>,
}

/// Проверить папку. Ничего не создаёт на диске.
pub fn inspect(path: &Path) -> DirCheck {
    let text = path.display().to_string();
    // «D:Games» в Windows — путь относительно текущей папки диска, а не корня:
    // данные уехали бы непонятно куда. Принимаем только полный путь.
    if !path.is_absolute() {
        return DirCheck {
            path: text,
            free_bytes: None,
            has_data: false,
            writable: false,
            warnings: vec![
                r"Укажите полный путь от буквы диска, например D:\MajorLauncherData".into(),
            ],
        };
    }
    let mut warnings = Vec::new();
    if !text.is_ascii() {
        warnings.push(
            "В пути есть русские буквы или другие не латинские символы: старые версии Forge с такой папкой не запускаются. Лучше выбрать путь латиницей.".into(),
        );
    }
    if text.to_lowercase().contains("onedrive") {
        warnings.push(
            "Папка внутри OneDrive: синхронизация портит файлы игры во время записи. Лучше выбрать папку вне OneDrive.".into(),
        );
    }
    let free_bytes = free_space(path);
    if free_bytes.is_some_and(|f| f < LOW_SPACE) {
        warnings.push("На диске меньше 5 ГБ свободного места: одна сборка с модами может занимать несколько гигабайт.".into());
    }
    let has_data = path.join("accounts.json").exists() || path.join("versions").is_dir();
    DirCheck {
        path: text,
        free_bytes,
        has_data,
        writable: can_write(path),
        warnings,
    }
}

/// Где предложить папку данных: на несистемном несъёмном диске с наибольшим
/// свободным местом, иначе — в профиле пользователя (не в корне диска C).
pub fn suggest() -> PathBuf {
    let system = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let best = ('D'..='Z')
        .map(|letter| format!("{letter}:\\"))
        .filter(|root| !root.starts_with(&system))
        .filter(|root| is_fixed_drive(root))
        .filter_map(|root| free_space(Path::new(&root)).map(|free| (root, free)))
        .max_by_key(|(_, free)| *free);
    match best {
        Some((root, _)) => PathBuf::from(root).join(FOLDER),
        None => std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(format!("{system}\\")))
            .join(FOLDER),
    }
}

/// Можно ли писать в папку. Ничего не создаёт: пробный файл кладётся в саму
/// папку или, если её ещё нет, в ближайшую существующую — и сразу удаляется.
/// (Проверка идёт, пока игрок печатает путь, — иначе на диске оставались бы
/// папки от каждой набранной буквы.)
fn can_write(path: &Path) -> bool {
    let Some(existing) = path.ancestors().find(|p| p.is_dir()) else {
        return false;
    };
    let probe = existing.join(format!(".majorlauncher-write-test-{}", std::process::id()));
    let ok = std::fs::write(&probe, b"ok").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// Свободное место на диске, где лежит (или будет лежать) папка.
pub fn free_space(path: &Path) -> Option<u64> {
    // Папки может ещё не быть — спрашиваем про ближайшую существующую.
    let existing = path.ancestors().find(|p| p.exists())?;
    imp::free_space(existing)
}

#[cfg(windows)]
fn is_fixed_drive(root: &str) -> bool {
    imp::is_fixed_drive(root)
}

#[cfg(not(windows))]
fn is_fixed_drive(_: &str) -> bool {
    false
}

#[cfg(windows)]
mod imp {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows_sys::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW};

    /// Несъёмный диск (константа из WindowsProgramming — не тянем ради неё весь модуль).
    const DRIVE_FIXED: u32 = 3;

    fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    }

    pub fn free_space(path: &Path) -> Option<u64> {
        let p = wide(path.as_os_str());
        let mut free = 0u64;
        // SAFETY: строка заканчивается нулём, указатель на u64 действителен.
        let ok = unsafe {
            GetDiskFreeSpaceExW(
                p.as_ptr(),
                &mut free,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        (ok != 0).then_some(free)
    }

    pub fn is_fixed_drive(root: &str) -> bool {
        let p = wide(std::ffi::OsStr::new(root));
        // SAFETY: строка заканчивается нулём.
        unsafe { GetDriveTypeW(p.as_ptr()) == DRIVE_FIXED }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn free_space(_: &std::path::Path) -> Option<u64> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warns_about_known_traps() {
        let dir = tempfile::tempdir().unwrap();
        let ok = inspect(dir.path());
        assert!(ok.writable);
        assert!(!ok.has_data);

        assert!(
            !inspect(Path::new("D:relative")).writable,
            "путь от текущей папки диска не принимается"
        );

        let missing = dir.path().join("not").join("yet");
        assert!(inspect(&missing).writable);
        assert!(!missing.exists(), "проверка не создаёт папок");

        let cyr = inspect(&dir.path().join("Игры"));
        assert!(cyr.warnings.iter().any(|w| w.contains("русские")));
        let od = inspect(&dir.path().join("OneDrive").join("mc"));
        assert!(od.warnings.iter().any(|w| w.contains("OneDrive")));

        std::fs::write(dir.path().join("accounts.json"), b"{}").unwrap();
        assert!(inspect(dir.path()).has_data);
    }

    #[cfg(windows)]
    #[test]
    fn free_space_and_suggestion() {
        assert!(free_space(Path::new("C:\\")).is_some_and(|f| f > 0));
        let s = suggest();
        assert!(s.ends_with(FOLDER), "{s:?}");
    }
}
