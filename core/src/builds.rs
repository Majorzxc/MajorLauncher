//! Сборки (разделы 5.5 и 6.2 ТЗ). Сборка — папка `instances/<id>/`: это
//! папка игры (миры, моды, настройки), а описание сборки лежит в ней же,
//! в `.majorlauncher.json`, — поэтому переносится вместе с папкой.
//!
//! Удалённые сборки живут в `trash/` 30 дней, архивные — в `archive/`
//! упакованными в zip, рядом с описанием (чтобы показывать их в списке,
//! не распаковывая).

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{Error, IoContext, Result, json};
use crate::paths::DataDir;

const META: &str = ".majorlauncher.json";
const COVER: &str = ".cover.png";
/// Сколько дней сборка лежит в корзине, прежде чем удалиться насовсем.
pub const TRASH_DAYS: u64 = 30;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    /// Без загрузчика модов. Fabric, Forge и NeoForge — этап 4.
    #[default]
    Vanilla,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instance {
    /// Имя папки: латиница, цифры и «-», без кириллицы — старые версии Forge
    /// падают, если в пути есть русские буквы.
    pub id: String,
    pub name: String,
    /// Версия игры.
    pub version: String,
    #[serde(default)]
    pub loader: Loader,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    /// Секунды с 1970 года.
    pub created: u64,
    #[serde(default)]
    pub last_played: Option<u64>,
    /// Время в игре, секунды.
    #[serde(default)]
    pub playtime: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Active,
    Archived,
}

/// Сборка для списка в окне.
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    #[serde(flatten)]
    pub instance: Instance,
    pub state: State,
    pub has_cover: bool,
    /// Размер архива — у архивных сборок он известен сразу.
    pub archive_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashEntry {
    /// Имя папки в корзине.
    pub trash_id: String,
    pub instance: Instance,
    pub deleted_at: u64,
    /// Сколько места занимает, байт (считается при показе корзины).
    #[serde(default)]
    pub size: u64,
}

/// Что поменять в сборке. `None` — не трогать.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Patch {
    pub name: Option<String>,
    /// `Some(None)` — убрать из группы.
    #[serde(default, with = "double_option")]
    pub group: Option<Option<String>>,
    pub pinned: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ArchiveMeta {
    instance: Instance,
    archived_at: u64,
    size: u64,
}

/// Порядок групп, в том числе пустых (пустая группа нигде больше не видна).
#[derive(Debug, Default, Serialize, Deserialize)]
struct GroupsFile {
    #[serde(default)]
    groups: Vec<String>,
}

