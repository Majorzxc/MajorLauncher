//! Скины (раздел 5.9 ТЗ): библиотека скинов игрока, скин по нику (Minecraft
//! и Ely.by) и по ссылке, каталоги Ely.by и MineSkin.
//!
//! Библиотека — `skins/` в папке данных: `<sha1>.png` и `library.json` с
//! названиями. Один и тот же скин второй раз не добавляется (имя файла —
//! хеш содержимого).
//!
//! Сменить скин аккаунта Ely.by через API нельзя (у Ely.by такого API нет):
//! окно открывает страницу скина на сайте. Плащи Ely.by носить нельзя —
//! их система «не даёт игрокам надевать плащи самим» (документация Ely.by).

use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use image::{GenericImageView, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::download::Downloader;
use crate::error::{Error, IoContext, Result};
use crate::paths::DataDir;

/// Больше такого файл скина не бывает (64×64 PNG — единицы килобайт).
const MAX_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skin {
    /// SHA-1 картинки — и имя файла.
    pub id: String,
    pub name: String,
    /// Тонкие руки (модель «Alex»).
    pub slim: bool,
    /// Откуда: «файл», «ник jeb_ (Minecraft)», «каталог Ely.by» и т. п.
    #[serde(default)]
    pub source: String,
    /// Страница скина (каталог Ely.by) — для «Надеть на Ely.by».
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    pub added: u64,
}

/// Настройки скинов.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkinSettings {
    /// Программа для «Редактировать»; нет — программа Windows для PNG.
    #[serde(default)]
    pub editor: Option<PathBuf>,
    /// Показывать в каталоге скины со спорными тегами.
    #[serde(default)]
    pub show_sensitive: bool,
}

/// Проверить, что это скин: PNG 64×64 или старый 64×32.
pub fn validate(bytes: &[u8]) -> Result<RgbaImage> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::Other("файл слишком большой для скина".into()));
    }
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|_| Error::Other("это не картинка PNG".into()))?;
    match img.dimensions() {
        (64, 64) | (64, 32) => Ok(img.to_rgba8()),
        (w, h) => Err(Error::Other(format!(
            "картинка {w}×{h}, а скин Minecraft — 64×64 (или старый 64×32)"
        ))),
    }
}

/// Тонкие руки: у модели «Alex» руки шириной 3 пикселя, и на развёртке
/// столбцы 54–55 правой руки (строки 20–31) пустые.
pub fn detect_slim(img: &RgbaImage) -> bool {
    if img.height() < 64 {
        return false;
    }
    (20..32).all(|y| (54..56).all(|x| img.get_pixel(x, y)[3] == 0))
}

pub struct Library {
    dir: PathBuf,
}

impl Library {
    pub fn new(data: &DataDir) -> Self {
        Self {
            dir: data.root().join("skins"),
        }
    }

    fn index(&self) -> PathBuf {
        self.dir.join("library.json")
    }

