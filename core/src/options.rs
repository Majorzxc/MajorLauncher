//! Настройки игры (раздел 5.18 ТЗ): один общий профиль `options.txt` для всех
//! сборок. Перед запуском лаунчер записывает общие параметры в `options.txt`
//! сборки, после выхода забирает изменения обратно — поменял FOV в одной
//! сборке, он поменялся во всех.
//!
//! Разные версии хранят настройки по-разному: строки в кавычках с 1.19
//! (`mainHand:"right"`), язык маленькими буквами с 1.11 (`ru_ru`), клавиши
//! именами с 1.13 (`key.keyboard.w`). Профиль хранит значения как новые
//! версии, а при записи в сборку переводит их в её формат.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, IoContext, Result, json};
use crate::paths::DataDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Group {
    View,
    Controls,
    Keys,
    Sound,
    Chat,
    Interface,
    Language,
    Graphics,
    Packs,
}

impl Group {
    pub const ALL: [Group; 9] = [
        Group::View,
        Group::Controls,
        Group::Keys,
        Group::Sound,
        Group::Chat,
        Group::Interface,
        Group::Language,
        Group::Graphics,
        Group::Packs,
    ];

    /// Общие для всех сборок по умолчанию: всё, кроме графики — у тяжёлой
    /// сборки своя дальность прорисовки.
    pub fn shared_by_default(self) -> bool {
        self != Group::Graphics
    }
}