pub struct Builds {
    data: DataDir,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Builds {
    pub fn new(data: DataDir) -> Self {
        Self { data }
    }

    /// Все сборки: рабочие и архивные. Папки, созданные `mlcli` без описания,
    /// подхватываются как сборки, если такая версия игры уже скачана.
    pub fn list(&self) -> Result<Vec<Entry>> {
        let mut out = Vec::new();
        for dir in subdirs(&self.data.instances())? {
            let Some(id) = dir.file_name().and_then(|n| n.to_str()).map(str::to_string) else {
                continue;
            };
            let instance = match read_meta(&dir)? {
                Some(i) => i,
                None => match self.adopt(&id, &dir)? {
                    Some(i) => i,
                    None => continue,
                },
            };
            out.push(Entry {
                has_cover: dir.join(COVER).exists(),
                instance,
                state: State::Active,
                archive_size: None,
            });
        }
        for meta in files_with_ext(&self.data.archive(), "json")? {
            let bytes = std::fs::read(&meta).at(&meta)?;
            let m: ArchiveMeta = json(&bytes, "описание архивной сборки")?;
            out.push(Entry {
                has_cover: meta.with_extension("png").exists(),
                instance: m.instance,
                state: State::Archived,
                archive_size: Some(m.size),
            });
        }
        Ok(out)
    }

    /// Папка без описания, но с уже скачанной версией (так их создаёт `mlcli`).
    fn adopt(&self, id: &str, dir: &Path) -> Result<Option<Instance>> {
        if !self.data.version_json(id).exists() {
            return Ok(None);
        }
        let created = std::fs::metadata(dir)
            .and_then(|m| m.created())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or_else(now, |d| d.as_secs());
        let instance = Instance {
            id: id.to_string(),
            name: id.to_string(),
            version: id.to_string(),
            loader: Loader::Vanilla,
            group: None,
            pinned: false,
            created,
            last_played: None,
            playtime: 0,
        };
        write_meta(dir, &instance)?;
        Ok(Some(instance))
    }

    pub fn get(&self, id: &str) -> Result<Instance> {
        read_meta(&self.data.instance(id))?.ok_or_else(|| Error::BuildNotFound(id.to_string()))
    }

    pub fn create(&self, name: &str, version: &str, group: Option<&str>) -> Result<Instance> {
        let name = clean_name(name)?;
        if version.trim().is_empty() {
            return Err(Error::Other("не выбрана версия игры".into()));
        }
        let id = self.free_id(&slug(&name))?;
        let instance = Instance {
            id: id.clone(),
            name,
            version: version.trim().to_string(),
            loader: Loader::Vanilla,
            group: clean_group(group),
            pinned: false,
            created: now(),
            last_played: None,
            playtime: 0,
        };
        let dir = self.data.instance(&id);
        std::fs::create_dir_all(&dir).at(&dir)?;
        write_meta(&dir, &instance)?;
        self.remember_group(instance.group.as_deref())?;
        Ok(instance)
    }

    pub fn update(&self, id: &str, patch: Patch) -> Result<Instance> {
        let mut instance = self.get(id)?;
        if let Some(name) = patch.name {
            instance.name = clean_name(&name)?;
        }
        if let Some(group) = patch.group {
            instance.group = clean_group(group.as_deref());
        }
        if let Some(pinned) = patch.pinned {
            instance.pinned = pinned;
        }
        write_meta(&self.data.instance(id), &instance)?;
        self.remember_group(instance.group.as_deref())?;
        Ok(instance)
    }

    /// Копия сборки со всеми мирами и настройками. Время в игре не копируется.
    pub fn duplicate(&self, id: &str) -> Result<Instance> {
        let source = self.get(id)?;
        let name = format!("{} (копия)", source.name);
        let new_id = self.free_id(&slug(&name))?;
        let dir = self.data.instance(&new_id);
        copy_dir(&self.data.instance(id), &dir)?;
        let copy = Instance {
            id: new_id,
            name,
            created: now(),
            last_played: None,
            playtime: 0,
            pinned: false,
            ..source
        };
        write_meta(&dir, &copy)?;
        Ok(copy)
    }

    /// Переместить в корзину. Папка переезжает целиком — мгновенно, если
    /// корзина на том же диске (она всегда в той же папке данных).
    pub fn trash(&self, id: &str) -> Result<()> {
        let instance = self.get(id)?;
        let deleted_at = now();
        let trash_id = format!("{id}__{deleted_at}");
        let target = self.data.trash().join(&trash_id);
        std::fs::create_dir_all(self.data.trash()).at(&self.data.trash())?;
        let source = self.data.instance(id);
        std::fs::rename(&source, &target).at(&source)?;
        let entry = TrashEntry {
            trash_id,
            instance,
            deleted_at,
            size: 0,
        };
        let path = target.join(".trash.json");
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&entry).expect("сериализуется"),
        )
        .at(&path)
    }

    /// Содержимое корзины; заодно удаляет то, что пролежало дольше срока.
    pub fn trash_list(&self) -> Result<Vec<TrashEntry>> {
        self.purge_expired(TRASH_DAYS)?;
        let mut out = Vec::new();
        for dir in subdirs(&self.data.trash())? {
            if let Some(mut entry) = read_trash_entry(&dir)? {
                entry.size = dir_size(&dir);
                out.push(entry);
            }
        }
        out.sort_by_key(|e| std::cmp::Reverse(e.deleted_at));
        Ok(out)
    }

    /// Вернуть из корзины. Если имя папки за это время заняли — берём свободное.
    pub fn restore(&self, trash_id: &str) -> Result<Instance> {
        let dir = self.data.trash().join(trash_id);
        let entry = read_trash_entry(&dir)?.ok_or_else(|| Error::BuildNotFound(trash_id.into()))?;
        let id = self.free_id(&entry.instance.id)?;
        let target = self.data.instance(&id);
        std::fs::create_dir_all(self.data.instances()).at(&self.data.instances())?;
        std::fs::rename(&dir, &target).at(&dir)?;
        let _ = std::fs::remove_file(target.join(".trash.json"));
        let instance = Instance {
            id,
            ..entry.instance
        };
        write_meta(&target, &instance)?;
        Ok(instance)
    }

    /// Удалить из корзины насовсем.
    pub fn purge(&self, trash_id: &str) -> Result<()> {
        // Только имя папки, без путей: из окна не должно прийти «../».
        if trash_id.contains(['/', '\\']) || trash_id.contains("..") {
            return Err(Error::BuildNotFound(trash_id.into()));
        }
        let dir = self.data.trash().join(trash_id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).at(&dir)?;
        }
        Ok(())
    }

    pub fn purge_expired(&self, days: u64) -> Result<usize> {
        let limit = now().saturating_sub(days * 24 * 60 * 60);
        let mut removed = 0;
        for dir in subdirs(&self.data.trash())? {
            if let Some(entry) = read_trash_entry(&dir)?
                && entry.deleted_at < limit
            {
                std::fs::remove_dir_all(&dir).at(&dir)?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// Упаковать в архив и убрать из рабочих сборок.
    pub fn archive(&self, id: &str) -> Result<()> {
        let instance = self.get(id)?;
        let dir = self.data.instance(id);
        let archive = self.data.archive();
        std::fs::create_dir_all(&archive).at(&archive)?;

        let zip_path = archive.join(format!("{id}.zip"));
        let tmp = archive.join(format!("{id}.zip.tmp"));
        zip_dir(&dir, &tmp)?;
        std::fs::rename(&tmp, &zip_path).at(&zip_path)?;
        if dir.join(COVER).exists() {
            let cover = archive.join(format!("{id}.png"));
            std::fs::copy(dir.join(COVER), &cover).at(&cover)?;
        }
        let size = std::fs::metadata(&zip_path).at(&zip_path)?.len();
        let meta = ArchiveMeta {
            instance,
            archived_at: now(),
            size,
        };
        let meta_path = archive.join(format!("{id}.json"));
        std::fs::write(
            &meta_path,
            serde_json::to_vec_pretty(&meta).expect("сериализуется"),
        )
        .at(&meta_path)?;
        std::fs::remove_dir_all(&dir).at(&dir)
    }

    /// Распаковать из архива обратно в рабочие сборки.
    pub fn unarchive(&self, id: &str) -> Result<Instance> {
        let archive = self.data.archive();
        let meta_path = archive.join(format!("{id}.json"));
        let bytes = std::fs::read(&meta_path).map_err(|_| Error::BuildNotFound(id.into()))?;
        let meta: ArchiveMeta = json(&bytes, "описание архивной сборки")?;
        // Своё имя занимает только собственный архив — его и распаковываем.
        // Другое берём, лишь если такую папку за это время создали заново.
        let new_id = if self.data.instance(id).exists() {
            self.free_id(id)?
        } else {
            id.to_string()
        };
        let dir = self.data.instance(&new_id);
        let zip_path = archive.join(format!("{id}.zip"));
        unzip(&zip_path, &dir)?;
        let instance = Instance {
            id: new_id,
            ..meta.instance
        };
        write_meta(&dir, &instance)?;
        for path in [zip_path, meta_path, archive.join(format!("{id}.png"))] {
            let _ = std::fs::remove_file(path);
        }
        Ok(instance)
    }

    /// Размер папки сборки на диске.
    pub fn size(&self, id: &str) -> Result<u64> {
        Ok(dir_size(&self.data.instance(id)))
    }

    /// Обложка: уже уменьшенная окном картинка PNG (256×256).
    pub fn set_cover(&self, id: &str, png: &[u8]) -> Result<()> {
        const MAX: usize = 2 * 1024 * 1024;
        if !png.starts_with(b"\x89PNG") || png.len() > MAX {
            return Err(Error::Other(
                "обложка должна быть картинкой PNG до 2 МБ".into(),
            ));
        }
        self.get(id)?;
        let path = self.data.instance(id).join(COVER);
        std::fs::write(&path, png).at(&path)
    }

    pub fn cover(&self, id: &str, state: State) -> Result<Option<Vec<u8>>> {
        let path = match state {
            State::Active => self.data.instance(id).join(COVER),
            State::Archived => self.data.archive().join(format!("{id}.png")),
        };
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).at(&path),
        }
    }

    pub fn remove_cover(&self, id: &str) -> Result<()> {
        let path = self.data.instance(id).join(COVER);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e).at(&path),
        }
    }

    /// Группы: сохранённый порядок плюс те, что встречаются у сборок.
    pub fn groups(&self) -> Result<Vec<String>> {
        let mut groups = self.read_groups()?.groups;
        let used: BTreeSet<String> = self
            .list()?
            .into_iter()
            .filter_map(|e| e.instance.group)
            .collect();
        for g in used {
            if !groups.contains(&g) {
                groups.push(g);
            }
        }
        Ok(groups)
    }

    pub fn add_group(&self, name: &str) -> Result<()> {
        let name =
            clean_group(Some(name)).ok_or_else(|| Error::Other("пустое название группы".into()))?;
        self.remember_group(Some(&name))
    }

    /// Удалить группу: сборки остаются, просто без группы.
    pub fn delete_group(&self, name: &str) -> Result<()> {
        for entry in self.list()? {
            if entry.state == State::Active && entry.instance.group.as_deref() == Some(name) {
                self.update(
                    &entry.instance.id,
                    Patch {
                        group: Some(None),
                        ..Patch::default()
                    },
                )?;
            }
        }
        let mut file = self.read_groups()?;
        file.groups.retain(|g| g != name);
        self.write_groups(&file)
    }

    fn remember_group(&self, group: Option<&str>) -> Result<()> {
        let Some(group) = group else { return Ok(()) };
        let mut file = self.read_groups()?;
        if !file.groups.iter().any(|g| g == group) {
            file.groups.push(group.to_string());
            self.write_groups(&file)?;
        }
        Ok(())
    }

    fn read_groups(&self) -> Result<GroupsFile> {
        let path = self.data.root().join("groups.json");
        match std::fs::read(&path) {
            Ok(bytes) => json(&bytes, "список групп"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(GroupsFile::default()),
            Err(e) => Err(e).at(&path),
        }
    }

    fn write_groups(&self, file: &GroupsFile) -> Result<()> {
        let path = self.data.root().join("groups.json");
        std::fs::create_dir_all(self.data.root()).at(self.data.root())?;
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(file).expect("сериализуется"),
        )
        .at(&path)
    }

    /// Свободное имя папки: `base`, `base-2`, `base-3`…
    fn free_id(&self, base: &str) -> Result<String> {
        for n in 1.. {
            let id = if n == 1 {
                base.to_string()
            } else {
                format!("{base}-{n}")
            };
            let taken = self.data.instance(&id).exists()
                || self.data.archive().join(format!("{id}.zip")).exists();
            if !taken {
                return Ok(id);
            }
        }
        unreachable!()
    }
}