    pub fn path(&self, id: &str) -> Result<PathBuf> {
        if id.len() != 40 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(Error::Other(format!("нет такого скина «{id}»")));
        }
        Ok(self.dir.join(format!("{id}.png")))
    }

    /// Скины — новые сверху. Файлы, которых нет на диске, пропускаются.
    pub fn list(&self) -> Vec<Skin> {
        let mut list: Vec<Skin> = std::fs::read(self.index())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        list.retain(|s| self.path(&s.id).is_ok_and(|p| p.exists()));
        list.sort_by_key(|s| std::cmp::Reverse(s.added));
        list
    }

    fn save(&self, list: &[Skin]) -> Result<()> {
        std::fs::create_dir_all(&self.dir).at(&self.dir)?;
        let tmp = self.dir.join("library.json.tmp");
        std::fs::write(
            &tmp,
            serde_json::to_vec_pretty(list).expect("сериализуется"),
        )
        .at(&tmp)?;
        std::fs::rename(&tmp, self.index()).at(&self.index())
    }

    /// Добавить скин. Уже есть такой — вернуть его (без дубля).
    pub fn add(
        &self,
        bytes: &[u8],
        name: &str,
        source: &str,
        page: Option<String>,
        slim: Option<bool>,
    ) -> Result<Skin> {
        let img = validate(bytes)?;
        let id = crate::verify::sha1_bytes(bytes);
        let mut list = self.list();
        if let Some(s) = list.iter().find(|s| s.id == id) {
            return Ok(s.clone());
        }
        let path = self.path(&id)?;
        std::fs::create_dir_all(&self.dir).at(&self.dir)?;
        std::fs::write(&path, bytes).at(&path)?;
        let name = name.trim();
        let skin = Skin {
            id,
            name: if name.is_empty() {
                "Скин".into()
            } else {
                name.chars().take(64).collect()
            },
            slim: slim.unwrap_or_else(|| detect_slim(&img)),
            source: source.to_string(),
            page,
            added: crate::builds::now(),
        };
        list.push(skin.clone());
        self.save(&list)?;
        Ok(skin)
    }

    /// Добавить из файла на диске; название — имя файла.
    pub fn add_file(&self, path: &Path) -> Result<Skin> {
        let bytes = std::fs::read(path).at(path)?;
        let name = path
            .file_stem()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        self.add(&bytes, &name, "файл", None, None)
    }

    fn update(&self, id: &str, change: impl FnOnce(&mut Skin)) -> Result<Skin> {
        let mut list = self.list();
        let skin = list
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| Error::Other("такого скина в библиотеке нет".into()))?;
        change(skin);
        let out = skin.clone();
        self.save(&list)?;
        Ok(out)
    }

    pub fn rename(&self, id: &str, name: &str) -> Result<Skin> {
        let name: String = name.trim().chars().take(64).collect();
        if name.is_empty() {
            return Err(Error::Other("название не может быть пустым".into()));
        }
        self.update(id, |s| s.name = name)
    }

    pub fn set_slim(&self, id: &str, slim: bool) -> Result<Skin> {
        self.update(id, |s| s.slim = slim)
    }

    pub fn remove(&self, id: &str) -> Result<()> {
        let path = self.path(id)?;
        let mut list = self.list();
        list.retain(|s| s.id != id);
        self.save(&list)?;
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e).at(&path),
            _ => Ok(()),
        }
    }

    /// Файл изменили в редакторе: картинка всё ещё скин? Тонкие руки — по
    /// новой картинке. Имя файла остаётся прежним (по нему следит окно).
    pub fn changed(&self, id: &str) -> Result<Skin> {
        let path = self.path(id)?;
        let bytes = std::fs::read(&path).at(&path)?;
        let img = validate(&bytes)?;
        let slim = detect_slim(&img);
        self.update(id, |s| s.slim = slim)
    }
}

/// Какой скин надет на офлайн-ник: `skins/worn.json` (аккаунт → скин).
fn worn_path(data: &DataDir) -> PathBuf {
    data.root().join("skins").join("worn.json")
}

fn worn_all(data: &DataDir) -> std::collections::HashMap<String, String> {
    std::fs::read(worn_path(data))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Скин, надетый на офлайн-аккаунт (его кладёт в игру CustomSkinLoader).
pub fn worn(data: &DataDir, account: &str) -> Option<String> {
    worn_all(data).remove(account)
}

pub fn set_worn(data: &DataDir, account: &str, skin: Option<&str>) -> Result<()> {
    let mut all = worn_all(data);
    match skin {
        Some(id) => {
            Library::new(data).path(id)?;
            all.insert(account.to_string(), id.to_string());
        }
        None => {
            all.remove(account);
        }
    }
    let path = worn_path(data);
    std::fs::create_dir_all(data.root().join("skins")).at(&data.root().join("skins"))?;
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&all).expect("сериализуется"),
    )
    .at(&path)
}

/// Плащ: PNG вдвое шире высоты, 64×32 (обычный) и больше (HD — до 1024).
pub fn validate_cape(bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::Other("файл слишком большой для плаща".into()));
    }
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|_| Error::Other("это не картинка PNG".into()))?;
    let (w, h) = img.dimensions();
    if w == h * 2 && w % 64 == 0 && w <= 1024 {
        Ok(())
    } else {
        Err(Error::Other(format!(
            "картинка {w}×{h}, а плащ Minecraft — 64×32 (или больше в той же пропорции)"
        )))
    }
}

