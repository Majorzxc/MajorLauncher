//! Запись SRV `_minecraft._tcp.<адрес>`: многие серверы дают адрес без порта,
//! а настоящие машина и порт — в DNS. Так ищет и игра. Спрашиваем DNS
//! Windows — своего резолвера не тащим.

use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(3);

/// Куда подключаться на самом деле: машина и порт. Нет записи — `None`.
pub async fn srv(host: &str) -> Option<(String, u16)> {
    let name = format!("_minecraft._tcp.{host}");
    // Свой поток, а не spawn_blocking: зависший запрос не держит выход из
    // программы (tokio ждёт свои потоки), ответ просто никто не заберёт.
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(query(&name));
    });
    tokio::time::timeout(TIMEOUT, rx).await.ok()?.ok()?
}

#[cfg(windows)]
fn query(name: &str) -> Option<(String, u16)> {
    use windows_sys::Win32::NetworkManagement::Dns::{
        DNS_QUERY_STANDARD, DNS_RECORDA, DNS_RECORDW, DNS_TYPE_SRV, DnsFree, DnsFreeRecordList,
        DnsQuery_W,
    };

    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let mut results: *mut DNS_RECORDA = std::ptr::null_mut();
    // SAFETY: имя — строка с нулём на конце; ответ освобождаем DnsFree ниже.
    let status = unsafe {
        DnsQuery_W(
            wide.as_ptr(),
            DNS_TYPE_SRV,
            DNS_QUERY_STANDARD,
            std::ptr::null_mut(),
            &mut results,
            std::ptr::null_mut(),
        )
    };
    if status != 0 || results.is_null() {
        return None;
    }
    // Записей может быть несколько — берём с наименьшим приоритетом, как игра.
    let mut best: Option<(u16, String, u16)> = None;
    // DnsQuery_W отдаёт записи в «широком» виде, хоть тип в объявлении и A.
    let mut at = results as *const DNS_RECORDW;
    while !at.is_null() {
        // SAFETY: список из DnsQuery_W, жив до DnsFree.
        let record = unsafe { &*at };
        if record.wType == DNS_TYPE_SRV {
            // SAFETY: у записи SRV в объединении — данные SRV.
            let data = unsafe { record.Data.SRV };
            if !data.pNameTarget.is_null() && best.as_ref().is_none_or(|b| data.wPriority < b.0) {
                // SAFETY: строка Windows с нулём на конце.
                let target = unsafe { wide_str(data.pNameTarget) };
                best = Some((data.wPriority, target, data.wPort));
            }
        }
        at = record.pNext;
    }
    // SAFETY: тот самый список из DnsQuery_W.
    unsafe { DnsFree(results as *const _, DnsFreeRecordList) };
    let (_, target, port) = best?;
    let target = target.trim_end_matches('.').to_string();
    (!target.is_empty() && port != 0).then_some((target, port))
}

#[cfg(windows)]
unsafe fn wide_str(p: *const u16) -> String {
    let mut n = 0;
    // SAFETY: вызывающий обещает строку с нулём на конце.
    while unsafe { *p.add(n) } != 0 {
        n += 1;
    }
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, n) })
}

#[cfg(not(windows))]
fn query(_name: &str) -> Option<(String, u16)> {
    None
}
