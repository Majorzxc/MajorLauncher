//! ИИ-разбор ошибок (раздел 5.8 ТЗ): лог и разбор анализатора уходят в сервис,
//! который выбрал игрок, — его ключом, своих серверов у лаунчера нет.
//!
//! Выключено, пока игрок сам не включит: данные уходят в чужой сервис
//! (решение автора 18.10.2026). Перед отправкой лог очищается от личных данных
//! ([`sanitize`]): имя пользователя Windows в путях, ник и UUID игрока, IP,
//! адреса серверов, токены, почта. По кнопке игрок видит ровно тот текст,
//! что уйдёт.
//!
//! Два протокола: Messages API Anthropic и OpenAI-совместимый
//! `chat/completions` — его понимают OpenAI, Gemini (адрес `…/openai`),
//! OpenRouter, Ollama, LM Studio, DeepSeek, Groq, Mistral и другие.

use std::time::Duration;

use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::analyzer::Analysis;
use crate::error::{Error, Result};
use crate::paths::DataDir;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provider {
    #[default]
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "openrouter")]
    OpenRouter,
    #[serde(rename = "ollama")]
    Ollama,
    #[serde(rename = "lmstudio")]
    LmStudio,
    /// Любой OpenAI-совместимый: адрес, ключ и модель — свои.
    #[serde(rename = "custom")]
    Custom,
}

impl Provider {
    pub fn id(self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::OpenAi => "openai",
            Provider::Gemini => "gemini",
            Provider::OpenRouter => "openrouter",
            Provider::Ollama => "ollama",
            Provider::LmStudio => "lmstudio",
            Provider::Custom => "custom",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Provider::Anthropic => "Claude",
            Provider::OpenAi => "ChatGPT",
            Provider::Gemini => "Gemini",
            Provider::OpenRouter => "OpenRouter",
            Provider::Ollama => "Ollama",
            Provider::LmStudio => "LM Studio",
            Provider::Custom => "Свой сервис",
        }
    }

    /// Адрес API без `/messages` и `/chat/completions`.
    pub fn default_url(self) -> &'static str {
        match self {
            Provider::Anthropic => "https://api.anthropic.com/v1",
            Provider::OpenAi => "https://api.openai.com/v1",
            Provider::Gemini => "https://generativelanguage.googleapis.com/v1beta/openai",
            Provider::OpenRouter => "https://openrouter.ai/api/v1",
            Provider::Ollama => "http://localhost:11434/v1",
            Provider::LmStudio => "http://localhost:1234/v1",
            Provider::Custom => "",
        }
    }

    /// Локальные модели ключа не требуют.
    pub fn needs_key(self) -> bool {
        !matches!(self, Provider::Ollama | Provider::LmStudio)
    }

    /// Где лежит ключ в «Диспетчере учётных данных».
    pub fn secret(self) -> String {
        format!("ai-{}", self.id())
    }
}

/// «Настройки → ИИ-разбор».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiSettings {
    /// Выключено по умолчанию: лог уходит в чужой сервис.
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub provider: Provider,
    /// Пусто — модель сервиса по умолчанию (есть только у Claude).
    #[serde(default)]
    pub model: String,
    /// Пусто — адрес сервиса по умолчанию; у «Своего сервиса» обязателен.
    #[serde(default)]
    pub url: String,
    /// Показывать перед отправкой по кнопке, что именно уйдёт.
    #[serde(default = "yes")]
    pub preview: bool,
    /// Сам после игры: упала, а правила причину не узнали.
    #[serde(default)]
    pub auto_unknown: bool,
    /// Сам после игры: любая критическая ошибка.
    #[serde(default)]
    pub auto_critical: bool,
    /// Сам после игры: были предупреждения.
    #[serde(default)]
    pub auto_warnings: bool,
    /// Сколько раз в день лаунчер может спросить сам (кнопка не считается).
    #[serde(default = "default_limit")]
    pub daily_limit: u32,
}

fn yes() -> bool {
    true
}

fn default_limit() -> u32 {
    5
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: Provider::default(),
            model: String::new(),
            url: String::new(),
            preview: true,
            auto_unknown: false,
            auto_critical: false,
            auto_warnings: false,
            daily_limit: default_limit(),
        }
    }
}

/// Модель Claude, если игрок не выбрал другую.
pub const CLAUDE_MODEL: &str = "claude-opus-5-5";

impl AiSettings {
    pub fn url(&self) -> String {
        let url = if self.url.trim().is_empty() {
            self.provider.default_url()
        } else {
            self.url.trim()
        };
        url.trim_end_matches('/').to_string()
    }

