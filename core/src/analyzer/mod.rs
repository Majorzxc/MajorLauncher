//! Анализатор ошибок (раздел 5.8 ТЗ). Читает вывод игры во время игры и
//! после неё, отчёт о падении (`crash-reports/`) и отчёт Java о падении
//! (`hs_err_pid*.log`), сверяет строки с правилами и говорит человеческим
//! языком, что случилось, кто виноват и что сделать.
//!
//! Правила — данные (`rules.json`, встроены в лаунчер; этап 7.2 — обновление
//! с GitHub). Виновник — по прямым указаниям в логе («provided by 'мод'»,
//! «from mod …», «Suspected Mods» у Forge) или по стеку вызовов: какому моду
//! принадлежит пакет упавшего кода (пакеты читаются из jar вместе с
//! остальными сведениями о моде).
//!
//! Уровни: критическое — сразу на экран; предупреждение — в список проблем
//! сборки и сводку после игры; остальное — шум, его видно только в логе.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::mods::Action;

/// Встроенные правила.
const BUILTIN: &str = include_str!("rules.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    Warning,
}

#[derive(Debug, Clone, Deserialize)]
struct RuleDef {
    id: String,
    level: Severity,
    /// Быстрая проверка: строка должна содержать хоть одну из подстрок.
    any: Vec<String>,
    /// Уточнение и захват имён (`mod`, `dep`, `depid`, `name` …).
    #[serde(default)]
    regex: Option<String>,
    title: String,
    text: String,
    #[serde(default)]
    actions: Vec<String>,
    /// Учитывать только при падении игры (иначе это обычный шум модов).
    #[serde(default)]
    crash_only: bool,
    /// Пропустить, если захваченное имя — id мода сборки (например, пак
    /// ресурсов, встроенный в мод, — это шум мода, а не пак игрока).
    #[serde(default)]
    skip_if_mod: Option<String>,
    /// Узнав строку, не проверять её следующими правилами (частный случай
    /// стоит раньше общего).
    #[serde(default)]
    exclusive: bool,
    /// Склеивать находки с одной и той же зависимостью у разных модов.
    #[serde(default)]
    group_by: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RuleFile {
    version: u32,
    rules: Vec<RuleDef>,
}

struct Rule {
    def: RuleDef,
    re: Option<Regex>,
}

/// Набор правил.
pub struct Rules {
    pub version: u32,
    list: Vec<Rule>,
}

impl Rules {
    /// Встроенные правила (всегда разбираются — проверено тестом).
    pub fn builtin() -> Arc<Rules> {
        Arc::new(Self::parse(BUILTIN).expect("встроенные правила разбираются"))
    }

    /// Правила из JSON. Неверное регулярное выражение — ошибка всего файла:
    /// лучше остаться на прежних правилах, чем молча потерять одно.
    pub fn parse(json: &str) -> Result<Rules> {
        let file: RuleFile = serde_json::from_str(json)
            .map_err(|e| Error::Other(format!("правила анализатора не разбираются: {e}")))?;
        let mut list = Vec::new();
        for def in file.rules {
            let re = match &def.regex {
                Some(r) => Some(Regex::new(r).map_err(|e| {
                    Error::Other(format!("правило «{}»: неверный шаблон: {e}", def.id))
                })?),
                None => None,
            };
            if def.any.is_empty() {
                return Err(Error::Other(format!(
                    "правило «{}» без подстрок для быстрой проверки",
                    def.id
                )));
            }
            list.push(Rule { def, re });
        }
        Ok(Rules {
            version: file.version,
            list,
        })
    }
}

/// Сколько примеров строк хранить на находку и сколько блоков исключений.
const EXAMPLES: usize = 3;
const MAX_BLOCKS: usize = 64;
const MAX_FRAMES: usize = 40;

/// Совпадение правила.
#[derive(Debug, Clone)]
struct Hit {
    rule: usize,
    caps: BTreeMap<String, String>,
    count: u32,
    lines: Vec<String>,
}

/// Исключение со стеком вызовов (`… Exception: …` и строки `\tat …`).
#[derive(Debug, Clone, Default)]
struct Block {
    header: String,
    frames: Vec<String>,
    /// Строка была на уровне ERROR (не просто сведения).
    error: bool,
}

/// Разбор потока строк: во время игры строки идут по одной.
pub struct Analyzer {
    rules: Arc<Rules>,
    /// Ключ — правило и захваченные имена: одинаковые ошибки складываются.
    hits: Vec<Hit>,
    blocks: Vec<Block>,
    current: Option<Block>,
    last_error_line: bool,
}

impl Analyzer {
    pub fn new(rules: Arc<Rules>) -> Self {
        Self {
            rules,
            hits: Vec::new(),
            blocks: Vec::new(),
            current: None,
            last_error_line: false,
        }
    }