fn read_meta(dir: &Path) -> Result<Option<Instance>> {
    let path = dir.join(META);
    match std::fs::read(&path) {
        Ok(bytes) => json(&bytes, &format!("описание сборки {}", dir.display())).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).at(&path),
    }
}

fn write_meta(dir: &Path, instance: &Instance) -> Result<()> {
    let path = dir.join(META);
    let tmp = dir.join(".majorlauncher.json.tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(instance).expect("сериализуется"),
    )
    .at(&tmp)?;
    std::fs::rename(&tmp, &path).at(&path)
}

fn read_trash_entry(dir: &Path) -> Result<Option<TrashEntry>> {
    let path = dir.join(".trash.json");
    match std::fs::read(&path) {
        Ok(bytes) => json(&bytes, "описание сборки в корзине").map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).at(&path),
    }
}

fn subdirs(dir: &Path) -> Result<Vec<PathBuf>> {
    let read = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).at(dir),
    };
    let mut dirs: Vec<PathBuf> = read
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect();
    dirs.sort();
    Ok(dirs)
}

fn files_with_ext(dir: &Path, ext: &str) -> Result<Vec<PathBuf>> {
    let read = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).at(dir),
    };
    Ok(read
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == ext))
        .collect())
}

fn clean_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(Error::Other("название сборки — от 1 до 64 символов".into()));
    }
    Ok(name.to_string())
}