    pub fn model(&self) -> String {
        match self.model.trim() {
            "" if self.provider == Provider::Anthropic => CLAUDE_MODEL.to_string(),
            m => m.to_string(),
        }
    }

    /// Готово ли к запросу (без проверки ключа).
    pub fn check(&self) -> Result<()> {
        if self.url().is_empty() {
            return Err(Error::Other(
                "укажите адрес сервиса в «Настройки → ИИ-разбор»".into(),
            ));
        }
        if !self.url().starts_with("https://") && !self.url().starts_with("http://") {
            return Err(Error::Other(
                "адрес сервиса должен начинаться с https:// или http://".into(),
            ));
        }
        if self.model().is_empty() {
            return Err(Error::Other(
                "выберите модель в «Настройки → ИИ-разбор»".into(),
            ));
        }
        Ok(())
    }

    /// После этой игры спросить ИИ самому?
    pub fn auto_for(&self, analysis: &Analysis, crashed: bool) -> bool {
        self.enabled
            && ((self.auto_unknown && crashed && analysis.critical.is_empty())
                || (self.auto_critical && !analysis.critical.is_empty())
                || (self.auto_warnings && !analysis.warnings.is_empty()))
    }
}

/// Ответ ИИ.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Answer {
    /// Секунды с 1970 года.
    pub at: u64,
    /// Сервис и модель — «Claude · claude-opus-5-5».
    pub by: String,
    /// Markdown.
    pub text: String,
    /// Ответ оборвался на лимите длины.
    #[serde(default)]
    pub truncated: bool,
    /// Спросил сам лаунчер после игры.
    #[serde(default)]
    pub auto: bool,
}

const SYSTEM: &str = "Ты — помощник лаунчера Minecraft MajorLauncher. Тебе присылают сведения о сборке, \
разбор встроенного анализатора лаунчера и выдержку из лога игры, которая упала или работала с ошибками. \
Найди причину и ответь игроку по-русски, простыми словами: игрок — не программист.\n\n\
Формат ответа — Markdown без таблиц, коротко:\n\
1. **Что случилось** — одна-две фразы.\n\
2. **Причина** — какой мод, библиотека, Java, память или драйвер виноваты и почему; приведи 1–3 строки лога, \
на которые опираешься.\n\
3. **Что сделать** — шаги по порядку. В MajorLauncher у сборки есть вкладка «Моды» (включить, выключить, \
обновить мод), каталог Modrinth (поставить мод или нужную версию), настройки сборки (память, аргументы Java, \
версия Java) и снапшоты (откатить сборку к прошлому состоянию).\n\n\
Если причина неочевидна — так и скажи и перечисли варианты, начиная с самого вероятного. Не выдумывай моды и \
версии, которых нет в присланных данных. Если не согласен с разбором анализатора — объясни почему. \
Личные данные в логе заменены метками вроде <user>, <player>, <ip> — это не ошибка, не упоминай их.";

/// Сведения для запроса — собирает окно из сборки и её логов.
pub struct Request<'a> {
    pub version: &'a str,
    pub loader: &'a str,
    pub memory_mb: Option<u32>,
    /// Файлы включённых модов.
    pub mods: Vec<String>,
    pub disabled: usize,
    pub analysis: &'a Analysis,
    /// Упала ли игра; `None` — игрок разбирает любой лог.
    pub crashed: Option<bool>,
    /// Имя файла и текст отчёта о падении.
    pub crash_report: Option<(String, String)>,
    pub log: Option<String>,
}

/// Больше этого из лога и отчёта не отправляем (~15 тыс. токенов).
const LOG_MAX: usize = 40_000;
const REPORT_MAX: usize = 30_000;
const MODS_MAX: usize = 400;

