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
    /// «Для опытных»: при запуске игры окно лаунчера остаётся и показывает
    /// лог игры, пока она идёт.
    #[serde(default)]
    pub log_window: bool,
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

/// Первый запуск и обучение (раздел 6.4 ТЗ). Сами подсказки — данные окна
/// (`app/src/lib/tours.ts`), здесь — что игрок уже прошёл.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tutorial {
    /// Мастер первого запуска пройден.
    #[serde(default)]
    pub onboarded: bool,
    /// Обучение включено: подсказки раздела показываются при первом его открытии.
    #[serde(default)]
    pub enabled: bool,
    /// Подсказки, которые игрок уже видел (id тура).
    #[serde(default)]
    pub seen: Vec<String>,
    /// До какого времени (Unix, с) кнопка «Пройти обучение» видна в шапке
    /// окна. Потом она остаётся только в настройках.
    #[serde(default)]
    pub offer_until: Option<u64>,
}

/// Сколько кнопка «Пройти обучение» видна в шапке после отказа.
pub const OFFER_SECS: u64 = 7 * 24 * 3600;

fn yes() -> bool {
    true
}

/// «Настройки → Загрузки» (раздел 5.16 ТЗ). Применяются сразу, к следующим запросам.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadSettings {
    /// Сколько файлов качать одновременно.
    #[serde(default = "default_parallel")]
    pub parallel: u32,
    /// Запасные зеркала для ассетов, библиотек и загрузчиков (всегда с проверкой хеша).
    #[serde(default = "yes")]
    pub mirrors: bool,
    /// Ограничение скорости, КБ/с; 0 — без ограничения.
    #[serde(default)]
    pub limit_kbps: u32,
    /// Прокси `http://адрес:порт`; пусто — напрямую.
    #[serde(default)]
    pub proxy: String,
}

fn default_parallel() -> u32 {
    16
}

impl Default for DownloadSettings {
    fn default() -> Self {
        Self {
            parallel: default_parallel(),
            mirrors: true,
            limit_kbps: 0,
            proxy: String::new(),
        }
    }
}

impl DownloadSettings {
    pub fn options(&self) -> crate::download::DownloadOptions {
        crate::download::DownloadOptions {
            parallel: self.parallel as usize,
            mirrors: self.mirrors,
            limit_kbps: self.limit_kbps,
            proxy: Some(self.proxy.trim().to_string()).filter(|p| !p.is_empty()),
            ..Default::default()
        }
    }
}

/// Снапшоты сборок и корзина (раздел 5.16 ТЗ).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageSettings {
    /// Снапшот сам перед каждым изменением сборки.
    #[serde(default = "yes")]
    pub auto_snapshots: bool,
    /// Сколько автоматических снапшотов держать на сборку.
    #[serde(default = "default_keep")]
    pub keep_snapshots: u32,
    /// Сколько дней удалённая сборка лежит в корзине.
    #[serde(default = "default_trash_days")]
    pub trash_days: u32,
}

fn default_keep() -> u32 {
    crate::snapshots::DEFAULT_KEEP as u32
}

fn default_trash_days() -> u32 {
    crate::builds::TRASH_DAYS as u32
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            auto_snapshots: true,
            keep_snapshots: default_keep(),
            trash_days: default_trash_days(),
        }
    }
}

/// Какие предупреждения показывать (раздел 5.16 ТЗ, «Уведомления»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoticeSettings {
    /// Советы 💡 — ниже предупреждений ⚠ и ошибок ⛔, их можно скрыть.
    #[serde(default = "yes")]
    pub tips: bool,
}

impl Default for NoticeSettings {
    fn default() -> Self {
        Self { tips: true }
    }
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
    /// ИИ-разбор ошибок (раздел 5.8 ТЗ); ключи — в хранилище Windows.
    #[serde(default)]
    pub ai: crate::ai::AiSettings,
    /// Скины: программа-редактор, спорные скины в каталоге.
    #[serde(default)]
    pub skins: crate::skins::SkinSettings,
    /// Мастер первого запуска и обучение.
    #[serde(default)]
    pub tutorial: Tutorial,
    #[serde(default)]
    pub downloads: DownloadSettings,
    #[serde(default)]
    pub storage: StorageSettings,
    #[serde(default)]
    pub notices: NoticeSettings,
}

