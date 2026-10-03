//! Настройки лаунчера, которые нужны ещё до выбора папки данных: где эта папка
//! и как выглядит окно. Крошечный файл в папке настроек Windows (её выбирает
//! окно лаунчера), всё тяжёлое — в папке данных.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Appearance {
    #[serde(default)]
    pub mode: ThemeMode,
    /// Свой акцентный цвет `#rrggbb`; `None` — чёрно-белый акцент темы.
    #[serde(default)]
    pub accent: Option<String>,
    /// Свой цвет фона `#rrggbb`; остальные оттенки окно вычисляет из него.
    #[serde(default)]
    pub background: Option<String>,
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// Папка данных; `None` — ещё не выбрана (первый запуск).
    #[serde(default)]
    pub data_dir: Option<PathBuf>,
    #[serde(default)]
    pub appearance: Appearance,
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
        self.config.data_dir = Some(path);
        self.save()
    }

    pub fn set_appearance(&mut self, appearance: Appearance) -> Result<()> {
        appearance.validate()?;
        self.config.appearance = appearance;
        self.save()
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
            background: None,
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
    fn broken_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, b"{not json").unwrap();
        assert_eq!(ConfigStore::load(path).get(), &Config::default());
    }
}