/// Вид параметра и как его показывать.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Kind {
    Bool,
    /// Ползунок. Показанное значение = хранимое × `mul` + `add`.
    Slider {
        min: f64,
        max: f64,
        step: f64,
        mul: f64,
        add: f64,
        unit: &'static str,
    },
    /// Целое число; `special` — подпись особого значения («Авто» у масштаба).
    Int {
        min: i64,
        max: i64,
        special: Option<(i64, &'static str)>,
    },
    /// Выбор из списка: значение и подпись. `quoted` — с 1.19 игра пишет
    /// значение в кавычках.
    Choice {
        options: &'static [(&'static str, &'static str)],
        quoted: bool,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct Setting {
    pub key: &'static str,
    pub group: Group,
    pub label: &'static str,
    #[serde(skip_serializing_if = "str::is_empty")]
    pub hint: &'static str,
    pub kind: Kind,
    /// Значение по умолчанию в игре.
    pub default: &'static str,
    /// С какой версии параметр есть в игре — для подписи «с 1.16.2».
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<&'static str>,
    /// Дата выхода этой версии — для сравнения с версией сборки.
    #[serde(skip)]
    pub since_date: &'static str,
}

const fn pct(mul: f64) -> Kind {
    Kind::Slider {
        min: 0.0,
        max: mul,
        step: 1.0,
        mul,
        add: 0.0,
        unit: "%",
    }
}

const fn s(
    key: &'static str,
    group: Group,
    label: &'static str,
    hint: &'static str,
    kind: Kind,
    default: &'static str,
    since: Option<(&'static str, &'static str)>,
) -> Setting {
    let (since, since_date) = match since {
        Some((v, d)) => (Some(v), d),
        None => (None, ""),
    };
    Setting {
        key,
        group,
        label,
        hint,
        kind,
        default,
        since,
        since_date,
    }
}

// Даты выхода версий: с какой версии есть параметр (minecraft.wiki/w/Options.txt).
const V1_7_2: (&str, &str) = ("1.7.2", "2013-10-25");
const V1_8: (&str, &str) = ("1.8", "2014-09-02");
const V1_9: (&str, &str) = ("1.9", "2016-02-29");
const V1_10: (&str, &str) = ("1.10", "2016-06-08");
const V1_13: (&str, &str) = ("1.13", "2018-07-18");
const V1_14: (&str, &str) = ("1.14", "2019-04-23");
const V1_15: (&str, &str) = ("1.15", "2019-12-10");
const V1_16: (&str, &str) = ("1.16", "2020-06-23");
const V1_16_2: (&str, &str) = ("1.16.2", "2020-08-11");
const V1_18: (&str, &str) = ("1.18", "2021-11-30");
const V1_19: (&str, &str) = ("1.19", "2022-06-07");
const V1_19_4: (&str, &str) = ("1.19.4", "2023-03-14");

/// С этой даты игра пишет строковые значения в кавычках.
const QUOTES_SINCE: &str = V1_19.1;
/// С этой даты язык записывается маленькими буквами (`ru_ru`, а не `ru_RU`).
const LOWERCASE_LANG_SINCE: &str = "2016-11-14";
/// С этой даты клавиши и ресурспаки хранятся в нынешнем формате.
const KEY_NAMES_SINCE: &str = V1_13.1;

use Group::*;

pub static SETTINGS: &[Setting] = &[
    // Обзор и эффекты — то, что в игре разбросано по «Настройкам», «Видео» и
    // «Специальным возможностям».
    s(
        "fov",
        View,
        "Поле зрения (FOV)",
        "",
        Kind::Slider {
            min: 30.0,
            max: 110.0,
            step: 1.0,
            mul: 40.0,
            add: 70.0,
            unit: "°",
        },
        "0.0",
        None,
    ),
    s(
        "fovEffectScale",
        View,
        "Эффекты поля зрения",
        "Насколько меняется обзор при беге, ускорении и зельях",
        pct(100.0),
        "1.0",
        Some(V1_16_2),
    ),
    s(
        "bobView",
        View,
        "Покачивание камеры",
        "При ходьбе",
        Kind::Bool,
        "true",
        None,
    ),
    s(
        "screenEffectScale",
        View,
        "Эффекты искажения",
        "Тошнота, портал в Незер",
        pct(100.0),
        "1.0",
        Some(V1_16_2),
    ),
    s(
        "damageTiltStrength",
        View,
        "Наклон камеры при уроне",
        "",
        pct(100.0),
        "1.0",
        Some(V1_19_4),
    ),
    s(
        "darknessEffectScale",
        View,
        "Пульсация эффекта «Тьма»",
        "Рядом с хранителем",
        pct(100.0),
        "1.0",
        Some(V1_19),
    ),
    s(
        "hideLightningFlashes",
        View,
        "Без вспышек молний",
        "",
        Kind::Bool,
        "false",
        Some(V1_18),
    ),
    s("gamma", View, "Яркость", "", pct(100.0), "0.5", None),
    s(
        "glintStrength",
        View,
        "Блеск зачарований",
        "",
        pct(100.0),
        "0.75",
        Some(V1_19_4),
    ),
    // Управление.
    s(
        "mouseSensitivity",
        Controls,
        "Чувствительность мыши",
        "100% — как в игре по умолчанию",
        pct(200.0),
        "0.5",
        None,
    ),
    s(
        "invertYMouse",
        Controls,
        "Инверсия мыши по вертикали",
        "",
        Kind::Bool,
        "false",
        None,
    ),
    s(
        "toggleCrouch",
        Controls,
        "Приседание переключением",
        "Нажал — присел, нажал ещё раз — встал",
        Kind::Bool,
        "false",
        Some(V1_15),
    ),
    s(
        "toggleSprint",
        Controls,
        "Бег переключением",
        "",
        Kind::Bool,
        "false",
        Some(V1_15),
    ),
    s(
        "autoJump",
        Controls,
        "Автопрыжок",
        "",
        Kind::Bool,
        "false",
        Some(V1_10),
    ),
    s(
        "mainHand",
        Controls,
        "Основная рука",
        "",
        Kind::Choice {
            options: &[("right", "Правая"), ("left", "Левая")],
            quoted: true,
        },
        "right",
        Some(V1_9),
    ),
    // Звук.
    s(
        "soundCategory_master",
        Sound,
        "Общая громкость",
        "",
        pct(100.0),
        "1.0",
        Some(V1_7_2),
    ),
    s(
        "soundCategory_music",
        Sound,
        "Музыка",
        "",
        pct(100.0),
        "1.0",
        Some(V1_7_2),
    ),
    s(
        "soundCategory_record",
        Sound,
        "Проигрыватели и нотные блоки",
        "",
        pct(100.0),
        "1.0",
        Some(V1_7_2),
    ),
    s(
        "soundCategory_weather",
        Sound,
        "Погода",
        "",
        pct(100.0),
        "1.0",
        Some(V1_7_2),
    ),
    s(
        "soundCategory_block",
        Sound,
        "Блоки",
        "",
        pct(100.0),
        "1.0",
        Some(V1_7_2),
    ),
    s(
        "soundCategory_hostile",
        Sound,
        "Враждебные существа",
        "",
        pct(100.0),
        "1.0",
        Some(V1_7_2),
    ),
    s(
        "soundCategory_neutral",
        Sound,
        "Дружелюбные существа",
        "",
        pct(100.0),
        "1.0",
        Some(V1_7_2),
    ),
    s(
        "soundCategory_player",
        Sound,
        "Игроки",
        "",
        pct(100.0),
        "1.0",
        Some(V1_7_2),
    ),
    s(
        "soundCategory_ambient",
        Sound,
        "Окружение",
        "",
        pct(100.0),
        "1.0",
        Some(V1_7_2),
    ),
    s(
        "soundCategory_voice",
        Sound,
        "Голос и речь",
        "",
        pct(100.0),
        "1.0",
        Some(V1_9),
    ),
    s(
        "showSubtitles",
        Sound,
        "Субтитры",
        "Подписи к звукам",
        Kind::Bool,
        "false",
        Some(V1_9),
    ),
    // Чат.
    s(
        "chatVisibility",
        Chat,
        "Чат",
        "",
        Kind::Choice {
            options: &[
                ("0", "Показывать"),
                ("1", "Только команды"),
                ("2", "Скрыть"),
            ],
            quoted: false,
        },
        "0",
        None,
    ),
    s(
        "chatOpacity",
        Chat,
        "Непрозрачность текста чата",
        "",
        pct(100.0),
        "1.0",
        None,
    ),
    s(
        "textBackgroundOpacity",
        Chat,
        "Непрозрачность фона чата",
        "",
        pct(100.0),
        "0.5",
        Some(V1_14),
    ),
    s(
        "chatScale",
        Chat,
        "Размер текста чата",
        "",
        pct(100.0),
        "1.0",
        None,
    ),
    s(
        "chatWidth",
        Chat,
        "Ширина чата",
        "",
        pct(100.0),
        "1.0",
        None,
    ),
    s(
        "chatLineSpacing",
        Chat,
        "Межстрочный интервал",
        "",
        pct(100.0),
        "0.0",
        Some(V1_16),
    ),
    s(
        "chatColors",
        Chat,
        "Цвета в чате",
        "",
        Kind::Bool,
        "true",
        None,
    ),
    s(
        "chatLinks",
        Chat,
        "Ссылки в чате",
        "",
        Kind::Bool,
        "true",
        None,
    ),
    s(
        "autoSuggestions",
        Chat,
        "Подсказки команд",
        "",
        Kind::Bool,
        "true",
        Some(V1_13),
    ),
    // Интерфейс.
    s(
        "guiScale",
        Interface,
        "Масштаб интерфейса",
        "",
        Kind::Int {
            min: 0,
            max: 6,
            special: Some((0, "Авто")),
        },
        "0",
        None,
    ),
    s(
        "attackIndicator",
        Interface,
        "Индикатор атаки",
        "",
        Kind::Choice {
            options: &[("1", "У прицела"), ("2", "На панели"), ("0", "Выключен")],
            quoted: false,
        },
        "1",
        Some(V1_9),
    ),
    s(
        "pauseOnLostFocus",
        Interface,
        "Пауза, когда окно свёрнуто",
        "",
        Kind::Bool,
        "true",
        None,
    ),
    s(
        "advancedItemTooltips",
        Interface,
        "Подробные подсказки предметов",
        "То же, что F3+H",
        Kind::Bool,
        "false",
        None,
    ),
    // Язык.
    s(
        "lang",
        Language,
        "Язык игры",
        "",
        Kind::Choice {
            options: &[
                ("ru_ru", "Русский"),
                ("en_us", "English"),
                ("uk_ua", "Українська"),
                ("be_by", "Беларуская"),
                ("kk_kz", "Қазақша"),
                ("de_de", "Deutsch"),
                ("pl_pl", "Polski"),
            ],
            quoted: false,
        },
        "en_us",
        None,
    ),
    // Графика — по умолчанию у каждой сборки своя.
    s(
        "renderDistance",
        Graphics,
        "Дальность прорисовки",
        "В чанках",
        Kind::Int {
            min: 2,
            max: 32,
            special: None,
        },
        "12",
        Some(V1_7_2),
    ),
    s(
        "simulationDistance",
        Graphics,
        "Дальность симуляции",
        "",
        Kind::Int {
            min: 5,
            max: 32,
            special: None,
        },
        "12",
        Some(V1_18),
    ),
    s(
        "maxFps",
        Graphics,
        "Ограничение кадров",
        "",
        Kind::Int {
            min: 10,
            max: 260,
            special: Some((260, "Без ограничения")),
        },
        "120",
        Some(V1_7_2),
    ),
    s(
        "enableVsync",
        Graphics,
        "Вертикальная синхронизация",
        "",
        Kind::Bool,
        "true",
        None,
    ),
    s(
        "graphicsMode",
        Graphics,
        "Графика",
        "",
        Kind::Choice {
            options: &[
                ("0", "Быстрая"),
                ("1", "Детальная"),
                ("2", "Сверхдетальная"),
            ],
            quoted: false,
        },
        "1",
        Some(V1_16),
    ),
    s(
        "renderClouds",
        Graphics,
        "Облака",
        "",
        Kind::Choice {
            options: &[
                ("true", "Детальные"),
                ("fast", "Быстрые"),
                ("false", "Выключены"),
            ],
            quoted: true,
        },
        "true",
        Some(V1_8),
    ),
    s(
        "particles",
        Graphics,
        "Частицы",
        "",
        Kind::Choice {
            options: &[("0", "Все"), ("1", "Меньше"), ("2", "Минимум")],
            quoted: false,
        },
        "0",
        None,
    ),
    s(
        "ao",
        Graphics,
        "Мягкое освещение",
        "",
        Kind::Bool,
        "true",
        Some(V1_14),
    ),
    s(
        "entityShadows",
        Graphics,
        "Тени существ",
        "",
        Kind::Bool,
        "true",
        Some(V1_8),
    ),
    s(
        "biomeBlendRadius",
        Graphics,
        "Смешивание биомов",
        "",
        Kind::Int {
            min: 0,
            max: 7,
            special: Some((0, "Выкл")),
        },
        "2",
        Some(V1_13),
    ),
    s(
        "mipmapLevels",
        Graphics,
        "Уровни MIP-текстур",
        "",
        Kind::Int {
            min: 0,
            max: 4,
            special: None,
        },
        "4",
        Some(V1_7_2),
    ),
    s(
        "entityDistanceScaling",
        Graphics,
        "Дальность прорисовки существ",
        "",
        Kind::Slider {
            min: 50.0,
            max: 500.0,
            step: 25.0,
            mul: 100.0,
            add: 0.0,
            unit: "%",
        },
        "1.0",
        Some(V1_16),
    ),
    s(
        "fullscreen",
        Graphics,
        "Полный экран",
        "",
        Kind::Bool,
        "false",
        None,
    ),
];

fn setting(key: &str) -> Option<&'static Setting> {
    SETTINGS.iter().find(|s| s.key == key)
}

/// Что лаунчеру нужно знать о версии сборки, чтобы записать её формат.
#[derive(Debug, Clone, Default)]
pub struct GameVersion {
    /// Дата выхода, ISO 8601 (`releaseTime` из описания версии).
    pub release: String,
    /// Номер формата данных — пишется первой строкой нового options.txt.
    pub data_version: Option<i32>,
}

impl GameVersion {
    /// Есть ли у версии то, что появилось в `date`. Неизвестная дата — новая версия.
    fn has(&self, date: &str) -> bool {
        date.is_empty() || self.release.is_empty() || self.release.as_str() >= date
    }
}

/// Общий профиль: `game-options.json` в папке данных.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// Значения как в новых версиях: строки без кавычек, язык маленькими.
    #[serde(default)]
    pub values: BTreeMap<String, String>,
    /// Клавиши: строки `key_*` из сборок 1.13+.
    #[serde(default)]
    pub keys: BTreeMap<String, String>,
    /// Включённые ресурспаки по порядку, как в `resourcePacks` 1.13+.
    #[serde(default)]
    pub packs: Option<Vec<String>>,
    /// Какие группы общие; `None` — по умолчанию.
    #[serde(default)]
    pub shared: Option<Vec<Group>>,
    /// Из какой сборки профиль взят при первом заполнении.
    #[serde(default)]
    pub seeded_from: Option<String>,
}

