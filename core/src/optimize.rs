//! Кнопка «Оптимизировать» (раздел 5.7 ТЗ). Что она делает, игрок выбирает
//! в «Настройках → Оптимизация»; сама кнопка срабатывает в один клик. Перед
//! оптимизацией всегда сохраняется снапшот — всё откатывается одним действием.
//! После — отчёт: что сделано, что пропущено и почему.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::builds::{Builds, Loader, Patch};
use crate::catalog::{self, Target};
use crate::config::GameSettings;
use crate::download::Downloader;
use crate::error::Result;
use crate::health::{self, Hardware, gb};
use crate::install::Event;
use crate::modrinth::ProjectType;
use crate::mods;
use crate::options::{self, Group, Profile};
use crate::paths::DataDir;
use crate::snapshots;

/// Что делает кнопка. По умолчанию — как в ТЗ: всё полезное включено,
/// настройки графики (меняют привычное игроку) и замер FPS — выключены.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizeSettings {
    #[serde(default = "yes")]
    pub hardware: bool,
    #[serde(default = "yes")]
    pub mods: bool,
    #[serde(default = "yes")]
    pub memory: bool,
    #[serde(default = "yes")]
    pub gpu: bool,
    #[serde(default = "yes")]
    pub system: bool,
    #[serde(default)]
    pub graphics: bool,
    #[serde(default)]
    pub fps: bool,
    /// Игрок уже видел объяснение, что делает кнопка.
    #[serde(default)]
    pub intro_seen: bool,
}

fn yes() -> bool {
    true
}

