//! Загрузчики модов (раздел 5.3 ТЗ): Fabric, Forge, NeoForge.
//!
//! Версия с загрузчиком — это ванильная версия плюс файлы загрузчика сверху:
//! своё описание версии с `inheritsFrom` (его собирает [`crate::install`]),
//! свои библиотеки, а у Forge и NeoForge — ещё и пропатченный клиент, который
//! делают «процессоры» из установщика. Ставится один раз на пару «версия игры
//! + версия загрузчика» и дальше общая для всех сборок.

mod fabric;
mod forge;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::download::Downloader;
use crate::error::{Error, IoContext, Result};
use crate::install::Progress;
use crate::paths::DataDir;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    /// Без загрузчика модов.
    #[default]
    Vanilla,
    Fabric,
    Forge,
    #[serde(rename = "neoforge")]
    NeoForge,
}

impl Loader {
    pub fn title(self) -> &'static str {
        match self {
            Loader::Vanilla => "Ванилла",
            Loader::Fabric => "Fabric",
            Loader::Forge => "Forge",
            Loader::NeoForge => "NeoForge",
        }
    }

    pub fn parse(text: &str) -> Option<Loader> {
        match text.to_lowercase().as_str() {
            "vanilla" | "ванилла" => Some(Loader::Vanilla),
            "fabric" => Some(Loader::Fabric),
            "forge" => Some(Loader::Forge),
            "neoforge" => Some(Loader::NeoForge),
            _ => None,
        }
    }
}

/// Версия загрузчика в списке выбора.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LoaderVersion {
    pub version: String,
    /// Не бета.
    pub stable: bool,
    /// Её лаунчер предлагает по умолчанию: «рекомендованная» у Forge,
    /// последняя стабильная у Fabric и NeoForge.
    pub recommended: bool,
}

/// Версии загрузчика для версии игры, новые первыми.
pub async fn versions(
    data: &DataDir,
    dl: &Downloader,
    loader: Loader,
    game: &str,
) -> Result<Vec<LoaderVersion>> {
    let mut list = match loader {
        Loader::Vanilla => Vec::new(),
        Loader::Fabric => fabric::versions(data, dl, game).await?,
        Loader::Forge | Loader::NeoForge => forge::versions(data, dl, loader, game).await?,
    };
    // Рекомендованной нет — предлагаем последнюю стабильную, затем просто последнюю.
    if !list.iter().any(|v| v.recommended) {
        let pick = list.iter().position(|v| v.stable).unwrap_or(0);
        if let Some(v) = list.get_mut(pick) {
            v.recommended = true;
        }
    }
    Ok(list)
}

/// Версии игры, для которых есть загрузчик.
pub async fn games(data: &DataDir, dl: &Downloader, loader: Loader) -> Result<Vec<String>> {
    match loader {
        Loader::Vanilla => Ok(Vec::new()),
        Loader::Fabric => fabric::games(data, dl).await,
        Loader::Forge | Loader::NeoForge => forge::games(data, dl, loader).await,
    }
}

/// Имя версии с загрузчиком в папке `versions`.
pub fn version_id(loader: Loader, game: &str, version: &str) -> String {
    match loader {
        Loader::Vanilla => game.to_string(),
        Loader::Fabric => format!("fabric-loader-{version}-{game}"),
        Loader::Forge => format!("{game}-forge-{version}"),
        Loader::NeoForge => format!("{game}-neoforge-{version}"),
    }
}

/// Разобрать имя версии с загрузчиком обратно: загрузчик, версия игры,
/// версия загрузчика. Нужно для папок, созданных `mlcli`.
pub fn parse_version_id(id: &str) -> Option<(Loader, String, String)> {
    if let Some(rest) = id.strip_prefix("fabric-loader-") {
        let (version, game) = rest.split_once('-')?;
        return Some((Loader::Fabric, game.to_string(), version.to_string()));
    }
    for (loader, sep) in [(Loader::NeoForge, "-neoforge-"), (Loader::Forge, "-forge-")] {
        if let Some((game, version)) = id.split_once(sep) {
            return Some((loader, game.to_string(), version.to_string()));
        }
    }
    None
}

/// Метка «загрузчик поставлен целиком» и список файлов, которые он сделал:
/// пропали — ставим заново.
fn marker(data: &DataDir, id: &str) -> PathBuf {
    data.version_dir(id).join(".loader.json")
}

#[derive(Default, Serialize, Deserialize)]
struct Marker {
    /// Пути от корня папки данных.
    files: Vec<String>,
}

fn installed(data: &DataDir, id: &str) -> bool {
    let Ok(bytes) = std::fs::read(marker(data, id)) else {
        return false;
    };
    let Ok(m) = serde_json::from_slice::<Marker>(&bytes) else {
        return false;
    };
    data.version_json(id).exists() && m.files.iter().all(|f| data.root().join(f).exists())
}