impl Profile {
    fn path(data: &DataDir) -> std::path::PathBuf {
        data.root().join("game-options.json")
    }

    pub fn load(data: &DataDir) -> Result<Self> {
        let path = Self::path(data);
        match std::fs::read(&path) {
            Ok(bytes) => json(&bytes, "общие настройки игры"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).at(&path),
        }
    }

    pub fn save(&self, data: &DataDir) -> Result<()> {
        let path = Self::path(data);
        let text = serde_json::to_vec_pretty(self).expect("профиль всегда сериализуется");
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text).at(&tmp)?;
        std::fs::rename(&tmp, &path).at(&path)
    }

    pub fn is_shared(&self, group: Group) -> bool {
        match &self.shared {
            Some(list) => list.contains(&group),
            None => group.shared_by_default(),
        }
    }

    pub fn set_shared(&mut self, group: Group, shared: bool) {
        let mut list: Vec<Group> = Group::ALL
            .into_iter()
            .filter(|g| self.is_shared(*g))
            .collect();
        list.retain(|g| *g != group);
        if shared {
            list.push(group);
        }
        list.sort();
        self.shared = Some(list);
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty() && self.keys.is_empty() && self.packs.is_none()
    }

    /// Поменять значение из окна лаунчера. Значение проверяется по каталогу.
    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        let s =
            setting(key).ok_or_else(|| Error::Other(format!("неизвестный параметр «{key}»")))?;
        let bad = || Error::Other(format!("«{value}» не подходит для «{}»", s.label));
        let value = value.trim();
        match &s.kind {
            Kind::Bool => {
                if value != "true" && value != "false" {
                    return Err(bad());
                }
            }
            Kind::Slider {
                min, max, mul, add, ..
            } => {
                let v: f64 = value.parse().map_err(|_| bad())?;
                let shown = v * mul + add;
                if !(min - 1e-6..=max + 1e-6).contains(&shown) {
                    return Err(bad());
                }
            }
            Kind::Int { min, max, .. } => {
                let v: i64 = value.parse().map_err(|_| bad())?;
                if !(*min..=*max).contains(&v) {
                    return Err(bad());
                }
            }
            Kind::Choice { options, .. } => {
                if !options.iter().any(|(v, _)| *v == value) {
                    return Err(bad());
                }
            }
        }
        self.values.insert(key.to_string(), value.to_string());
        Ok(())
    }
}

