//! Подпись Ed25519 того, что лаунчер берёт с GitHub: правил анализатора
//! ([`crate::analyzer::remote`]) и выпусков самого лаунчера
//! ([`crate::selfupdate`]). Закрытые ключи — у автора и в секретах GitHub,
//! в лаунчер вшиты только открытые.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ring::signature::{ED25519, Ed25519KeyPair, KeyPair, UnparsedPublicKey};

use crate::error::{Error, Result};

/// Подпись (base64) верна для этих байт и открытого ключа (base64).
pub fn verify(bytes: &[u8], sig_b64: &str, public_key_b64: &str) -> bool {
    let (Ok(sig), Ok(key)) = (
        STANDARD.decode(sig_b64.trim()),
        STANDARD.decode(public_key_b64.trim()),
    ) else {
        return false;
    };
    UnparsedPublicKey::new(&ED25519, key)
        .verify(bytes, &sig)
        .is_ok()
}

/// Подписать закрытым ключом (PKCS#8) — для `mlcli`. Подпись в base64.
pub fn sign(bytes: &[u8], pkcs8: &[u8]) -> Result<String> {
    let pair = Ed25519KeyPair::from_pkcs8(pkcs8)
        .map_err(|_| Error::Other("это не ключ подписи Ed25519 (PKCS#8)".into()))?;
    Ok(STANDARD.encode(pair.sign(bytes).as_ref()))
}

/// Новый ключ: закрытый (PKCS#8) и открытый (base64).
pub fn keygen() -> Result<(Vec<u8>, String)> {
    let rng = ring::rand::SystemRandom::new();
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng)
        .map_err(|_| Error::Other("не удалось создать ключ".into()))?;
    let pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref())
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
    fn signatures_are_checked() {
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
        assert!(sign(json, b"not a key").is_err());
    }
}
