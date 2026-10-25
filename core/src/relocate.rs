//! Перенос папки данных в другое место (раздел 5.15 ТЗ). На том же диске —
//! переименование: мгновенно и без риска. На другой диск — копия с процентами,
//! отменой и сверкой; при любой ошибке копия удаляется, а старая папка
//! остаётся как была. Старую папку после копии удаляет только игрок.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;

use crate::datadir;
use crate::error::{Error, IoContext, Result};
use crate::paths::DataDir;

/// Запас свободного места сверх размера данных.
const RESERVE: u64 = 256 * 1024 * 1024;
/// Код Windows «нельзя переместить на другой диск».
const ERROR_NOT_SAME_DEVICE: i32 = 17;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    /// Считаем файлы.
    Scan,
    Copy,
    /// Сверяем копию с оригиналом.
    Verify,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Progress {
    pub stage: Stage,
    /// Байт или файлов (при сверке).
    pub done: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    /// Тот же диск: папку просто переименовали, старой больше нет.
    Renamed,
    /// Другой диск: данные скопированы, старая папка осталась.
    Copied,
}

/// Что окно показывает до переноса.
#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    /// Куда на самом деле: в непустую папку переносим во вложенную `MajorLauncherData`.
    pub to: String,
    /// Тот же диск — переименование без копирования.
    pub same_disk: bool,
    /// Свободно на новом диске.
    pub free: Option<u64>,
    /// Почему нельзя; `None` — можно.
    pub error: Option<String>,
}

/// Сколько занимает папка, байт.
pub fn size(dir: &Path) -> u64 {
    crate::builds::dir_size(dir)
}

/// Куда переносить, если игрок выбрал папку: пустую или новую — в неё саму,
/// непустую (например, корень диска) — во вложенную `MajorLauncherData`.
pub fn target(picked: &Path) -> PathBuf {
    // «E:/Games» и «E:\Games» — одно и то же; показываем привычные для Windows «\».
    #[cfg(windows)]
    let picked = &PathBuf::from(picked.to_string_lossy().replace('/', "\\"));
    let busy = std::fs::read_dir(picked).is_ok_and(|mut d| d.next().is_some());
    if busy && !datadir::has_data(picked) {
        picked.join(datadir::FOLDER)
    } else {
        picked.to_path_buf()
    }
}

/// Проверить перенос. `size` — сколько занимают данные ([`size`]): считать
/// заново на каждую букву пути долго. Ничего не меняет на диске.
pub fn plan(from: &Path, to: &Path, size: u64) -> Plan {
    let error = check(from, to).err().map(|e| e.to_string());
    let same_disk = same_volume(from, to);
    let free = datadir::free_space(to);
    let error = error.or_else(|| {
        (!same_disk && free.is_some_and(|f| f < size + RESERVE)).then(|| no_space(size, free))
    });
    Plan {
        to: to.display().to_string(),
        same_disk,
        free,
        error,
    }
}

fn no_space(size: u64, free: Option<u64>) -> String {
    format!(
        "на новом диске не хватает места: нужно {}, свободно {}",
        gb(size + RESERVE),
        free.map_or_else(|| "неизвестно".into(), gb)
    )
}

fn gb(bytes: u64) -> String {
    format!("{:.1} ГБ", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
}

/// Можно ли перенести папку `from` в `to`.
pub fn check(from: &Path, to: &Path) -> Result<()> {
    let fail = |text: &str| Err(Error::Other(text.into()));
    if !to.is_absolute() {
        return fail(r"укажите полный путь от буквы диска, например E:\MajorLauncherData");
    }
    if !from.is_dir() {
        return fail("текущей папки данных нет на диске");
    }
    if from.parent().is_none() {
        return fail("папка данных — корень диска: такую перенести нельзя");
    }
    let (a, b) = (norm(from), norm(to));
    if a == b {
        return fail("это та же папка");
    }
    if b.starts_with(&format!("{a}\\")) {
        return fail("новая папка не может быть внутри текущей");
    }
    if a.starts_with(&format!("{b}\\")) {
        return fail("текущая папка данных лежит внутри выбранной — выберите другую");
    }
    if to.exists() {
        if !to.is_dir() {
            return fail("по этому пути лежит файл, а не папка");
        }
        if std::fs::read_dir(to).at(to)?.next().is_some() {
            return fail("папка не пустая: выберите пустую или новую — лаунчер создаст её сам");
        }
    }
    if !datadir::can_write(to) {
        return fail("в эту папку нельзя записывать — выберите другую");
    }
    Ok(())
}

/// Перенести данные. `progress` вызывается часто — окно само прореживает.
pub fn relocate(
    from: &Path,
    to: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Method> {
    check(from, to)?;
    let existed = to.exists();
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).at(parent)?;
    }
    // Пустую папку убираем: переименовать можно только на свободное место.
    if existed {
        std::fs::remove_dir(to).at(to)?;
    }
    let method = match std::fs::rename(from, to) {
        Ok(()) => Method::Renamed,
        Err(e) if e.raw_os_error() == Some(ERROR_NOT_SAME_DEVICE) => {
            match copy_all(from, to, cancel, progress) {
                Ok(()) => Method::Copied,
                Err(e) => {
                    // Откат: только то, что создали мы, — папка была пустой.
                    let _ = std::fs::remove_dir_all(to);
                    restore_empty(to, existed);
                    return Err(e);
                }
            }
        }
        Err(e) => {
            restore_empty(to, existed);
            return Err(busy(e, from));
        }
    };
    rebase_cache(from, to);
    Ok(method)
}