/// options.txt построчно: порядок строк сохраняется, незнакомые не трогаем.
struct OptionsFile {
    lines: Vec<(String, String)>,
}

impl OptionsFile {
    fn read(path: &Path) -> Result<Option<Self>> {
        let text = match std::fs::read(path) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e).at(path),
        };
        let lines = text
            .lines()
            .filter_map(|l| l.split_once(':'))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Ok(Some(Self { lines }))
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.lines
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    fn set(&mut self, key: &str, value: String) {
        match self.lines.iter_mut().find(|(k, _)| k == key) {
            Some((_, v)) => *v = value,
            None => self.lines.push((key.to_string(), value)),
        }
    }

    fn write(&self, path: &Path) -> Result<()> {
        let mut text = String::new();
        for (k, v) in &self.lines {
            text.push_str(k);
            text.push(':');
            text.push_str(v);
            text.push('\n');
        }
        let tmp = path.with_extension("txt.tmp");
        std::fs::write(&tmp, text).at(&tmp)?;
        std::fs::rename(&tmp, path).at(path)
    }
}

fn unquote(v: &str) -> &str {
    v.strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(v)
}

/// Значение профиля → строка options.txt этой версии.
fn to_file(s: &Setting, value: &str, existing: Option<&str>, v: &GameVersion) -> String {
    let mut value = value.to_string();
    if s.key == "lang" && !v.has(LOWERCASE_LANG_SINCE) {
        // До 1.11: «ru_RU».
        if let Some((lang, country)) = value.split_once('_') {
            value = format!("{lang}_{}", country.to_uppercase());
        }
    }
    let quoted = match (&s.kind, existing) {
        // Как уже записано в этом файле — так игра и ждёт.
        (Kind::Choice { quoted: true, .. }, Some(old)) => old.starts_with('"'),
        (Kind::Choice { quoted: true, .. }, None) => v.has(QUOTES_SINCE),
        _ => false,
    };
    if quoted {
        format!("\"{value}\"")
    } else {
        value
    }
}

