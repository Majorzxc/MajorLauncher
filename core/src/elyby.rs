//! Ely.by: вход по логину и паролю (протокол Yggdrasil, документация —
//! docs.ely.by/en/minecraft-auth.html), обновление токена и authlib-injector,
//! через который игра получает скины Ely.by и пускает на серверы с Ely.by.

use std::path::PathBuf;

use base64::Engine;
use serde::Deserialize;
use serde_json::json;

use crate::download::{Downloader, Task};
use crate::error::{Error, Result};
use crate::paths::DataDir;
use crate::verify::sha1_file;

const AUTH_SERVER: &str = "https://authserver.ely.by";
/// Корень API для authlib-injector — на него указывает сам Ely.by (заголовок
/// X-Authlib-Injector-Api-Location). Короткое «ely.by» нельзя: вместе с заранее
/// переданным описанием сервера agent не ищет настоящий адрес и стучится
/// на ely.by, где профиль со скином отдаёт 404 (скина в игре нет).
const AUTHLIB_API: &str = "https://account.ely.by/api/authlib-injector";

/// Версия authlib-injector закреплена вместе с хешем: лаунчер не скачает
/// другой файл, даже если источник подменят. Сверено 03.10.2026: файлы
/// с authlib-injector.yushi.moe и из релиза на GitHub совпадают.
const AGENT_VERSION: &str = "1.2.8";
const AGENT_URL: &str = "https://github.com/yushijinhun/authlib-injector/releases/download/v1.2.8/authlib-injector-1.2.8.jar";
const AGENT_SHA1: &str = "0e0e66d8a4f91a26f33b9c09f5cdffce4a11f0b8";
const AGENT_SIZE: u64 = 349_681;

/// Результат входа: токен и игровой профиль.
#[derive(Debug, Clone)]
pub struct Login {
    pub access_token: String,
    /// UUID без дефисов.
    pub uuid: String,
    pub name: String,
}

#[derive(Deserialize)]
struct Profile {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct AuthResponse {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "selectedProfile")]
    selected_profile: Option<Profile>,
}

#[derive(Deserialize, Default)]
struct ErrorResponse {
    error: Option<String>,
    #[serde(rename = "errorMessage")]
    error_message: Option<String>,
}

/// Что за запрос — от этого зависит, как понимать ответ «запрещено».
#[derive(Clone, Copy)]
enum Call {
    Authenticate,
    Refresh,
}

async fn post(http: &Downloader, path: &str, body: serde_json::Value) -> Result<(u16, Vec<u8>)> {
    let url = format!("{AUTH_SERVER}{path}");
    let resp = http
        .client()
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|source| Error::Network {
            url: url.clone(),
            source,
        })?;
    let status = resp.status().as_u16();
    let bytes = resp
        .bytes()
        .await
        .map_err(|source| Error::Network { url, source })?;
    Ok((status, bytes.to_vec()))
}

fn parse_login(call: Call, status: u16, bytes: &[u8]) -> Result<Login> {
    if status != 200 {
        return Err(map_error(call, status, bytes));
    }
    let resp: AuthResponse = crate::error::json(bytes, "ответ Ely.by")?;
    let profile = resp
        .selected_profile
        .ok_or_else(|| Error::ElyService("у аккаунта Ely.by нет игрового профиля".into()))?;
    Ok(Login {
        access_token: resp.access_token,
        uuid: profile.id.replace('-', ""),
        name: profile.name,
    })
}

fn map_error(call: Call, status: u16, bytes: &[u8]) -> Error {
    let err: ErrorResponse = serde_json::from_slice(bytes).unwrap_or_default();
    let message = err.error_message.unwrap_or_default();
    if message.to_lowercase().contains("two factor") {
        return Error::ElyTwoFactorRequired;
    }
    match (call, err.error.as_deref()) {
        (Call::Authenticate, Some("ForbiddenOperationException")) => Error::ElyInvalidCredentials,
        (Call::Refresh, Some("ForbiddenOperationException")) => Error::ElySessionExpired,
        _ if message.is_empty() => Error::ElyService(format!("сервер ответил {status}")),
        _ => Error::ElyService(message),
    }
}

/// Вход по логину (нику или почте) и паролю. Если на аккаунте включена
/// двухфакторная защита, код дописывается к паролю через двоеточие.
pub async fn authenticate(
    http: &Downloader,
    login: &str,
    password: &str,
    totp: Option<&str>,
    client_token: &str,
) -> Result<Login> {
    let password = match totp {
        Some(code) => format!("{password}:{code}"),
        None => password.to_string(),
    };
    let body = json!({
        "username": login,
        "password": password,
        "clientToken": client_token,
        "requestUser": false,
    });
    let (status, bytes) = post(http, "/auth/authenticate", body).await?;
    parse_login(Call::Authenticate, status, &bytes)
}

/// Получить новый токен взамен старого (старый перестаёт действовать).
pub async fn refresh(http: &Downloader, access_token: &str, client_token: &str) -> Result<Login> {
    let body = json!({
        "accessToken": access_token,
        "clientToken": client_token,
        "requestUser": false,
    });
    let (status, bytes) = post(http, "/auth/refresh", body).await?;
    parse_login(Call::Refresh, status, &bytes)
}

/// Действует ли токен. Ошибка — только если до Ely.by не достучаться.
pub async fn validate(http: &Downloader, access_token: &str) -> Result<bool> {
    let (status, _) = post(
        http,
        "/auth/validate",
        json!({ "accessToken": access_token }),
    )
    .await?;
    Ok(status == 200)
}

