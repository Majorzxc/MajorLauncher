//! Содержимое папки игры сборки для вкладок страницы сборки (раздел 6.3 ТЗ):
//! миры, моды, ресурспаки, шейдеры, скриншоты, логи.
//!
//! Окно передаёт только имя файла внутри известной папки — путь строит ядро
//! и не выпускает его за пределы папки сборки.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::error::{Error, IoContext, Result};
use crate::paths::DataDir;

/// Папки с файлами, которые игрок кладёт в сборку.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Folder {
    Mods,
    ResourcePacks,
    ShaderPacks,
    Screenshots,
}

impl Folder {
    fn dir(self) -> &'static str {
        match self {
            Folder::Mods => "mods",
            Folder::ResourcePacks => "resourcepacks",
            Folder::ShaderPacks => "shaderpacks",
            Folder::Screenshots => "screenshots",
        }
    }
}

/// Мод отключается приставкой `.disabled` — так делают все лаунчеры.
const DISABLED: &str = ".disabled";

#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    /// Имя файла или папки — по нему окно просит действия.
    pub file: String,
    /// Имя для показа: без `.disabled`.
    pub name: String,
    pub size: u64,
    pub modified: u64,
    pub enabled: bool,
    pub is_dir: bool,
}

/// Файлы в папке сборки, новые сверху.
pub fn files(game_dir: &Path, folder: Folder) -> Result<Vec<FileEntry>> {
    let dir = game_dir.join(folder.dir());
    let mut out = Vec::new();
    let read = match std::fs::read_dir(&dir) {
        Ok(read) => read,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e).at(&dir),
    };
    for entry in read.flatten() {
        let Ok(file) = entry.file_name().into_string() else {
            continue;
        };
        let Ok(meta) = entry.metadata() else { continue };
        let lower = file.to_lowercase();
        let wanted = match folder {
            Folder::Mods => lower.ends_with(".jar") || lower.ends_with(".jar.disabled"),
            Folder::ResourcePacks | Folder::ShaderPacks => {
                meta.is_dir() || lower.ends_with(".zip") || lower.ends_with(".zip.disabled")
            }
            Folder::Screenshots => lower.ends_with(".png") || lower.ends_with(".jpg"),
        };
        if !wanted {
            continue;
        }
        let enabled = !lower.ends_with(DISABLED);
        out.push(FileEntry {
            name: file.strip_suffix(DISABLED).unwrap_or(&file).to_string(),
            size: if meta.is_dir() { 0 } else { meta.len() },
            modified: mtime(&meta),
            enabled,
            is_dir: meta.is_dir(),
            file,
        });
    }
    out.sort_by_key(|f| std::cmp::Reverse(f.modified));
    Ok(out)
}

/// Включить или выключить мод. Возвращает новое имя файла.
pub fn set_enabled(game_dir: &Path, folder: Folder, file: &str, enabled: bool) -> Result<String> {
    let from = inside(game_dir, folder, file)?;
    let base = file.strip_suffix(DISABLED).unwrap_or(file);
    let new = if enabled {
        base.to_string()
    } else {
        format!("{base}{DISABLED}")
    };
    if new != file {
        let to = from.with_file_name(&new);
        if to.exists() {
            return Err(Error::Other(format!("файл «{new}» уже есть в папке")));
        }
        std::fs::rename(&from, &to).at(&from)?;
    }
    Ok(new)
}

/// Путь к файлу внутри папки сборки. Имена с `/`, `\` и `..` отклоняются.
pub fn inside(game_dir: &Path, folder: Folder, file: &str) -> Result<PathBuf> {
    check_name(file)?;
    Ok(game_dir.join(folder.dir()).join(file))
}

fn check_name(file: &str) -> Result<()> {
    if file.is_empty() || file.contains(['/', '\\', ':']) || file == "." || file == ".." {
        return Err(Error::Other(format!("недопустимое имя файла «{file}»")));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct World {
    /// Имя папки мира.
    pub folder: String,
    /// Название мира из level.dat (может отличаться от папки).
    pub name: String,
    pub size: u64,
    /// Когда в мире играли последний раз (время изменения level.dat).
    pub last_played: u64,
    /// Значок мира `icon.png` как data URL — он маленький, 64×64.
    pub icon: Option<String>,
}

pub fn worlds(game_dir: &Path) -> Result<Vec<World>> {
    let saves = game_dir.join("saves");
    let mut out = Vec::new();
    let read = match std::fs::read_dir(&saves) {
        Ok(read) => read,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e).at(&saves),
    };
    for entry in read.flatten() {
        let dir = entry.path();
        let level = dir.join("level.dat");
        let Ok(meta) = std::fs::metadata(&level) else {
            continue;
        };
        let Ok(folder) = entry.file_name().into_string() else {
            continue;
        };
        let icon = std::fs::read(dir.join("icon.png")).ok().map(|png| {
            use base64::Engine;
            format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(png)
            )
        });
        out.push(World {
            name: level_name(&level).unwrap_or_else(|| folder.clone()),
            folder,
            size: crate::builds::dir_size(&dir),
            last_played: mtime(&meta),
            icon,
        });
    }
    out.sort_by_key(|w| std::cmp::Reverse(w.last_played));
    Ok(out)
}

