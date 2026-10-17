//! Ядро MajorLauncher: вся логика лаунчера без интерфейса.
//!
//! Интерфейс (`app`) и консольная утилита (`cli`) только вызывают функции ядра
//! и ничего не решают сами (принцип 5 из ТЗ).
//!
//! [`install::prepare`] ставит версию (Java, библиотеки, ассеты, клиент),
//! [`account::Accounts`] хранит аккаунты и готовит сессию,
//! [`launch::build`] собирает команду запуска,
//! [`game`] запускает сборку и следит за игрой, [`playtime`] ведёт статистику.

pub mod account;
pub mod builds;
pub mod catalog;
pub mod config;
pub mod content;
pub mod datadir;
pub mod download;
pub mod elyby;
pub mod error;
pub mod game;
pub mod install;
pub mod java;
pub mod launch;
pub mod library;
pub mod loaders;
pub mod malware;
pub mod meta;
pub mod modlib;
pub mod modmeta;
pub mod modrinth;
pub mod mods;
pub mod mrpack;
pub mod nbt;
pub mod options;
pub mod paths;
pub mod playtime;
pub mod relocate;
pub mod rules;
pub mod secrets;
pub mod servers;
pub mod system;
pub mod verify;

pub use account::{Account, Accounts, Kind, Session};
pub use builds::{Builds, Instance};
pub use config::{
    Appearance, CatalogSettings, Config, ConfigStore, GameSettings, OnLaunch, ThemeMode,
};
pub use download::{DownloadOptions, Downloader};
pub use error::{Error, Result};
pub use install::{Event, Mode, Prepared, Progress};
pub use launch::{LaunchCommand, LaunchOptions};
pub use paths::DataDir;

/// Версия ядра, совпадает с версией всего проекта.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!super::VERSION.is_empty());
    }
}
