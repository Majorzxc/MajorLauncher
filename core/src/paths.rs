//! Раскладка папки данных. Общие файлы игры (версии, библиотеки, ассеты, Java)
//! лежат один раз на все сборки, у каждой сборки — только своя папка игры.
//!
//! ```text
//! <данные>/
//!   versions/<id>/<id>.json, <id>.jar, natives/
//!   libraries/...
//!   assets/indexes, objects, virtual, log_configs
//!   java/<компонент>/...   java/manifests/<компонент>.json
//!   instances/<сборка>/    — папка игры: миры, моды, настройки
//!   trash/, archive/       — корзина и архив сборок
//!   cache/                 — списки версий и кэш проверенных файлов
//! ```

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    /// Путь приводится к абсолютному: игра запускается из папки сборки,
    /// и относительные пути в её аргументах указывали бы не туда.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let root = std::path::absolute(&root).unwrap_or(root);
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn version_dir(&self, id: &str) -> PathBuf {
        self.root.join("versions").join(id)
    }

    pub fn version_json(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.json"))
    }

    pub fn client_jar(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.jar"))
    }

    pub fn natives_dir(&self, id: &str) -> PathBuf {
        self.version_dir(id).join("natives")
    }

    pub fn libraries(&self) -> PathBuf {
        self.root.join("libraries")
    }

    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }

    pub fn asset_index(&self, id: &str) -> PathBuf {
        self.assets().join("indexes").join(format!("{id}.json"))
    }

    pub fn asset_object(&self, hash: &str) -> PathBuf {
        self.assets().join("objects").join(&hash[..2]).join(hash)
    }

    pub fn virtual_assets(&self, index_id: &str) -> PathBuf {
        self.assets().join("virtual").join(index_id)
    }

    pub fn log_config(&self, id: &str) -> PathBuf {
        self.assets().join("log_configs").join(id)
    }

    pub fn java_dir(&self, component: &str) -> PathBuf {
        self.root.join("java").join(component)
    }

    pub fn java_manifest(&self, component: &str) -> PathBuf {
        self.root
            .join("java")
            .join("manifests")
            .join(format!("{component}.json"))
    }

    pub fn instances(&self) -> PathBuf {
        self.root.join("instances")
    }

    /// Корзина: удалённые сборки целиком, 30 дней (раздел 6.2 ТЗ).
    pub fn trash(&self) -> PathBuf {
        self.root.join("trash")
    }

    /// Архив: сборки, упакованные в zip, и их описания.
    pub fn archive(&self) -> PathBuf {
        self.root.join("archive")
    }

    pub fn instance(&self, name: &str) -> PathBuf {
        self.root.join("instances").join(name)
    }

    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }
}