fn restore_empty(to: &Path, existed: bool) {
    if existed {
        let _ = std::fs::create_dir(to);
    }
}

fn busy(e: std::io::Error, from: &Path) -> Error {
    match e.raw_os_error() {
        // Нет доступа или файл открыт: переименовать папку с открытыми файлами нельзя.
        Some(5 | 32) => Error::Other(
            "какой-то файл в папке данных занят другой программой (проводник, антивирус, игра) — закройте её и попробуйте снова".into(),
        ),
        _ => Error::Io {
            path: from.to_path_buf(),
            source: e,
        },
    }
}

/// Удалить старую папку после копирования — только по подтверждению игрока.
pub fn remove_old(old: &Path, current: &Path) -> Result<()> {
    let (a, b) = (norm(old), norm(current));
    if a == b || b.starts_with(&format!("{a}\\")) {
        return Err(Error::Other("это текущая папка данных".into()));
    }
    if !datadir::has_data(old) {
        return Err(Error::Other(format!(
            "{} не похожа на папку данных MajorLauncher — не удаляю",
            old.display()
        )));
    }
    std::fs::remove_dir_all(old).at(old)
}

struct Scan {
    dirs: Vec<PathBuf>,
    files: Vec<(PathBuf, u64)>,
    total: u64,
}

/// Все папки и файлы с размерами, пути — от корня.
fn scan(root: &Path) -> Result<Scan> {
    let mut out = Scan {
        dirs: Vec::new(),
        files: Vec::new(),
        total: 0,
    };
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let dir = root.join(&rel);
        for entry in std::fs::read_dir(&dir).at(&dir)? {
            let entry = entry.at(&dir)?;
            let path = rel.join(entry.file_name());
            let kind = entry.file_type().at(&entry.path())?;
            if kind.is_symlink() {
                return Err(Error::Other(format!(
                    "в папке есть ссылка на другое место ({}) — такую папку лаунчер перенести не может",
                    root.join(&path).display()
                )));
            }
            if kind.is_dir() {
                out.dirs.push(path.clone());
                stack.push(path);
            } else {
                let size = entry.metadata().at(&entry.path())?.len();
                out.total += size;
                out.files.push((path, size));
            }
        }
    }
    Ok(out)
}

fn copy_all(
    from: &Path,
    to: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<()> {
    progress(Progress {
        stage: Stage::Scan,
        done: 0,
        total: 0,
    });
    let scan = scan(from)?;
    let free = datadir::free_space(to);
    if free.is_some_and(|f| f < scan.total + RESERVE) {
        return Err(Error::Other(no_space(scan.total, free)));
    }
    std::fs::create_dir_all(to).at(to)?;
    for dir in &scan.dirs {
        let d = to.join(dir);
        std::fs::create_dir_all(&d).at(&d)?;
    }

    let total = scan.total;
    let mut done = 0;
    let mut buf = vec![0u8; 1024 * 1024];
    // Жёсткие ссылки (паки библиотеки в сборках) остаются ссылками, а не копиями.
    let mut linked: HashMap<(u32, u64), PathBuf> = HashMap::new();
    for (rel, size) in &scan.files {
        let src = from.join(rel);
        let dst = to.join(rel);
        let input = std::fs::File::open(&src).at(&src)?;
        if let Some((id, links)) = identity(&input)
            && links > 1
        {
            if let Some(first) = linked.get(&id)
                && std::fs::hard_link(first, &dst).is_ok()
            {
                done += size;
                progress(Progress {
                    stage: Stage::Copy,
                    done,
                    total,
                });
                continue;
            }
            linked.entry(id).or_insert_with(|| dst.clone());
        }
        copy_file(input, &src, &dst, &mut buf, cancel, &mut |n| {
            done += n;
            progress(Progress {
                stage: Stage::Copy,
                done,
                total,
            });
        })?;
    }

    // Сверка: каждый файл на месте и того же размера.
    let count = scan.files.len() as u64;
    for (i, (rel, size)) in scan.files.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(cancelled());
        }
        let dst = to.join(rel);
        let got = std::fs::metadata(&dst).at(&dst)?.len();
        if got != *size {
            return Err(Error::Other(format!(
                "копия {} не совпала с оригиналом ({got} байт вместо {size}) — перенос отменён, старая папка не тронута",
                dst.display()
            )));
        }
        if i % 256 == 0 {
            progress(Progress {
                stage: Stage::Verify,
                done: i as u64,
                total: count,
            });
        }
    }
    Ok(())
}

