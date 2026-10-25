//! Настройки лаунчера, которые нужны ещё до выбора папки данных: где эта папка
//! и как выглядит окно. Крошечный файл в папке настроек Windows (её выбирает
//! окно лаунчера), всё тяжёлое — в папке данных.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::datadir::{self, Located};
use crate::error::{Error, IoContext, Result, json};

/// Тема окна. Готовых тем две — чёрная и белая (раздел 6.0 ТЗ), остальное
/// игрок настраивает своими цветами.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Dark,
    Light,
    /// Как в Windows: тёмная или светлая.
    #[default]
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Appearance {
    #[serde(default)]
    pub mode: ThemeMode,
    /// Свой акцентный цвет `#rrggbb`; `None` — чёрно-белый акцент темы.
    #[serde(default)]
    pub accent: Option<String>,
    /// Свой цвет фона `#rrggbb`; остальные оттенки окно вычисляет из него.
    #[serde(default)]
    pub background: Option<String>,
    /// Картинка фона: файл `background.jpg` рядом с настройками.
    #[serde(default)]
    pub image: bool,
    /// Насколько приглушить картинку цветом фона, %: текст поверх неё
    /// должен читаться.
    #[serde(default = "default_dim")]
    pub image_dim: u8,
}

fn default_dim() -> u8 {
    60
}

/// Сильнее 90 % картинку уже не видно.
pub const MAX_DIM: u8 = 90;

impl Default for Appearance {
    fn default() -> Self {
        Self {
            mode: ThemeMode::default(),
            accent: None,
            background: None,
            image: false,
            image_dim: default_dim(),
        }
    }
}

/// Фон готовых тем. Те же значения — в app/src/lib/theme.svelte.ts.
pub const DARK_BACKGROUND: &str = "#0e0e10";
pub const LIGHT_BACKGROUND: &str = "#f4f4f5";

impl Appearance {
    /// Цвет фона окна `[r, g, b]`. `system_dark` — включена ли тёмная тема Windows.
    /// Нужен, чтобы покрасить окно ещё до загрузки интерфейса — без белой вспышки.
    pub fn background_rgb(&self, system_dark: bool) -> [u8; 3] {
        let dark = match self.mode {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => system_dark,
        };
        let base = if dark {
            DARK_BACKGROUND
        } else {
            LIGHT_BACKGROUND
        };
        let hex = self.background.as_deref().unwrap_or(base);
        let n = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
        [(n >> 16) as u8, (n >> 8) as u8, n as u8]
    }

    fn validate(&self) -> Result<()> {
        if self.image_dim > MAX_DIM {
            return Err(Error::Other(format!(
                "приглушение картинки — от 0 до {MAX_DIM} %"
            )));
        }
        for color in [&self.accent, &self.background].into_iter().flatten() {
            let ok = color.len() == 7
                && color.starts_with('#')
                && color[1..].chars().all(|c| c.is_ascii_hexdigit());
            if !ok {
                return Err(Error::Other(format!(
                    "цвет «{color}» не подходит: нужен вид #rrggbb"
                )));
            }
        }
        Ok(())
    }
}

/// Что делать с окном лаунчера, пока идёт игра (раздел 5.16 ТЗ).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnLaunch {
    /// Закрыть окно: остаётся только маленький фоновый процесс (принцип 4).
    #[default]
    Close,
    Minimize,
    Keep,
}

/// Общие настройки запуска игры. Память и аргументы JVM сборка может переопределить.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameSettings {
    #[serde(default)]
    pub on_launch: OnLaunch,
    /// Память для игры, МБ; `None` — подобрать по объёму ОЗУ.
    #[serde(default)]
    pub memory_mb: Option<u32>,
    /// Дополнительные аргументы JVM через пробел.
    #[serde(default)]
    pub jvm_args: String,
}

/// Допустимая память для игры, МБ.
pub const MEMORY_RANGE: std::ops::RangeInclusive<u32> = 512..=65536;