/// Текст запроса — до очистки от личных данных.
pub fn prompt(r: &Request) -> String {
    let mut out = String::new();
    out += &format!(
        "## Сборка\nMinecraft {}, {}{}. Модов включено: {}{}.\n",
        r.version,
        r.loader,
        r.memory_mb
            .map_or(String::new(), |m| format!(", память {m} МБ")),
        r.mods.len(),
        if r.disabled > 0 {
            format!(", выключено: {}", r.disabled)
        } else {
            String::new()
        },
    );
    out += match r.crashed {
        Some(true) => "Игра закрылась с ошибкой.\n",
        Some(false) => "Игра закрылась сама (не упала), но в логе были ошибки.\n",
        None => "Игрок просит разобрать этот лог.\n",
    };

    out += "\n## Разбор лаунчера\n";
    if r.analysis.critical.is_empty() && r.analysis.warnings.is_empty() {
        out += "Встроенные правила ничего не узнали.\n";
    }
    for (level, list) in [
        ("Критично", &r.analysis.critical),
        ("Предупреждение", &r.analysis.warnings),
    ] {
        for f in list {
            out += &format!("- {level}: {} — {}", f.title, f.text);
            if let Some(c) = &f.culprit {
                out += &format!(" (виновник: {}", c.name);
                if let Some(file) = &c.file {
                    out += &format!(", {file}");
                }
                out += ")";
            }
            out += "\n";
        }
    }

    if !r.mods.is_empty() {
        out += "\n## Моды (файлы)\n";
        let shown = r.mods.len().min(MODS_MAX);
        out += &r.mods[..shown].join("\n");
        if r.mods.len() > shown {
            out += &format!("\n… и ещё {}", r.mods.len() - shown);
        }
        out += "\n";
    }

    if let Some((file, text)) = &r.crash_report {
        out += &format!("\n## Отчёт о падении ({file})\n```\n");
        out += &head(text, REPORT_MAX);
        out += "\n```\n";
    }
    if let Some(log) = &r.log {
        let marks: Vec<&str> = r
            .analysis
            .critical
            .iter()
            .chain(&r.analysis.warnings)
            .flat_map(|f| f.lines.iter().map(String::as_str))
            .collect();
        out += "\n## Лог игры (выдержка)\n```\n";
        out += &excerpt(log, &marks, LOG_MAX);
        out += "\n```\n";
    }
    out
}

/// Начало текста не длиннее `max` байт (по границе символа).
fn head(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n… (дальше обрезано)", &text[..end])
}