/// Строка options.txt → значение профиля.
fn from_file(s: &Setting, raw: &str) -> String {
    let v = unquote(raw.trim());
    if s.key == "lang" {
        v.to_lowercase()
    } else {
        v.to_string()
    }
}

/// Номер формата данных версии: `world_version` в `version.json` внутри
/// клиента (есть с 1.14). Старым версиям он в options.txt не нужен.
pub fn data_version(client_jar: &Path) -> Option<i32> {
    let file = std::fs::File::open(client_jar).ok()?;
    let mut zip = zip::ZipArchive::new(file).ok()?;
    let entry = zip.by_name("version.json").ok()?;
    let value: serde_json::Value = serde_json::from_reader(entry).ok()?;
    value.get("world_version")?.as_i64().map(|v| v as i32)
}

/// Версия, уже скачанная в папку данных; `None` — её описания ещё нет.
pub fn installed_version(data: &DataDir, id: &str) -> Option<GameVersion> {
    let bytes = std::fs::read(data.version_json(id)).ok()?;
    let json: crate::meta::VersionJson = serde_json::from_slice(&bytes).ok()?;
    Some(GameVersion {
        release: json.release_time,
        data_version: data_version(&data.client_jar(id)),
    })
}

/// Заполнить профиль настройками сборки (при первом открытии или по кнопке
/// «Взять из сборки»). Прежние значения общих групп заменяются.
pub fn seed(profile: &mut Profile, data: &DataDir, build: &crate::builds::Instance) -> Result<()> {
    let version = installed_version(data, &build.version).ok_or_else(|| {
        Error::Other(format!(
            "версия {} ещё не скачана — запустите сборку один раз",
            build.version
        ))
    })?;
    let game_dir = data.instance(&build.id);
    if !game_dir.join("options.txt").exists() {
        return Err(Error::Other(format!(
            "в сборке «{}» ещё нет настроек игры — запустите её один раз",
            build.name
        )));
    }
    for s in SETTINGS {
        if profile.is_shared(s.group) {
            profile.values.remove(s.key);
        }
    }
    import(profile, &game_dir, &version)?;
    profile.seeded_from = Some(build.name.clone());
    Ok(())
}