fn write_marker(data: &DataDir, id: &str, files: &[PathBuf]) -> Result<()> {
    let files = files
        .iter()
        .filter_map(|p| p.strip_prefix(data.root()).ok())
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    let path = marker(data, id);
    let bytes = serde_json::to_vec_pretty(&Marker { files }).expect("метка сериализуется");
    std::fs::write(&path, bytes).at(&path)
}

/// Поставить загрузчик, если он ещё не стоит. Возвращает имя версии для
/// [`crate::install::prepare`].
pub async fn ensure(
    data: &DataDir,
    dl: &Downloader,
    loader: Loader,
    game: &str,
    version: Option<&str>,
    progress: Progress,
) -> Result<String> {
    if loader == Loader::Vanilla {
        return Ok(game.to_string());
    }
    let version = version
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| Error::Other(format!("не выбрана версия {}", loader.title())))?;
    let id = version_id(loader, game, version);
    if installed(data, &id) {
        return Ok(id);
    }
    let files = match loader {
        Loader::Fabric => fabric::install(data, dl, game, version, &id).await?,
        _ => forge::install(data, dl, loader, game, version, &id, progress).await?,
    };
    write_marker(data, &id, &files)?;
    Ok(id)
}

/// Записать описание версии загрузчика под нашим именем.
fn save_version(data: &DataDir, id: &str, mut version: serde_json::Value) -> Result<()> {
    version["id"] = serde_json::Value::String(id.to_string());
    if let Some(obj) = version.as_object_mut() {
        obj.remove("_comment_");
    }
    let path = data.version_json(id);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).at(dir)?;
    }
    let bytes = serde_json::to_vec_pretty(&version).expect("описание версии сериализуется");
    std::fs::write(&path, bytes).at(&path)
}

/// Скачать список (версии загрузчика) и сохранить в кэш; без сети — взять
/// сохранённый.
async fn cached_get(data: &DataDir, dl: &Downloader, url: &str, name: &str) -> Result<Vec<u8>> {
    let path = data.cache().join("loaders").join(name);
    match dl.get_bytes(url).await {
        Ok(bytes) => {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).at(dir)?;
            }
            std::fs::write(&path, &bytes).at(&path)?;
            Ok(bytes)
        }
        Err(e) if e.is_network() && path.exists() => {
            tracing::warn!("нет связи, берём сохранённый список {name}: {e}");
            std::fs::read(&path).at(&path)
        }
        Err(e) => Err(e),
    }
}

/// Сравнение версий вида `47.4.0`, `0.16.10`, `14.23.5.2860`: по числам,
/// хвосты вроде `-beta` — после чисел.
pub(crate) fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    fn parts(v: &str) -> Vec<u64> {
        v.split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty())
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    }
    parts(a).cmp(&parts(b))
}

/// Версия игры не младше `min` (по числам: `1.6` < `1.12.2` < `26.1`).
fn game_at_least(game: &str, min: &[u64]) -> bool {
    let parts: Vec<u64> = game.split('.').map_while(|p| p.parse().ok()).collect();
    !parts.is_empty() && parts.as_slice() >= min
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_numbers() {
        use std::cmp::Ordering::*;
        assert_eq!(compare_versions("47.4.10", "47.4.9"), Greater);
        assert_eq!(compare_versions("0.16.10", "0.9.3"), Greater);
        assert_eq!(compare_versions("21.1.7", "21.1.7"), Equal);
        assert!(game_at_least("1.12.2", &[1, 6]));
        assert!(game_at_least("26.1", &[1, 6]));
        assert!(!game_at_least("1.5.2", &[1, 6]));
        assert!(!game_at_least("25w14craftmine", &[1, 6]));
    }

    #[test]
    fn ids_and_names() {
        assert_eq!(
            version_id(Loader::Fabric, "1.21.1", "0.16.10"),
            "fabric-loader-0.16.10-1.21.1"
        );
        assert_eq!(
            version_id(Loader::Forge, "1.20.1", "47.4.0"),
            "1.20.1-forge-47.4.0"
        );
        assert_eq!(Loader::parse("NeoForge"), Some(Loader::NeoForge));
        for (loader, game, version) in [
            (Loader::Fabric, "1.21.1", "0.16.10"),
            (Loader::Forge, "1.20.1", "47.4.0"),
            (Loader::NeoForge, "1.21.1", "21.1.77"),
        ] {
            assert_eq!(
                parse_version_id(&version_id(loader, game, version)),
                Some((loader, game.to_string(), version.to_string()))
            );
        }
        assert_eq!(parse_version_id("1.21.1"), None);
        assert_eq!(
            serde_json::to_string(&Loader::NeoForge).unwrap(),
            "\"neoforge\""
        );
    }
}