fn clean_group(group: Option<&str>) -> Option<String> {
    group
        .map(str::trim)
        .filter(|g| !g.is_empty())
        .map(|g| g.chars().take(32).collect())
}

/// Имя папки из названия сборки: латиница, цифры и «-». Кириллица
/// переводится в латиницу («Выживание» → `vyzhivanie`).
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        let part = match c {
            'a'..='z' | '0'..='9' => {
                out.push(c);
                continue;
            }
            'а' => "a",
            'б' => "b",
            'в' => "v",
            'г' => "g",
            'д' => "d",
            'е' | 'ё' | 'э' => "e",
            'ж' => "zh",
            'з' => "z",
            'и' => "i",
            'й' | 'ы' => "y",
            'к' => "k",
            'л' => "l",
            'м' => "m",
            'н' => "n",
            'о' => "o",
            'п' => "p",
            'р' => "r",
            'с' => "s",
            'т' => "t",
            'у' => "u",
            'ф' => "f",
            'х' => "h",
            'ц' => "ts",
            'ч' => "ch",
            'ш' => "sh",
            'щ' => "sch",
            'ю' => "yu",
            'я' => "ya",
            'ъ' | 'ь' => "",
            _ => "-",
        };
        out.push_str(part);
    }
    let mut slug = String::new();
    for part in out.split('-').filter(|p| !p.is_empty()) {
        if !slug.is_empty() {
            slug.push('-');
        }
        slug.push_str(part);
    }
    let slug: String = slug.chars().take(40).collect();
    let slug = slug.trim_end_matches('-').to_string();
    if slug.is_empty() {
        "build".into()
    } else {
        slug
    }
}