/// Записать общий профиль в options.txt сборки перед запуском.
pub fn apply(profile: &Profile, game_dir: &Path, v: &GameVersion) -> Result<()> {
    if profile.is_empty() {
        return Ok(());
    }
    let path = game_dir.join("options.txt");
    let mut file = match OptionsFile::read(&path)? {
        Some(file) => file,
        // Новая сборка: файл создаём сами. Номер формата — первой строкой, иначе
        // игра решит, что файл из очень старой версии, и начнёт его переделывать.
        None => OptionsFile {
            lines: v
                .data_version
                .map(|dv| ("version".to_string(), dv.to_string()))
                .into_iter()
                .collect(),
        },
    };
    for (key, value) in &profile.values {
        let Some(s) = setting(key) else { continue };
        if !profile.is_shared(s.group) || !v.has(s.since_date) {
            continue;
        }
        let line = to_file(s, value, file.get(key), v);
        file.set(key, line);
    }
    // Клавиши и ресурспаки до 1.13 хранились иначе — их в старых сборках не трогаем.
    if v.has(KEY_NAMES_SINCE) {
        if profile.is_shared(Group::Keys) {
            for (key, value) in &profile.keys {
                file.set(key, value.clone());
            }
        }
        if profile.is_shared(Group::Packs)
            && let Some(packs) = &profile.packs
        {
            file.set(
                "resourcePacks",
                serde_json::to_string(packs).expect("строки"),
            );
        }
    }
    std::fs::create_dir_all(game_dir).at(game_dir)?;
    file.write(&path)
}

