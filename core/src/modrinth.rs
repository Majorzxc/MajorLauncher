//! Каталог Modrinth (раздел 5.4 ТЗ): поиск, проекты, версии. Открытое API
//! без ключа; Modrinth просит узнаваемый User-Agent — он у загрузчика файлов
//! свой («MajorLauncher/версия» и адрес репозитория).

use serde::{Deserialize, Serialize};

use crate::download::Downloader;
use crate::error::{Error, Result, json};
use crate::loaders::Loader;

pub const API: &str = "https://api.modrinth.com/v2";
/// Сколько результатов поиска за раз.
pub const PAGE: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectType {
    Mod,
    ResourcePack,
    Shader,
    Modpack,
}

impl ProjectType {
    pub fn facet(self) -> &'static str {
        match self {
            ProjectType::Mod => "mod",
            ProjectType::ResourcePack => "resourcepack",
            ProjectType::Shader => "shader",
            ProjectType::Modpack => "modpack",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sort {
    #[default]
    Relevance,
    Downloads,
    Follows,
    Newest,
    Updated,
}

impl Sort {
    fn index(self) -> &'static str {
        match self {
            Sort::Relevance => "relevance",
            Sort::Downloads => "downloads",
            Sort::Follows => "follows",
            Sort::Newest => "newest",
            Sort::Updated => "updated",
        }
    }
}

/// Названия загрузчика у Modrinth. NeoForge для 1.20.1 понимает и моды Forge.
pub fn loader_names(loader: Loader, game: &str) -> Vec<&'static str> {
    match loader {
        Loader::Vanilla => Vec::new(),
        Loader::Fabric => vec!["fabric"],
        Loader::Forge => vec!["forge"],
        Loader::NeoForge if game == "1.20.1" => vec!["neoforge", "forge"],
        Loader::NeoForge => vec!["neoforge"],
    }
}

/// Что искать. `game` и `loaders` — фильтр совместимости; пустые — без фильтра.
#[derive(Debug, Clone, Default)]
pub struct Search<'a> {
    pub query: &'a str,
    pub kind: Option<ProjectType>,
    pub game: Option<&'a str>,
    pub loaders: Vec<&'a str>,
    pub sort: Sort,
    pub offset: u32,
    /// Категории: все выбранные сразу (как на сайте Modrinth).
    pub categories: &'a [String],
    /// Где работает: нужен на клиенте или на сервере.
    pub environment: Option<Environment>,
    /// Только с открытым исходным кодом.
    pub open_source: bool,
}

/// Где должен работать проект (фильтр «Клиент / Сервер»).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Client,
    Server,
}

/// Категория Modrinth для фильтра.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub name: String,
    pub project_type: String,
    /// Группа: categories, features, resolutions, performance impact.
    pub header: String,
}