fn cancelled() -> Error {
    Error::Other("перенос отменён — данные остались в старой папке".into())
}

/// Копия файла кусками (ради процентов и отмены) с датой изменения оригинала:
/// по размеру и дате лаунчер узнаёт уже проверенные файлы.
fn copy_file(
    mut input: std::fs::File,
    src: &Path,
    dst: &Path,
    buf: &mut [u8],
    cancel: &AtomicBool,
    on_bytes: &mut dyn FnMut(u64),
) -> Result<()> {
    let meta = input.metadata().at(src)?;
    let mut out = std::fs::File::create(dst).at(dst)?;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(cancelled());
        }
        let n = input.read(buf).at(src)?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).at(dst)?;
        on_bytes(n as u64);
    }
    if let Ok(modified) = meta.modified() {
        out.set_modified(modified).at(dst)?;
    }
    drop(out);
    if meta.permissions().readonly() {
        std::fs::set_permissions(dst, meta.permissions()).at(dst)?;
    }
    Ok(())
}

/// Кэш проверенных файлов хранит полные пути — переводим их на новое место,
/// иначе первый запуск каждой версии пересчитывал бы хеши заново.
pub fn rebase_cache(from: &Path, to: &Path) {
    let old = DataDir::new(from);
    let new = DataDir::new(to);
    let _ = crate::verify::VerifyCache::rebase(&new.verified(), old.root(), new.root());
}

/// Путь для сравнения: полный, без `\\?\`, в нижнем регистре (Windows не
/// различает регистр). Несуществующий хвост пути берётся как есть.
fn norm(path: &Path) -> String {
    let mut existing = path;
    let mut rest = Vec::new();
    while !existing.exists() {
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name);
                existing = parent;
            }
            _ => break,
        }
    }
    let mut full = std::fs::canonicalize(existing).unwrap_or_else(|_| existing.to_path_buf());
    for name in rest.iter().rev() {
        full.push(name);
    }
    let text = full.display().to_string().replace('/', "\\");
    let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
    text.trim_end_matches('\\').to_lowercase()
}

fn same_volume(a: &Path, b: &Path) -> bool {
    match (imp::volume(a), imp::volume(b)) {
        (Some(x), Some(y)) => x.eq_ignore_ascii_case(&y),
        _ => false,
    }
}

/// Сколько имён (жёстких ссылок) у файла; `None` — не удалось узнать.
pub(crate) fn links(path: &Path) -> Option<u32> {
    identity(&std::fs::File::open(path).ok()?).map(|(_, n)| n)
}

/// Один и тот же файл на диске (у жёстких ссылок он общий) и сколько у него имён.
#[cfg(windows)]
fn identity(file: &std::fs::File) -> Option<((u32, u64), u32)> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };
    // SAFETY: структура из чисел, нули допустимы.
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: дескриптор открытого файла действителен, пока жив `file`.
    let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) };
    (ok != 0).then(|| {
        let index = (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow);
        ((info.dwVolumeSerialNumber, index), info.nNumberOfLinks)
    })
}

#[cfg(not(windows))]
fn identity(_: &std::fs::File) -> Option<((u32, u64), u32)> {
    None
}

#[cfg(windows)]
mod imp {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows_sys::Win32::Storage::FileSystem::GetVolumePathNameW;