/// Выдержка из лога: начало (версии), ошибки со стеком, строки, по которым
/// узнал анализатор, и конец. Пропуски помечены «…».
pub fn excerpt(log: &str, marks: &[&str], max: usize) -> String {
    let lines: Vec<&str> = log.lines().collect();
    let n = lines.len();
    if log.len() <= max {
        return log.trim_end().to_string();
    }
    // Важность строки: 3 — конец лога, 2 — ошибки и их стек, 1 — начало.
    let mut weight = vec![0u8; n];
    for w in weight.iter_mut().take(25) {
        *w = 1;
    }
    for w in weight.iter_mut().skip(n.saturating_sub(150)) {
        *w = 3;
    }
    let error = |l: &str| {
        l.contains("/ERROR]")
            || l.contains("/FATAL]")
            || l.contains("[ERROR]")
            || l.contains("Exception")
            || l.contains("Caused by:")
            || marks.iter().any(|m| !m.is_empty() && l.contains(m))
    };
    let mut i = 0;
    while i < n {
        if error(lines[i]) {
            weight[i] = weight[i].max(2);
            // Стек под ошибкой — до 15 строк.
            let mut j = i + 1;
            while j < n && j <= i + 15 {
                let t = lines[j].trim_start();
                if !(t.starts_with("at ") || t.starts_with("Caused by") || t.starts_with("...")) {
                    break;
                }
                weight[j] = weight[j].max(2);
                j += 1;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    // Не влезает — сначала выкидываем начало, потом ранние ошибки.
    let size = |w: &[u8], min: u8| -> usize {
        (0..n)
            .filter(|&k| w[k] >= min)
            .map(|k| lines[k].len() + 1)
            .sum()
    };
    let mut min = 1;
    if size(&weight, 1) > max {
        min = 2;
    }
    if size(&weight, min) > max {
        let mut budget = size(&weight, 3);
        for k in (0..n).rev() {
            if weight[k] == 2 {
                if budget + lines[k].len() + 1 > max {
                    weight[k] = 0;
                } else {
                    budget += lines[k].len() + 1;
                }
            }
        }
    }
    let mut out = String::new();
    let mut gap = false;
    let mut repeat: Option<(&str, usize)> = None;
    for k in 0..n {
        if weight[k] < min {
            gap = true;
            continue;
        }
        // Одинаковые строки подряд — одной.
        match &mut repeat {
            Some((last, count)) if *last == lines[k] && !gap => {
                *count += 1;
                continue;
            }
            Some((_, count)) if *count > 1 => {
                out += &format!("(повторилось ещё {} раз)\n", *count - 1);
            }
            _ => {}
        }
        if gap && !out.is_empty() {
            out += "…\n";
        }
        gap = false;
        repeat = Some((lines[k], 1));
        out += lines[k];
        out += "\n";
    }
    if let Some((_, count)) = repeat
        && count > 1
    {
        out += &format!("(повторилось ещё {} раз)\n", count - 1);
    }
    head(out.trim_end(), max)
}

/// Что заменить в тексте: (строка, метка) — ник, UUID, имя ПК.
pub type Private = Vec<(String, &'static str)>;

/// Имя этого компьютера. Имя пользователя Windows меняется только в путях
/// (`C:\Users\…`): оно бывает обычным словом («User», «Admin»), и замена
/// по всему тексту испортила бы лог.
pub fn system_private() -> Private {
    std::env::var("COMPUTERNAME")
        .map(|v| vec![(v, "<pc>")])
        .unwrap_or_default()
}

/// Убрать личные данные: имя пользователя в путях, ник и UUID игрока, IP,
/// адреса серверов, токены и пароли, почту.
pub fn sanitize(text: &str, private: &Private) -> String {
    use std::sync::LazyLock;
    static PATH: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r#"(?i)([a-z]:[\\/]+(?:users|documents and settings)[\\/]+)([^\\/\r\n"'<>:|*?]+)"#,
        )
        .unwrap()
    });
    static HOME: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(/home/|/Users/)([^/\s]+)").unwrap());
    static JWT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}").unwrap()
    });
    static SECRET: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r#"(?i)(access[_-]?token|client[_-]?token|session[_-]?id|token|password|passwd|api[_-]?key|secret)(["']?\s*[:=]\s*["']?)([^\s"',;)\]]{8,})"#,
        )
        .unwrap()
    });
    static ARG: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)(--(?:accessToken|clientId|xuid|session)\s+)(\S+)").unwrap()
    });
    static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}").unwrap()
    });
    static IP: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(^|[^\w.])((?:\d{1,3}\.){3}\d{1,3})(:\d{1,5})?($|[^\w.])").unwrap()
    });
    static SERVER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)(connecting to |connect(?:ed|ing)? to server |server address: )([^\s,]+)")
            .unwrap()
    });

    // Ник из самого лога: аккаунт мог быть и не из лаунчера.
    static NICK: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)(Setting user: |Username: |\bplayer name: )([A-Za-z0-9_]{3,16})\b")
            .unwrap()
    });
    let mut private = private.clone();
    for c in NICK.captures_iter(text) {
        private.push((c[2].to_string(), "<player>"));
    }
    let private = &private;

    // Чат — чужие ники и переписка, для разбора ошибки не нужен.
    static CHAT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\[CHAT\] ).*").unwrap());

    let mut s = CHAT.replace_all(text, "${1}<chat>").into_owned();
    s = PATH.replace_all(&s, "${1}<user>").into_owned();
    s = HOME.replace_all(&s, "${1}<user>").into_owned();
    s = JWT.replace_all(&s, "<token>").into_owned();
    s = SECRET.replace_all(&s, "${1}${2}<hidden>").into_owned();
    s = ARG.replace_all(&s, "${1}<hidden>").into_owned();
    s = EMAIL.replace_all(&s, "<email>").into_owned();
    s = SERVER.replace_all(&s, "${1}<server>").into_owned();
    // IP. Четыре числа через точку бывают и версией («Java 17.0.10.7»),
    // поэтому без порта адрес меняется, только если строка — о сети.
    s = s
        .split_inclusive('\n')
        .map(|line| {
            let lower = line.to_lowercase();
            let network = [
                "connect",
                "address",
                " ip ",
                " ip:",
                "ip=",
                "host",
                "server",
                "proxy",
                "logging in",
            ]
            .iter()
            .any(|w| lower.contains(w));
            let mut line = line.to_string();
            // По два раза — соседние адреса делят разделитель.
            for _ in 0..2 {
                line = IP
                    .replace_all(&line, |c: &Captures| {
                        let ip = &c[2];
                        let valid = ip
                            .split('.')
                            .all(|o| o.parse::<u16>().is_ok_and(|v| v <= 255));
                        let port = c.get(3).is_some();
                        if !valid || ip == "127.0.0.1" || ip == "0.0.0.0" || !(port || network) {
                            return c[0].to_string();
                        }
                        format!("{}<ip>{}", &c[1], &c[4])
                    })
                    .into_owned();
            }
            line
        })
        .collect();
    // Ник, UUID, имя пользователя и ПК — где угодно в тексте. Короткие (до
    // трёх букв) не трогаем: заменят половину лога.
    let mut private: Vec<&(String, &str)> = private
        .iter()
        .filter(|(v, _)| v.chars().count() >= 3)
        .collect();
    private.sort_by_key(|(v, _)| std::cmp::Reverse(v.len()));
    for (value, mark) in private {
        let mut variants = vec![value.clone()];
        if value.len() == 36 && value.matches('-').count() == 4 {
            variants.push(value.replace('-', ""));
        }
        for v in variants {
            s = replace_word(&s, &v, mark);
        }
    }
    s
}

