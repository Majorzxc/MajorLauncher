//! Система и железо (разделы 5.6 «Система» и 5.7 ТЗ): что за ПК, хватит ли
//! места и памяти, на какой видеокарте пойдёт игра, не включена ли экономия
//! энергии. Отсюда — предупреждения у сборки и решения кнопки «Оптимизировать».
//! Всё читается из Windows без сторонних программ и быстро: проверка идёт
//! перед каждым запуском.

use std::path::PathBuf;

use serde::Serialize;

use crate::builds::{Instance, Loader};
use crate::config::GameSettings;
use crate::mods::{Action, Issue, Level};
use crate::paths::DataDir;

/// Видеокарта.
#[derive(Debug, Clone, Serialize)]
pub struct Gpu {
    pub name: String,
    /// Отдельная (NVIDIA, AMD RX, Intel Arc), а не встроенная в процессор.
    pub discrete: bool,
    pub driver: Option<String>,
    /// Дата драйвера `ГГГГ-ММ-ДД`.
    pub driver_date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hardware {
    pub ram_total_mb: Option<u64>,
    pub ram_free_mb: Option<u64>,
    pub cores: usize,
    pub cpu: Option<String>,
    pub gpus: Vec<Gpu>,
    /// Есть батарея — ноутбук.
    pub laptop: bool,
    /// Схема питания «Экономия энергии» или включён режим экономии батареи.
    pub power_saver: bool,
}

impl Hardware {
    /// Две видеокарты, встроенная и отдельная: Windows может пустить игру
    /// на встроенную.
    pub fn hybrid(&self) -> bool {
        self.gpus.iter().any(|g| g.discrete) && self.gpus.iter().any(|g| !g.discrete)
    }

    /// Слабый ПК: мало памяти, нет отдельной видеокарты или старый
    /// процессор с 4 потоками и меньше (FX-4300 из ТЗ).
    pub fn weak(&self) -> bool {
        self.ram_total_mb.is_some_and(|m| m <= 8 * 1024)
            || !self.gpus.iter().any(|g| g.discrete)
            || (self.cores <= 4 && self.cpu.as_deref().is_some_and(old_cpu))
    }