pub struct ConfigStore {
    path: PathBuf,
    config: Config,
}

impl ConfigStore {
    /// Загрузить настройки. Нет файла — настройки по умолчанию; битый файл —
    /// тоже по умолчанию, чтобы лаунчер всегда открывался.
    pub fn load(path: PathBuf) -> Self {
        let raw: Option<serde_json::Value> = std::fs::read(&path)
            .ok()
            .and_then(|bytes| json(&bytes, "настройки лаунчера").ok());
        // Настройки лаунчера, который стоял до мастера первого запуска: мастер
        // не показывается, обучение — только предлагается.
        let before_tutorial = raw.as_ref().is_some_and(|v| v.get("tutorial").is_none());
        let mut config: Config = raw
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        let mut store = Self {
            path,
            config: Config::default(),
        };
        if before_tutorial && config.data_dir.is_some() {
            config.tutorial = Tutorial {
                onboarded: true,
                offer_until: Some(now() + OFFER_SECS),
                ..Tutorial::default()
            };
            store.config = config;
            let _ = store.save();
        } else {
            store.config = config;
        }
        store
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

    pub fn set_downloads(&mut self, d: DownloadSettings) -> Result<()> {
        if !(1..=64).contains(&d.parallel) {
            return Err(Error::Other("одновременных загрузок — от 1 до 64".into()));
        }
        self.config.downloads = DownloadSettings {
            proxy: d.proxy.trim().to_string(),
            ..d
        };
        self.save()
    }

    pub fn set_storage(&mut self, st: StorageSettings) -> Result<()> {
        if !(1..=200).contains(&st.keep_snapshots) {
            return Err(Error::Other(
                "автоматических снапшотов — от 1 до 200 на сборку".into(),
            ));
        }
        if !(1..=365).contains(&st.trash_days) {
            return Err(Error::Other("срок корзины — от 1 до 365 дней".into()));
        }
        self.config.storage = st;
        self.save()
    }

    pub fn set_notices(&mut self, n: NoticeSettings) -> Result<()> {
        self.config.notices = n;
        self.save()
    }

    pub fn set_skins(&mut self, skins: crate::skins::SkinSettings) -> Result<()> {
        self.config.skins = skins;
        self.save()
    }

    pub fn set_ai(&mut self, ai: crate::ai::AiSettings) -> Result<()> {
        self.config.ai = ai;
        self.save()
    }

    pub fn set_optimize(&mut self, optimize: crate::optimize::OptimizeSettings) -> Result<()> {
        self.config.optimize = optimize;
        self.save()
    }

    /// Мастер первого запуска пройден. Отказался от обучения — кнопка
    /// «Пройти обучение» неделю видна в шапке, потом — только в настройках.
    pub fn finish_onboarding(&mut self, tutorial: bool) -> Result<()> {
        let t = &mut self.config.tutorial;
        t.onboarded = true;
        t.enabled = tutorial;
        t.offer_until = (!tutorial).then(|| now() + OFFER_SECS);
        self.save()
    }

    /// Игрок посмотрел (или пропустил) подсказки раздела — больше их сам не показывать.
    pub fn tour_seen(&mut self, id: &str) -> Result<()> {
        let id = id.trim();
        let t = &mut self.config.tutorial;
        if id.is_empty() || id.len() > 64 || t.seen.iter().any(|s| s == id) {
            return Ok(());
        }
        t.seen.push(id.to_string());
        self.save()
    }

    /// «Пройти обучение»: подсказки всех разделов покажутся заново.
    pub fn restart_tutorial(&mut self) -> Result<()> {
        let t = &mut self.config.tutorial;
        t.enabled = true;
        t.seen.clear();
        t.offer_until = None;
        self.save()
    }

    /// «Выключить подсказки»: больше ничего не показывать само.
    pub fn stop_tutorial(&mut self) -> Result<()> {
        let t = &mut self.config.tutorial;
        t.enabled = false;
        t.offer_until = None;
        self.save()
    }

    /// Убрать кнопку «Пройти обучение» из шапки — она остаётся в настройках.
    pub fn hide_tutorial_offer(&mut self) -> Result<()> {
        self.config.tutorial.offer_until = None;
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

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
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
    fn launcher_settings_are_checked() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut store = ConfigStore::load(path.clone());
        assert_eq!(store.get().downloads.parallel, 16);
        assert_eq!(store.get().storage.trash_days, 30);
        let d = DownloadSettings {
            parallel: 4,
            proxy: "  http://127.0.0.1:3128 ".into(),
            ..DownloadSettings::default()
        };
        store.set_downloads(d).unwrap();
        let opts = ConfigStore::load(path).get().downloads.options();
        assert_eq!(
            (opts.parallel, opts.proxy.as_deref()),
            (4, Some("http://127.0.0.1:3128"))
        );
        assert!(DownloadSettings::default().options().proxy.is_none());
        let zero = DownloadSettings {
            parallel: 0,
            ..DownloadSettings::default()
        };
        assert!(store.set_downloads(zero).is_err());
        let bad = StorageSettings {
            trash_days: 0,
            ..StorageSettings::default()
        };
        assert!(store.set_storage(bad).is_err());
        // Старые настройки без новых разделов читаются со значениями по умолчанию.
        let old: Config = serde_json::from_str(r#"{"game":{"on_launch":"keep"}}"#).unwrap();
        assert!(old.notices.tips && old.storage.auto_snapshots && !old.game.log_window);
    }

    #[test]
    fn tutorial_flow() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut store = ConfigStore::load(path.clone());
        assert!(
            !store.get().tutorial.onboarded,
            "новый лаунчер начинает с мастера"
        );
        store.set_data_dir(dir.path().join("data")).unwrap();
        // Окно закрыли посреди мастера — при следующем запуске он продолжится.
        assert!(!ConfigStore::load(path.clone()).get().tutorial.onboarded);

        store.finish_onboarding(false).unwrap();
        let t = &store.get().tutorial;
        assert!(t.onboarded && !t.enabled);
        assert!(t.offer_until.unwrap() > now() + OFFER_SECS - 60);
        store.hide_tutorial_offer().unwrap();
        assert_eq!(store.get().tutorial.offer_until, None);

        store.restart_tutorial().unwrap();
        store.tour_seen("home").unwrap();
        store.tour_seen("home").unwrap();
        store.tour_seen(" ").unwrap();
        let t = ConfigStore::load(path.clone()).get().tutorial.clone();
        assert!(t.enabled);
        assert_eq!(t.seen, ["home"]);
        store.restart_tutorial().unwrap();
        assert!(store.get().tutorial.seen.is_empty());
        store.stop_tutorial().unwrap();
        assert!(!store.get().tutorial.enabled);
    }

    #[test]
    fn launcher_from_before_the_wizard_skips_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(
            &path,
            br#"{"data_dir":"D:\\MajorLauncherData","appearance":{"mode":"dark"}}"#,
        )
        .unwrap();
        let store = ConfigStore::load(path.clone());
        let t = &store.get().tutorial;
        assert!(t.onboarded && !t.enabled);
        assert!(t.offer_until.is_some(), "обучение предлагается кнопкой");
        assert_eq!(store.get().appearance.mode, ThemeMode::Dark);
        // Записано сразу: следующий запуск не продлевает срок кнопки.
        let again = ConfigStore::load(path);
        assert_eq!(again.get().tutorial, *t);

        // Без папки данных — это ещё первый запуск.
        let fresh = dir.path().join("fresh.json");
        std::fs::write(&fresh, br#"{"appearance":{"mode":"light"}}"#).unwrap();
        assert!(!ConfigStore::load(fresh).get().tutorial.onboarded);
    }

    #[test]
    fn broken_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, b"{not json").unwrap();
        assert_eq!(ConfigStore::load(path).get(), &Config::default());
    }
}