fn capes_dir(data: &DataDir) -> PathBuf {
    data.root().join("skins").join("capes")
}

/// Плащ офлайн-аккаунта: `skins/capes/<аккаунт>.png`.
pub fn cape_path(data: &DataDir, account: &str) -> Result<PathBuf> {
    if account.is_empty() || account.contains(['/', '\\']) || account.contains("..") {
        return Err(Error::Other(format!("нет такого аккаунта «{account}»")));
    }
    Ok(capes_dir(data).join(format!("{account}.png")))
}

/// Надеть плащ на офлайн-аккаунт (`None` — снять).
pub fn set_cape(data: &DataDir, account: &str, bytes: Option<&[u8]>) -> Result<()> {
    let path = cape_path(data, account)?;
    match bytes {
        Some(b) => {
            validate_cape(b)?;
            std::fs::create_dir_all(capes_dir(data)).at(&capes_dir(data))?;
            std::fs::write(&path, b).at(&path)
        }
        None => match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e).at(&path),
            _ => Ok(()),
        },
    }
}

/// Включён ли в сборке мод CustomSkinLoader.
pub fn has_skin_loader(game_dir: &Path) -> bool {
    std::fs::read_dir(game_dir.join("mods"))
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            name.contains("customskinloader") && name.ends_with(".jar")
        })
}

/// Перед запуском офлайн-ника: надетый скин — в папку CustomSkinLoader
/// (`CustomSkinLoader/LocalSkin/skins/<ник>.png`, так мод берёт скины из
/// файлов). Модель (тонкие руки) мод определяет по картинке сам. Нет мода
/// или скина — ничего не делается. `true` — скин положен.
pub fn apply_local(data: &DataDir, game_dir: &Path, account: &str, nick: &str) -> Result<bool> {
    if !has_skin_loader(game_dir) || check_nick(nick).is_err() {
        return Ok(false);
    }
    let local = game_dir.join("CustomSkinLoader").join("LocalSkin");
    let copy = |from: &Path, kind: &str| -> Result<bool> {
        if !from.exists() {
            return Ok(false);
        }
        let dir = local.join(kind);
        std::fs::create_dir_all(&dir).at(&dir)?;
        let to = dir.join(format!("{nick}.png"));
        std::fs::copy(from, &to).at(&to)?;
        Ok(true)
    };
    let skin = match worn(data, account) {
        Some(id) => copy(&Library::new(data).path(&id)?, "skins")?,
        None => false,
    };
    // Плащ: надет — положить; снят — убрать прежний, который клал лаунчер.
    let cape = cape_path(data, account)?;
    let placed = local.join("capes").join(format!("{nick}.png"));
    let cape = if cape.exists() {
        copy(&cape, "capes")?
    } else {
        if placed.exists() && local.join("capes").join(".majorlauncher").exists() {
            let _ = std::fs::remove_file(&placed);
        }
        false
    };
    if cape {
        let _ = std::fs::write(
            local.join("capes").join(".majorlauncher"),
            "плащи кладёт MajorLauncher",
        );
    }
    Ok(skin || cape)
}

/// Откуда брать скин по нику.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NickSource {
    /// Лицензия Minecraft (Mojang).
    #[serde(rename = "mojang")]
    Mojang,
    #[serde(rename = "ely")]
    Ely,
}

/// Текстуры игрока: скин, модель, плащ.
#[derive(Debug, Clone, Serialize)]
pub struct Textures {
    pub name: String,
    pub skin: Option<String>,
    pub slim: bool,
    pub cape: Option<String>,
}

fn textures_json(name: &str, v: &Value) -> Textures {
    let skin = v.get("SKIN");
    Textures {
        name: name.to_string(),
        skin: skin
            .and_then(|s| s.get("url"))
            .and_then(Value::as_str)
            .map(https),
        slim: skin
            .and_then(|s| s.pointer("/metadata/model"))
            .and_then(Value::as_str)
            == Some("slim"),
        cape: v.pointer("/CAPE/url").and_then(Value::as_str).map(https),
    }
}

/// Адреса текстур Mojang и Ely.by приходят с `http://` — берём `https://`.
fn https(url: &str) -> String {
    match url.strip_prefix("http://") {
        Some(rest) => format!("https://{rest}"),
        None => url.to_string(),
    }
}