fn dir_size(dir: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in read.flatten() {
            match entry.file_type() {
                Ok(t) if t.is_dir() => stack.push(entry.path()),
                Ok(_) => total += entry.metadata().map_or(0, |m| m.len()),
                Err(_) => {}
            }
        }
    }
    total
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to).at(to)?;
    for entry in std::fs::read_dir(from).at(from)?.flatten() {
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if entry.file_type().at(&src)?.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst).at(&dst)?;
        }
    }
    Ok(())
}

fn zip_dir(dir: &Path, out: &Path) -> Result<()> {
    let file = std::fs::File::create(out).at(out)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(true);
    let zip_err = |source| Error::Zip {
        path: out.to_path_buf(),
        source,
    };
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).at(&d)?.flatten() {
            let path = entry.path();
            let rel = path
                .strip_prefix(dir)
                .expect("путь внутри папки сборки")
                .to_string_lossy()
                .replace('\\', "/");
            if entry.file_type().at(&path)?.is_dir() {
                zip.add_directory(format!("{rel}/"), options)
                    .map_err(zip_err)?;
                stack.push(path);
            } else {
                zip.start_file(rel, options).map_err(zip_err)?;
                let mut src = std::fs::File::open(&path).at(&path)?;
                std::io::copy(&mut src, &mut zip).at(&path)?;
            }
        }
    }
    zip.finish().map_err(zip_err)?.flush().at(out)
}

fn unzip(zip_path: &Path, to: &Path) -> Result<()> {
    let file = std::fs::File::open(zip_path).at(zip_path)?;
    let zip_err = |source| Error::Zip {
        path: zip_path.to_path_buf(),
        source,
    };
    let mut archive = zip::ZipArchive::new(file).map_err(zip_err)?;
    std::fs::create_dir_all(to).at(to)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(zip_err)?;
        // enclosed_name защищает от путей вида «../../» внутри архива.
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let out = to.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).at(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).at(parent)?;
        }
        let mut target = std::fs::File::create(&out).at(&out)?;
        std::io::copy(&mut entry, &mut target).at(&out)?;
    }
    Ok(())
}

/// Отличить «поле не передано» от «передано null» (убрать из группы).
mod double_option {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        Option::<T>::deserialize(d).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn builds() -> (tempfile::TempDir, Builds) {
        let dir = tempfile::tempdir().unwrap();
        let builds = Builds::new(DataDir::new(dir.path()));
        (dir, builds)
    }