    /// Ещё одна строка вывода игры или файла.
    pub fn line(&mut self, line: &str) {
        let trimmed = line.trim_end();
        self.track_blocks(trimmed);
        for (i, rule) in self.rules.list.iter().enumerate() {
            if !rule.def.any.iter().any(|s| trimmed.contains(s.as_str())) {
                continue;
            }
            let caps = match &rule.re {
                Some(re) => match re.captures(trimmed) {
                    Some(c) => re
                        .capture_names()
                        .flatten()
                        .filter_map(|n| {
                            c.name(n)
                                .map(|m| (n.to_string(), m.as_str().trim().to_string()))
                        })
                        .collect(),
                    None => continue,
                },
                None => BTreeMap::new(),
            };
            match self.hits.iter_mut().find(|h| h.rule == i && h.caps == caps) {
                Some(h) => {
                    h.count += 1;
                    if h.lines.len() < EXAMPLES {
                        h.lines.push(trimmed.to_string());
                    }
                }
                None => self.hits.push(Hit {
                    rule: i,
                    caps,
                    count: 1,
                    lines: vec![trimmed.to_string()],
                }),
            }
            if rule.def.exclusive {
                break;
            }
        }
    }

    /// Целый текст (лог, отчёт о падении).
    pub fn text(&mut self, text: &str) {
        for l in text.lines() {
            self.line(l);
        }
        self.flush_block();
    }

    fn track_blocks(&mut self, line: &str) {
        let frame = line.trim_start();
        if (line.starts_with('\t') || line.starts_with("    ")) && frame.starts_with("at ") {
            if let Some(b) = &mut self.current
                && b.frames.len() < MAX_FRAMES
            {
                b.frames.push(frame[3..].to_string());
            }
            return;
        }
        if frame.starts_with("Caused by:") || frame.starts_with("... ") {
            if let Some(b) = &mut self.current {
                // Причина важнее обёртки: её кадры — ближе к виновнику.
                if frame.starts_with("Caused by:") {
                    b.header = frame.to_string();
                    b.frames.clear();
                }
            }
            return;
        }
        self.flush_block();
        let is_error_level =
            line.contains("/ERROR]") || line.contains("[ERROR]") || line.contains("/FATAL]");
        let looks_like_exception =
            (frame.contains("Exception") || frame.contains("Error:")) && !frame.starts_with('[');
        if looks_like_exception || is_error_level {
            self.current = Some(Block {
                header: frame.to_string(),
                frames: Vec::new(),
                error: is_error_level || self.last_error_line,
            });
        }
        self.last_error_line = is_error_level;
    }

    fn flush_block(&mut self) {
        if let Some(b) = self.current.take()
            && !b.frames.is_empty()
        {
            if self.blocks.len() == MAX_BLOCKS {
                self.blocks.remove(0);
            }
            self.blocks.push(b);
        }
    }