/// Заменить `value` целым словом (без учёта регистра): не часть другого
/// слова и не уже поставленная метка `<…>`.
fn replace_word(text: &str, value: &str, mark: &str) -> String {
    let Ok(re) = Regex::new(&format!("(?i){}", regex::escape(value))) else {
        return text.to_string();
    };
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for m in re.find_iter(text) {
        let before = text[..m.start()].chars().next_back();
        let after = text[m.end()..].chars().next();
        if word(before) || word(after) || (before == Some('<') && after == Some('>')) {
            continue;
        }
        out += &text[last..m.start()];
        out += mark;
        last = m.end();
    }
    out += &text[last..];
    out
}

/// Ники, UUID и логины аккаунтов лаунчера плюс имя пользователя и ПК.
pub fn private(data: &DataDir) -> Private {
    let mut out = system_private();
    if let Ok(accounts) = crate::account::Accounts::load(data) {
        for a in accounts.list() {
            out.push((a.name.clone(), "<player>"));
            out.push((a.uuid.clone(), "<player-uuid>"));
            if let Some(login) = &a.login {
                out.push((login.clone(), "<login>"));
            }
        }
    }
    out
}

/// Собрать запрос по сборке и очистить его. `file` — лог или отчёт из
/// вкладки «Логи»; `None` — последняя игра: её разбор, `logs/latest.log` и
/// отчёт о падении.
pub async fn gather(
    dl: &crate::download::Downloader,
    data: &DataDir,
    build: &str,
    file: Option<&str>,
) -> Result<String> {
    use crate::analyzer;
    use crate::content::read_log;

    let instance = crate::builds::Builds::new(data.clone()).get(build)?;
    let game = data.instance(build);
    let (analysis, crashed, report, log) = match file {
        Some(f) => {
            let a = analyzer::analyze_file(dl, data, build, f).await?;
            if f.starts_with("crash-reports/") {
                (a, None, Some(f.to_string()), None)
            } else {
                (a, None, None, Some(f.to_string()))
            }
        }
        None => {
            let s = analyzer::last_session(data, build).ok_or_else(|| {
                Error::Other("у этой сборки ещё нет разбора последней игры".into())
            })?;
            let report = s
                .analysis
                .crash_report
                .as_ref()
                .map(|r| format!("crash-reports/{r}"));
            (
                s.analysis,
                Some(s.crashed),
                report,
                Some("logs/latest.log".to_string()),
            )
        }
    };
    let crash_report = match &report {
        Some(f) => Some((
            f.trim_start_matches("crash-reports/").to_string(),
            read_log(&game, f)?.text,
        )),
        None => None,
    };
    let log = match &log {
        Some(f) => read_log(&game, f).ok().map(|l| l.text),
        None => None,
    };
    let local = {
        let (data, build) = (data.clone(), build.to_string());
        tokio::task::spawn_blocking(move || crate::mods::local(&data, &build))
            .await
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or_default()
    };
    let mut mods: Vec<String> = local
        .iter()
        .filter(|m| m.enabled)
        .map(|m| m.file.clone())
        .collect();
    mods.sort_by_key(|m| m.to_lowercase());
    let loader = match &instance.loader_version {
        Some(v) => format!("{} {v}", instance.loader.title()),
        None => instance.loader.title().to_string(),
    };
    let text = prompt(&Request {
        version: &instance.version,
        loader: &loader,
        memory_mb: instance.settings.memory_mb,
        disabled: local.len() - mods.len(),
        mods,
        analysis: &analysis,
        crashed,
        crash_report,
        log,
    });
    let private = {
        let data = data.clone();
        tokio::task::spawn_blocking(move || private(&data))
            .await
            .unwrap_or_default()
    };
    Ok(sanitize(&text, &private))
}

/// Текст для отправки: размер для окна («уйдёт 24 КБ, ~6 тыс. токенов»).
#[derive(Debug, Clone, Serialize)]
pub struct Prepared {
    pub text: String,
    pub bytes: usize,
    /// Примерно: ~4 байта на токен.
    pub tokens: usize,
    /// Куда уйдёт: «Claude · claude-opus-5-5».
    pub to: String,
}

pub fn prepared(settings: &AiSettings, text: String) -> Prepared {
    Prepared {
        bytes: text.len(),
        tokens: text.len() / 4,
        to: format!("{} · {}", settings.provider.title(), settings.model()),
        text,
    }
}

fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(format!("MajorLauncher/{}", crate::VERSION))
        .connect_timeout(Duration::from_secs(15))
        // Модели с размышлением отвечают до пары минут.
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| Error::Other(format!("не удалось создать HTTP-клиент: {e}")))
}

fn auth(
    req: reqwest::RequestBuilder,
    settings: &AiSettings,
    key: Option<&str>,
) -> reqwest::RequestBuilder {
    let Some(key) = key.filter(|k| !k.is_empty()) else {
        return req;
    };
    match settings.provider {
        Provider::Anthropic => req
            .header("x-api-key", key)
            .header("anthropic-version", "2026-10-21"),
        _ => req.bearer_auth(key),
    }
}

/// Ошибка сервиса — понятными словами и с его собственным текстом.
fn service_error(settings: &AiSettings, status: u16, body: &[u8]) -> Error {
    let detail = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|v| {
            let e = v.get("error")?;
            e.get("message")
                .and_then(Value::as_str)
                .or(e.as_str())
                .map(str::to_string)
        })
        .unwrap_or_default();
    let detail: String = detail.chars().take(300).collect();
    let what = match status {
        401 | 403 => "сервис не принял ключ — проверьте его в «Настройки → ИИ-разбор»",
        404 => "нет такой модели или адреса — проверьте их в настройках",
        402 | 429 => "сервис отказал из-за лимита запросов или нехватки средств на счёте",
        400 | 413 | 422 => "сервис не принял запрос",
        500..=599 => "сервис сейчас не отвечает или перегружен — попробуйте позже",
        _ => "сервис ответил ошибкой",
    };
    let name = settings.provider.title();
    if detail.is_empty() {
        Error::Other(format!("{name}: {what} (код {status})"))
    } else {
        Error::Other(format!("{name}: {what} (код {status}): {detail}"))
    }
}

fn network_error(settings: &AiSettings, e: reqwest::Error) -> Error {
    let name = settings.provider.title();
    if e.is_timeout() {
        return Error::Other(format!("{name}: ответ не пришёл за 5 минут"));
    }
    if e.is_connect() && !settings.provider.needs_key() {
        return Error::Other(format!(
            "не удалось подключиться к {} — запущен ли {name}?",
            settings.url()
        ));
    }
    Error::Other(format!("{name}: нет связи с {} ({e})", settings.url()))
}

async fn post(settings: &AiSettings, key: Option<&str>, path: &str, body: &Value) -> Result<Value> {
    let req = client()?
        .post(format!("{}{path}", settings.url()))
        .json(body);
    let mut req = auth(req, settings, key);
    if settings.provider == Provider::OpenRouter {
        req = req
            .header("HTTP-Referer", "https://github.com/Majorzxc/MajorLauncher")
            .header("X-Title", "MajorLauncher");
    }
    let resp = req.send().await.map_err(|e| network_error(settings, e))?;
    let status = resp.status().as_u16();
    let bytes = resp.bytes().await.map_err(|e| network_error(settings, e))?;
    if !(200..300).contains(&status) {
        return Err(service_error(settings, status, &bytes));
    }
    serde_json::from_slice(&bytes).map_err(|_| {
        Error::Other(format!(
            "{}: непонятный ответ сервиса",
            settings.provider.title()
        ))
    })
}