    /// Мощный ПК: от 16 ГБ, современная отдельная видеокарта и не старый
    /// процессор — игра упирается в процессор не меньше, чем в видеокарту.
    pub fn strong(&self) -> bool {
        self.ram_total_mb.is_some_and(|m| m >= 15 * 1024)
            && self.gpus.iter().any(|g| g.discrete && modern_gpu(&g.name))
            && !self.cpu.as_deref().is_some_and(old_cpu)
    }
}

/// Процессоры, которым тяжело с большой дальностью: AMD до Ryzen, Intel до
/// Core i (и бюджетные Pentium/Celeron), а также Core i первых поколений.
fn old_cpu(name: &str) -> bool {
    let n = name.to_lowercase();
    [
        "fx(tm)",
        "fx-",
        "phenom",
        "athlon",
        "a4-",
        "a6-",
        "a8-",
        "a10-",
        "core(tm)2",
        "core 2",
        "pentium",
        "celeron",
    ]
    .iter()
    .any(|p| n.contains(p))
        || n.contains("i3-2")
        || n.contains("i5-2")
        || n.contains("i7-2")
        || n.contains("i3-3")
        || n.contains("i5-3")
        || n.contains("i7-3")
}

/// Видеокарты последних поколений: RTX, RX 6000+, Arc.
fn modern_gpu(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("rtx") || n.contains("arc") || ["rx 6", "rx 7", "rx 9"].iter().any(|p| n.contains(p))
}

/// Отдельная ли видеокарта — по названию.
pub fn is_discrete(name: &str) -> bool {
    let n = name.to_lowercase();
    if n.contains("nvidia") {
        return true;
    }
    if n.contains("intel") {
        return n.contains("arc");
    }
    if n.contains("amd") || n.contains("ati ") || n.contains("radeon") {
        // Встроенные в процессор или материнскую плату называются «… Graphics»:
        // «AMD Radeon(TM) Graphics», «Radeon Vega 8 Graphics», «ATI Radeon 3000
        // Graphics». Отдельные — «Radeon RX 580», «Radeon HD 7700 Series».
        let integrated = !n.contains("rx") && (n.ends_with("graphics") || n.contains("vega"));
        return !integrated;
    }
    false
}

/// «6 ГБ», «5,5 ГБ» — как пишут по-русски.
pub fn gb(mb: u64) -> String {
    let v = (mb as f64 / 1024.0 * 10.0).round() / 10.0;
    if v.fract() == 0.0 {
        format!("{v:.0} ГБ")
    } else {
        format!("{v:.1} ГБ").replace('.', ",")
    }
}

/// «1 мод», «2 мода», «5 модов».
pub fn mods_word(n: usize) -> String {
    let (d, h) = (n % 10, n % 100);
    let word = if d == 1 && h != 11 {
        "мод"
    } else if (2..=4).contains(&d) && !(12..=14).contains(&h) {
        "мода"
    } else {
        "модов"
    };
    format!("{n} {word}")
}

/// Что за ПК.
pub fn hardware() -> Hardware {
    let total = crate::system::total_ram_mb();
    Hardware {
        ram_total_mb: total,
        ram_free_mb: imp::free_ram_mb(),
        cores: std::thread::available_parallelism().map_or(1, |n| n.get()),
        cpu: imp::cpu_name(),
        gpus: imp::gpus(),
        laptop: imp::has_battery(),
        power_saver: imp::power_saver(),
    }
}

/// Сколько памяти дать сборке: ваниле 2 ГБ (до 1.13 — 1 ГБ), со модами —
/// 2 ГБ и по 25 МБ на мод, с округлением до 512 МБ. Не больше половины ОЗУ
/// с запасом системе 2,5 ГБ и не больше 8 ГБ: больше игре во вред — сборщик
/// мусора дольше останавливает игру.
pub fn recommended_memory_mb(instance: &Instance, mods: usize, total_mb: Option<u64>) -> u32 {
    let base: u64 = if instance.loader == Loader::Vanilla {
        if old_version(&instance.version) {
            1024
        } else {
            2048
        }
    } else {
        2048 + mods as u64 * 25
    };
    let rounded = base.div_ceil(512) * 512;
    let total = total_mb.unwrap_or(8192);
    let cap = total.saturating_sub(2560).clamp(1024, 8192);
    rounded.min(cap).max(1024) as u32
}

/// Версия до 1.13 (старые сборки и сама игра просят меньше памяти).
fn old_version(v: &str) -> bool {
    let mut parts = v.split('.');
    let (Some("1"), Some(minor)) = (parts.next(), parts.next()) else {
        return false;
    };
    minor.parse::<u32>().is_ok_and(|m| m < 13)
}

/// Сколько памяти сборка получит на деле (своя, общая или «авто»).
pub fn allocated_mb(instance: &Instance, game: &GameSettings) -> u32 {
    crate::game::memory_mb(instance, game)
}

/// Системные предупреждения у сборки (раздел 5.6 «Система»).
pub fn issues(
    data: &DataDir,
    instance: &Instance,
    game: &GameSettings,
    mods: usize,
    hw: &Hardware,
) -> Vec<Issue> {
    let mut out = Vec::new();
    let issue = |level, text: String, action| Issue {
        level,
        text,
        file: None,
        action,
    };

    // Место на диске с папкой данных.
    if let Some(free) = crate::datadir::free_space(data.root()) {
        let free_mb = free / (1024 * 1024);
        if free_mb < 300 {
            out.push(issue(
                Level::Block,
                format!(
                    "На диске с папкой данных свободно всего {} — игре негде сохранить мир и скачать файлы. Освободите место.",
                    gb(free_mb)
                ),
                None,
            ));
        } else if free_mb < 2048 {
            out.push(issue(
                Level::Warn,
                format!(
                    "На диске с папкой данных свободно {} — мало для обновлений и новых миров.",
                    gb(free_mb)
                ),
                None,
            ));
        }
    }

    // Память.
    let given = allocated_mb(instance, game) as u64;
    if let Some(total) = hw.ram_total_mb {
        if given > total {
            out.push(issue(
                Level::Block,
                format!(
                    "Игре выделено {}, а в ПК всего {} — Java не запустится. Уменьшите память.",
                    gb(given),
                    gb(total)
                ),
                Some(Action::Memory),
            ));
        } else if given + 1536 > total {
            out.push(issue(
                Level::Warn,
                format!(
                    "Игре выделено {} из {} — Windows останется меньше 1,5 ГБ, ПК будет тормозить.",
                    gb(given),
                    gb(total)
                ),
                Some(Action::Memory),
            ));
        }
        let wanted = recommended_memory_mb(instance, mods, Some(total)) as u64;
        if mods >= 30 && given + 512 <= wanted {
            out.push(issue(
                Level::Warn,
                format!(
                    "Для сборки из {} мало памяти: выделено {}, нужно около {}. Игра может зависать и падать.",
                    mods_word(mods),
                    gb(given),
                    gb(wanted)
                ),
                Some(Action::Optimize),
            ));
        }
    }

    // Ноутбук с двумя видеокартами: игра может пойти на встроенной.
    if hw.laptop && hw.hybrid() && !java_on_discrete(data) {
        out.push(issue(
            Level::Tip,
            "Ноутбук с двумя видеокартами: Windows может запустить игру на встроенной — FPS будет в разы ниже. «Оптимизировать» переведёт Java на отдельную.".into(),
            Some(Action::Optimize),
        ));
    }

    if hw.power_saver {
        out.push(issue(
            Level::Tip,
            "Включён режим «Экономия энергии» — процессор и видеокарта работают медленнее, игра тоже.".into(),
            Some(Action::PowerSettings),
        ));
    }

    // Старый драйвер видеокарты (больше трёх лет).
    let year = crate::builds::now() / (365 * 24 * 3600) + 1970;
    for g in hw.gpus.iter().filter(|g| g.discrete || hw.gpus.len() == 1) {
        if let Some(date) = &g.driver_date
            && date[..4].parse::<u64>().is_ok_and(|y| y + 3 < year)
        {
            out.push(issue(
                Level::Tip,
                format!(
                    "Драйвер видеокарты {} от {date} — ему больше трёх лет. Новый драйвер часто прибавляет FPS и чинит вылеты.",
                    g.name
                ),
                None,
            ));
        }
    }
    out
}

/// Где Windows хранит выбор видеокарты для программ (Параметры → Дисплей →
/// Графика): значение — путь к exe, данные — `GpuPreference=2;` (отдельная).
const GPU_PREFS: &str = r"Software\Microsoft\DirectX\UserGpuPreferences";

/// Все `javaw.exe`, которые ставил лаунчер.
pub fn java_executables(data: &DataDir) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(dirs) = std::fs::read_dir(data.root().join("java")) else {
        return out;
    };
    for d in dirs.flatten() {
        // Windows сравнивает путь к программе строкой — только «\», как у
        // запущенного процесса.
        for exe in ["javaw.exe", "java.exe"] {
            let p = d.path().join("bin").join(exe);
            if p.exists() {
                out.push(PathBuf::from(p.display().to_string().replace('/', "\\")));
            }
        }
    }
    out
}

