//! Fabric: готовое описание версии с сервера Fabric Meta, у всех библиотек
//! есть хеши. Процессоров нет — ставится за секунды.

use std::path::PathBuf;

use serde::Deserialize;

use super::{LoaderVersion, cached_get, save_version};
use crate::download::Downloader;
use crate::error::{Result, json};
use crate::paths::DataDir;

const META: &str = "https://meta.fabricmc.net/v2";

#[derive(Deserialize)]
struct Game {
    version: String,
}

#[derive(Deserialize)]
struct Entry {
    loader: Version,
}

#[derive(Deserialize)]
struct Version {
    version: String,
    #[serde(default)]
    stable: bool,
}

pub(super) async fn games(data: &DataDir, dl: &Downloader) -> Result<Vec<String>> {
    let bytes = cached_get(
        data,
        dl,
        &format!("{META}/versions/game"),
        "fabric-games.json",
    )
    .await?;
    let list: Vec<Game> = json(&bytes, "список версий Fabric")?;
    Ok(list.into_iter().map(|g| g.version).collect())
}

pub(super) async fn versions(
    data: &DataDir,
    dl: &Downloader,
    game: &str,
) -> Result<Vec<LoaderVersion>> {
    let url = format!("{META}/versions/loader/{game}");
    let bytes = cached_get(data, dl, &url, &format!("fabric-{game}.json")).await?;
    let list: Vec<Entry> = json(&bytes, "список версий Fabric")?;
    Ok(list
        .into_iter()
        .map(|e| LoaderVersion {
            version: e.loader.version,
            stable: e.loader.stable,
            recommended: false,
        })
        .collect())
}

/// Описание версии с Fabric Meta. Библиотеки качает обычная установка версии.
pub(super) async fn install(
    data: &DataDir,
    dl: &Downloader,
    game: &str,
    version: &str,
    id: &str,
) -> Result<Vec<PathBuf>> {
    let url = format!("{META}/versions/loader/{game}/{version}/profile/json");
    let bytes = dl.get_bytes(&url).await?;
    let profile: serde_json::Value = json(&bytes, "описание версии Fabric")?;
    save_version(data, id, profile)?;
    Ok(Vec::new())
}
