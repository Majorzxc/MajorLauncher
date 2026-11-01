//! ИИ-разбор для окна (раздел 5.8 ТЗ): настройки, ключи в хранилище Windows,
//! список моделей, «что уйдёт» перед отправкой, запрос и разбор сам после игры.

use majorlauncher_core::ai::{self, AiSettings, Answer, Prepared, Provider};
use majorlauncher_core::analyzer::{self, Analysis};
use majorlauncher_core::config::Config;
use majorlauncher_core::paths::DataDir;
use majorlauncher_core::secrets;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::builds::blocking;
use crate::{AppState, CmdResult, text};

const PROVIDERS: [Provider; 7] = [
    Provider::Anthropic,
    Provider::OpenAi,
    Provider::Gemini,
    Provider::OpenRouter,
    Provider::Ollama,
    Provider::LmStudio,
    Provider::Custom,
];

fn key_of(provider: Provider) -> Option<String> {
    secrets::get(&provider.secret())
        .ok()
        .flatten()
        .filter(|k| !k.is_empty())
}

#[tauri::command]
pub fn set_ai(state: State<AppState>, ai: AiSettings) -> CmdResult<Config> {
    let mut config = state.config.lock().unwrap();
    config.set_ai(ai).map_err(text)?;
    Ok(config.get().clone())
}

/// У каких сервисов сохранён ключ (сами ключи в окно не уходят).
#[tauri::command]
pub async fn ai_keys() -> CmdResult<Vec<Provider>> {
    blocking(|| {
        Ok(PROVIDERS
            .into_iter()
            .filter(|p| key_of(*p).is_some())
            .collect())
    })
    .await
}

/// Сохранить ключ сервиса; пустой — удалить.
#[tauri::command]
pub async fn ai_set_key(provider: Provider, key: String) -> CmdResult<()> {
    blocking(move || {
        let key = key.trim();
        if key.is_empty() {
            secrets::delete(&provider.secret())
        } else {
            secrets::set(&provider.secret(), key)
        }
        .map_err(text)
    })
    .await
}

/// Модели сервиса — заодно проверка адреса и ключа (бесплатно).
#[tauri::command]
pub async fn ai_models(ai: AiSettings) -> CmdResult<ai::Models> {
    let provider = ai.provider;
    let key = blocking(move || Ok(key_of(provider))).await?;
    ai::models(&ai, key.as_deref()).await.map_err(text)
}

/// Сколько раз сегодня лаунчер спросил ИИ сам.
#[tauri::command]
pub fn ai_used_today(state: State<AppState>) -> CmdResult<u32> {
    Ok(ai::used_today(&state.data()?))
}

/// Что уйдёт в сервис: очищенный текст и его размер.
#[tauri::command]
pub async fn ai_preview(
    state: State<'_, AppState>,
    build: String,
    file: Option<String>,
) -> CmdResult<Prepared> {
    let data = state.data()?;
    let settings = state.config.lock().unwrap().get().ai.clone();
    let text = ai::gather(&state.http, &data, &build, file.as_deref())
        .await
        .map_err(text)?;
    Ok(ai::prepared(&settings, text))
}

/// Спросить ИИ. `text` — тот, что игрок видел в «что уйдёт»; без него
/// лаунчер соберёт запрос сам. Ответ по последней игре запоминается.
#[tauri::command]
pub async fn ai_ask(
    state: State<'_, AppState>,
    build: String,
    file: Option<String>,
    text: Option<String>,
) -> CmdResult<Answer> {
    let data = state.data()?;
    let settings = state.config.lock().unwrap().get().ai.clone();
    if !settings.enabled {
        return Err("ИИ-разбор выключен — включите его в «Настройки → ИИ-разбор»".into());
    }
    let text = match text {
        Some(t) => t,
        None => ai::gather(&state.http, &data, &build, file.as_deref())
            .await
            .map_err(crate::text)?,
    };
    let provider = settings.provider;
    let key = blocking(move || Ok(key_of(provider))).await?;
    let answer = ai::ask(&settings, key.as_deref(), &text)
        .await
        .map_err(crate::text)?;
    if file.is_none() {
        analyzer::save_answer(&data, &build, &answer);
    }
    Ok(answer)
}

/// Ответ разбора, который лаунчер запустил сам после игры.
#[derive(Clone, Serialize)]
struct AiEvent {
    build: String,
    answer: Option<Answer>,
    error: Option<String>,
}

/// После игры: если игрок так настроил и лимит дня не исчерпан — спросить
/// ИИ самому (без показа текста: так игрок выбрал в настройках). `true` —
/// разбор запущен, ответ придёт событием `ai-answer`.
pub fn auto(
    app: &AppHandle,
    data: &DataDir,
    build: &str,
    analysis: &Analysis,
    crashed: bool,
) -> bool {
    let settings = app
        .state::<AppState>()
        .config
        .lock()
        .unwrap()
        .get()
        .ai
        .clone();
    if !settings.auto_for(analysis, crashed) || settings.check().is_err() {
        return false;
    }
    let key = key_of(settings.provider);
    if settings.provider.needs_key() && key.is_none() {
        return false;
    }
    if !ai::take_auto(data, settings.daily_limit) {
        return false;
    }
    let (app, data, build) = (app.clone(), data.clone(), build.to_string());
    tauri::async_runtime::spawn(async move {
        let http = &app.state::<AppState>().http;
        let result = async {
            let text = ai::gather(http, &data, &build, None).await?;
            ai::ask(&settings, key.as_deref(), &text).await
        }
        .await;
        let event = match result {
            Ok(mut answer) => {
                answer.auto = true;
                analyzer::save_answer(&data, &build, &answer);
                AiEvent {
                    build,
                    answer: Some(answer),
                    error: None,
                }
            }
            Err(e) => AiEvent {
                build,
                answer: None,
                error: Some(e.to_string()),
            },
        };
        let _ = app.emit("ai-answer", event);
    });
    true
}