/// Java лаунчера уже настроена на отдельную видеокарту.
pub fn java_on_discrete(data: &DataDir) -> bool {
    let list = java_executables(data);
    !list.is_empty() && list.iter().all(|p| imp::gpu_preference(p) == Some(2))
}

/// Пустить Java лаунчера на отдельную видеокарту (как в «Параметрах Windows →
/// Графика»). Возвращает, сколько программ настроено.
pub fn prefer_discrete(data: &DataDir) -> crate::Result<usize> {
    let list = java_executables(data);
    for p in &list {
        imp::set_gpu_preference(p, 2)?;
    }
    Ok(list.len())
}

#[cfg(windows)]
mod imp {
    use std::path::Path;

    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};

    use super::{GPU_PREFS, Gpu, is_discrete};

    pub fn free_ram_mb() -> Option<u64> {
        use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
        // SAFETY: структура обнулена и её размер записан, как требует функция.
        unsafe {
            let mut s: MEMORYSTATUSEX = std::mem::zeroed();
            s.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
            (GlobalMemoryStatusEx(&mut s) != 0).then_some(s.ullAvailPhys / 1_048_576)
        }
    }

    pub fn cpu_name() -> Option<String> {
        RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0")
            .and_then(|k| k.get_value::<String, _>("ProcessorNameString"))
            .ok()
            .map(|s| s.trim().to_string())
    }

    /// Видеокарты из списка драйверов Windows (класс «Видеоадаптеры»).
    pub fn gpus() -> Vec<Gpu> {
        const CLASS: &str =
            r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";
        let Ok(class) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(CLASS, KEY_READ)
        else {
            return Vec::new();
        };
        let mut out: Vec<Gpu> = Vec::new();
        for key in class.enum_keys().flatten() {
            let Ok(k) = class.open_subkey_with_flags(&key, KEY_READ) else {
                continue;
            };
            let Ok(name) = k.get_value::<String, _>("DriverDesc") else {
                continue;
            };
            let lower = name.to_lowercase();
            // Служебные «видеокарты» Windows и удалённого рабочего стола.
            if lower.contains("basic display")
                || lower.contains("basic render")
                || lower.contains("remote")
                || lower.contains("virtual")
                || out.iter().any(|g| g.name == name)
            {
                continue;
            }
            // «10-14-2025» → «2025-10-14».
            let driver_date = k.get_value::<String, _>("DriverDate").ok().and_then(|d| {
                let p: Vec<&str> = d.split('-').collect();
                (p.len() == 3).then(|| format!("{}-{:0>2}-{:0>2}", p[2], p[0], p[1]))
            });
            out.push(Gpu {
                discrete: is_discrete(&name),
                driver: k.get_value::<String, _>("DriverVersion").ok(),
                driver_date,
                name,
            });
        }
        out
    }

    pub fn has_battery() -> bool {
        use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
        // SAFETY: структура из чисел, функция её заполняет.
        unsafe {
            let mut s: SYSTEM_POWER_STATUS = std::mem::zeroed();
            // 128 — батареи нет, 255 — неизвестно.
            GetSystemPowerStatus(&mut s) != 0 && s.BatteryFlag != 128 && s.BatteryFlag != 255
        }
    }

    /// Схема «Экономия энергии» или режим экономии батареи.
    pub fn power_saver() -> bool {
        use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
        const SAVER: &str = "a1841308-3541-4fab-bc81-f71556f20b4a";
        let scheme = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey(r"SYSTEM\CurrentControlSet\Control\Power\User\PowerSchemes")
            .and_then(|k| k.get_value::<String, _>("ActivePowerScheme"))
            .is_ok_and(|g| g.eq_ignore_ascii_case(SAVER));
        // SAFETY: структура из чисел, функция её заполняет.
        let battery_saver = unsafe {
            let mut s: SYSTEM_POWER_STATUS = std::mem::zeroed();
            GetSystemPowerStatus(&mut s) != 0 && s.SystemStatusFlag == 1
        };
        scheme || battery_saver
    }

    pub fn gpu_preference(exe: &Path) -> Option<u32> {
        let v: String = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(GPU_PREFS)
            .ok()?
            .get_value(exe.display().to_string())
            .ok()?;
        v.split(';')
            .find_map(|p| p.trim().strip_prefix("GpuPreference="))
            .and_then(|n| n.parse().ok())
    }

    pub fn set_gpu_preference(exe: &Path, pref: u32) -> crate::Result<()> {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey(GPU_PREFS)
            .map_err(|e| crate::Error::Other(format!("настройки видеокарты Windows: {e}")))?;
        key.set_value(exe.display().to_string(), &format!("GpuPreference={pref};"))
            .map_err(|e| crate::Error::Other(format!("настройки видеокарты Windows: {e}")))
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::Path;

    use super::Gpu;

    pub fn free_ram_mb() -> Option<u64> {
        None
    }
    pub fn cpu_name() -> Option<String> {
        None
    }
    pub fn gpus() -> Vec<Gpu> {
        Vec::new()
    }
    pub fn has_battery() -> bool {
        false
    }
    pub fn power_saver() -> bool {
        false
    }
    pub fn gpu_preference(_: &Path) -> Option<u32> {
        None
    }
    pub fn set_gpu_preference(_: &Path, _: u32) -> crate::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance(loader: Loader, version: &str) -> Instance {
        serde_json::from_value(serde_json::json!({
            "id": "b", "name": "b", "version": version, "loader": loader, "created": 0
        }))
        .unwrap()
    }

    #[test]
    fn gpu_kinds() {
        assert!(is_discrete("NVIDIA GeForce RTX 3050"));
        assert!(is_discrete("NVIDIA GeForce GTX 750 Ti"));
        assert!(is_discrete("AMD Radeon RX 580 Series"));
        assert!(!is_discrete("AMD Radeon(TM) Graphics"));
        assert!(!is_discrete("AMD Radeon(TM) Vega 8 Graphics"));
        assert!(is_discrete("Radeon RX Vega 56"));
        assert!(!is_discrete("ATI Radeon 3000 Graphics"));
        assert!(!is_discrete("AMD Radeon R7 Graphics"));
        assert!(is_discrete("AMD Radeon HD 7700 Series"));
        assert!(!is_discrete("Intel(R) UHD Graphics 620"));
        assert!(is_discrete("Intel(R) Arc(TM) A750 Graphics"));
        assert!(modern_gpu("NVIDIA GeForce RTX 3050"));
        assert!(!modern_gpu("NVIDIA GeForce GTX 750 Ti"));
        assert!(old_cpu("AMD FX(tm)-8300 Eight-Core Processor"));
        assert!(old_cpu("Intel(R) Core(TM) i5-3470 CPU @ 3.20GHz"));
        assert!(!old_cpu("AMD Ryzen 5 5600X 6-Core Processor"));
        assert!(!old_cpu("12th Gen Intel(R) Core(TM) i5-12400F"));
        // ПК автора: FX-8300, 16 ГБ, RTX 3050 — не слабый, но и не мощный.
        let pc = Hardware {
            ram_total_mb: Some(16381),
            ram_free_mb: None,
            cores: 8,
            cpu: Some("AMD FX(tm)-8300 Eight-Core Processor".into()),
            gpus: vec![Gpu {
                name: "NVIDIA GeForce RTX 3050".into(),
                discrete: true,
                driver: None,
                driver_date: None,
            }],
            laptop: false,
            power_saver: false,
        };
        assert!(!pc.weak() && !pc.strong());
        let fx4300 = Hardware {
            cores: 4,
            cpu: Some("AMD FX(tm)-4300 Quad-Core Processor".into()),
            ..pc.clone()
        };
        assert!(fx4300.weak());
    }

    #[test]
    fn russian_numbers() {
        assert_eq!(gb(6144), "6 ГБ");
        assert_eq!(gb(5632), "5,5 ГБ");
        assert_eq!(gb(16381), "16 ГБ");
        assert_eq!(mods_word(1), "1 мод");
        assert_eq!(mods_word(162), "162 мода");
        assert_eq!(mods_word(11), "11 модов");
        assert_eq!(mods_word(45), "45 модов");
        assert_eq!(mods_word(21), "21 мод");
    }

    #[test]
    fn memory_by_mods_and_ram() {
        let fabric = instance(Loader::Fabric, "1.21.1");
        // 160 модов на 16 ГБ: 2 ГБ + 4 ГБ → 6 ГБ.
        assert_eq!(recommended_memory_mb(&fabric, 160, Some(16384)), 6144);
        // Тот же список на 8 ГБ: системе нужен запас — 5,5 ГБ максимум.
        assert_eq!(recommended_memory_mb(&fabric, 160, Some(8192)), 5632);
        // Огромная сборка на 32 ГБ — не больше 8 ГБ.
        assert_eq!(recommended_memory_mb(&fabric, 400, Some(32768)), 8192);
        assert_eq!(
            recommended_memory_mb(&instance(Loader::Vanilla, "1.21.1"), 0, Some(8192)),
            2048
        );
        assert_eq!(
            recommended_memory_mb(&instance(Loader::Vanilla, "1.12.2"), 0, Some(8192)),
            1024
        );
        assert_eq!(recommended_memory_mb(&fabric, 10, Some(4096)), 1536);
    }

    #[test]
    fn reads_this_pc() {
        let hw = hardware();
        assert!(hw.cores >= 1);
        #[cfg(windows)]
        assert!(hw.ram_total_mb.is_some());
        eprintln!("{hw:#?}");
    }
}