    /// Итог разбора. `crash` — игра упала; `report` — отчёт о падении
    /// (`crash-reports/…txt`), если он появился за эту игру; `index` — моды
    /// сборки, чтобы назвать виновника и предложить действие.
    pub fn finish(
        mut self,
        crash: bool,
        report: Option<&CrashReport>,
        index: &ModIndex,
    ) -> Analysis {
        self.flush_block();
        if let Some(r) = report {
            let mut more = Analyzer::new(self.rules.clone());
            more.text(&r.text);
            for h in more.hits {
                if !self
                    .hits
                    .iter()
                    .any(|x| x.rule == h.rule && x.caps == h.caps)
                {
                    self.hits.push(h);
                }
            }
            self.blocks.extend(more.blocks);
        }
        let mut critical = Vec::new();
        let mut warnings = Vec::new();
        let hits = group(&self.hits, &self.rules);
        for h in &hits {
            let rule = &self.rules.list[h.rule].def;
            if rule.crash_only && !crash {
                continue;
            }
            if let Some(cap) = &rule.skip_if_mod
                && h.caps.get(cap).is_some_and(|v| index.known(v).is_some())
            {
                continue;
            }
            let mut f = render(rule, h, index);
            if f.culprit.is_none() && (rule.id == "class_missing" || rule.id == "java_version") {
                // Кто обратился к недостающему коду — по стеку.
                if let Some(m) = h
                    .caps
                    .get("class")
                    .and_then(|c| index.by_class(&c.replace('/', ".")))
                    .or_else(|| self.stack_culprit(index))
                {
                    f.set_culprit(m, index, rule);
                }
            }
            match rule.level {
                Severity::Critical if crash => critical.push(f),
                // Без падения «критическое» правило — предупреждение: игра
                // справилась, но игроку стоит знать.
                Severity::Critical => warnings.push(f),
                Severity::Warning => warnings.push(f),
            }
        }

        // Ошибки модов во время игры (исключения на уровне ERROR) — по
        // одному предупреждению на мод.
        let mut by_mod: BTreeMap<String, (u32, String)> = BTreeMap::new();
        for b in self.blocks.iter().filter(|b| b.error) {
            if let Some(m) = culprit_of(&b.frames, index) {
                let e = by_mod.entry(m).or_insert((0, b.header.clone()));
                e.0 += 1;
            }
        }
        for (id, (count, header)) in by_mod {
            if warnings
                .iter()
                .chain(critical.iter())
                .any(|f| f.culprit.as_ref().is_some_and(|c| c.id == id))
            {
                continue;
            }
            let name = index.name(&id);
            warnings.push(Finding {
                rule: "mod_error".into(),
                severity: Severity::Warning,
                title: format!("Мод «{name}» выдавал ошибки во время игры"),
                text: format!(
                    "{} в логе. Игра не упала, но часть функций мода могла не сработать. Если что-то ведёт себя странно — обновите мод.",
                    times(count)
                ),
                culprit: Some(index.culprit(&id)),
                actions: index.update_action(&id).into_iter().collect(),
                count,
                lines: vec![header],
            });
        }

        // Одинаковые находки (тот же мод, разные строки лога) — одной.
        let merge = |list: Vec<Finding>| {
            let mut out: Vec<Finding> = Vec::new();
            for f in list {
                match out
                    .iter_mut()
                    .find(|x| x.rule == f.rule && x.title == f.title)
                {
                    Some(x) => {
                        x.count += f.count;
                        for l in f.lines {
                            if x.lines.len() < EXAMPLES {
                                x.lines.push(l);
                            }
                        }
                    }
                    None => out.push(f),
                }
            }
            out
        };
        let mut critical = merge(critical);
        let mut warnings = merge(warnings);

        // Упала, а правила не узнали причину — главное из отчёта о падении.
        if crash && critical.is_empty() {
            critical.push(self.fallback(report, index));
        }
        // Важнее — правило выше в списке (сначала причины, потом следствия).
        let order = |f: &Finding| {
            self.rules
                .list
                .iter()
                .position(|r| r.def.id == f.rule)
                .unwrap_or(usize::MAX)
        };
        critical.sort_by_key(order);
        warnings.sort_by_key(|f| std::cmp::Reverse(f.count));
        Analysis {
            critical,
            warnings,
            crash_report: report.map(|r| r.file.clone()),
            rules_version: self.rules.version,
        }
    }

    /// Виновник по последнему исключению со стеком.
    fn stack_culprit(&self, index: &ModIndex) -> Option<String> {
        self.blocks
            .iter()
            .rev()
            .find_map(|b| culprit_of(&b.frames, index))
    }