pub fn check_memory(mb: Option<u32>) -> Result<()> {
    match mb {
        Some(mb) if !MEMORY_RANGE.contains(&mb) => Err(Error::Other(format!(
            "память {mb} МБ не подходит: нужно от {} до {} МБ",
            MEMORY_RANGE.start(),
            MEMORY_RANGE.end()
        ))),
        _ => Ok(()),
    }
}

/// Каталог Modrinth (раздел 5.4 ТЗ).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogSettings {
    /// Ставить обязательные зависимости без окна подтверждения.
    #[serde(default)]
    pub auto_deps: bool,
    /// Переводить описания с Modrinth на русский (через Google). Выключено,
    /// пока игрок сам не включит в настройках: текст уходит в чужой сервис.
    #[serde(default)]
    pub translate: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// Папка данных; `None` — ещё не выбрана (первый запуск).
    #[serde(default)]
    pub data_dir: Option<PathBuf>,
    /// Серийный номер тома папки данных: по нему лаунчер находит её, если
    /// диск сменил букву.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_volume: Option<u32>,
    #[serde(default)]
    pub appearance: Appearance,
    #[serde(default)]
    pub game: GameSettings,
    #[serde(default)]
    pub catalog: CatalogSettings,
    /// Что делает кнопка «Оптимизировать» (раздел 5.7 ТЗ).
    #[serde(default)]
    pub optimize: crate::optimize::OptimizeSettings,
    /// Фоновые обновления (раздел 5.13 ТЗ).
    #[serde(default)]
    pub updates: crate::updates::UpdateSettings,
}

pub struct ConfigStore {
    path: PathBuf,
    config: Config,
}

impl ConfigStore {
    /// Загрузить настройки. Нет файла — настройки по умолчанию; битый файл —
    /// тоже по умолчанию, чтобы лаунчер всегда открывался.
    pub fn load(path: PathBuf) -> Self {
        let config = std::fs::read(&path)
            .ok()
            .and_then(|bytes| json(&bytes, "настройки лаунчера").ok())
            .unwrap_or_default();
        Self { path, config }
    }

    pub fn get(&self) -> &Config {
        &self.config
    }

    /// Запомнить папку данных. Папка создаётся, если её нет.
    pub fn set_data_dir(&mut self, path: PathBuf) -> Result<()> {
        if !path.is_absolute() {
            return Err(Error::Other(format!(
                r"«{}» — не полный путь: укажите его от буквы диска, например D:\MajorLauncherData",
                path.display()
            )));
        }
        std::fs::create_dir_all(&path).at(&path)?;
        self.config.data_volume = datadir::volume_serial(&path);
        self.config.data_dir = Some(path);
        self.save()
    }

    /// Проверить папку данных при запуске ([`datadir::locate`]): диск сменил
    /// букву — путь обновляется. Настройкам без номера тома он дописывается.
    pub fn locate_data_dir(&mut self) -> Located {
        let Some(saved) = self.config.data_dir.clone() else {
            return Located::Here;
        };
        let found = datadir::locate(&saved, self.config.data_volume);
        match &found {
            Located::Moved(to) => {
                self.config.data_dir = Some(to.clone());
                self.config.data_volume = datadir::volume_serial(to);
                let _ = self.save();
            }
            Located::Here if self.config.data_volume.is_none() => {
                self.config.data_volume = datadir::volume_serial(&saved);
                if self.config.data_volume.is_some() {
                    let _ = self.save();
                }
            }
            _ => {}
        }
        found
    }

    pub fn set_game(&mut self, game: GameSettings) -> Result<()> {
        check_memory(game.memory_mb)?;
        self.config.game = GameSettings {
            jvm_args: game.jvm_args.trim().to_string(),
            ..game
        };
        self.save()
    }

    pub fn set_catalog(&mut self, catalog: CatalogSettings) -> Result<()> {
        self.config.catalog = catalog;
        self.save()
    }

    pub fn set_updates(&mut self, updates: crate::updates::UpdateSettings) -> Result<()> {
        self.config.updates = updates;
        self.save()
    }

