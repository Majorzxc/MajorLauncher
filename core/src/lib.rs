//! Ядро MajorLauncher: вся логика лаунчера без интерфейса.
//!
//! Интерфейс (`app`) и консольная утилита (`cli`) только вызывают функции ядра
//! и ничего не решают сами (принцип 5 из ТЗ).
//!
//! Этап 1: скачать и запустить ванильную версию с офлайн-ником.
//! [`install::prepare`] ставит версию (Java, библиотеки, ассеты, клиент),
//! [`launch::build`] собирает команду запуска.

pub mod account;
pub mod download;
pub mod error;
pub mod install;
pub mod java;
pub mod launch;
pub mod meta;
pub mod paths;
pub mod rules;
pub mod verify;

pub use account::OfflineAccount;
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