    fn fallback(&self, report: Option<&CrashReport>, index: &ModIndex) -> Finding {
        let suspect = report
            .and_then(|r| r.suspects.iter().find_map(|s| index.known(s)))
            .or_else(|| report.and_then(|r| culprit_of(&r.frames, index)))
            .or_else(|| self.stack_culprit(index));
        let what = report
            .and_then(|r| r.exception.clone())
            .or_else(|| self.blocks.last().map(|b| b.header.clone()));
        let description = report.and_then(|r| r.description.clone());
        let mut text = String::from("Правила лаунчера эту ошибку не узнали.");
        if let Some(m) = &suspect {
            text.push_str(&format!(
                " По стеку вызовов упал код мода «{}» — начните с него: обновите или выключите.",
                index.name(m)
            ));
        } else {
            text.push_str(" Виновника по логу определить не удалось — подробности ниже.");
        }
        let mut actions = Vec::new();
        if let Some(m) = &suspect {
            actions.extend(index.update_action(m));
            actions.extend(index.disable_action(m));
        }
        Finding {
            rule: "unknown".into(),
            severity: Severity::Critical,
            title: match (&suspect, &description) {
                (Some(m), _) => format!("Игра упала — похоже, из-за мода «{}»", index.name(m)),
                (None, Some(d)) => format!("Игра упала: {d}"),
                (None, None) => "Игра закрылась с ошибкой".into(),
            },
            text,
            culprit: suspect.map(|m| index.culprit(&m)),
            actions,
            count: 1,
            lines: what.into_iter().collect(),
        }
    }
}

fn times(n: u32) -> String {
    let (d, h) = (n % 10, n % 100);
    let w = if d == 1 && h != 11 {
        "раз"
    } else if (2..=4).contains(&d) && !(12..=14).contains(&h) {
        "раза"
    } else {
        "раз"
    };
    format!("{n} {w}")
}

/// Пакеты, по которым мод не определить: сама игра, загрузчики и общие
/// библиотеки — они есть в стеке почти любой ошибки.
const SHARED: &[&str] = &[
    "java.",
    "javax.",
    "jdk.",
    "sun.",
    "com.sun.",
    "net.minecraft.",
    "com.mojang.",
    "net.fabricmc.",
    "org.quiltmc.",
    "net.minecraftforge.",
    "net.neoforged.",
    "cpw.mods.",
    "org.spongepowered.",
    "com.llamalad7.",
    "com.google.",
    "org.apache.",
    "it.unimi.",
    "io.netty.",
    "org.lwjgl.",
    "kotlin.",
    "kotlinx.",
    "org.slf4j.",
    "org.joml.",
    "oshi.",
    "com.electronwill.",
    "org.objectweb.",
    "knot//",
    "MC-BOOTSTRAP",
];

/// Чей код в стеке: первый кадр не из игры, загрузчика и общих библиотек.
fn culprit_of(frames: &[String], index: &ModIndex) -> Option<String> {
    for f in frames {
        // «knot//com.x.y.Z.m(Z.java:1)», «TRANSFORMER/mod@1.0/com.x...».
        let call = f.rsplit('/').next().unwrap_or(f);
        let call = call.split('(').next().unwrap_or(call);
        if SHARED.iter().any(|p| call.starts_with(p)) {
            continue;
        }
        if let Some(m) = index.by_class(call) {
            return Some(m);
        }
    }
    None
}

/// Склеить находки одного правила с одной зависимостью у разных модов:
/// «нужен fabric-api» у шести модов — одна находка со списком модов
/// (`{mods}`), без выключения каждого по отдельности.
fn group(hits: &[Hit], rules: &Rules) -> Vec<Hit> {
    let mut out: Vec<Hit> = Vec::new();
    let mut seen: HashMap<(usize, String), usize> = HashMap::new();
    for h in hits {
        let Some(key_cap) = &rules.list[h.rule].def.group_by else {
            out.push(h.clone());
            continue;
        };
        let key = h.caps.get(key_cap).cloned().unwrap_or_default();
        match seen.get(&(h.rule, key.clone())) {
            Some(&i) => {
                let g = &mut out[i];
                g.count += h.count;
                let name = h
                    .caps
                    .get("name")
                    .or(h.caps.get("mod"))
                    .cloned()
                    .unwrap_or_default();
                let list = g.caps.entry("mods".into()).or_default();
                if !list.split(", ").any(|n| n == name) {
                    list.push_str(", ");
                    list.push_str(&name);
                }
                g.caps.insert("grouped".into(), "1".into());
                for l in &h.lines {
                    if g.lines.len() < EXAMPLES {
                        g.lines.push(l.clone());
                    }
                }
            }
            None => {
                let mut h = h.clone();
                let name = h
                    .caps
                    .get("name")
                    .or(h.caps.get("mod"))
                    .cloned()
                    .unwrap_or_default();
                h.caps.insert("mods".into(), name);
                seen.insert((h.rule, key), out.len());
                out.push(h);
            }
        }
    }
    out
}

/// Подставить захваченные имена в текст правила: `{mod}` → название мода.
fn fill(template: &str, caps: &BTreeMap<String, String>, index: &ModIndex) -> String {
    let mut out = template.to_string();
    for (k, v) in caps {
        let shown = if k == "mod" || k == "depid" {
            index.name(v)
        } else {
            v.clone()
        };
        out = out.replace(&format!("{{{k}}}"), &shown);
    }
    // Не захватилось — не оставлять «{mod}» в тексте.
    out.replace("«{mod}»", "")
        .replace("{mod}", "мод")
        .replace("  ", " ")
}

fn render(rule: &RuleDef, h: &Hit, index: &ModIndex) -> Finding {
    let grouped = h.caps.contains_key("grouped");
    let culprit_id = if grouped {
        None
    } else {
        h.caps
            .get("mod")
            .and_then(|m| index.known(m).or(Some(m.clone())))
    };
    let dep = h
        .caps
        .get("dep")
        .or(h.caps.get("depid"))
        .cloned()
        .unwrap_or_default();
    let (title, text) = if grouped {
        (
            format!("Модам нужен «{dep}», а его нет"),
            format!(
                "Без «{dep}» не запустятся: {}. {}",
                h.caps.get("mods").cloned().unwrap_or_default(),
                if index.enable_action(&dep).is_some()
                    || h.caps
                        .get("depid")
                        .is_some_and(|d| index.enable_action(d).is_some())
                {
                    "Он есть в сборке, но выключен — включите его."
                } else {
                    "Скачайте его из каталога."
                }
            ),
        )
    } else {
        (
            fill(&rule.title, &h.caps, index),
            fill(&rule.text, &h.caps, index),
        )
    };
    let mut f = Finding {
        rule: rule.id.clone(),
        severity: rule.level,
        title,
        text,
        culprit: culprit_id.as_ref().map(|m| index.culprit(m)),
        actions: Vec::new(),
        count: h.count,
        lines: h.lines.clone(),
    };
    for a in &rule.actions {
        match a.as_str() {
            "memory" => f.actions.push(Action::Memory),
            "optimize" => f.actions.push(Action::Optimize),
            "driver" => f.actions.push(Action::Driver),
            "open_backups" => f.actions.push(Action::Backups),
            "disable_mod" => f
                .actions
                .extend(culprit_id.as_ref().and_then(|m| index.disable_action(m))),
            "update_mod" => f
                .actions
                .extend(culprit_id.as_ref().and_then(|m| index.update_action(m))),
            "disable_dep" => f
                .actions
                .extend(h.caps.get("depid").and_then(|m| index.disable_action(m))),
            "find_dep" => {
                // Зависимость в сборке есть, но выключена — включить её.
                let enable = h
                    .caps
                    .get("depid")
                    .or(h.caps.get("dep"))
                    .and_then(|d| index.enable_action(d));
                match enable {
                    Some(a) => f.actions.push(a),
                    None => {
                        if let Some(dep) = h.caps.get("dep").or(h.caps.get("depid")) {
                            f.actions.push(Action::Search { query: dep.clone() });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    f
}

/// Что анализатор нашёл.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub rule: String,
    pub severity: Severity,
    /// Одна фраза: что случилось.
    pub title: String,
    /// Почему и что делать.
    pub text: String,
    pub culprit: Option<Culprit>,
    /// Кнопки решений.
    pub actions: Vec<Action>,
    /// Сколько раз встретилось.
    pub count: u32,
    /// Строки лога, по которым узнали, — их подсвечивает «Подробнее».
    pub lines: Vec<String>,
}

impl Finding {
    fn set_culprit(&mut self, id: String, index: &ModIndex, rule: &RuleDef) {
        for a in &rule.actions {
            match a.as_str() {
                "disable_mod" => self.actions.extend(index.disable_action(&id)),
                "update_mod" => self.actions.extend(index.update_action(&id)),
                _ => {}
            }
        }
        self.text = format!(
            "{} По стеку вызовов — мод «{}».",
            self.text,
            index.name(&id)
        );
        self.culprit = Some(index.culprit(&id));
    }
}

/// Виновник.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Culprit {
    /// id мода из его описания.
    pub id: String,
    pub name: String,
    /// Файл в сборке; `None` — такого мода в сборке нет (например, это
    /// недостающая зависимость).
    pub file: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Analysis {
    pub critical: Vec<Finding>,
    pub warnings: Vec<Finding>,
    /// Имя файла отчёта о падении (в `crash-reports/`).
    pub crash_report: Option<String>,
    pub rules_version: u32,
}

/// Отчёт о падении игры (`crash-reports/crash-….txt`).
#[derive(Debug, Clone, Default)]
pub struct CrashReport {
    pub file: String,
    pub text: String,
    pub description: Option<String>,
    /// Первая строка исключения (или его причина).
    pub exception: Option<String>,
    /// Кадры стека главного исключения.
    pub frames: Vec<String>,
    /// Моды, которых называет сам отчёт: «Suspected Mods» (Forge, NeoForge),
    /// «(from mod x)» в «Mixins in Stacktrace» (Fabric).
    pub suspects: Vec<String>,
}

impl CrashReport {
    pub fn parse(file: &str, text: &str) -> Self {
        let mut r = CrashReport {
            file: file.to_string(),
            text: text.to_string(),
            ..Default::default()
        };
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            let l = lines[i].trim();
            if let Some(d) = l.strip_prefix("Description:") {
                r.description = Some(d.trim().to_string());
                // Сразу после описания — исключение и его стек.
                let mut j = i + 1;
                while j < lines.len() && lines[j].trim().is_empty() {
                    j += 1;
                }
                if j < lines.len() {
                    r.exception = Some(lines[j].trim().to_string());
                    let mut k = j + 1;
                    while k < lines.len() {
                        let t = lines[k].trim();
                        if let Some(f) = t.strip_prefix("at ") {
                            r.frames.push(f.to_string());
                        } else if t.starts_with("Caused by:") {
                            // Причина точнее обёртки.
                            r.exception = Some(t.to_string());
                            r.frames.clear();
                        } else if !t.starts_with("...") {
                            break;
                        }
                        k += 1;
                    }
                }
            }
            if l.starts_with("Suspected Mod") {
                // Forge: «Suspected Mod(s): Iris (iris), Version: 1.7»; NeoForge — списком ниже.
                let mut j = i;
                loop {
                    let t = lines[j];
                    // Под модом бывает его стек — это не моды.
                    let t = if t.trim_start().starts_with("at ") {
                        ""
                    } else {
                        t
                    };
                    for cap in t.split(',').filter_map(|p| {
                        let open = p.rfind('(')?;
                        let close = p[open..].find(')')? + open;
                        Some(p[open + 1..close].trim().to_string())
                    }) {
                        if !cap.is_empty() && !cap.contains(' ') && !r.suspects.contains(&cap) {
                            r.suspects.push(cap);
                        }
                    }
                    j += 1;
                    if j >= lines.len() || !lines[j].starts_with('\t') {
                        break;
                    }
                }
            }
            if let Some(pos) = l.find("(from mod ") {
                let id = l[pos + 10..].trim_end_matches(')').trim().to_string();
                if !id.is_empty() && !r.suspects.contains(&id) {
                    r.suspects.push(id);
                }
            }
            i += 1;
        }
        r
    }
}

/// Самый свежий отчёт о падении в папке игры, появившийся после `since`
/// (секунды с 1970 года).
pub fn find_crash_report(game_dir: &Path, since: u64) -> Option<CrashReport> {
    let newest = newest_file(&game_dir.join("crash-reports"), since, |n| {
        n.ends_with(".txt")
    })
    .or_else(|| {
        newest_file(game_dir, since, |n| {
            n.starts_with("hs_err_pid") && n.ends_with(".log")
        })
    })?;
    let text = std::fs::read(&newest).ok()?;
    // Отчёт может быть огромным (список модов, модулей) — нам хватит начала.
    let text = String::from_utf8_lossy(&text[..text.len().min(2 * 1024 * 1024)]).to_string();
    let name = newest.file_name()?.to_string_lossy().to_string();
    Some(CrashReport::parse(&name, &text))
}

fn newest_file(dir: &Path, since: u64, want: impl Fn(&str) -> bool) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| want(&e.file_name().to_string_lossy()))
        .filter_map(|e| {
            let t = e
                .metadata()
                .ok()?
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_secs();
            (t + 2 >= since).then(|| (t, e.path()))
        })
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

/// Моды сборки для анализатора: id → файл и название, пакеты → id,
/// обновления — чтобы предложить «Обновить мод».
#[derive(Debug, Clone, Default)]
pub struct ModIndex {
    mods: HashMap<String, ModRef>,
    /// Пакет (`com.example.mod`) → id мода. Пакеты, которые есть сразу у
    /// нескольких модов (общие библиотеки внутри jar), выкинуты.
    packages: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct ModRef {
    pub name: String,
    pub file: String,
    pub enabled: bool,
    pub update: Option<(String, String)>,
}

impl ModIndex {
    /// Собрать по сведениям о модах сборки (без сети).
    pub fn new(entries: Vec<(Vec<String>, ModRef, Vec<String>)>) -> Self {
        let mut mods = HashMap::new();
        let mut owners: HashMap<String, Vec<String>> = HashMap::new();
        for (ids, r, packages) in entries {
            let Some(main) = ids.first().cloned() else {
                continue;
            };
            for id in &ids {
                mods.entry(id.to_lowercase()).or_insert_with(|| r.clone());
            }
            for p in packages {
                owners.entry(p).or_default().push(main.to_lowercase());
            }
        }
        let mut packages: Vec<(String, String)> = owners
            .into_iter()
            .filter_map(|(p, mut ids)| {
                ids.dedup();
                (ids.len() == 1).then(|| (p, ids.remove(0)))
            })
            .collect();
        // Длинные пакеты — первыми: точнее.
        packages.sort_by_key(|(p, _)| std::cmp::Reverse(p.len()));
        Self { mods, packages }
    }

    /// Такой мод есть в сборке — вернуть его id в общем виде.
    pub fn known(&self, id: &str) -> Option<String> {
        let id = id.to_lowercase();
        self.mods.contains_key(&id).then_some(id)
    }

    /// Мод по полному имени класса (`com.example.mod.Thing`).
    pub fn by_class(&self, class: &str) -> Option<String> {
        self.packages
            .iter()
            .find(|(p, _)| class.starts_with(p.as_str()) && class[p.len()..].starts_with('.'))
            .map(|(_, id)| id.clone())
    }

    pub fn name(&self, id: &str) -> String {
        self.mods
            .get(&id.to_lowercase())
            .map_or_else(|| id.to_string(), |m| m.name.clone())
    }

    pub fn culprit(&self, id: &str) -> Culprit {
        let m = self.mods.get(&id.to_lowercase());
        Culprit {
            id: id.to_lowercase(),
            name: m.map_or_else(|| id.to_string(), |m| m.name.clone()),
            file: m.map(|m| m.file.clone()),
        }
    }

    pub fn disable_action(&self, id: &str) -> Option<Action> {
        let m = self.mods.get(&id.to_lowercase())?;
        m.enabled.then(|| Action::Disable {
            file: m.file.clone(),
        })
    }

    /// Мод есть в сборке, но выключен — «Включить».
    pub fn enable_action(&self, id: &str) -> Option<Action> {
        let m = self.mods.get(&id.to_lowercase())?;
        (!m.enabled).then(|| Action::Enable {
            file: m.file.clone(),
        })
    }

    pub fn update_action(&self, id: &str) -> Option<Action> {
        let m = self.mods.get(&id.to_lowercase())?;
        let (version_id, _) = m.update.clone()?;
        Some(Action::Update {
            version_id,
            file: m.file.clone(),
        })
    }
}

/// Индекс модов сборки: сведения из jar и обновления из кэша (без сети).
pub async fn index(
    dl: &crate::download::Downloader,
    data: &crate::paths::DataDir,
    build: &str,
) -> ModIndex {
    let local = {
        let (data, build) = (data.clone(), build.to_string());
        tokio::task::spawn_blocking(move || crate::mods::local(&data, &build))
            .await
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or_default()
    };
    let updates: HashMap<String, (String, String)> = crate::mods::report(dl, data, build, false)
        .await
        .map(|r| {
            r.mods
                .into_iter()
                .filter_map(|m| m.update.map(|u| (m.file, (u.version_id, u.version_number))))
                .collect()
        })
        .unwrap_or_default();
    ModIndex::new(
        local
            .into_iter()
            .map(|l| {
                let ids: Vec<String> = l
                    .meta
                    .mods
                    .iter()
                    .map(|m| m.id.clone())
                    .chain(l.meta.provides.iter().cloned())
                    .collect();
                let name = l
                    .meta
                    .mods
                    .first()
                    .map_or_else(|| l.file.clone(), |m| m.name.clone());
                let r = ModRef {
                    name,
                    file: l.file.clone(),
                    enabled: l.enabled,
                    update: updates.get(&l.file).cloned(),
                };
                (ids, r, l.meta.packages.clone())
            })
            .collect(),
    )
}

/// Разбор последней игры сборки: его показывает вкладка «Логи» и значок на
/// плитке (`cache/problems/<сборка>.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Секунды с 1970 года — когда закончилась игра.
    pub at: u64,
    pub analysis: Analysis,
    /// Игра упала (а не закрылась сама).
    #[serde(default)]
    pub crashed: bool,
    /// Ответ ИИ-разбора по этой игре.
    #[serde(default)]
    pub ai: Option<crate::ai::Answer>,
}

fn session_path(data: &crate::paths::DataDir, build: &str) -> Option<PathBuf> {
    // Только имя папки — из окна не должно прийти «../».
    (!build.is_empty() && !build.contains(['/', '\\']) && !build.contains(".."))
        .then(|| data.cache().join("problems").join(format!("{build}.json")))
}

pub fn save_session(data: &crate::paths::DataDir, build: &str, analysis: &Analysis, crashed: bool) {
    write_session(
        data,
        build,
        &Session {
            at: crate::builds::now(),
            analysis: analysis.clone(),
            crashed,
            ai: None,
        },
    );
}

/// Запомнить ответ ИИ у последней игры.
pub fn save_answer(data: &crate::paths::DataDir, build: &str, answer: &crate::ai::Answer) {
    if let Some(mut s) = last_session(data, build) {
        s.ai = Some(answer.clone());
        write_session(data, build, &s);
    }
}

fn write_session(data: &crate::paths::DataDir, build: &str, session: &Session) {
    let Some(path) = session_path(data, build) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, serde_json::to_vec(session).expect("сериализуется"));
}

pub fn last_session(data: &crate::paths::DataDir, build: &str) -> Option<Session> {
    let bytes = std::fs::read(session_path(data, build)?).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Сколько проблем было в последней игре у каждой сборки.
pub fn problem_counts(data: &crate::paths::DataDir) -> HashMap<String, usize> {
    let Ok(dir) = std::fs::read_dir(data.cache().join("problems")) else {
        return HashMap::new();
    };
    dir.flatten()
        .filter_map(|e| {
            let name = e
                .file_name()
                .to_string_lossy()
                .strip_suffix(".json")?
                .to_string();
            let s: Session = serde_json::from_slice(&std::fs::read(e.path()).ok()?).ok()?;
            let n = s.analysis.critical.len() + s.analysis.warnings.len();
            (n > 0).then_some((name, n))
        })
        .collect()
}

/// Разобрать файл лога сборки по кнопке «Разобрать» (`logs/…`,
/// `crash-reports/…`). Путь проверяет `content::log_path`.
pub async fn analyze_file(
    dl: &crate::download::Downloader,
    data: &crate::paths::DataDir,
    build: &str,
    file: &str,
) -> Result<Analysis> {
    let game = data.instance(build);
    let path = crate::content::log_path(&game, file)?;
    let text = crate::content::read_log(&game, file)?.text;
    let index = index(dl, data, build).await;
    let is_report = file.starts_with("crash-reports");
    let report = is_report.then(|| {
        CrashReport::parse(
            path.file_name()
                .map_or(file.to_string(), |n| n.to_string_lossy().to_string())
                .as_str(),
            &text,
        )
    });
    let mut a = Analyzer::new(remote::current(data));
    if !is_report {
        a.text(&text);
    }
    Ok(a.finish(is_report, report.as_ref(), &index))
}

pub mod remote;

#[cfg(test)]
mod tests;