/// После игры забрать из options.txt сборки то, что игрок поменял.
pub fn import(profile: &mut Profile, game_dir: &Path, v: &GameVersion) -> Result<()> {
    let Some(file) = OptionsFile::read(&game_dir.join("options.txt"))? else {
        return Ok(());
    };
    for s in SETTINGS {
        if !profile.is_shared(s.group) || !v.has(s.since_date) {
            continue;
        }
        if let Some(raw) = file.get(s.key) {
            profile.values.insert(s.key.to_string(), from_file(s, raw));
        }
    }
    if v.has(KEY_NAMES_SINCE) {
        if profile.is_shared(Group::Keys) {
            for (k, val) in &file.lines {
                if k.starts_with("key_") {
                    profile.keys.insert(k.clone(), val.clone());
                }
            }
        }
        if profile.is_shared(Group::Packs)
            && let Some(raw) = file.get("resourcePacks")
            && let Ok(list) = serde_json::from_str::<Vec<String>>(raw)
        {
            profile.packs = Some(merge_packs(profile.packs.as_deref(), list, game_dir));
        }
    }
    Ok(())
}

/// Игра молча убирает из списка ресурспаки, которых нет в папке сборки. Если
/// пака нет только в этой сборке, из общего списка его не убираем.
fn merge_packs(old: Option<&[String]>, new: Vec<String>, game_dir: &Path) -> Vec<String> {
    let mut out = new;
    for pack in old.unwrap_or_default() {
        let missing_here = pack
            .strip_prefix("file/")
            .is_some_and(|f| !game_dir.join("resourcepacks").join(f).exists());
        if missing_here && !out.contains(pack) {
            out.push(pack.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(release: &str) -> GameVersion {
        GameVersion {
            release: release.into(),
            data_version: Some(4556),
        }
    }

    #[test]
    fn catalog_defaults_are_valid() {
        let mut p = Profile::default();
        for s in SETTINGS {
            p.set(s.key, s.default)
                .unwrap_or_else(|e| panic!("{}: {e}", s.key));
        }
        assert!(p.set("fov", "2.0").is_err(), "FOV 150° — слишком много");
        assert!(p.set("mainHand", "middle").is_err());
        assert!(p.set("nope", "1").is_err());
        assert!(p.set("guiScale", "4").is_ok());
    }

    /// Профиль из 1.21 попадает в 1.5.2: только знакомые ей параметры, язык
    /// по-старому, кавычек нет, остальные строки на месте.
    #[test]
    fn writes_each_version_in_its_format() {
        let game = tempfile::tempdir().unwrap();
        let old = "invertYMouse:false\nfov:0.0\nlang:en_US\nmusic:1.0\nkey_key.forward:17\n";
        std::fs::write(game.path().join("options.txt"), old).unwrap();
        let mut p = Profile::default();
        p.set("fov", "0.5").unwrap();
        p.set("lang", "ru_ru").unwrap();
        p.set("fovEffectScale", "0.0").unwrap();
        p.set("mainHand", "left").unwrap();
        p.keys
            .insert("key_key.forward".into(), "key.keyboard.w".into());
        apply(&p, game.path(), &v("2013-04-25T15:45:00+00:00")).unwrap();
        let text = std::fs::read_to_string(game.path().join("options.txt")).unwrap();
        assert_eq!(
            text,
            "invertYMouse:false\nfov:0.5\nlang:ru_RU\nmusic:1.0\nkey_key.forward:17\n"
        );

        // Новая сборка 1.21.10: файла нет — создаётся с номером формата.
        let fresh = tempfile::tempdir().unwrap();
        apply(&p, fresh.path(), &v("2025-10-07T09:00:00+00:00")).unwrap();
        let text = std::fs::read_to_string(fresh.path().join("options.txt")).unwrap();
        assert!(text.starts_with("version:4556\n"), "{text}");
        assert!(text.contains("mainHand:\"left\"\n"));
        assert!(text.contains("lang:ru_ru\n"));
        assert!(text.contains("fovEffectScale:0.0\n"));
        assert!(text.contains("key_key.forward:key.keyboard.w\n"));
    }

    #[test]
    fn graphics_stays_per_build_unless_shared() {
        let game = tempfile::tempdir().unwrap();
        std::fs::write(
            game.path().join("options.txt"),
            "renderDistance:6\nfov:0.0\n",
        )
        .unwrap();
        let mut p = Profile::default();
        p.set("renderDistance", "32").unwrap();
        p.set("fov", "0.25").unwrap();
        let ver = v("2024-08-08T11:51:42+00:00");
        apply(&p, game.path(), &ver).unwrap();
        let text = std::fs::read_to_string(game.path().join("options.txt")).unwrap();
        assert_eq!(text, "renderDistance:6\nfov:0.25\n");

        p.set_shared(Group::Graphics, true);
        apply(&p, game.path(), &ver).unwrap();
        let text = std::fs::read_to_string(game.path().join("options.txt")).unwrap();
        assert_eq!(text, "renderDistance:32\nfov:0.25\n");
    }

    #[test]
    fn imports_changes_after_game() {
        let game = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(game.path().join("resourcepacks")).unwrap();
        std::fs::write(game.path().join("resourcepacks/here.zip"), "x").unwrap();
        std::fs::write(
            game.path().join("options.txt"),
            "version:3955\nfov:0.75\nmainHand:\"left\"\nlang:ru_ru\nrenderDistance:8\nkey_key.jump:key.keyboard.space\nresourcePacks:[\"vanilla\",\"file/here.zip\"]\n",
        )
        .unwrap();
        let mut p = Profile {
            packs: Some(vec![
                "vanilla".into(),
                "file/elsewhere.zip".into(),
                "file/here.zip".into(),
            ]),
            ..Default::default()
        };
        import(&mut p, game.path(), &v("2024-08-08T11:51:42+00:00")).unwrap();
        assert_eq!(p.values["fov"], "0.75");
        assert_eq!(p.values["mainHand"], "left");
        assert!(!p.values.contains_key("renderDistance"), "графика не общая");
        assert_eq!(p.keys["key_key.jump"], "key.keyboard.space");
        // «elsewhere» в этой сборке нет — из общего списка не пропадает.
        assert_eq!(
            p.packs.unwrap(),
            vec![
                "vanilla".to_string(),
                "file/here.zip".into(),
                "file/elsewhere.zip".into()
            ]
        );

        // Старая версия: язык приводится к новому виду, клавиши не берутся.
        let old = tempfile::tempdir().unwrap();
        std::fs::write(
            old.path().join("options.txt"),
            "lang:ru_RU\nkey_key.jump:57\n",
        )
        .unwrap();
        let mut p = Profile::default();
        import(&mut p, old.path(), &v("2013-04-25T15:45:00+00:00")).unwrap();
        assert_eq!(p.values["lang"], "ru_ru");
        assert!(p.keys.is_empty());
    }

    #[test]
    fn profile_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path());
        assert!(Profile::load(&data).unwrap().is_empty());
        let mut p = Profile::default();
        p.set("bobView", "false").unwrap();
        p.set_shared(Group::Sound, false);
        p.save(&data).unwrap();
        let back = Profile::load(&data).unwrap();
        assert_eq!(back, p);
        assert!(!back.is_shared(Group::Sound));
        assert!(back.is_shared(Group::View));
        assert!(!back.is_shared(Group::Graphics));
    }
}
