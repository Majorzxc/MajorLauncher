//! Выбор папки данных при первом запуске (раздел 5.15 ТЗ): подсказать, где её
//! создать, и проверить выбранную — можно ли писать, хватит ли места, нет ли
//! известных ловушек (OneDrive, кириллица в пути).

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Меньше этого свободного места — предупреждаем: одна сборка с модами
/// и её версия игры легко занимают несколько гигабайт.
const LOW_SPACE: u64 = 5 * 1024 * 1024 * 1024;
pub(crate) const FOLDER: &str = "MajorLauncherData";

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
    let has_data = has_data(path);
    DirCheck {
        path: text,
        free_bytes,
        has_data,
        writable: can_write(path),
        warnings,
    }
}

/// В папке уже лежат данные MajorLauncher.
pub(crate) fn has_data(path: &Path) -> bool {
    path.join("accounts.json").exists()
        || path.join("versions").is_dir()
        || path.join("instances").is_dir()
}

/// Что с папкой данных при запуске лаунчера.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Located {
    /// Папка на месте (или ещё не выбрана).
    Here,
    /// Диск сменил букву: тот же путь нашёлся на другой.
    Moved(PathBuf),
    /// Папки нигде нет: диск отключён или папку переместили вручную.
    Missing,
}

/// Найти папку данных, если по старому пути её нет: Windows могла дать
/// диску другую букву (подключили в другом порядке, сменили в «Управлении
/// дисками»). Ищем тот же путь на других буквах. `serial` — серийный номер
/// тома, запомненный при выборе папки: по нему узнаём «свой» диск, даже если
/// на старой букве теперь другой диск со своей папкой MajorLauncher. Без
/// номера (настройки до этой проверки) подходит только единственная находка.
pub fn locate(saved: &Path, serial: Option<u32>) -> Located {
    let here = has_data(saved);
    let ours = serial.is_none_or(|s| volume_serial(saved).is_none_or(|v| v == s));
    if here && ours {
        return Located::Here;
    }
    let found = split_drive(saved).and_then(|(letter, rest)| {
        let candidates = imp::drives()
            .into_iter()
            .filter(|&d| d != letter)
            .map(|d| PathBuf::from(format!("{d}:{rest}")))
            .filter(|p| has_data(p))
            .collect();
        pick(candidates, serial, volume_serial)
    });
    match found {
        Some(path) => Located::Moved(path),
        None if here => Located::Here,
        None => Located::Missing,
    }
}

/// Какую из найденных папок взять: на томе с тем же номером, а без номера —
/// только если нашлась ровно одна (иначе не угадать, какая наша).
fn pick(
    candidates: Vec<PathBuf>,
    serial: Option<u32>,
    serial_of: impl Fn(&Path) -> Option<u32>,
) -> Option<PathBuf> {
    match serial {
        Some(s) => candidates.into_iter().find(|p| serial_of(p) == Some(s)),
        None if candidates.len() == 1 => candidates.into_iter().next(),
        None => None,
    }
}

/// «G:\MajorLauncherData» → ('G', "\\MajorLauncherData").
fn split_drive(path: &Path) -> Option<(char, String)> {
    let text = path.to_str()?;
    let mut chars = text.chars();
    let letter = chars.next().filter(char::is_ascii_alphabetic)?;
    (chars.next() == Some(':')).then(|| (letter.to_ascii_uppercase(), text[2..].to_string()))
}