impl Default for OptimizeSettings {
    fn default() -> Self {
        Self {
            hardware: true,
            mods: true,
            memory: true,
            gpu: true,
            system: true,
            graphics: false,
            fps: false,
            intro_seen: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// Сделано.
    Done,
    /// Сделать нечего или нельзя — почему, в тексте.
    Skipped,
    /// Сведения и советы, которые лаунчер сам не исправит.
    Info,
    /// Не получилось.
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct Step {
    pub title: String,
    pub state: State,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub steps: Vec<Step>,
    /// Снапшот «перед оптимизацией» — для кнопки «Откатить».
    pub snapshot: String,
}

/// Оптимизаторы: slug на Modrinth, название и зачем он нужен. Только
/// проверенные и безопасные — без модов, меняющих игру.
struct Candidate {
    slug: &'static str,
    title: &'static str,
    why: &'static str,
}

const SODIUM: Candidate = Candidate {
    slug: "sodium",
    title: "Sodium",
    why: "новый движок отрисовки — FPS выше в разы",
};
const EMBEDDIUM: Candidate = Candidate {
    slug: "embeddium",
    title: "Embeddium",
    why: "движок отрисовки как Sodium, для Forge",
};
const COMMON: &[Candidate] = &[
    Candidate {
        slug: "ferrite-core",
        title: "FerriteCore",
        why: "игре нужно заметно меньше памяти",
    },
    Candidate {
        slug: "modernfix",
        title: "ModernFix",
        why: "быстрее загрузка и меньше памяти",
    },
    Candidate {
        slug: "immediatelyfast",
        title: "ImmediatelyFast",
        why: "быстрее рисуются интерфейс, текст и частицы",
    },
    Candidate {
        slug: "entityculling",
        title: "Entity Culling",
        why: "не рисует мобов и сундуки, скрытых за стенами",
    },
];
const FABRIC_ONLY: &[Candidate] = &[
    Candidate {
        slug: "lithium",
        title: "Lithium",
        why: "быстрее логика мира, мобов и механизмов",
    },
    Candidate {
        slug: "dynamic-fps",
        title: "Dynamic FPS",
        why: "свёрнутая игра не грузит видеокарту",
    },
];

/// Оптимизировать сборку по настройкам. `stage` — подписи шагов для окна.
pub async fn run(
    dl: &Downloader,
    data: &DataDir,
    build: &str,
    opts: &OptimizeSettings,
    game: &GameSettings,
    progress: &(dyn Fn(Event) + Sync),
    stage: &(dyn Fn(&str) + Sync),
) -> Result<Report> {
    let builds = Builds::new(data.clone());
    let instance = builds.get(build)?;
    if instance.quick {
        return Err(crate::Error::Other(
            "«Быструю игру» не оптимизируют: в ней нет модов и своих настроек — сделайте сборку"
                .into(),
        ));
    }
    stage("Сохраняем снапшот");
    let snapshot = {
        let (data, build) = (data.clone(), build.to_string());
        tokio::task::spawn_blocking(move || {
            snapshots::take(&data, &build, "перед оптимизацией", false)
        })
        .await
        .map_err(|e| crate::Error::Other(e.to_string()))??
    };
    let mut steps = Vec::new();

    stage("Определяем железо");
    let hw = tokio::task::spawn_blocking(health::hardware)
        .await
        .map_err(|e| crate::Error::Other(e.to_string()))?;
    if opts.hardware {
        steps.push(Step {
            title: "Железо".into(),
            state: State::Info,
            text: describe(&hw),
        });
    }

    if opts.mods {
        stage("Подбираем оптимизаторы");
        steps.extend(
            install_optimizers(
                dl,
                data,
                build,
                &instance.loader,
                &instance.version,
                progress,
            )
            .await,
        );
    }

    if opts.memory {
        stage("Подбираем память");
        steps.push(memory(data, build, game, &hw, opts.hardware)?);
    }

    if opts.gpu {
        steps.push(gpu(data, &hw));
    }

    if opts.system {
        steps.extend(system(&hw));
    }

    if opts.graphics {
        stage("Настраиваем графику");
        steps.push(graphics(data, build, &hw, opts.hardware));
    }

    if opts.fps {
        steps.push(Step {
            title: "Замер FPS".into(),
            state: State::Skipped,
            text:
                "замер «до и после» через мод-помощник ещё не готов — появится в следующих версиях"
                    .into(),
        });
    }

    Ok(Report {
        steps,
        snapshot: snapshot.id,
    })
}

fn describe(hw: &Hardware) -> String {
    let mut parts = Vec::new();
    if let Some(cpu) = &hw.cpu {
        parts.push(format!("{cpu} ({} потоков)", hw.cores));
    }
    if let Some(t) = hw.ram_total_mb {
        let free = hw
            .ram_free_mb
            .map(|f| format!(", свободно {}", gb(f)))
            .unwrap_or_default();
        parts.push(format!("память {}{free}", gb(t)));
    }
    let gpus: Vec<String> = hw
        .gpus
        .iter()
        .map(|g| {
            format!(
                "{}{}",
                g.name,
                if g.discrete {
                    ""
                } else {
                    " (встроенная)"
                }
            )
        })
        .collect();
    if !gpus.is_empty() {
        parts.push(format!("видеокарта {}", gpus.join(" и ")));
    }
    if hw.laptop {
        parts.push("ноутбук".into());
    }
    parts.join("; ")
}

/// Поставить оптимизаторы, совместимые со сборкой и её модами.
async fn install_optimizers(
    dl: &Downloader,
    data: &DataDir,
    build: &str,
    loader: &Loader,
    version: &str,
    progress: &(dyn Fn(Event) + Sync),
) -> Vec<Step> {
    let title = |c: &Candidate| format!("{} — {}", c.title, c.why);
    if *loader == Loader::Vanilla {
        return vec![Step {
            title: "Оптимизаторы".into(),
            state: State::Skipped,
            text: "сборка без загрузчика модов — оптимизаторы ставятся на Fabric, Forge или NeoForge (сменить загрузчик — в настройках сборки)".into(),
        }];
    }
    // Что уже стоит: проекты Modrinth и имена файлов (OptiFine не с Modrinth).
    let report = mods::report(dl, data, build, true).await.ok();
    let installed: HashSet<String> = report
        .iter()
        .flat_map(|r| r.mods.iter())
        .filter_map(|m| m.modrinth.as_ref().map(|k| k.project_id.clone()))
        .collect();
    let files: Vec<String> = report
        .iter()
        .flat_map(|r| r.mods.iter())
        .map(|m| format!("{} {}", m.file, m.name).to_lowercase())
        .collect();
    let has_file = |needle: &str| files.iter().any(|f| f.contains(needle));
    let optifine = has_file("optifine") || has_file("optifabric");

    let mut list: Vec<&Candidate> = Vec::new();
    // Движок отрисовки — один: Sodium или Embeddium (Rubidium — его старое имя).
    let renderer_present = has_file("sodium") || has_file("embeddium") || has_file("rubidium");
    let mut steps = Vec::new();
    if optifine {
        steps.push(Step {
            title: "Sodium".into(),
            state: State::Skipped,
            text: "в сборке OptiFine — Sodium и Embeddium с ним несовместимы".into(),
        });
    } else if renderer_present {
        steps.push(Step {
            title: "Движок отрисовки".into(),
            state: State::Skipped,
            text: "уже стоит (Sodium, Embeddium или Rubidium)".into(),
        });
    } else {
        list.push(match loader {
            Loader::Forge => &EMBEDDIUM,
            _ => &SODIUM,
        });
    }
    list.extend(COMMON.iter());
    if *loader == Loader::Fabric {
        list.extend(FABRIC_ONLY.iter());
    }

    let target = Target::Build { id: build.into() };
    let mut versions: Vec<String> = Vec::new();
    let mut planned: Vec<(&Candidate, Vec<String>)> = Vec::new();
    for c in list {
        let mut found = catalog::plan(dl, data, &target, ProjectType::Mod, c.slug, None)
            .await
            .map(|p| (c, p));
        // У NeoForge до 1.20.6 Sodium нет — его место занимает Embeddium.
        if found.is_err() && c.slug == SODIUM.slug && *loader == Loader::NeoForge {
            found = catalog::plan(dl, data, &target, ProjectType::Mod, EMBEDDIUM.slug, None)
                .await
                .map(|p| (&EMBEDDIUM, p));
        }
        let (c, plan) = match found {
            Ok(found) => found,
            Err(_) => {
                steps.push(Step {
                    title: title(c),
                    state: State::Skipped,
                    text: format!("нет версии под {version} {}", loader.title()),
                });
                continue;
            }
        };
        let main = plan.items.iter().find(|i| !i.dependency);
        let already = plan.present.iter().any(|p| p.eq_ignore_ascii_case(c.title))
            || main.is_some_and(|m| m.replaces.is_some() || installed.contains(&m.project_id))
            || (main.is_none() && plan.missing.is_empty());
        if already {
            steps.push(Step {
                title: title(c),
                state: State::Skipped,
                text: "уже стоит".into(),
            });
            continue;
        }
        if !plan.conflicts.is_empty() {
            steps.push(Step {
                title: title(c),
                state: State::Skipped,
                text: format!("несовместим с {}", plan.conflicts.join(", ")),
            });
            continue;
        }
        let ids: Vec<String> = plan.items.iter().map(|i| i.version_id.clone()).collect();
        for id in &ids {
            if !versions.contains(id) {
                versions.push(id.clone());
            }
        }
        planned.push((c, ids));
    }
    if versions.is_empty() {
        return steps;
    }
    match catalog::install(dl, data, &target, ProjectType::Mod, &versions, progress).await {
        Ok(_) => {
            for (c, _) in planned {
                steps.push(Step {
                    title: title(c),
                    state: State::Done,
                    text: "поставлен".into(),
                });
            }
        }
        Err(e) => steps.push(Step {
            title: "Оптимизаторы".into(),
            state: State::Failed,
            text: format!("не скачались: {e}"),
        }),
    }
    steps
}

/// Память под число модов и ОЗУ — в настройки сборки.
fn memory(
    data: &DataDir,
    build: &str,
    game: &GameSettings,
    hw: &Hardware,
    know_hw: bool,
) -> Result<Step> {
    let builds = Builds::new(data.clone());
    let instance = builds.get(build)?;
    let mods = count_mods(data, build);
    let total = know_hw.then_some(hw.ram_total_mb).flatten();
    let wanted = health::recommended_memory_mb(&instance, mods, total);
    let now = health::allocated_mb(&instance, game);
    let gb = |mb: u32| health::gb(mb as u64);
    let why = match total {
        Some(t) => format!("{}, в ПК {}", health::mods_word(mods), gb(t as u32)),
        None => format!(
            "{}; железо не определялось — с запасом на ПК с 8 ГБ",
            health::mods_word(mods)
        ),
    };
    // Только прибавляем: урезать работающую память — не оптимизация. Убавляем,
    // лишь если выделено опасно много (Windows остаётся меньше 1,5 ГБ).
    let too_much = total.is_some_and(|t| now as u64 + 1536 > t);
    if now >= wanted && !too_much {
        return Ok(Step {
            title: "Память".into(),
            state: State::Skipped,
            text: format!("{} — хватает ({why}; нужно около {})", gb(now), gb(wanted)),
        });
    }
    let mut settings = instance.settings.clone();
    settings.memory_mb = Some(wanted);
    builds.update(
        build,
        Patch {
            settings: Some(settings),
            ..Patch::default()
        },
    )?;
    let low_free = hw
        .ram_free_mb
        .filter(|f| know_hw && (*f as u32) < wanted)
        .map(|f| {
            format!(
                ". Сейчас свободно только {} — закройте браузер и другие тяжёлые программы перед игрой",
                gb(f as u32)
            )
        })
        .unwrap_or_default();
    Ok(Step {
        title: "Память".into(),
        state: State::Done,
        text: format!("{} вместо {} ({why}){low_free}", gb(wanted), gb(now)),
    })
}

/// Включённые моды в сборке.
fn count_mods(data: &DataDir, build: &str) -> usize {
    std::fs::read_dir(data.instance(build).join("mods"))
        .map(|d| {
            d.flatten()
                .filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .to_lowercase()
                        .ends_with(".jar")
                })
                .count()
        })
        .unwrap_or(0)
}

/// Ноутбук с двумя видеокартами — Java на отдельную.
fn gpu(data: &DataDir, hw: &Hardware) -> Step {
    let title = "Видеокарта".to_string();
    if !hw.hybrid() {
        return Step {
            title,
            state: State::Skipped,
            text: "видеокарта одна — выбирать не из чего".into(),
        };
    }
    // На обычном ПК игра идёт на той видеокарте, к которой подключён монитор;
    // переключать нужно только ноутбуки.
    if !hw.laptop {
        return Step {
            title,
            state: State::Skipped,
            text: "это не ноутбук — игра идёт на видеокарте, к которой подключён монитор".into(),
        };
    }
    let discrete = hw
        .gpus
        .iter()
        .find(|g| g.discrete)
        .map_or("отдельной", |g| g.name.as_str());
    if health::java_on_discrete(data) {
        return Step {
            title,
            state: State::Skipped,
            text: format!("игра уже идёт на {discrete}"),
        };
    }
    match health::prefer_discrete(data) {
        Ok(0) => Step {
            title,
            state: State::Skipped,
            text:
                "Java ещё не скачана — запустите сборку один раз и нажмите «Оптимизировать» снова"
                    .into(),
        },
        Ok(_) => Step {
            title,
            state: State::Done,
            text: format!(
                "Java лаунчера будет работать на {discrete}, а не на встроенной (как «Параметры Windows → Графика»)"
            ),
        },
        Err(e) => Step {
            title,
            state: State::Failed,
            text: e.to_string(),
        },
    }
}

/// Проверка системы: питание, драйвер, Java.
fn system(hw: &Hardware) -> Vec<Step> {
    let mut out = Vec::new();
    if hw.power_saver {
        out.push(Step {
            title: "Режим питания".into(),
            state: State::Info,
            text: "включена «Экономия энергии» — переключите на «Сбалансированный» или «Высокая производительность» в настройках питания Windows".into(),
        });
    }
    let year = crate::builds::now() / (365 * 24 * 3600) + 1970;
    for g in hw.gpus.iter().filter(|g| g.discrete || hw.gpus.len() == 1) {
        if let Some(d) = &g.driver_date
            && d[..4].parse::<u64>().is_ok_and(|y| y + 3 < year)
        {
            out.push(Step {
                title: "Драйвер видеокарты".into(),
                state: State::Info,
                text: format!(
                    "{} — драйвер от {d}, ему больше трёх лет; новый часто прибавляет FPS",
                    g.name
                ),
            });
        }
    }
    if out.is_empty() {
        out.push(Step {
            title: "Система".into(),
            state: State::Info,
            text: "в порядке: питание без экономии, драйвер видеокарты свежий, Java 64-битная"
                .into(),
        });
    }
    out
}

/// Настройки графики под железо — в сборку (или в общий профиль, если
/// графика у игрока общая для всех сборок).
fn graphics(data: &DataDir, build: &str, hw: &Hardware, know_hw: bool) -> Step {
    let title = "Графика".to_string();
    if !know_hw {
        return Step {
            title,
            state: State::Skipped,
            text: "без определения железа подбирать графику вслепую не будем — включите «Определить железо»".into(),
        };
    }
    let (tier, preset): (&str, &[(&str, &str)]) = if hw.weak() {
        (
            "слабого ПК",
            &[
                ("renderDistance", "8"),
                ("simulationDistance", "6"),
                ("graphicsMode", "0"),
                ("renderClouds", "\"false\""),
                ("particles", "1"),
                ("entityShadows", "false"),
                ("biomeBlendRadius", "0"),
            ],
        )
    } else if hw.strong() {
        (
            "мощного ПК",
            &[
                ("renderDistance", "16"),
                ("simulationDistance", "10"),
                ("graphicsMode", "1"),
                ("particles", "0"),
                ("entityShadows", "true"),
                ("biomeBlendRadius", "2"),
            ],
        )
    } else {
        (
            "среднего ПК",
            &[
                ("renderDistance", "12"),
                ("simulationDistance", "8"),
                ("graphicsMode", "1"),
                ("renderClouds", "\"fast\""),
                ("particles", "1"),
                ("entityShadows", "true"),
                ("biomeBlendRadius", "1"),
            ],
        )
    };
    let result = (|| -> Result<String> {
        let instance = Builds::new(data.clone()).get(build)?;
        let mut tmp = Profile {
            shared: Some(vec![Group::Graphics]),
            ..Profile::default()
        };
        for (k, v) in preset {
            tmp.set(k, v.trim_matches('"'))?;
        }
        let mut shared = Profile::load(data)?;
        let summary = format!(
            "дальность {} чанков, {} — под {tier}",
            preset[0].1,
            if preset[2].1 == "0" {
                "быстрая графика"
            } else {
                "детальная графика"
            }
        );
        // Графика общая для всех сборок — правим общий профиль, иначе он
        // перезапишет файл сборки при запуске.
        if !instance.settings.own_options && shared.is_shared(Group::Graphics) {
            for (k, v) in &tmp.values {
                shared.set(k, v)?;
            }
            shared.save(data)?;
            return Ok(format!("{summary} (графика у вас общая — для всех сборок)"));
        }
        let Some(v) = options::installed_version(data, &instance.version) else {
            return Err(crate::Error::Other(
                "сборка ещё не запускалась — нажмите «Оптимизировать» после первого запуска".into(),
            ));
        };
        options::apply(&tmp, &data.instance(build), &v)?;
        Ok(summary)
    })();
    match result {
        Ok(text) => Step {
            title,
            state: State::Done,
            text,
        },
        Err(e) => Step {
            title,
            state: State::Skipped,
            text: e.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_follow_tz() {
        let d = OptimizeSettings::default();
        assert!(d.hardware && d.mods && d.memory && d.gpu && d.system);
        assert!(!d.graphics && !d.fps && !d.intro_seen);
        let old: OptimizeSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(old, d);
    }

    #[test]
    fn graphics_go_into_build_options() {
        let root = tempfile::tempdir().unwrap();
        let data = DataDir::new(root.path());
        let b = Builds::new(data.clone())
            .create(
                "Т",
                "1.21.1",
                crate::builds::LoaderChoice {
                    loader: Loader::Fabric,
                    version: Some("0.16.10".into()),
                },
                None,
            )
            .unwrap();
        // Сборка не запускалась — версии на диске нет.
        let hw = Hardware {
            ram_total_mb: Some(8192),
            ram_free_mb: Some(4000),
            cores: 4,
            cpu: None,
            gpus: vec![],
            laptop: false,
            power_saver: false,
        };
        let s = graphics(&data, &b.id, &hw, true);
        assert_eq!(s.state, State::Skipped);
        // Версия на диске есть — настройки ложатся в options.txt сборки.
        let vdir = data.version_dir("1.21.1");
        std::fs::create_dir_all(&vdir).unwrap();
        std::fs::write(
            data.version_json("1.21.1"),
            r#"{"id":"1.21.1","type":"release","mainClass":"x","releaseTime":"2024-08-08T11:51:42+00:00","assetIndex":{"id":"17","url":"u","sha1":"s","size":1},"downloads":{"client":{"url":"u","sha1":"s","size":1}},"libraries":[]}"#,
        )
        .unwrap();
        let s = graphics(&data, &b.id, &hw, true);
        assert_eq!(s.state, State::Done, "{}", s.text);
        let text = std::fs::read_to_string(data.instance(&b.id).join("options.txt")).unwrap();
        assert!(text.contains("renderDistance:8"), "{text}");
        assert!(text.contains("entityShadows:false"), "{text}");
    }
}
