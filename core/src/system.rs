//! Сведения от Windows: объём памяти и заголовок окна игры.

/// Объём оперативной памяти, МБ. `None` — Windows не ответила.
pub fn total_ram_mb() -> Option<u64> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
        // SAFETY: структура обнулена и её размер записан, как требует функция.
        unsafe {
            let mut status: MEMORYSTATUSEX = std::mem::zeroed();
            status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
            if GlobalMemoryStatusEx(&mut status) != 0 {
                return Some(status.ullTotalPhys / 1_048_576);
            }
        }
    }
    None
}

/// Память для игры в режиме «авто»: четверть ОЗУ, от 1 до 4 ГБ, с округлением
/// до 512 МБ (Windows показывает чуть меньше 8 ГБ — часть забирает железо).
/// На 8 ГБ это 2 ГБ: ванильной игре и лёгким сборкам хватает, а системе и
/// браузеру остаётся. Подбор по числу модов — кнопка «Оптимизировать» (этап 10).
pub fn auto_memory_mb(total_mb: Option<u64>) -> u32 {
    let quarter = total_mb.unwrap_or(8192) / 4;
    ((quarter + 256) / 512 * 512).clamp(1024, 4096) as u32
}

/// Заголовок видимого окна процесса `pid`; `None` — окна ещё (или уже) нет.
pub fn window_title(pid: u32) -> Option<String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{HWND, LPARAM};
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
            IsWindowVisible,
        };

        struct Search {
            pid: u32,
            title: Option<String>,
        }

        unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> windows_sys::core::BOOL {
            // SAFETY: lparam — указатель на Search из вызова EnumWindows ниже,
            // он жив всё время перебора.
            unsafe {
                let search = &mut *(lparam as *mut Search);
                let mut owner = 0u32;
                GetWindowThreadProcessId(hwnd, &mut owner);
                if owner != search.pid || IsWindowVisible(hwnd) == 0 {
                    return 1;
                }
                let len = GetWindowTextLengthW(hwnd);
                if len <= 0 {
                    return 1;
                }
                let mut buf = vec![0u16; len as usize + 1];
                let got = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
                search.title = Some(String::from_utf16_lossy(&buf[..got.max(0) as usize]));
                0
            }
        }

        let mut search = Search { pid, title: None };
        // SAFETY: колбэк получает указатель на живую `search` и не хранит его.
        unsafe {
            EnumWindows(Some(visit), &mut search as *mut Search as LPARAM);
        }
        search.title
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_memory_is_quarter_within_limits() {
        assert_eq!(auto_memory_mb(Some(8192)), 2048);
        assert_eq!(auto_memory_mb(Some(8100)), 2048);
        assert_eq!(auto_memory_mb(Some(6144)), 1536);
        assert_eq!(auto_memory_mb(Some(4096)), 1024);
        assert_eq!(auto_memory_mb(Some(65536)), 4096);
        assert_eq!(auto_memory_mb(None), 2048);
    }

    #[test]
    fn knows_ram_size() {
        assert!(total_ram_mb().is_some_and(|mb| mb > 512));
    }
}