fn check_nick(nick: &str) -> Result<&str> {
    let nick = nick.trim();
    if nick.len() < 2
        || nick.len() > 16
        || !nick.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(Error::Other(format!(
            "«{nick}» — не ник Minecraft: 2–16 латинских букв, цифр и «_»"
        )));
    }
    Ok(nick)
}

/// Текстуры игрока по нику.
pub async fn textures(dl: &Downloader, nick: &str, from: NickSource) -> Result<Textures> {
    let nick = check_nick(nick)?;
    let not_found = || Error::Other(format!("у игрока «{nick}» нет скина или такого ника нет"));
    match from {
        NickSource::Ely => {
            let bytes = dl
                .get_bytes(&format!("https://skinsystem.ely.by/textures/{nick}"))
                .await?;
            // Нет текстур — пустой ответ 204.
            let v: Value = serde_json::from_slice(&bytes).map_err(|_| not_found())?;
            let t = textures_json(nick, &v);
            if t.skin.is_none() {
                return Err(not_found());
            }
            Ok(t)
        }
        NickSource::Mojang => {
            let profile = match dl
                .get_bytes(&format!(
                    "https://api.mojang.com/users/profiles/minecraft/{nick}"
                ))
                .await
            {
                Err(Error::HttpStatus { status: 404, .. }) => return Err(not_found()),
                r => r?,
            };
            let profile: Value = serde_json::from_slice(&profile).map_err(|_| not_found())?;
            let id = profile
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(not_found)?;
            let name = profile.get("name").and_then(Value::as_str).unwrap_or(nick);
            let session = dl
                .get_bytes(&format!(
                    "https://sessionserver.mojang.com/session/minecraft/profile/{id}"
                ))
                .await?;
            let session: Value = serde_json::from_slice(&session).map_err(|_| not_found())?;
            let value = session
                .get("properties")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .find(|p| p.get("name").and_then(Value::as_str) == Some("textures"))
                .and_then(|p| p.get("value"))
                .and_then(Value::as_str)
                .ok_or_else(not_found)?;
            let decoded = STANDARD.decode(value).map_err(|_| not_found())?;
            let v: Value = serde_json::from_slice(&decoded).map_err(|_| not_found())?;
            let t = textures_json(name, v.get("textures").unwrap_or(&Value::Null));
            if t.skin.is_none() {
                return Err(not_found());
            }
            Ok(t)
        }
    }
}

/// Откуда лаунчер готов скачивать картинки скинов для окна.
fn allowed(url: &str) -> bool {
    [
        "https://textures.minecraft.net/",
        "https://ely.by/storage/",
        "https://skinsystem.ely.by/",
    ]
    .iter()
    .any(|p| url.starts_with(p))
}

/// Картинка скина или плаща с известного сервиса (с кэшем на диске).
/// Окну картинки нужны байтами: для 2D-фигуры читаются пиксели.
pub async fn texture(dl: &Downloader, data: &DataDir, url: &str) -> Result<Vec<u8>> {
    let url = https(url);
    if !allowed(&url) {
        return Err(Error::Other(format!(
            "скины берутся только с Minecraft и Ely.by, а не с {url}"
        )));
    }
    let dir = data.cache().join("skin-textures");
    let path = dir.join(format!("{}.png", crate::verify::sha1_bytes(url.as_bytes())));
    if let Ok(bytes) = std::fs::read(&path) {
        return Ok(bytes);
    }
    let bytes = dl.get_bytes(&url).await?;
    if bytes.len() > MAX_BYTES {
        return Err(Error::Other("картинка слишком большая".into()));
    }
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(&path, &bytes);
    Ok(bytes)
}

/// Добавить в библиотеку картинку по ссылке (любой адрес, только PNG-скин).
pub async fn download(dl: &Downloader, url: &str) -> Result<Vec<u8>> {
    let url = url.trim();
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err(Error::Other(
            "ссылка должна начинаться с https:// или http://".into(),
        ));
    }
    let bytes = dl.get_bytes(url).await?;
    validate(&bytes)?;
    Ok(bytes)
}

