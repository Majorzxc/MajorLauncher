//! Фоновые обновления (раздел 5.13 ТЗ): игрок сам выбирает, что обновлять в
//! фоне. Проверка только спрашивает Modrinth (как вкладка «Моды») и ничего не
//! меняет; автообновление модов и сборок из каталога — по отдельным
//! переключателям, всегда со снапшотом, закреплённые моды не трогаются.

use serde::{Deserialize, Serialize};

use crate::builds::{Builds, Loader, State};
use crate::download::Downloader;
use crate::error::Result;
use crate::mrpack::{self, PackUpdate};
use crate::paths::DataDir;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateSettings {
    /// Проверять обновления модов и сборок из каталога в фоне.
    #[serde(default = "yes")]
    pub check: bool,
    /// Ставить обновления модов самому (кроме закреплённых).
    #[serde(default)]
    pub auto_mods: bool,
    /// Обновлять сборки из каталога самому.
    #[serde(default)]
    pub auto_packs: bool,
}

fn yes() -> bool {
    true
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self {
            check: true,
            auto_mods: false,
            auto_packs: false,
        }
    }
}

/// Что нового у сборки.
#[derive(Debug, Clone, Serialize)]
pub struct BuildUpdates {
    pub build: String,
    pub name: String,
    /// Модов с обновлением (включённых и не закреплённых).
    pub mods: usize,
    /// Названия этих модов — для уведомления.
    pub titles: Vec<String>,
    /// Новая версия самой сборки из каталога.
    pub pack: Option<PackUpdate>,
}

/// Проверить одну сборку: обновления модов и новая версия пакета.
pub async fn check_build(dl: &Downloader, data: &DataDir, build: &str) -> Result<BuildUpdates> {
    let instance = Builds::new(data.clone()).get(build)?;
    let (mods, titles) = if instance.loader == Loader::Vanilla {
        (0, Vec::new())
    } else {
        let report = crate::mods::report(dl, data, build, true).await?;
        let list: Vec<String> = report
            .mods
            .iter()
            .filter(|m| m.enabled && !m.pinned && m.update.is_some())
            .map(|m| m.name.clone())
            .collect();
        (list.len(), list)
    };
    let pack = mrpack::check_update(dl, data, build).await.unwrap_or(None);
    Ok(BuildUpdates {
        build: build.to_string(),
        name: instance.name,
        mods,
        titles,
        pack,
    })
}

/// Рабочие сборки, у которых есть что проверять (не «Быстрая игра», не архив).
pub fn builds_to_check(data: &DataDir) -> Vec<String> {
    Builds::new(data.clone())
        .list()
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.state == State::Active && !e.instance.quick)
        .filter(|e| {
            e.instance.loader != Loader::Vanilla
                || mrpack::origin(data, &e.instance.id).is_some_and(|o| o.project_id.is_some())
        })
        .map(|e| e.instance.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_only_check() {
        let d = UpdateSettings::default();
        assert!(d.check && !d.auto_mods && !d.auto_packs);
        let old: UpdateSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(old, d);
    }
}
