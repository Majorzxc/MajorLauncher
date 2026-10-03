//! Секреты (токены Ely.by) — в защищённом хранилище Windows, «Диспетчере учётных
//! данных», а не в файлах (принцип 3 ТЗ). Записи видны в «Панель управления →
//! Диспетчер учётных данных → Учётные данные Windows».
//!
//! На других ОС хранилища пока нет: лаунчер выходит только под Windows.

use crate::error::{Error, Result};

const SERVICE: &str = "MajorLauncher";

#[cfg(windows)]
mod imp {
    use keyring_core::Entry;
    use keyring_core::api::CredentialStoreApi;

    use super::*;

    fn entry(service: &str, key: &str) -> Result<Entry> {
        let store = windows_native_keyring_store::Store::new().map_err(fail)?;
        store.build(service, key, None).map_err(fail)
    }

    fn fail(e: keyring_core::Error) -> Error {
        Error::Secrets(e.to_string())
    }

    pub fn set(service: &str, key: &str, value: &str) -> Result<()> {
        entry(service, key)?.set_password(value).map_err(fail)
    }

    pub fn get(service: &str, key: &str) -> Result<Option<String>> {
        match entry(service, key)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(e) => Err(fail(e)),
        }
    }

    pub fn delete(service: &str, key: &str) -> Result<()> {
        match entry(service, key)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(fail(e)),
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    fn unsupported() -> Error {
        Error::Secrets("хранилище паролей поддерживается только в Windows".into())
    }

    pub fn set(_: &str, _: &str, _: &str) -> Result<()> {
        Err(unsupported())
    }

    pub fn get(_: &str, _: &str) -> Result<Option<String>> {
        Err(unsupported())
    }

    pub fn delete(_: &str, _: &str) -> Result<()> {
        Err(unsupported())
    }
}

pub fn set(key: &str, value: &str) -> Result<()> {
    imp::set(SERVICE, key, value)
}

/// `None` — такой записи нет (например, её удалили вручную).
pub fn get(key: &str) -> Result<Option<String>> {
    imp::get(SERVICE, key)
}

/// Удалить запись; отсутствие записи — не ошибка.
pub fn delete(key: &str) -> Result<()> {
    imp::delete(SERVICE, key)
}

#[cfg(all(test, windows))]
mod tests {
    use super::imp;

    // Отдельное имя сервиса, чтобы тесты не трогали настоящие записи лаунчера.
    const TEST_SERVICE: &str = "MajorLauncher-test";

    #[test]
    fn round_trip_in_windows_credential_manager() {
        let key = format!("roundtrip-{}", std::process::id());
        imp::set(TEST_SERVICE, &key, "секрет 123").unwrap();
        assert_eq!(
            imp::get(TEST_SERVICE, &key).unwrap().as_deref(),
            Some("секрет 123")
        );
        imp::delete(TEST_SERVICE, &key).unwrap();
        assert_eq!(imp::get(TEST_SERVICE, &key).unwrap(), None);
        imp::delete(TEST_SERVICE, &key).unwrap();
    }
}