/// Каталог скинов.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Catalog {
    #[serde(rename = "ely")]
    Ely,
    #[serde(rename = "mineskin")]
    MineSkin,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CatalogQuery {
    pub catalog: Catalog,
    /// Ely.by — теги через запятую; MineSkin — часть названия.
    #[serde(default)]
    pub search: String,
    /// `best` / `time` (Ely.by), `popular` / `new` (MineSkin).
    #[serde(default)]
    pub sort: String,
    /// Раздел Ely.by: `Fantasy`, `Heroes`…
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub slim_only: bool,
    /// Страница (Ely.by, с 1) или курсор MineSkin.
    #[serde(default)]
    pub page: String,
    #[serde(default)]
    pub show_sensitive: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogSkin {
    pub id: String,
    pub url: String,
    pub name: Option<String>,
    /// `None` — неизвестно (MineSkin): окно определит по картинке.
    pub slim: Option<bool>,
    pub tags: Vec<String>,
    pub likes: Option<u64>,
    pub wearers: Option<u64>,
    /// Страница скина на сайте.
    pub page: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogPage {
    pub items: Vec<CatalogSkin>,
    /// Следующая страница; `None` — это последняя.
    pub next: Option<String>,
    pub total: Option<u64>,
    /// Сколько спорных скинов скрыто на этой странице.
    pub hidden: usize,
}

/// Спорные темы в тегах (каталог Ely.by никто не проверяет: на первом месте
/// в «лучших» у него — скин с нацистской темой).
const SENSITIVE: &[&str] = &[
    "hitler",
    "adolf",
    "nazi",
    "swastika",
    "porn",
    "sex",
    "nude",
    "naked",
    "nsfw",
    "hentai",
    "18+",
    "panties",
    "lingerie",
    "гитлер",
    "нацист",
    "свастик",
    "порн",
    "секс",
    "голая",
    "голый",
    "хентай",
    "трусы",
];

pub fn sensitive(tags: &[String]) -> bool {
    tags.iter().any(|t| {
        let t = t.to_lowercase();
        SENSITIVE.iter().any(|w| t.contains(w))
    })
}

pub async fn catalog(dl: &Downloader, q: &CatalogQuery) -> Result<CatalogPage> {
    let mut page = match q.catalog {
        Catalog::Ely => ely(dl, q).await?,
        Catalog::MineSkin => mineskin(dl, q).await?,
    };
    if !q.show_sensitive {
        let before = page.items.len();
        page.items.retain(|s| {
            !sensitive(&s.tags)
                && !s
                    .name
                    .as_deref()
                    .is_some_and(|n| sensitive(&[n.to_string()]))
        });
        page.hidden = before - page.items.len();
    }
    Ok(page)
}

fn bad(what: &str) -> Error {
    Error::Other(format!("{what}: непонятный ответ каталога"))
}

async fn ely(dl: &Downloader, q: &CatalogQuery) -> Result<CatalogPage> {
    let mut params: Vec<(String, String)> = Vec::new();
    let page: u64 = q.page.parse().unwrap_or(1).max(1);
    params.push(("page".into(), page.to_string()));
    if q.sort == "time" {
        params.push(("sort".into(), "time".into()));
    }
    if !q.kind.is_empty() {
        params.push(("kind".into(), q.kind.clone()));
    }
    if q.slim_only {
        params.push(("type".into(), "slim".into()));
    }
    for tag in q.search.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        params.push(("tags[]".into(), tag.to_string()));
    }
    let url = reqwest::Url::parse_with_params("https://ely.by/api/legacy/skins", &params)
        .map_err(|e| Error::Other(e.to_string()))?;
    let bytes = dl.get_bytes(url.as_str()).await?;
    let v: Value = serde_json::from_slice(&bytes).map_err(|_| bad("Ely.by"))?;
    let items = v
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("Ely.by"))?
        .iter()
        .filter_map(|s| {
            let id = s.get("id")?.as_u64()?;
            Some(CatalogSkin {
                id: id.to_string(),
                url: https(s.get("skin_url")?.as_str()?),
                name: None,
                slim: s.get("is_slim").and_then(Value::as_bool),
                tags: s
                    .get("tags")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|t| t.as_str().map(str::to_string))
                    .collect(),
                likes: s.get("count_cubes").and_then(Value::as_u64),
                wearers: s.get("count_wearers").and_then(Value::as_u64),
                page: Some(format!("https://ely.by/skins/s{id}")),
            })
        })
        .collect();
    let last = v.get("last").and_then(Value::as_u64).unwrap_or(page);
    Ok(CatalogPage {
        items,
        next: (page < last).then(|| (page + 1).to_string()),
        total: v.get("total_items").and_then(Value::as_u64),
        hidden: 0,
    })
}