    #[test]
    fn slugs_are_latin_and_safe() {
        assert_eq!(slug("Выживание с друзьями"), "vyzhivanie-s-druzyami");
        assert_eq!(slug("Техно 1.20.1"), "tehno-1-20-1");
        assert_eq!(slug("   !!!   "), "build");
        assert_eq!(slug("Ёжик"), "ezhik");
    }

    #[test]
    fn create_update_and_unique_ids() {
        let (_d, b) = builds();
        let a = b.create("Выживание", "1.21.1", Some("Серверы")).unwrap();
        let a2 = b.create("Выживание", "1.21.1", None).unwrap();
        assert_eq!(a.id, "vyzhivanie");
        assert_eq!(a2.id, "vyzhivanie-2");
        assert!(b.create("  ", "1.21.1", None).is_err());

        let u = b
            .update(
                &a.id,
                Patch {
                    name: Some("Новое имя".into()),
                    pinned: Some(true),
                    group: Some(None),
                },
            )
            .unwrap();
        assert_eq!(u.name, "Новое имя");
        assert!(u.pinned);
        assert_eq!(u.group, None);
        assert_eq!(b.get(&a.id).unwrap(), u, "изменения сохранились");
        assert!(
            b.groups().unwrap().contains(&"Серверы".to_string()),
            "пустая группа осталась"
        );
    }

    #[test]
    fn trash_restore_and_purge() {
        let (_d, b) = builds();
        let a = b.create("Тест", "1.21.1", None).unwrap();
        std::fs::write(b.data.instance(&a.id).join("options.txt"), b"fov:90").unwrap();
        b.trash(&a.id).unwrap();
        assert!(b.list().unwrap().is_empty());

        let trash = b.trash_list().unwrap();
        assert_eq!(trash.len(), 1);
        let restored = b.restore(&trash[0].trash_id).unwrap();
        assert_eq!(restored.name, "Тест");
        let options = b.data.instance(&restored.id).join("options.txt");
        assert_eq!(
            std::fs::read(options).unwrap(),
            b"fov:90",
            "файлы вернулись"
        );

        b.trash(&restored.id).unwrap();
        let id = b.trash_list().unwrap()[0].trash_id.clone();
        assert!(b.purge("../escape").is_err(), "пути из окна не принимаются");
        b.purge(&id).unwrap();
        assert!(b.trash_list().unwrap().is_empty());
    }

    #[test]
    fn archive_round_trip() {
        let (_d, b) = builds();
        let a = b.create("Архив", "1.12.2", None).unwrap();
        let saves = b.data.instance(&a.id).join("saves").join("Мир");
        std::fs::create_dir_all(&saves).unwrap();
        std::fs::write(saves.join("level.dat"), vec![7u8; 4096]).unwrap();

        b.archive(&a.id).unwrap();
        let list = b.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].state, State::Archived);
        assert!(list[0].archive_size.unwrap() < 4096, "сжалось");
        assert!(!b.data.instance(&a.id).exists());

        let back = b.unarchive(&a.id).unwrap();
        assert_eq!(back.id, a.id, "вернулась под своим именем");
        let level = b
            .data
            .instance(&back.id)
            .join("saves")
            .join("Мир")
            .join("level.dat");
        assert_eq!(std::fs::read(level).unwrap(), vec![7u8; 4096]);
        assert_eq!(b.list().unwrap()[0].state, State::Active);
    }

    #[test]
    fn duplicate_and_adopt() {
        let (_d, b) = builds();
        let a = b.create("Сборка", "1.21.1", None).unwrap();
        let c = b.duplicate(&a.id).unwrap();
        assert_eq!(c.name, "Сборка (копия)");
        assert_ne!(c.id, a.id);

        // Папка от mlcli без описания: подхватывается, если версия скачана.
        std::fs::create_dir_all(b.data.instance("26.3")).unwrap();
        std::fs::create_dir_all(b.data.version_dir("26.3")).unwrap();
        std::fs::write(b.data.version_json("26.3"), b"{}").unwrap();
        std::fs::create_dir_all(b.data.instance("unknown")).unwrap();
        let ids: Vec<String> = b
            .list()
            .unwrap()
            .into_iter()
            .map(|e| e.instance.id)
            .collect();
        assert!(ids.contains(&"26.3".to_string()));
        assert!(!ids.contains(&"unknown".to_string()));
    }
}
