//! Библиотека (разделы 5.4 и 5.18 ТЗ). Файл хранится один раз в `library/`,
//! в сборки попадает жёсткой ссылкой — на диске он один, сколько бы сборок
//! его ни использовали. Ресурспаки и шейдеры раскладываются перед запуском
//! по списку «где использовать»; моды из каталога кладутся в сборку сразу,
//! и дальше главная — папка модов сборки (выключенный или удалённый там мод
//! лаунчер обратно не вернёт).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::builds::now;
use crate::content::Folder;
use crate::error::{Error, IoContext, Result, json};
use crate::paths::DataDir;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub folder: Folder,
    pub file: String,
    pub size: u64,
    pub added: u64,
    /// Класть во все сборки.
    #[serde(default)]
    pub all_builds: bool,
    /// Или только в эти.
    #[serde(default)]
    pub builds: Vec<String>,
    /// Откуда файл: проект и версия на Modrinth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
}

/// Файл из каталога Modrinth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub version_number: String,
    pub sha1: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct LibraryFile {
    #[serde(default)]
    items: Vec<Item>,
}

pub struct Library {
    root: PathBuf,
}

fn check_folder(folder: Folder) -> Result<()> {
    match folder {
        Folder::ResourcePacks | Folder::ShaderPacks => Ok(()),
        _ => Err(Error::Other(
            "в библиотеке пока только ресурспаки и шейдеры".into(),
        )),
    }
}

impl Library {
    pub fn new(data: &DataDir) -> Self {
        Self {
            root: data.root().join("library"),
        }
    }

    fn path(&self, folder: Folder, file: &str) -> PathBuf {
        self.root.join(folder.dir()).join(file)
    }

    fn load(&self) -> Result<LibraryFile> {
        let meta = self.root.join("library.json");
        match std::fs::read(&meta) {
            Ok(bytes) => json(&bytes, "библиотека"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(LibraryFile::default()),
            Err(e) => Err(e).at(&meta),
        }
    }

    fn save(&self, file: &LibraryFile) -> Result<()> {
        std::fs::create_dir_all(&self.root).at(&self.root)?;
        let meta = self.root.join("library.json");
        let text = serde_json::to_vec_pretty(file).expect("библиотека сериализуется");
        std::fs::write(&meta, text).at(&meta)
    }

    pub fn list(&self) -> Result<Vec<Item>> {
        Ok(self.load()?.items)
    }

    /// Добавить файл (zip), выбранный игроком. Он копируется в библиотеку.
    pub fn add(&self, folder: Folder, src: &Path) -> Result<Item> {
        check_folder(folder)?;
        let file = src
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|n| n.to_lowercase().ends_with(".zip"))
            .ok_or_else(|| Error::Other("нужен файл .zip".into()))?
            .to_string();
        let mut lib = self.load()?;
        if lib
            .items
            .iter()
            .any(|i| i.folder == folder && i.file == file)
        {
            return Err(Error::Other(format!("«{file}» уже есть в библиотеке")));
        }
        let to = self.path(folder, &file);
        std::fs::create_dir_all(to.parent().expect("есть папка")).at(&to)?;
        std::fs::copy(src, &to).at(src)?;
        let item = Item {
            folder,
            size: std::fs::metadata(&to).at(&to)?.len(),
            file,
            added: now(),
            all_builds: true,
            builds: Vec::new(),
            source: None,
        };
        lib.items.push(item.clone());
        self.save(&lib)?;
        Ok(item)
    }

    /// Где использовать: во всех сборках или в выбранных.
    pub fn set(
        &self,
        folder: Folder,
        file: &str,
        all_builds: bool,
        builds: Vec<String>,
    ) -> Result<Item> {
        let mut lib = self.load()?;
        let item = lib
            .items
            .iter_mut()
            .find(|i| i.folder == folder && i.file == file)
            .ok_or_else(|| Error::Other(format!("«{file}» нет в библиотеке")))?;
        item.all_builds = all_builds;
        item.builds = builds;
        let out = item.clone();
        self.save(&lib)?;
        Ok(out)
    }

    /// Убрать из библиотеки и из сборок, куда его положила библиотека.
    pub fn remove(&self, folder: Folder, file: &str, game_dirs: &[PathBuf]) -> Result<()> {
        let mut lib = self.load()?;
        let path = self.path(folder, file);
        if let Ok(meta) = std::fs::metadata(&path) {
            for dir in game_dirs {
                let copy = dir.join(folder.dir()).join(file);
                if is_same(&meta, &copy) {
                    let _ = std::fs::remove_file(&copy);
                }
            }
            std::fs::remove_file(&path).at(&path)?;
        }
        lib.items
            .retain(|i| !(i.folder == folder && i.file == file));
        self.save(&lib)
    }

    /// Куда класть файл из каталога. Имя занято другим файлом — к имени
    /// добавляется версия.
    pub(crate) fn store_name(
        &self,
        folder: Folder,
        file: &str,
        sha1: &str,
        version_id: &str,
    ) -> String {
        let path = self.path(folder, file);
        let taken = path.exists()
            && !crate::verify::sha1_file(&path).is_ok_and(|h| h.eq_ignore_ascii_case(sha1));
        if !taken {
            return file.to_string();
        }
        match file.rsplit_once('.') {
            Some((stem, ext)) => format!("{stem}-{version_id}.{ext}"),
            None => format!("{file}-{version_id}"),
        }
    }