    pub fn set_optimize(&mut self, optimize: crate::optimize::OptimizeSettings) -> Result<()> {
        self.config.optimize = optimize;
        self.save()
    }

    pub fn set_appearance(&mut self, appearance: Appearance) -> Result<()> {
        appearance.validate()?;
        // Картинку включает только set_background: без файла её нет.
        let image = appearance.image && self.background_path().exists();
        self.config.appearance = Appearance {
            image,
            ..appearance
        };
        self.save()
    }

    fn background_path(&self) -> PathBuf {
        self.path.with_file_name("background.jpg")
    }

    /// Поставить картинку фона (уже уменьшенный JPEG, см. [`crate::content::background`]).
    pub fn set_background(&mut self, jpeg: &[u8]) -> Result<()> {
        let path = self.background_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).at(dir)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, jpeg).at(&tmp)?;
        std::fs::rename(&tmp, &path).at(&path)?;
        self.config.appearance.image = true;
        self.save()
    }

    pub fn remove_background(&mut self) -> Result<()> {
        let path = self.background_path();
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).at(&path),
        }
        self.config.appearance.image = false;
        self.save()
    }

    /// Картинка фона, если она включена.
    pub fn background(&self) -> Option<Vec<u8>> {
        self.config
            .appearance
            .image
            .then(|| std::fs::read(self.background_path()).ok())
            .flatten()
    }

    fn save(&self) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).at(dir)?;
        }
        let bytes = serde_json::to_vec_pretty(&self.config).expect("настройки сериализуются");
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, bytes).at(&tmp)?;
        std::fs::rename(&tmp, &self.path).at(&self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appearance_round_trip_and_validation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut store = ConfigStore::load(path.clone());
        assert_eq!(store.get().appearance.mode, ThemeMode::System);

        let custom = Appearance {
            mode: ThemeMode::Dark,
            accent: Some("#7c3aed".into()),
            ..Appearance::default()
        };
        store.set_appearance(custom.clone()).unwrap();
        assert_eq!(ConfigStore::load(path).get().appearance, custom);

        let bad = Appearance {
            accent: Some("red".into()),
            ..Appearance::default()
        };
        assert!(store.set_appearance(bad).is_err());
    }

    #[test]
    fn background_follows_mode_and_custom_color() {
        let mut a = Appearance::default();
        assert_eq!(a.background_rgb(true), [0x0e, 0x0e, 0x10]);
        assert_eq!(a.background_rgb(false), [0xf4, 0xf4, 0xf5]);
        a.mode = ThemeMode::Light;
        assert_eq!(a.background_rgb(true), [0xf4, 0xf4, 0xf5]);
        a.background = Some("#14182a".into());
        assert_eq!(a.background_rgb(true), [0x14, 0x18, 0x2a]);
    }

    #[test]
    fn background_image_needs_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ConfigStore::load(dir.path().join("config.json"));
        let on = Appearance {
            image: true,
            ..Appearance::default()
        };
        store.set_appearance(on.clone()).unwrap();
        assert!(!store.get().appearance.image, "без файла картинки нет");
        assert!(store.background().is_none());

        store.set_background(b"jpeg").unwrap();
        assert_eq!(store.background().as_deref(), Some(&b"jpeg"[..]));
        let dim = Appearance {
            image_dim: 30,
            ..on
        };
        store.set_appearance(dim).unwrap();
        assert!(store.get().appearance.image);
        let too_much = Appearance {
            image_dim: 95,
            ..Appearance::default()
        };
        assert!(store.set_appearance(too_much).is_err());

        store.remove_background().unwrap();
        assert!(!store.get().appearance.image);
        assert!(!dir.path().join("background.jpg").exists());
        // Старый config.json без новых полей читается с приглушением по умолчанию.
        let old: Appearance = serde_json::from_str(r#"{"mode":"dark"}"#).unwrap();
        assert_eq!(old.image_dim, 60);
    }

    #[test]
    fn broken_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, b"{not json").unwrap();
        assert_eq!(ConfigStore::load(path).get(), &Config::default());
    }
}