/// Название мира, в котором игрок сейчас: тот, чей `session.lock` обновлён
/// последним (игра пишет его при открытии мира).
pub fn current_world(game_dir: &Path) -> Option<String> {
    let saves = game_dir.join("saves");
    let newest = std::fs::read_dir(&saves)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let lock = std::fs::metadata(e.path().join("session.lock")).ok()?;
            Some((lock.modified().ok()?, e.path()))
        })
        .max_by_key(|(time, _)| *time)?
        .1;
    level_name(&newest.join("level.dat"))
        .or_else(|| newest.file_name()?.to_str().map(str::to_string))
}

/// `LevelName` из level.dat (NBT в gzip). Разбирать весь NBT не нужно:
/// достаточно найти строковый тег с этим именем.
pub fn level_name(level_dat: &Path) -> Option<String> {
    let file = std::fs::File::open(level_dat).ok()?;
    let mut bytes = Vec::new();
    // level.dat небольшой, но ограничиваем на случай чужого мусора в папке.
    flate2::read::GzDecoder::new(file)
        .take(16 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .ok()?;
    const TAG: &[u8] = b"\x08\x00\x09LevelName";
    let at = bytes.windows(TAG.len()).position(|w| w == TAG)? + TAG.len();
    let len = u16::from_be_bytes([*bytes.get(at)?, *bytes.get(at + 1)?]) as usize;
    let raw = bytes.get(at + 2..at + 2 + len)?;
    let name = String::from_utf8_lossy(raw).trim().to_string();
    (!name.is_empty()).then_some(name)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogKind {
    Log,
    Crash,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogFile {
    /// Путь внутри папки сборки: `logs/latest.log`, `crash-reports/…txt`.
    pub file: String,
    pub kind: LogKind,
    pub size: u64,
    pub modified: u64,
}

/// Логи игры и краш-отчёты, новые сверху.
pub fn logs(game_dir: &Path) -> Result<Vec<LogFile>> {
    let mut out = Vec::new();
    for (sub, kind) in [("logs", LogKind::Log), ("crash-reports", LogKind::Crash)] {
        let Ok(read) = std::fs::read_dir(game_dir.join(sub)) else {
            continue;
        };
        for entry in read.flatten() {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let Ok(meta) = entry.metadata() else { continue };
            let lower = name.to_lowercase();
            if !meta.is_file()
                || !(lower.ends_with(".log") || lower.ends_with(".gz") || lower.ends_with(".txt"))
            {
                continue;
            }
            out.push(LogFile {
                file: format!("{sub}/{name}"),
                kind,
                size: meta.len(),
                modified: mtime(&meta),
            });
        }
    }
    out.sort_by_key(|f| std::cmp::Reverse(f.modified));
    Ok(out)
}

/// Путь к логу из списка [`logs`].
pub fn log_path(game_dir: &Path, file: &str) -> Result<PathBuf> {
    let (sub, name) = file
        .split_once('/')
        .ok_or_else(|| Error::Other(format!("недопустимый путь «{file}»")))?;
    if !matches!(sub, "logs" | "crash-reports") {
        return Err(Error::Other(format!("недопустимый путь «{file}»")));
    }
    check_name(name)?;
    Ok(game_dir.join(sub).join(name))
}

/// Ширина миниатюры скриншота: в сетке они около 240 точек.
const THUMB_WIDTH: u32 = 320;

/// Миниатюра скриншота (PNG). Полный скриншот 1920×1080 в окне занял бы
/// 8 МБ памяти, миниатюра — 0,2 МБ. Готовые миниатюры лежат в `cache/thumbs`.
pub fn thumbnail(data: &DataDir, game_dir: &Path, file: &str) -> Result<Vec<u8>> {
    let path = inside(game_dir, Folder::Screenshots, file)?;
    let meta = std::fs::metadata(&path).at(&path)?;
    let key = {
        use sha1::{Digest, Sha1};
        let mut h = Sha1::new();
        h.update(path.to_string_lossy().as_bytes());
        h.update(mtime(&meta).to_le_bytes());
        h.update(meta.len().to_le_bytes());
        crate::verify::hex(&h.finalize())
    };
    let cached = data.cache().join("thumbs").join(format!("{key}.png"));
    if let Ok(bytes) = std::fs::read(&cached) {
        return Ok(bytes);
    }
    let image = open_image(&path)?;
    let height = (THUMB_WIDTH as u64 * image.height() as u64 / image.width().max(1) as u64) as u32;
    let thumb = image.thumbnail(THUMB_WIDTH, height.max(1));
    let png = encode_png(&thumb)?;
    if let Some(parent) = cached.parent() {
        std::fs::create_dir_all(parent).at(parent)?;
    }
    std::fs::write(&cached, &png).at(&cached)?;
    Ok(png)
}

/// Квадратная обложка 256×256 из середины картинки.
pub fn cover_from(path: &Path) -> Result<Vec<u8>> {
    let image = open_image(path)?;
    let side = image.width().min(image.height());
    let x = (image.width() - side) / 2;
    let y = (image.height() - side) / 2;
    let square = image.crop_imm(x, y, side, side);
    let cover = square.resize_exact(256, 256, image::imageops::FilterType::Triangle);
    encode_png(&cover)
}

fn open_image(path: &Path) -> Result<image::DynamicImage> {
    image::ImageReader::open(path)
        .at(path)?
        .with_guessed_format()
        .at(path)?
        .decode()
        .map_err(|e| {
            Error::Other(format!(
                "не удалось открыть картинку {}: {e}",
                path.display()
            ))
        })
}

fn encode_png(image: &image::DynamicImage) -> Result<Vec<u8>> {
    let mut out = std::io::Cursor::new(Vec::new());
    image
        .to_rgb8()
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| Error::Other(format!("не удалось сохранить картинку: {e}")))?;
    Ok(out.into_inner())
}

fn mtime(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn level_dat(dir: &Path, name: &str) {
        let mut nbt = Vec::new();
        nbt.extend_from_slice(b"\x0a\x00\x00\x0a\x00\x04Data");
        nbt.extend_from_slice(b"\x08\x00\x09LevelName");
        nbt.extend_from_slice(&(name.len() as u16).to_be_bytes());
        nbt.extend_from_slice(name.as_bytes());
        nbt.extend_from_slice(b"\x00\x00");
        std::fs::create_dir_all(dir).unwrap();
        let file = std::fs::File::create(dir.join("level.dat")).unwrap();
        let mut gz = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        gz.write_all(&nbt).unwrap();
        gz.finish().unwrap();
    }

    #[test]
    fn worlds_have_names_from_level_dat() {
        let game = tempfile::tempdir().unwrap();
        level_dat(&game.path().join("saves/New World"), "Мой мир");
        std::fs::create_dir_all(game.path().join("saves/not a world")).unwrap();
        let worlds = worlds(game.path()).unwrap();
        assert_eq!(worlds.len(), 1);
        assert_eq!(worlds[0].folder, "New World");
        assert_eq!(worlds[0].name, "Мой мир");
        std::fs::write(game.path().join("saves/New World/session.lock"), "☃").unwrap();
        assert_eq!(current_world(game.path()).as_deref(), Some("Мой мир"));
    }

    #[test]
    fn mods_toggle_and_stay_inside() {
        let game = tempfile::tempdir().unwrap();
        let mods = game.path().join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("sodium.jar"), "x").unwrap();
        std::fs::write(mods.join("readme.txt"), "x").unwrap();
        let list = files(game.path(), Folder::Mods).unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].enabled);

        let off = set_enabled(game.path(), Folder::Mods, "sodium.jar", false).unwrap();
        assert_eq!(off, "sodium.jar.disabled");
        let list = files(game.path(), Folder::Mods).unwrap();
        assert_eq!(
            (list[0].name.as_str(), list[0].enabled),
            ("sodium.jar", false)
        );
        set_enabled(game.path(), Folder::Mods, &off, true).unwrap();
        assert!(mods.join("sodium.jar").exists());

        assert!(inside(game.path(), Folder::Mods, "../evil.jar").is_err());
        assert!(inside(game.path(), Folder::Mods, "C:evil.jar").is_err());
        assert!(log_path(game.path(), "saves/x.log").is_err());
        assert!(log_path(game.path(), "logs/../../x.log").is_err());
        assert!(log_path(game.path(), "logs/latest.log").is_ok());
    }

    #[test]
    fn thumbnails_and_covers() {
        let data_dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(data_dir.path());
        let game = tempfile::tempdir().unwrap();
        let shots = game.path().join("screenshots");
        std::fs::create_dir_all(&shots).unwrap();
        let img = image::RgbImage::from_pixel(640, 360, image::Rgb([200, 30, 30]));
        img.save(shots.join("a.png")).unwrap();

        let thumb =
            image::load_from_memory(&thumbnail(&data, game.path(), "a.png").unwrap()).unwrap();
        assert_eq!((thumb.width(), thumb.height()), (320, 180));
        // Второй раз — из кэша.
        assert_eq!(
            std::fs::read_dir(data.cache().join("thumbs"))
                .unwrap()
                .count(),
            1
        );
        thumbnail(&data, game.path(), "a.png").unwrap();

        let cover = image::load_from_memory(&cover_from(&shots.join("a.png")).unwrap()).unwrap();
        assert_eq!((cover.width(), cover.height()), (256, 256));
    }
}