    pub(crate) fn store_path(&self, folder: Folder, file: &str) -> PathBuf {
        self.path(folder, file)
    }

    /// Записать скачанный из каталога файл (он уже лежит в библиотеке).
    pub(crate) fn register(&self, folder: Folder, file: &str, source: Source) -> Result<Item> {
        let mut lib = self.load()?;
        let path = self.path(folder, file);
        let size = std::fs::metadata(&path).at(&path)?.len();
        let item = match lib
            .items
            .iter_mut()
            .find(|i| i.folder == folder && i.file == file)
        {
            Some(item) => {
                item.size = size;
                item.source = Some(source);
                item.clone()
            }
            None => {
                let item = Item {
                    folder,
                    file: file.to_string(),
                    size,
                    added: now(),
                    all_builds: false,
                    builds: Vec::new(),
                    source: Some(source),
                };
                lib.items.push(item.clone());
                item
            }
        };
        self.save(&lib)?;
        Ok(item)
    }

    /// Ресурспак или шейдер из каталога — ещё и в эту сборку.
    pub(crate) fn use_in(&self, folder: Folder, file: &str, build: &str) -> Result<()> {
        let mut lib = self.load()?;
        if let Some(item) = lib
            .items
            .iter_mut()
            .find(|i| i.folder == folder && i.file == file)
            && !item.all_builds
            && !item.builds.iter().any(|b| b == build)
        {
            item.builds.push(build.to_string());
            self.save(&lib)?;
        }
        Ok(())
    }

    /// Положить файл библиотеки в папку сборки сейчас.
    pub(crate) fn link(&self, folder: Folder, file: &str, game_dir: &Path) -> Result<PathBuf> {
        let from = self.path(folder, file);
        let to = game_dir.join(folder.dir()).join(file);
        crate::install::link_file(&from, &to)?;
        Ok(to)
    }

    /// Перед запуском: положить в сборку ресурспаки и шейдеры, что ей положены.
    /// Моды так не раскладываются — их кладёт установка из каталога.
    pub fn link_into(&self, build: &str, game_dir: &Path) -> Result<()> {
        for item in self.load()?.items {
            if item.folder == Folder::Mods {
                continue;
            }
            if !item.all_builds && !item.builds.iter().any(|b| b == build) {
                continue;
            }
            let from = self.path(item.folder, &item.file);
            let to = game_dir.join(item.folder.dir()).join(&item.file);
            if to.exists() || !from.exists() {
                continue;
            }
            std::fs::create_dir_all(to.parent().expect("есть папка")).at(&to)?;
            // Жёсткая ссылка — тот же файл без копии. На другом диске так нельзя,
            // тогда обычная копия.
            if std::fs::hard_link(&from, &to).is_err() {
                std::fs::copy(&from, &to).at(&to)?;
            }
        }
        Ok(())
    }
}

/// Тот же файл, что в библиотеке: у жёсткой ссылки те же размер и время.
pub(crate) fn is_same(lib: &std::fs::Metadata, other: &Path) -> bool {
    std::fs::metadata(other)
        .is_ok_and(|m| m.len() == lib.len() && m.modified().ok() == lib.modified().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_go_into_builds_once() {
        let root = tempfile::tempdir().unwrap();
        let data = DataDir::new(root.path().join("data"));
        let lib = Library::new(&data);
        let src = root.path().join("Faithful.zip");
        std::fs::write(&src, "pack").unwrap();
        assert!(lib.add(Folder::Mods, &src).is_err());
        lib.add(Folder::ResourcePacks, &src).unwrap();
        assert!(
            lib.add(Folder::ResourcePacks, &src).is_err(),
            "второй раз не добавляется"
        );

        let a = root.path().join("a");
        let b = root.path().join("b");
        lib.link_into("a", &a).unwrap();
        assert!(a.join("resourcepacks/Faithful.zip").exists());

        // Только для сборки «b».
        lib.set(
            Folder::ResourcePacks,
            "Faithful.zip",
            false,
            vec!["b".into()],
        )
        .unwrap();
        lib.link_into("b", &b).unwrap();
        assert!(b.join("resourcepacks/Faithful.zip").exists());

        // Свой файл с тем же именем в сборке не трогаем.
        let c = root.path().join("c");
        std::fs::create_dir_all(c.join("resourcepacks")).unwrap();
        std::fs::write(c.join("resourcepacks/Faithful.zip"), "свой пак, другой").unwrap();

        lib.remove(
            Folder::ResourcePacks,
            "Faithful.zip",
            &[a.clone(), b.clone(), c.clone()],
        )
        .unwrap();
        assert!(!a.join("resourcepacks/Faithful.zip").exists());
        assert!(!b.join("resourcepacks/Faithful.zip").exists());
        assert!(c.join("resourcepacks/Faithful.zip").exists());
        assert!(lib.list().unwrap().is_empty());
    }
}