/// Отозвать токен при удалении аккаунта из лаунчера.
pub async fn invalidate(http: &Downloader, access_token: &str, client_token: &str) -> Result<()> {
    let body = json!({ "accessToken": access_token, "clientToken": client_token });
    post(http, "/auth/invalidate", body).await.map(|_| ())
}

/// authlib-injector, подготовленный к запуску игры.
#[derive(Debug, Clone)]
pub struct Agent {
    pub jar: PathBuf,
    /// Описание сервера Ely.by в base64: игра не тратит время на запрос при старте.
    pub prefetched: Option<String>,
}

impl Agent {
    pub fn jvm_args(&self) -> Vec<String> {
        let mut args = vec![format!("-javaagent:{}={AUTHLIB_API}", self.jar.display())];
        if let Some(meta) = &self.prefetched {
            args.push(format!("-Dauthlibinjector.yggdrasil.prefetched={meta}"));
        }
        args
    }
}

/// Скачать (если нужно) и проверить authlib-injector, подтянуть описание сервера.
pub async fn agent(data: &DataDir, http: &Downloader) -> Result<Agent> {
    let jar = data
        .root()
        .join("agents")
        .join(format!("authlib-injector-{AGENT_VERSION}.jar"));
    let ok = std::fs::metadata(&jar).is_ok_and(|m| m.len() == AGENT_SIZE)
        && sha1_file(&jar).is_ok_and(|s| s == AGENT_SHA1);
    if !ok {
        http.download(&Task {
            url: AGENT_URL.into(),
            path: jar.clone(),
            sha1: Some(AGENT_SHA1.into()),
            size: Some(AGENT_SIZE),
        })
        .await?;
    }
    // Без описания игра запросит его сама — это не ошибка, только чуть дольше старт.
    let prefetched = match http.get_bytes(AUTHLIB_API).await {
        Ok(bytes) => Some(base64::engine::general_purpose::STANDARD.encode(bytes)),
        Err(e) => {
            tracing::warn!("не удалось заранее получить описание Ely.by: {e}");
            None
        }
    };
    Ok(Agent { jar, prefetched })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_successful_login() {
        let body = br#"{"accessToken":"tok","clientToken":"c","selectedProfile":{"id":"ffc8fdc95824509e8a57c99b940fb996","name":"ErickSkrauch"},"availableProfiles":[]}"#;
        let login = parse_login(Call::Authenticate, 200, body).unwrap();
        assert_eq!(login.name, "ErickSkrauch");
        assert_eq!(login.uuid, "ffc8fdc95824509e8a57c99b940fb996");
        assert_eq!(login.access_token, "tok");
    }

    #[test]
    fn maps_errors_from_documentation() {
        let two_factor = br#"{"error":"ForbiddenOperationException","errorMessage":"Account protected with two factor auth."}"#;
        assert!(matches!(
            parse_login(Call::Authenticate, 401, two_factor),
            Err(Error::ElyTwoFactorRequired)
        ));
        let wrong = br#"{"error":"ForbiddenOperationException","errorMessage":"Invalid credentials. Invalid email or password."}"#;
        assert!(matches!(
            parse_login(Call::Authenticate, 401, wrong),
            Err(Error::ElyInvalidCredentials)
        ));
        assert!(matches!(
            parse_login(Call::Refresh, 401, wrong),
            Err(Error::ElySessionExpired)
        ));
        assert!(matches!(
            parse_login(Call::Authenticate, 500, b"oops"),
            Err(Error::ElyService(_))
        ));
    }

    // Сетевые проверки на настоящем Ely.by. На автосборке не запускаются:
    // cargo test -p majorlauncher-core elyby -- --ignored

    fn http() -> Downloader {
        Downloader::new(crate::download::DownloadOptions::default()).unwrap()
    }

    #[tokio::test]
    #[ignore = "ходит в сеть"]
    async fn live_rejects_unknown_account() {
        let result = authenticate(
            &http(),
            "majorlauncher_no_such_user_8f3a",
            "definitely-not-a-password",
            None,
            "0123456789abcdef0123456789abcdef",
        )
        .await;
        assert!(
            matches!(result, Err(Error::ElyInvalidCredentials)),
            "{result:?}"
        );
    }

    #[tokio::test]
    #[ignore = "ходит в сеть"]
    async fn live_prepares_agent() {
        let dir = tempfile::tempdir().unwrap();
        let agent = agent(&DataDir::new(dir.path()), &http()).await.unwrap();
        assert_eq!(sha1_file(&agent.jar).unwrap(), AGENT_SHA1);
        let meta = base64::engine::general_purpose::STANDARD
            .decode(agent.prefetched.expect("описание сервера скачано"))
            .unwrap();
        assert!(String::from_utf8_lossy(&meta).contains("Ely.by"));
    }

    #[test]
    fn agent_arguments() {
        let agent = Agent {
            jar: PathBuf::from("D:/data/agents/a.jar"),
            prefetched: Some("eyJ9".into()),
        };
        assert_eq!(
            agent.jvm_args(),
            vec![
                "-javaagent:D:/data/agents/a.jar=https://account.ely.by/api/authlib-injector"
                    .to_string(),
                "-Dauthlibinjector.yggdrasil.prefetched=eyJ9".to_string(),
            ]
        );
    }
}