async fn mineskin(dl: &Downloader, q: &CatalogQuery) -> Result<CatalogPage> {
    let search = q.search.trim();
    // «Популярные» — только без поиска: поиск у MineSkin есть лишь в общем списке.
    let base = if q.sort == "popular" && search.is_empty() {
        "https://api.mineskin.org/v2/skins/popular"
    } else {
        "https://api.mineskin.org/v2/skins"
    };
    let mut params: Vec<(&str, &str)> = vec![("size", "40")];
    if !search.is_empty() {
        params.push(("filter", search));
    }
    if !q.page.is_empty() {
        params.push(("after", q.page.as_str()));
    }
    let url =
        reqwest::Url::parse_with_params(base, &params).map_err(|e| Error::Other(e.to_string()))?;
    let bytes = dl.get_bytes(url.as_str()).await?;
    let v: Value = serde_json::from_slice(&bytes).map_err(|_| bad("MineSkin"))?;
    let items = v
        .get("skins")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("MineSkin"))?
        .iter()
        .filter_map(|s| {
            let texture = s.get("texture")?.as_str()?;
            if !texture.chars().all(|c| c.is_ascii_hexdigit()) {
                return None;
            }
            let id = s.get("uuid")?.as_str()?.to_string();
            let short = s.get("shortId").and_then(Value::as_str).unwrap_or(&id);
            Some(CatalogSkin {
                page: Some(format!("https://mineskin.org/skins/{short}")),
                id,
                url: format!("https://textures.minecraft.net/texture/{texture}"),
                name: s
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|n| !n.trim().is_empty())
                    .map(str::to_string),
                slim: None,
                tags: Vec::new(),
                likes: None,
                wearers: None,
            })
        })
        .collect();
    let next = v
        .pointer("/pagination/next/after")
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(CatalogPage {
        items,
        next,
        total: None,
        hidden: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, slim: bool) -> Vec<u8> {
        let mut img = RgbaImage::from_pixel(w, h, image::Rgba([200, 150, 100, 255]));
        if slim && h == 64 {
            for y in 20..32 {
                for x in 54..56 {
                    img.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
                }
            }
        }
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn library_add_dedup_remove() {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::new(&DataDir::new(dir.path()));
        let a = lib
            .add(&png(64, 64, true), " Алекс ", "файл", None, None)
            .unwrap();
        assert!(a.slim, "тонкие руки — по картинке");
        assert_eq!(a.name, "Алекс");
        let again = lib
            .add(&png(64, 64, true), "другое", "файл", None, None)
            .unwrap();
        assert_eq!(again.id, a.id, "дубль не добавляется");
        let b = lib
            .add(&png(64, 32, false), "", "файл", None, None)
            .unwrap();
        assert!(!b.slim);
        assert_eq!(lib.list().len(), 2);
        lib.rename(&b.id, "Стив").unwrap();
        lib.set_slim(&b.id, true).unwrap();
        assert!(lib.list().iter().any(|s| s.name == "Стив" && s.slim));
        lib.remove(&a.id).unwrap();
        assert_eq!(lib.list().len(), 1);
        assert!(!lib.path(&a.id).unwrap().exists());
        assert!(lib.path("../x").is_err());
    }

    #[test]
    fn offline_skin_goes_to_skin_loader() {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path());
        let skin = Library::new(&data)
            .add(&png(64, 64, false), "мой", "файл", None, None)
            .unwrap();
        let game = dir.path().join("game");
        std::fs::create_dir_all(game.join("mods")).unwrap();
        set_worn(&data, "offline-steve", Some(&skin.id)).unwrap();
        // Без мода — ничего.
        assert!(!apply_local(&data, &game, "offline-steve", "Steve").unwrap());
        // Выключенный мод — тоже.
        std::fs::write(
            game.join("mods/CustomSkinLoader_Universal-15.1.jar.disabled"),
            b"",
        )
        .unwrap();
        assert!(!apply_local(&data, &game, "offline-steve", "Steve").unwrap());
        std::fs::write(game.join("mods/CustomSkinLoader_Universal-15.1.jar"), b"").unwrap();
        assert!(apply_local(&data, &game, "offline-steve", "Steve").unwrap());
        assert!(
            game.join("CustomSkinLoader/LocalSkin/skins/Steve.png")
                .exists()
        );
        // Другой аккаунт без скина.
        assert!(!apply_local(&data, &game, "offline-alex", "Alex").unwrap());
        // Плащ.
        let mut cape = std::io::Cursor::new(Vec::new());
        RgbaImage::new(64, 32)
            .write_to(&mut cape, image::ImageFormat::Png)
            .unwrap();
        assert!(
            set_cape(&data, "offline-steve", Some(&png(64, 64, false))).is_err(),
            "скин — не плащ"
        );
        set_cape(&data, "offline-steve", Some(cape.get_ref())).unwrap();
        apply_local(&data, &game, "offline-steve", "Steve").unwrap();
        let placed = game.join("CustomSkinLoader/LocalSkin/capes/Steve.png");
        assert!(placed.exists());
        set_cape(&data, "offline-steve", None).unwrap();
        apply_local(&data, &game, "offline-steve", "Steve").unwrap();
        assert!(!placed.exists(), "снятый плащ убирается из игры");
        set_worn(&data, "offline-steve", None).unwrap();
        assert_eq!(worn(&data, "offline-steve"), None);
        assert!(set_worn(&data, "x", Some("../evil")).is_err());
    }

    #[test]
    fn not_a_skin() {
        assert!(validate(b"nope").is_err());
        let e = validate(&png(128, 64, false)).unwrap_err().to_string();
        assert!(e.contains("128×64"), "{e}");
    }

    #[test]
    fn nicks_and_hosts() {
        assert!(check_nick("jeb_").is_ok());
        assert!(check_nick("a b").is_err());
        assert!(check_nick("../x").is_err());
        assert_eq!(
            https("http://textures.minecraft.net/texture/ab"),
            "https://textures.minecraft.net/texture/ab"
        );
        assert!(allowed("https://ely.by/storage/skins/x.png"));
        assert!(!allowed("https://evil.example/x.png"));
        assert!(!allowed("file:///C:/x.png"));
    }

    #[test]
    fn sensitive_tags() {
        assert!(sensitive(&["Adolf Hitler".into()]));
        assert!(sensitive(&["Порно".into()]));
        assert!(!sensitive(&["Knight".into(), "Рыцарь".into()]));
    }

    #[test]
    fn mojang_textures_parsed() {
        let v: Value = serde_json::from_str(
            r#"{"SKIN":{"url":"http://textures.minecraft.net/texture/ab","metadata":{"model":"slim"}},"CAPE":{"url":"http://textures.minecraft.net/texture/cd"}}"#,
        )
        .unwrap();
        let t = textures_json("jeb_", &v);
        assert_eq!(
            t.skin.as_deref(),
            Some("https://textures.minecraft.net/texture/ab")
        );
        assert!(t.slim);
        assert!(t.cape.is_some());
    }

    /// Настоящие сервисы — вручную: `cargo test -p majorlauncher-core skins -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn live_services() {
        let dl = Downloader::new(Default::default()).unwrap();
        let t = textures(&dl, "jeb_", NickSource::Mojang).await.unwrap();
        assert!(t.skin.is_some());
        let t = textures(&dl, "ErickSkrauch", NickSource::Ely)
            .await
            .unwrap();
        assert!(t.skin.is_some());
        for catalog in [Catalog::Ely, Catalog::MineSkin] {
            let q = CatalogQuery {
                catalog,
                search: "knight".into(),
                sort: String::new(),
                kind: String::new(),
                slim_only: false,
                page: String::new(),
                show_sensitive: false,
            };
            let p = super::catalog(&dl, &q).await.unwrap();
            assert!(!p.items.is_empty(), "{catalog:?}");
            assert!(p.next.is_some());
        }
    }
}
