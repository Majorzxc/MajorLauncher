//! Ошибки ядра. Тексты сразу на русском и понятны игроку: интерфейс и `mlcli`
//! показывают их как есть, а по варианту ошибки можно предложить решение.

use std::path::{Path, PathBuf};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("не удалось скачать {url}: {source}")]
    Network {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("сервер ответил {status} на запрос {url}")]
    HttpStatus { url: String, status: u16 },

    #[error("файл {path} повреждён: ожидался хеш {expected}, получен {actual}")]
    HashMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },

    #[error("ошибка чтения или записи {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("не удалось разобрать {what}: {source}")]
    Json {
        what: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("не удалось распаковать {path}: {source}")]
    Zip {
        path: PathBuf,
        #[source]
        source: zip::result::ZipError,
    },

    #[error("версия «{0}» не найдена в списке версий Mojang")]
    VersionNotFound(String),

    #[error("Mojang не раздаёт Java «{0}» для Windows x64")]
    JavaUnavailable(String),

    #[error(
        "версия «{0}» ещё не скачана, а интернета нет. Подключитесь к сети, чтобы скачать её один раз — дальше можно играть без интернета"
    )]
    NotInstalledOffline(String),

    #[error(
        "ник «{0}» не подходит: нужно от 3 до 16 символов, только латинские буквы, цифры и «_»"
    )]
    BadNickname(String),

    #[error("{0}")]
    Other(String),
}

impl Error {
    /// Ошибка сети (нет соединения, таймаут) — в отличие от ошибок данных.
    pub fn is_network(&self) -> bool {
        matches!(self, Error::Network { .. })
    }
}

/// Прикрепляет путь к ошибке ввода-вывода: «нет доступа» без пути бесполезно.
pub(crate) trait IoContext<T> {
    fn at(self, path: &Path) -> Result<T>;
}

impl<T> IoContext<T> for std::io::Result<T> {
    fn at(self, path: &Path) -> Result<T> {
        self.map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })
    }
}

pub(crate) fn json<T: serde::de::DeserializeOwned>(bytes: &[u8], what: &str) -> Result<T> {
    serde_json::from_slice(bytes).map_err(|source| Error::Json {
        what: what.to_string(),
        source,
    })
}