/// Все категории Modrinth (тег-лист, меняется редко).
pub async fn categories(dl: &Downloader) -> Result<Vec<Category>> {
    get(dl, &url("/tag/category", &[])?).await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hit {
    /// В ответе Modrinth — `project_id`, окну отдаём как `id`.
    #[serde(rename(deserialize = "project_id"))]
    pub id: String,
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    pub icon_url: Option<String>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub follows: u64,
    #[serde(default)]
    pub display_categories: Vec<String>,
    #[serde(default)]
    pub date_modified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub hits: Vec<Hit>,
    pub total_hits: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gallery {
    /// Уменьшенная картинка (до 350 точек).
    pub url: String,
    pub raw_url: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub featured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct License {
    pub id: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Donation {
    pub platform: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    /// Описание в Markdown (бывает и HTML внутри) — окно чистит его перед показом.
    #[serde(default)]
    pub body: String,
    pub project_type: ProjectType,
    pub icon_url: Option<String>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub followers: u64,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    pub license: Option<License>,
    pub source_url: Option<String>,
    pub issues_url: Option<String>,
    pub wiki_url: Option<String>,
    pub discord_url: Option<String>,
    #[serde(default)]
    pub donation_urls: Vec<Donation>,
    #[serde(default)]
    pub gallery: Vec<Gallery>,
    #[serde(default)]
    pub client_side: String,
    #[serde(default)]
    pub updated: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyType {
    Required,
    Optional,
    Incompatible,
    Embedded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub version_id: Option<String>,
    pub project_id: Option<String>,
    pub file_name: Option<String>,
    pub dependency_type: DependencyType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hashes {
    pub sha1: String,
    #[serde(default)]
    pub sha512: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionFile {
    pub hashes: Hashes,
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    /// `release`, `beta`, `alpha`.
    pub version_type: String,
    #[serde(default)]
    pub date_published: String,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub files: Vec<VersionFile>,
}

impl Version {
    /// Главный файл версии (у некоторых модов рядом лежат исходники и т. п.).
    pub fn file(&self) -> Option<&VersionFile> {
        self.files
            .iter()
            .find(|f| f.primary)
            .or_else(|| self.files.first())
    }
}

/// Коротко о проекте — для названий зависимостей.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brief {
    pub id: String,
    pub title: String,
    pub icon_url: Option<String>,
}

fn url(path: &str, params: &[(&str, String)]) -> Result<String> {
    reqwest::Url::parse_with_params(&format!("{API}{path}"), params)
        .map(|u| u.to_string())
        .map_err(|e| Error::Other(format!("непонятный адрес Modrinth: {e}")))
}

fn list(items: &[&str]) -> String {
    serde_json::to_string(items).expect("список строк сериализуется")
}

async fn get<T: serde::de::DeserializeOwned>(dl: &Downloader, url: &str) -> Result<T> {
    let bytes = dl.get_bytes(url).await?;
    json(&bytes, "ответ Modrinth")
}

pub async fn search(dl: &Downloader, s: &Search<'_>) -> Result<SearchResult> {
    let mut facets: Vec<Vec<String>> = Vec::new();
    if let Some(kind) = s.kind {
        facets.push(vec![format!("project_type:{}", kind.facet())]);
    }
    if let Some(game) = s.game {
        facets.push(vec![format!("versions:{game}")]);
    }
    if !s.loaders.is_empty() {
        facets.push(
            s.loaders
                .iter()
                .map(|l| format!("categories:{l}"))
                .collect(),
        );
    }
    for c in s.categories {
        let c = encode(c);
        if !c.is_empty() {
            facets.push(vec![format!("categories:{c}")]);
        }
    }
    match s.environment {
        Some(Environment::Client) => facets.push(vec![
            "client_side:required".into(),
            "client_side:optional".into(),
        ]),
        Some(Environment::Server) => facets.push(vec![
            "server_side:required".into(),
            "server_side:optional".into(),
        ]),
        None => {}
    }
    if s.open_source {
        facets.push(vec!["open_source:true".into()]);
    }
    let mut params = vec![
        ("query", s.query.trim().to_string()),
        ("index", s.sort.index().to_string()),
        ("offset", s.offset.to_string()),
        ("limit", PAGE.to_string()),
    ];
    if !facets.is_empty() {
        params.push((
            "facets",
            serde_json::to_string(&facets).expect("фасеты сериализуются"),
        ));
    }
    get(dl, &url("/search", &params)?).await
}

pub async fn project(dl: &Downloader, id: &str) -> Result<Project> {
    get(dl, &url(&format!("/project/{}", encode(id)), &[])?).await
}

/// Версии проекта, новые первыми. Пустые фильтры — все версии.
pub async fn versions(
    dl: &Downloader,
    id: &str,
    loaders: &[&str],
    game: Option<&str>,
) -> Result<Vec<Version>> {
    let mut params = Vec::new();
    if !loaders.is_empty() {
        params.push(("loaders", list(loaders)));
    }
    if let Some(game) = game {
        params.push(("game_versions", list(&[game])));
    }
    get(
        dl,
        &url(&format!("/project/{}/version", encode(id)), &params)?,
    )
    .await
}

/// Версии по id (одним запросом).
pub async fn versions_by_id(dl: &Downloader, ids: &[&str]) -> Result<Vec<Version>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    get(dl, &url("/versions", &[("ids", list(ids))])?).await
}

/// Названия и иконки проектов (одним запросом).
pub async fn briefs(dl: &Downloader, ids: &[&str]) -> Result<Vec<Brief>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    get(dl, &url("/projects", &[("ids", list(ids))])?).await
}

/// Версии по SHA-1 файлов: опознать моды, скачанные не через каталог.
/// Ответ — только опознанные хеши.
pub async fn by_hashes(
    dl: &Downloader,
    hashes: &[String],
) -> Result<std::collections::HashMap<String, Version>> {
    if hashes.is_empty() {
        return Ok(Default::default());
    }
    let body = serde_json::json!({ "hashes": hashes, "algorithm": "sha1" });
    let bytes = dl.post_json(&format!("{API}/version_files"), &body).await?;
    json(&bytes, "ответ Modrinth")
}

/// Новейшие версии для файлов под эту версию игры и загрузчик.
pub async fn updates(
    dl: &Downloader,
    hashes: &[String],
    loaders: &[&str],
    game: &str,
) -> Result<std::collections::HashMap<String, Version>> {
    if hashes.is_empty() {
        return Ok(Default::default());
    }
    let body = serde_json::json!({
        "hashes": hashes,
        "algorithm": "sha1",
        "loaders": loaders,
        "game_versions": [game],
    });
    let bytes = dl
        .post_json(&format!("{API}/version_files/update"), &body)
        .await?;
    json(&bytes, "ответ Modrinth")
}

/// id и slug проектов — латиница, цифры, «-» и «_»; остальное не пускаем в адрес.
fn encode(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .collect()
}

/// Лучшая версия из списка (новые первыми): релиз, если есть, иначе бета, иначе альфа.
pub fn best(versions: &[Version]) -> Option<&Version> {
    ["release", "beta", "alpha"]
        .iter()
        .find_map(|kind| versions.iter().find(|v| v.version_type == *kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(id: &str, kind: &str) -> Version {
        Version {
            id: id.into(),
            project_id: "p".into(),
            name: id.into(),
            version_number: id.into(),
            game_versions: vec![],
            loaders: vec![],
            version_type: kind.into(),
            date_published: String::new(),
            downloads: 0,
            dependencies: vec![],
            files: vec![],
        }
    }

    #[test]
    fn prefers_releases() {
        let list = [v("3", "beta"), v("2", "release"), v("1", "release")];
        assert_eq!(best(&list).unwrap().id, "2");
        assert_eq!(best(&[v("3", "alpha"), v("2", "beta")]).unwrap().id, "2");
        assert!(best(&[]).is_none());
    }

    #[test]
    fn ids_cannot_escape_the_path() {
        assert_eq!(encode("../../user"), "user");
        assert_eq!(encode("fabric-api"), "fabric-api");
        assert_eq!(
            loader_names(Loader::NeoForge, "1.20.1"),
            ["neoforge", "forge"]
        );
        assert!(loader_names(Loader::Vanilla, "1.21.1").is_empty());
    }

    #[test]
    fn parses_search_hit() {
        let hit: SearchResult = serde_json::from_str(
            r#"{"hits":[{"project_id":"AANobbMI","slug":"sodium","title":"Sodium","description":"d","author":"a",
            "icon_url":null,"downloads":5,"follows":1,"display_categories":["fabric"],"date_modified":"2026"}],"total_hits":1}"#,
        )
        .unwrap();
        assert_eq!(hit.hits[0].id, "AANobbMI");
        // Окну — как `id`: по нему оно открывает проект.
        let out = serde_json::to_value(&hit.hits[0]).unwrap();
        assert_eq!(out["id"], "AANobbMI");
    }

    /// Ручная проверка на живом API: cargo test -p majorlauncher-core modrinth_live -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn modrinth_live() {
        let dl = Downloader::new(Default::default()).unwrap();
        let found = search(
            &dl,
            &Search {
                query: "iris",
                kind: Some(ProjectType::Mod),
                game: Some("1.21.1"),
                loaders: vec!["fabric"],
                ..Default::default()
            },
        )
        .await
        .unwrap();
        println!(
            "{} найдено, первый: {}",
            found.total_hits, found.hits[0].title
        );
        let list = versions(&dl, "iris", &["fabric"], Some("1.21.1"))
            .await
            .unwrap();
        let v = best(&list).unwrap();
        println!("{} {:?}", v.version_number, v.dependencies);
        let p = project(&dl, "iris").await.unwrap();
        assert_eq!(p.slug, "iris");
    }
}