    /// Корень тома, на котором лежит путь (или будет лежать — по ближайшей
    /// существующей папке).
    pub fn volume(path: &Path) -> Option<String> {
        let existing = path.ancestors().find(|p| p.exists())?;
        let wide: Vec<u16> = existing
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut out = [0u16; 512];
        // SAFETY: строка заканчивается нулём, длина буфера передана верно.
        let ok = unsafe { GetVolumePathNameW(wide.as_ptr(), out.as_mut_ptr(), out.len() as u32) };
        if ok == 0 {
            return None;
        }
        let len = out.iter().position(|&c| c == 0).unwrap_or(out.len());
        Some(String::from_utf16_lossy(&out[..len]))
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn volume(_: &std::path::Path) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(root: &Path) {
        std::fs::create_dir_all(root.join("instances/a/saves/w")).unwrap();
        std::fs::create_dir_all(root.join("library/resourcepacks")).unwrap();
        std::fs::create_dir_all(root.join("instances/a/resourcepacks")).unwrap();
        std::fs::write(root.join("accounts.json"), b"{}").unwrap();
        std::fs::write(
            root.join("instances/a/saves/w/level.dat"),
            vec![7u8; 3 * 1024 * 1024 + 5],
        )
        .unwrap();
        std::fs::write(root.join("library/resourcepacks/p.zip"), b"pack").unwrap();
        std::fs::hard_link(
            root.join("library/resourcepacks/p.zip"),
            root.join("instances/a/resourcepacks/p.zip"),
        )
        .unwrap();
    }

    #[test]
    fn checks_targets() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("data");
        data(&from);
        assert!(check(&from, &from).is_err(), "та же папка");
        assert!(check(&from, &from.join("inner")).is_err(), "внутри текущей");
        assert!(check(&from, Path::new("relative")).is_err());
        let busy = dir.path().join("busy");
        std::fs::create_dir_all(&busy).unwrap();
        std::fs::write(busy.join("x"), b"x").unwrap();
        assert!(check(&from, &busy).is_err(), "непустая папка");
        assert_eq!(target(&busy), busy.join(datadir::FOLDER));
        assert!(check(&from, &dir.path().join("new")).is_ok());
        let empty = dir.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        assert_eq!(target(&empty), empty);
        assert!(check(&from, &empty).is_ok());

        assert!(size(&from) >= 3 * 1024 * 1024);
        let plan = plan(&from, &dir.path().join("new"), size(&from));
        assert!(plan.error.is_none(), "{:?}", plan.error);
        assert!(plan.same_disk);
    }

    #[test]
    fn rename_on_same_disk() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("data");
        data(&from);
        let to = dir.path().join("moved");
        std::fs::create_dir_all(&to).unwrap();
        let method = relocate(&from, &to, &AtomicBool::new(false), &mut |_| {}).unwrap();
        assert_eq!(method, Method::Renamed);
        assert!(!from.exists());
        assert!(to.join("instances/a/saves/w/level.dat").exists());
    }

    #[test]
    fn copy_keeps_links_dates_and_cache() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("data");
        data(&from);
        let level = from.join("instances/a/saves/w/level.dat");
        let cache = DataDir::new(&from).verified();
        std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
        let mut entries = serde_json::Map::new();
        entries.insert(
            level.display().to_string(),
            serde_json::json!({"stamp": {"size": 1, "mtime_ns": 2}, "sha1": "x"}),
        );
        std::fs::write(&cache, serde_json::to_vec(&entries).unwrap()).unwrap();

        let to = dir.path().join("copy");
        let mut last = None;
        copy_all(&from, &to, &AtomicBool::new(false), &mut |p| last = Some(p)).unwrap();
        rebase_cache(&from, &to);
        assert_eq!(last.unwrap().stage, Stage::Verify);

        let copied = to.join("instances/a/saves/w/level.dat");
        assert_eq!(
            std::fs::read(&copied).unwrap(),
            std::fs::read(&level).unwrap()
        );
        assert_eq!(
            std::fs::metadata(&copied).unwrap().modified().unwrap(),
            std::fs::metadata(&level).unwrap().modified().unwrap(),
            "дата изменения сохранилась"
        );
        let pack = std::fs::File::open(to.join("library/resourcepacks/p.zip")).unwrap();
        #[cfg(windows)]
        assert_eq!(identity(&pack).unwrap().1, 2, "пак остался жёсткой ссылкой");
        drop(pack);

        let rebased: HashMap<PathBuf, serde_json::Value> =
            serde_json::from_slice(&std::fs::read(DataDir::new(&to).verified()).unwrap()).unwrap();
        assert_eq!(
            rebased.keys().collect::<Vec<_>>(),
            [&DataDir::new(&to)
                .root()
                .join("instances/a/saves/w/level.dat")]
        );

        // Старую удаляем только как папку данных и не текущую.
        assert!(remove_old(&to, &to).is_err());
        assert!(remove_old(dir.path(), &to).is_err(), "не похожа на данные");
        remove_old(&from, &to).unwrap();
        assert!(!from.exists());
    }

    #[test]
    fn cancel_rolls_back() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("data");
        data(&from);
        let to = dir.path().join("empty");
        std::fs::create_dir_all(&to).unwrap();
        let cancel = AtomicBool::new(false);
        let err = copy_all(&from, &to, &cancel, &mut |p| {
            if p.stage == Stage::Copy && p.done > 1024 * 1024 {
                cancel.store(true, Ordering::Relaxed);
            }
        });
        assert!(err.is_err());
        // Откат делает relocate; проверим его сами на копии.
        let _ = std::fs::remove_dir_all(&to);
        restore_empty(&to, true);
        assert!(to.is_dir() && std::fs::read_dir(&to).unwrap().next().is_none());
        assert!(from.join("instances/a/saves/w/level.dat").exists());
    }
}