/// Серийный номер тома, на котором лежит папка (или ближайшая существующая
/// папка над ней). Не меняется при смене буквы диска.
pub fn volume_serial(path: &Path) -> Option<u32> {
    let existing = path.ancestors().find(|p| p.exists())?;
    imp::volume_serial(existing)
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
pub(crate) fn can_write(path: &Path) -> bool {
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

    use windows_sys::Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
        GetVolumePathNameW,
    };
    use windows_sys::Win32::System::Diagnostics::Debug::{
        SEM_FAILCRITICALERRORS, SetThreadErrorMode,
    };

    /// Типы дисков (константы из WindowsProgramming — не тянем ради них весь модуль).
    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;

    /// Буквы локальных дисков — несъёмных и съёмных (флешки, внешние диски).
    /// Сетевые и CD пропускаем: опрос первых может надолго зависнуть.
    pub fn drives() -> Vec<char> {
        // SAFETY: функция без аргументов.
        let mask = unsafe { GetLogicalDrives() };
        ('A'..='Z')
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, letter)| letter)
            .filter(|letter| {
                let p = wide(std::ffi::OsStr::new(&format!("{letter}:\\")));
                // SAFETY: строка заканчивается нулём.
                let kind = unsafe { GetDriveTypeW(p.as_ptr()) };
                kind == DRIVE_FIXED || kind == DRIVE_REMOVABLE
            })
            .collect()
    }

    pub fn volume_serial(path: &Path) -> Option<u32> {
        let p = wide(path.as_os_str());
        let mut root = [0u16; 512];
        // SAFETY: строка заканчивается нулём, длина буфера передана верно.
        let found =
            unsafe { GetVolumePathNameW(p.as_ptr(), root.as_mut_ptr(), root.len() as u32) != 0 };
        // У дисков из `subst` «корень тома» — обычная папка, номер у неё не
        // спросить; тогда спрашиваем корень буквы диска.
        let by_letter = super::split_drive(path)
            .map(|(letter, _)| wide(std::ffi::OsStr::new(&format!("{letter}:\\"))));
        found
            .then(|| serial_of(&root))
            .flatten()
            .or_else(|| serial_of(by_letter.as_deref()?))
    }

    /// Номер тома по его корню (строка с нулём на конце).
    fn serial_of(root: &[u16]) -> Option<u32> {
        let mut serial = 0u32;
        // SAFETY: строка заканчивается нулём, ненужные выходные параметры —
        // нулевые указатели, это разрешено. Ошибка «нет диска в устройстве» —
        // без системного окна.
        unsafe {
            let mut old = 0;
            SetThreadErrorMode(SEM_FAILCRITICALERRORS, &mut old);
            let ok = GetVolumeInformationW(
                root.as_ptr(),
                std::ptr::null_mut(),
                0,
                &mut serial,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
            ) != 0;
            SetThreadErrorMode(old, std::ptr::null_mut());
            ok.then_some(serial)
        }
    }

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

    pub fn drives() -> Vec<char> {
        Vec::new()
    }

    pub fn volume_serial(_: &std::path::Path) -> Option<u32> {
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

    #[test]
    fn finds_data_after_drive_letter_change() {
        assert_eq!(
            split_drive(Path::new(r"g:\MajorLauncherData")),
            Some(('G', r"\MajorLauncherData".into()))
        );
        assert_eq!(
            split_drive(Path::new("D:/Games/ML")),
            Some(('D', "/Games/ML".into()))
        );
        assert_eq!(split_drive(Path::new(r"\\server\share")), None);

        let (a, b) = (PathBuf::from(r"E:\ML"), PathBuf::from(r"H:\ML"));
        let serial_of = |p: &Path| Some(if p.starts_with("E:") { 1 } else { 2 });
        // Номер тома известен — только «свой» диск.
        assert_eq!(
            pick(vec![a.clone(), b.clone()], Some(2), serial_of),
            Some(b.clone())
        );
        assert_eq!(pick(vec![a.clone()], Some(2), serial_of), None);
        // Номера нет — только единственная находка.
        assert_eq!(pick(vec![a.clone()], None, serial_of), Some(a.clone()));
        assert_eq!(pick(vec![a, b], None, serial_of), None);

        // Папка на месте — ничего не ищем.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("accounts.json"), b"{}").unwrap();
        assert_eq!(locate(dir.path(), None), Located::Here);
        let serial = volume_serial(dir.path());
        #[cfg(windows)]
        assert!(serial.is_some(), "номер тома известен");
        assert_eq!(locate(dir.path(), serial), Located::Here);
        // Пропала, и на других дисках такой нет.
        let gone = dir.path().join("no-such-launcher-data-anywhere");
        assert_eq!(locate(&gone, serial), Located::Missing);
    }

    #[cfg(windows)]
    #[test]
    fn free_space_and_suggestion() {
        assert!(free_space(Path::new("C:\\")).is_some_and(|f| f > 0));
        let s = suggest();
        assert!(s.ends_with(FOLDER), "{s:?}");
    }
}
