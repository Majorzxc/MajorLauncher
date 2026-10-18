//! Перевод описаний с Modrinth на русский (просьба автора, 18.10.2026).
//!
//! Переводит Google — через тот же бесплатный адрес, что и расширения
//! браузеров (`translate.googleapis.com`, `client=gtx`). Ключ не нужен, но
//! адрес неофициальный: если Google его закроет, окно просто покажет
//! оригинал. Уходит только текст описания мода — никаких данных игрока.
//! Переводы хранятся в `cache/translations.json`: одно и то же описание
//! второй раз не переводится.

use std::collections::HashMap;

use serde_json::Value;

use crate::download::Downloader;
use crate::error::{Error, Result};
use crate::paths::DataDir;

const ENDPOINT: &str = "https://translate.googleapis.com/translate_a/single";
/// Сколько текста за один запрос (адрес GET не бесконечный).
const CHUNK: usize = 1800;
/// Больше этого переводов в кэше не держим — старые выбрасываются.
const CACHE_MAX: usize = 20_000;

fn key(text: &str) -> String {
    crate::verify::sha1_bytes(text.as_bytes())
}

fn cache_path(data: &DataDir) -> std::path::PathBuf {
    data.cache().join("translations.json")
}

/// Перевести тексты на русский. Порядок и число сохраняются; что не
/// перевелось — остаётся как было.
pub async fn to_russian(dl: &Downloader, data: &DataDir, texts: &[String]) -> Result<Vec<String>> {
    let path = cache_path(data);
    let mut cache: HashMap<String, String> = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let mut out: Vec<Option<String>> = texts
        .iter()
        .map(|t| {
            if needs(t) {
                cache.get(&key(t)).cloned()
            } else {
                Some(t.clone())
            }
        })
        .collect();

    // Непереведённое — пачками: строки через перевод строки.
    let todo: Vec<usize> = (0..texts.len()).filter(|&i| out[i].is_none()).collect();
    let mut batch: Vec<usize> = Vec::new();
    let mut size = 0;
    let mut failed = None;
    for (n, &i) in todo.iter().enumerate() {
        let t = single_line(&texts[i]);
        if !batch.is_empty() && size + t.len() > CHUNK {
            if let Err(e) = run(dl, texts, &batch, &mut out).await {
                failed = Some(e);
                break;
            }
            batch.clear();
            size = 0;
        }
        batch.push(i);
        size += t.len() + 1;
        if n + 1 == todo.len()
            && let Err(e) = run(dl, texts, &batch, &mut out).await
        {
            failed = Some(e);
        }
    }

    let mut fresh = false;
    for &i in &todo {
        if let Some(t) = &out[i] {
            cache.insert(key(&texts[i]), t.clone());
            fresh = true;
        }
    }
    if fresh {
        if cache.len() > CACHE_MAX {
            cache.clear();
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, serde_json::to_vec(&cache).unwrap_or_default());
    }
    // Ничего не перевелось из-за сети — пусть окно скажет об этом.
    if let Some(e) = failed
        && todo.iter().all(|&i| out[i].is_none())
    {
        return Err(e);
    }
    Ok(out
        .into_iter()
        .zip(texts)
        .map(|(t, orig)| t.unwrap_or_else(|| orig.clone()))
        .collect())
}

/// Есть что переводить: буквы латиницей, а не только числа и значки.
fn needs(text: &str) -> bool {
    text.chars().any(|c| c.is_ascii_alphabetic())
}

fn single_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Перевести пачку: строки через `\n`. Не совпало число строк — по одной.
async fn run(
    dl: &Downloader,
    texts: &[String],
    batch: &[usize],
    out: &mut [Option<String>],
) -> Result<()> {
    let joined = batch
        .iter()
        .map(|&i| single_line(&texts[i]))
        .collect::<Vec<_>>()
        .join("\n");
    let translated = request(dl, &joined).await?;
    let lines: Vec<&str> = translated.split('\n').collect();
    if lines.len() == batch.len() {
        for (&i, line) in batch.iter().zip(lines) {
            out[i] = Some(line.trim().to_string());
        }
        return Ok(());
    }
    for &i in batch {
        if let Ok(t) = request(dl, &single_line(&texts[i])).await {
            out[i] = Some(t.trim().to_string());
        }
    }
    Ok(())
}

async fn request(dl: &Downloader, text: &str) -> Result<String> {
    let url = reqwest::Url::parse_with_params(
        ENDPOINT,
        &[
            ("client", "gtx"),
            ("sl", "auto"),
            ("tl", "ru"),
            ("dt", "t"),
            ("q", text),
        ],
    )
    .map_err(|e| Error::Other(format!("адрес перевода: {e}")))?;
    let bytes = dl.get_bytes(url.as_str()).await?;
    parse(&bytes)
}

/// Ответ: `[[["перевод","оригинал",…],…],…]` — склеить переводы кусков.
fn parse(bytes: &[u8]) -> Result<String> {
    let v: Value = serde_json::from_slice(bytes)
        .map_err(|e| Error::Other(format!("непонятный ответ переводчика: {e}")))?;
    let parts = v[0]
        .as_array()
        .ok_or_else(|| Error::Other("переводчик ничего не вернул".into()))?;
    Ok(parts
        .iter()
        .filter_map(|p| p[0].as_str())
        .collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_google_answer() {
        let answer = r#"[[["Привет, мир.\n","Hello, world.\n",null,null,10],["Это мод.","This is a mod.",null,null,10]],null,"en"]"#;
        assert_eq!(parse(answer.as_bytes()).unwrap(), "Привет, мир.\nЭто мод.");
        assert!(parse(b"oops").is_err());
        assert!(!needs("1.21.1 · 42%"));
        assert!(needs("Adds dragons"));
        assert_eq!(single_line("  a\n b  "), "a b");
    }
}
