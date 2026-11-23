//! Credentials: password hashing, opaque bearer tokens, TOTP and login throttling.

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use data_encoding::{BASE32_NOPAD, BASE64URL_NOPAD};
use hmac::{Hmac, Mac};
use rand::RngCore;
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};

pub const TOTP_STEP_SECONDS: u64 = 30;
pub const TOTP_DIGITS: u32 = 6;

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| anyhow::anyhow!("hash password: {e}"))
}

pub fn verify_password(password: &str, stored: &str) -> bool {
    PasswordHash::new(stored).is_ok_and(|h| {
        Argon2::default()
            .verify_password(password.as_bytes(), &h)
            .is_ok()
    })
}

/// A hash that costs as much as a real verification, so unknown usernames take as long
/// as wrong passwords.
pub fn dummy_verify(password: &str) {
    static DUMMY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let stored = DUMMY.get_or_init(|| hash_password("vidcue-dummy").unwrap_or_default());
    let _ = verify_password(password, stored);
}

pub fn password_problem(password: &str) -> Option<&'static str> {
    let chars = password.chars().count();
    if chars < 10 {
        Some("Password must have at least 10 characters")
    } else if chars > 256 {
        Some("Password must have at most 256 characters")
    } else {
        None
    }
}

pub fn normalize_username(username: &str) -> Result<String, &'static str> {
    let name = username.trim().to_lowercase();
    if !(3..=32).contains(&name.len()) {
        return Err("Username must have 3 to 32 characters");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err("Username may only use letters, digits, '.', '_' and '-'");
    }
    Ok(name)
}

/// Random opaque secret, URL-safe. The server only ever stores its SHA-256.
pub fn new_token(prefix: &str) -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("{prefix}{}", BASE64URL_NOPAD.encode(&bytes))
}

/// Short single-use code meant to be typed or scanned (pairing): 10 base32 characters, 50 bits.
pub fn new_short_code() -> String {
    let mut bytes = [0u8; 7];
    OsRng.fill_bytes(&mut bytes);
    BASE32_NOPAD.encode(&bytes)[..10].to_owned()
}

pub fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.trim().as_bytes()).to_vec()
}

pub fn new_totp_secret() -> Vec<u8> {
    let mut bytes = vec![0u8; 20];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

pub fn totp_uri(issuer: &str, username: &str, secret: &[u8]) -> String {
    let enc = |s: &str| {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    (b as char).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect::<String>()
    };
    format!(
        "otpauth://totp/{}:{}?secret={}&issuer={}&algorithm=SHA1&digits={TOTP_DIGITS}&period={TOTP_STEP_SECONDS}",
        enc(issuer),
        enc(username),
        BASE32_NOPAD.encode(secret),
        enc(issuer)
    )
}

/// RFC 6238 TOTP (HMAC-SHA1) for one time step.
pub fn totp_at_step(secret: &[u8], step: u64) -> u32 {
    let mut mac = Hmac::<sha1::Sha1>::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(&step.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let bin = u32::from_be_bytes([
        digest[offset] & 0x7f,
        digest[offset + 1],
        digest[offset + 2],
        digest[offset + 3],
    ]);
    bin % 10u32.pow(TOTP_DIGITS)
}

/// Returns the matched time step when `code` is valid within ±1 step of `unix_seconds`
/// and newer than `last_step` (replay protection).
pub fn verify_totp(secret: &[u8], code: &str, unix_seconds: u64, last_step: u64) -> Option<u64> {
    let code = code.trim().replace(' ', "");
    if code.len() != TOTP_DIGITS as usize || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let wanted: u32 = code.parse().ok()?;
    let now = unix_seconds / TOTP_STEP_SECONDS;
    [now.saturating_sub(1), now, now + 1]
        .into_iter()
        .find(|step| *step > last_step && totp_at_step(secret, *step) == wanted)
}

/// Recovery codes are 10 base32 characters shown as `XXXXX-XXXXX`; authenticator codes
/// are 6 digits, so the two never overlap.
pub fn new_recovery_code() -> String {
    let code = new_short_code();
    format!("{}-{}", &code[..5], &code[5..])
}

pub fn normalize_recovery_code(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

pub fn is_recovery_code(code: &str) -> bool {
    normalize_recovery_code(code).len() == 10
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc6238_sha1_vectors() {
        // RFC 6238 Appendix B, SHA1 secret "12345678901234567890", 8-digit codes truncated to 6.
        let secret = b"12345678901234567890";
        for (time, code8) in [
            (59u64, 94287082u32),
            (1111111109, 7081804),
            (1111111111, 14050471),
            (1234567890, 89005924),
            (2000000000, 69279037),
        ] {
            assert_eq!(
                totp_at_step(secret, time / 30),
                code8 % 1_000_000,
                "t={time}"
            );
        }
    }

    #[test]
    fn totp_accepts_neighbour_steps_and_rejects_replays() {
        let secret = new_totp_secret();
        let now = 1_800_000_000u64;
        let step = now / 30;
        let code = format!("{:06}", totp_at_step(&secret, step));
        assert_eq!(verify_totp(&secret, &code, now, 0), Some(step));
        assert_eq!(verify_totp(&secret, &code, now + 30, 0), Some(step));
        assert_eq!(verify_totp(&secret, &code, now, step), None, "replay");
        assert_eq!(verify_totp(&secret, &code, now + 90, 0), None, "too old");
        assert_eq!(verify_totp(&secret, "12345", now, 0), None);
        assert_eq!(verify_totp(&secret, "abcdef", now, 0), None);
    }

    #[test]
    fn passwords_and_tokens() {
        let hash = hash_password("correct horse battery").unwrap();
        assert!(verify_password("correct horse battery", &hash));
        assert!(!verify_password("wrong", &hash));
        assert!(!verify_password("x", "not-a-hash"));
        let a = new_token("vcd_");
        assert!(a.starts_with("vcd_") && a.len() > 40);
        assert_ne!(a, new_token("vcd_"));
        assert_eq!(token_hash(&a), token_hash(&format!(" {a}\n")));
        assert_eq!(new_short_code().len(), 10);
        assert_eq!(
            password_problem("short"),
            Some("Password must have at least 10 characters")
        );
        assert_eq!(password_problem("long enough!"), None);
    }

    #[test]
    fn usernames() {
        assert_eq!(normalize_username(" Vanir.X ").unwrap(), "vanir.x");
        assert!(normalize_username("ab").is_err());
        assert!(normalize_username("bad name").is_err());
        assert!(normalize_username("ümlaut").is_err());
    }

    #[test]
    fn otpauth_uri_is_escaped() {
        let uri = totp_uri("Vidcue", "a b", b"12345678901234567890");
        assert!(uri.starts_with(
            "otpauth://totp/Vidcue:a%20b?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&issuer=Vidcue"
        ));
    }

    #[test]
    fn recovery_codes_are_distinct_from_totp() {
        let code = new_recovery_code();
        assert_eq!(code.len(), 11);
        assert!(is_recovery_code(&code));
        assert!(is_recovery_code(&code.to_lowercase().replace('-', " ")));
        assert_eq!(
            normalize_recovery_code(&code.to_lowercase()),
            code.replace('-', "")
        );
        assert!(!is_recovery_code("123456"));
    }
}
