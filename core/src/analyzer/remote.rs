//! База правил анализатора на GitHub (раздел 5.8 ТЗ): правила обновляются без
//! выпуска лаунчера. Файл подписан ключом автора (Ed25519); лаунчер берёт его,
//! только если подпись сходится и версия новее встроенной. Иначе — встроенные
//! правила: испорченный или подменённый файл ничего не ломает.
//!
//! Подписать: `mlcli rules sign --key <ключ> rules/analyzer-rules.json` —
//! рядом появится `analyzer-rules.json.sig` (подпись в base64).

use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ring::signature::{ED25519, UnparsedPublicKey};

use super::Rules;
use crate::download::Downloader;
use crate::error::{Error, IoContext, Result};
use crate::paths::DataDir;

pub const RULES_URL: &str =
    "https://raw.githubusercontent.com/Majorzxc/MajorLauncher/main/rules/analyzer-rules.json";

/// Открытый ключ подписи правил (Ed25519, 32 байта, base64).
pub const PUBLIC_KEY: &str = "soCI25HeETJi9fQqu+VwOzVze9TsdQ4ppot4Onffyxc=";

fn cache_file(data: &DataDir) -> std::path::PathBuf {
    data.cache().join("analyzer-rules.json")
}

/// Подпись верна для этих байт.
pub fn verify(json: &[u8], sig_b64: &str, public_key_b64: &str) -> bool {
    let (Ok(sig), Ok(key)) = (
        STANDARD.decode(sig_b64.trim()),
        STANDARD.decode(public_key_b64.trim()),
    ) else {
        return false;
    };
    UnparsedPublicKey::new(&ED25519, key)
        .verify(json, &sig)
        .is_ok()
}

/// Правила, которыми разбирать: свежие с GitHub (если подпись верна и
/// версия новее встроенных) или встроенные.
pub fn current(data: &DataDir) -> Arc<Rules> {
    let builtin = Rules::builtin();
    let loaded = (|| {
        let json = std::fs::read(cache_file(data)).ok()?;
        let sig = std::fs::read_to_string(cache_file(data).with_extension("json.sig")).ok()?;
        if !verify(&json, &sig, PUBLIC_KEY) {
            return None;
        }
        Rules::parse(std::str::from_utf8(&json).ok()?).ok()
    })();
    match loaded {
        Some(r) if r.version > builtin.version => Arc::new(r),
        _ => builtin,
    }
}

/// Какие правила сейчас: версия и взяты ли с GitHub.
pub fn info(data: &DataDir) -> (u32, bool) {
    let r = current(data);
    (r.version, r.version > Rules::builtin().version)
}

/// Скачать правила с GitHub. Возвращает версию, если появились новее
/// текущих; `None` — обновлять нечего.
pub async fn refresh(dl: &Downloader, data: &DataDir) -> Result<Option<u32>> {
    let json = dl.get_bytes(RULES_URL).await?;
    let sig = dl.get_bytes(&format!("{RULES_URL}.sig")).await?;
    let sig = String::from_utf8_lossy(&sig).to_string();
    if !verify(&json, &sig, PUBLIC_KEY) {
        return Err(Error::Other(
            "правила с GitHub не прошли проверку подписи — остаются прежние".into(),
        ));
    }
    let text = std::str::from_utf8(&json)
        .map_err(|_| Error::Other("правила с GitHub — не текст".into()))?;
    let rules = Rules::parse(text)?;
    if rules.version <= current(data).version {
        return Ok(None);
    }
    let path = cache_file(data);
    std::fs::create_dir_all(data.cache()).at(&data.cache())?;
    let sig_path = path.with_extension("json.sig");
    std::fs::write(&sig_path, sig.trim()).at(&sig_path)?;
    std::fs::write(&path, &json).at(&path)?;
    Ok(Some(rules.version))
}

/// Подписать файл правил закрытым ключом (PKCS#8) — для `mlcli`.
pub fn sign(json: &[u8], pkcs8: &[u8]) -> Result<String> {
    let pair = ring::signature::Ed25519KeyPair::from_pkcs8(pkcs8)
        .map_err(|_| Error::Other("это не ключ подписи Ed25519 (PKCS#8)".into()))?;
    Ok(STANDARD.encode(pair.sign(json).as_ref()))
}

/// Новый ключ: закрытый (PKCS#8) и открытый (base64).
pub fn keygen() -> Result<(Vec<u8>, String)> {
    use ring::signature::KeyPair;
    let rng = ring::rand::SystemRandom::new();
    let pkcs8 = ring::signature::Ed25519KeyPair::generate_pkcs8(&rng)
        .map_err(|_| Error::Other("не удалось создать ключ".into()))?;
    let pair = ring::signature::Ed25519KeyPair::from_pkcs8(pkcs8.as_ref())
        .map_err(|_| Error::Other("не удалось прочитать новый ключ".into()))?;
    Ok((
        pkcs8.as_ref().to_vec(),
        STANDARD.encode(pair.public_key().as_ref()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_rules_are_checked() {
        let (key, public) = keygen().unwrap();
        let json = br#"{"version":99,"rules":[]}"#;
        let sig = sign(json, &key).unwrap();
        assert!(verify(json, &sig, &public));
        // Изменили хоть байт — подпись не сходится.
        assert!(!verify(br#"{"version":100,"rules":[]}"#, &sig, &public));
        // Чужой ключ — тоже.
        let (_, other) = keygen().unwrap();
        assert!(!verify(json, &sig, &other));
        assert!(!verify(json, "не base64", &public));
    }

    /// Опубликованный в репозитории файл подписан тем же ключом, что вшит в
    /// лаунчер, и разбирается.
    #[test]
    fn published_rules_are_signed() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../rules");
        let json = std::fs::read(dir.join("analyzer-rules.json")).unwrap();
        let sig = std::fs::read_to_string(dir.join("analyzer-rules.json.sig")).unwrap();
        assert!(
            verify(&json, &sig, PUBLIC_KEY),
            "подпись не сходится с вшитым ключом"
        );
        let rules = Rules::parse(std::str::from_utf8(&json).unwrap()).unwrap();
        assert!(rules.version >= Rules::builtin().version);
    }

    #[test]
    fn bad_cache_falls_back_to_builtin() {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path());
        std::fs::create_dir_all(data.cache()).unwrap();
        std::fs::write(cache_file(&data), br#"{"version":999,"rules":[]}"#).unwrap();
        std::fs::write(cache_file(&data).with_extension("json.sig"), "AAAA").unwrap();
        assert_eq!(
            current(&data).version,
            Rules::builtin().version,
            "без верной подписи — встроенные"
        );
    }
}