/// Спросить ИИ. `text` — уже очищенный запрос (тот, что видел игрок).
pub async fn ask(settings: &AiSettings, key: Option<&str>, text: &str) -> Result<Answer> {
    settings.check()?;
    if settings.provider.needs_key() && key.is_none_or(str::is_empty) {
        return Err(Error::Other(format!(
            "нет ключа {} — добавьте его в «Настройки → ИИ-разбор»",
            settings.provider.title()
        )));
    }
    let model = settings.model();
    let (answer, truncated) = match settings.provider {
        Provider::Anthropic => {
            let body = json!({
                "model": model,
                "max_tokens": 16000,
                "system": SYSTEM,
                "messages": [{ "role": "user", "content": text }],
            });
            let v = post(settings, key, "/messages", &body).await?;
            let stop = v.get("stop_reason").and_then(Value::as_str).unwrap_or("");
            if stop == "refusal" {
                return Err(Error::Other(
                    "Claude отказался разбирать этот лог — попробуйте другую модель".into(),
                ));
            }
            let text: Vec<&str> = v
                .get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect();
            (text.join("\n\n"), stop == "max_tokens")
        }
        _ => {
            let body = json!({
                "model": model,
                "messages": [
                    { "role": "system", "content": SYSTEM },
                    { "role": "user", "content": text },
                ],
            });
            let v = post(settings, key, "/chat/completions", &body).await?;
            let choice = v.pointer("/choices/0");
            let text = choice
                .and_then(|c| c.pointer("/message/content"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let finish = choice
                .and_then(|c| c.get("finish_reason"))
                .and_then(Value::as_str)
                .unwrap_or("");
            (text, finish == "length")
        }
    };
    let answer = strip_thinking(&answer);
    if answer.trim().is_empty() {
        return Err(Error::Other(format!(
            "{}: пустой ответ — попробуйте другую модель",
            settings.provider.title()
        )));
    }
    Ok(Answer {
        at: crate::builds::now(),
        by: format!("{} · {model}", settings.provider.title()),
        text: answer.trim().to_string(),
        truncated,
        auto: false,
    })
}

/// Локальные модели с размышлением (DeepSeek R1, Qwen) пишут его в ответ
/// внутри `<think>…</think>` — игроку оно не нужно.
fn strip_thinking(text: &str) -> String {
    let mut s = text.to_string();
    while let Some(start) = s.find("<think>") {
        match s[start..].find("</think>") {
            Some(end) => s.replace_range(start..start + end + "</think>".len(), ""),
            None => break,
        }
    }
    s
}

/// Модели сервиса — для выбора в настройках.
pub async fn models(settings: &AiSettings, key: Option<&str>) -> Result<Vec<String>> {
    if settings.url().is_empty() {
        return Err(Error::Other("сначала укажите адрес сервиса".into()));
    }
    let req = client()?.get(format!("{}/models", settings.url()));
    let resp = auth(req, settings, key)
        .send()
        .await
        .map_err(|e| network_error(settings, e))?;
    let status = resp.status().as_u16();
    let bytes = resp.bytes().await.map_err(|e| network_error(settings, e))?;
    if !(200..300).contains(&status) {
        return Err(service_error(settings, status, &bytes));
    }
    let v: Value = serde_json::from_slice(&bytes).map_err(|_| {
        Error::Other(format!(
            "{}: непонятный список моделей",
            settings.provider.title()
        ))
    })?;
    let mut list: Vec<String> = v
        .get("data")
        .or(v.get("models"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|m| {
            m.get("id")
                .or(m.get("name"))
                .and_then(Value::as_str)
                .map(|s| s.trim_start_matches("models/").to_string())
        })
        .collect();
    list.dedup();
    if settings.provider != Provider::Anthropic {
        list.sort();
    }
    Ok(list)
}

/// Сколько раз сегодня лаунчер спросил сам: `cache/ai-usage.json`.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Usage {
    day: u64,
    count: u32,
}

fn usage_path(data: &DataDir) -> std::path::PathBuf {
    data.cache().join("ai-usage.json")
}

fn today() -> u64 {
    crate::builds::now() / 86_400
}

/// Занять один автоматический запрос, если лимит дня не исчерпан.
pub fn take_auto(data: &DataDir, limit: u32) -> bool {
    let path = usage_path(data);
    let mut u: Usage = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    if u.day != today() {
        u = Usage {
            day: today(),
            count: 0,
        };
    }
    if u.count >= limit {
        return false;
    }
    u.count += 1;
    let _ = std::fs::create_dir_all(data.cache());
    std::fs::write(&path, serde_json::to_vec(&u).expect("сериализуется")).is_ok()
}

/// Сколько автоматических запросов уже было сегодня.
pub fn used_today(data: &DataDir) -> u32 {
    std::fs::read(usage_path(data))
        .ok()
        .and_then(|b| serde_json::from_slice::<Usage>(&b).ok())
        .filter(|u| u.day == today())
        .map_or(0, |u| u.count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn private() -> Private {
        vec![
            ("Majorzxc".into(), "<player>"),
            (
                "0f7a1c2e-3b4d-4e5f-8a9b-0c1d2e3f4a5b".into(),
                "<player-uuid>",
            ),
            ("Иван".into(), "<user>"),
            ("DESKTOP-K2".into(), "<pc>"),
        ]
    }

    #[test]
    fn personal_data_removed() {
        let log = "\
[12:00:01] [main/INFO]: Setting user: Majorzxc
[12:00:01] [main/INFO]: UUID 0f7a1c2e3b4d4e5f8a9b0c1d2e3f4a5b
Loading C:\\Users\\Иван\\AppData\\Roaming\\x.jar and D:/Users/ivan/game
--accessToken eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N
token: abcdef1234567890 password=hunter2hunter2
Connecting to mc.example.org, 25565
Lost connection 192.168.1.15:25565 and 10.0.0.1 local 127.0.0.1
[Render thread/INFO]: [System] [CHAT] <Notch> hi Majorzxc, my home is at 100 64 200
mail me: player.one+mc@gmail.com on DESKTOP-K2
Fabric Loader 0.15.11 for 1.20.1, Java 17.0.10.7 com.mojang:authlib:4.0.43";
        let s = sanitize(log, &private());
        for leak in [
            "Majorzxc",
            "0f7a1c2e",
            "Иван",
            "ivan",
            "eyJhbGci",
            "abcdef1234567890",
            "hunter2",
            "mc.example.org",
            "192.168",
            "10.0.0.1",
            "player.one",
            "DESKTOP-K2",
            "Notch",
            "my home",
        ] {
            assert!(!s.contains(leak), "осталось «{leak}»:\n{s}");
        }
        assert!(s.contains("Setting user: <player>"));
        assert!(s.contains("[CHAT] <chat>"));
        assert!(s.contains(r"C:\Users\<user>\AppData"));
        assert!(s.contains("127.0.0.1"), "localhost — не личное");
        // Версии не путаются с IP.
        assert!(s.contains("Java 17.0.10.7"), "{s}");
        assert!(s.contains("0.15.11 for 1.20.1"));
        assert!(s.contains("authlib:4.0.43"));
    }

    /// Имя пользователя Windows «User» — обычное слово: меняется только в
    /// путях. Ник берётся и из строки «Setting user».
    #[test]
    fn common_words_kept() {
        let log =
            "Setting user: Steve_77\nUser settings loaded for Steve_77 from C:\\Users\\User\\x";
        let s = sanitize(log, &vec![("PC-1".into(), "<pc>")]);
        assert_eq!(
            s,
            "Setting user: <player>\nUser settings loaded for <player> from C:\\Users\\<user>\\x"
        );
        // Метка не заменяется второй раз, даже если совпала с ником.
        let s = sanitize("player", &vec![("player".into(), "<player>")]);
        assert_eq!(s, "<player>");
    }

    #[test]
    fn short_names_kept() {
        let s = sanitize("Loaded mod ab and Mixin", &vec![("ab".into(), "<player>")]);
        assert_eq!(s, "Loaded mod ab and Mixin");
    }

    #[test]
    fn excerpt_keeps_errors_and_tail() {
        let mut log = String::new();
        log += "[main/INFO]: Loading Minecraft 1.20.1\n";
        for i in 0..3000 {
            log += &format!("[Render thread/INFO]: noise line {i} with some padding text\n");
            if i == 1500 {
                log += "[Render thread/ERROR]: Mixin apply failed foo.mixins.json\n";
                log += "java.lang.RuntimeException: boom\n\tat com.foo.Bar.baz(Bar.java:10)\n";
            }
        }
        log += "[Render thread/ERROR]: same\n".repeat(5).as_str();
        log += "---- end ----\n";
        let e = excerpt(&log, &["Mixin apply failed"], 20_000);
        assert!(e.len() <= 20_000 + 40);
        assert!(e.starts_with("[main/INFO]: Loading Minecraft 1.20.1"));
        assert!(e.contains("Mixin apply failed"));
        assert!(e.contains("at com.foo.Bar.baz"));
        assert!(e.contains("---- end ----"));
        assert!(e.contains("повторилось ещё 4 раз"));
        assert!(e.contains("…"));
        assert!(!e.contains("noise line 100 "));
    }

    #[test]
    fn settings_defaults() {
        let d: AiSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(d, AiSettings::default());
        assert!(!d.enabled && d.preview && d.daily_limit == 5);
        assert_eq!(d.model(), CLAUDE_MODEL);
        assert_eq!(d.url(), "https://api.anthropic.com/v1");
        let custom = AiSettings {
            provider: Provider::Custom,
            ..d.clone()
        };
        assert!(custom.check().is_err(), "у своего сервиса нужен адрес");
        let auto = AiSettings {
            enabled: true,
            auto_unknown: true,
            ..d
        };
        let empty = Analysis::default();
        assert!(auto.auto_for(&empty, true));
        assert!(!auto.auto_for(&empty, false));
    }

    #[test]
    fn auto_limit() {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path());
        assert!(take_auto(&data, 2));
        assert!(take_auto(&data, 2));
        assert!(!take_auto(&data, 2));
        assert_eq!(used_today(&data), 2);
    }

    #[test]
    fn thinking_stripped() {
        assert_eq!(strip_thinking("<think>hm</think>Ответ"), "Ответ");
    }
}
