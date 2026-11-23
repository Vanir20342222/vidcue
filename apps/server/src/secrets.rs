//! Server-held key that protects secrets at rest (authenticator seeds, recovery codes).
//!
//! The key lives outside the database (a root-only file passed in as a systemd
//! credential), so a database dump or backup alone does not reveal authenticator seeds
//! and cannot be used to brute-force recovery codes offline.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{Context, bail, ensure};
use data_encoding::HEXLOWER;
use hmac::{Hmac, Mac};
use rand::RngCore;
use rand::rngs::OsRng;
use sha2::Sha256;
use uuid::Uuid;

/// Prefix of sealed values. Plain 20-byte TOTP seeds from before B2 hardening lack it.
const SEALED_V1: &[u8; 2] = b"v1";

pub struct ServerKey {
    enc: [u8; 32],
    mac: [u8; 32],
}

fn derive(master: &[u8; 32], label: &[u8]) -> [u8; 32] {
    let mut mac =
        <Hmac<Sha256> as Mac>::new_from_slice(master).expect("HMAC accepts any key length");
    mac.update(label);
    mac.finalize().into_bytes().into()
}

impl ServerKey {
    pub fn from_master(master: [u8; 32]) -> Self {
        Self {
            enc: derive(&master, b"vidcue seal v1"),
            mac: derive(&master, b"vidcue recovery v1"),
        }
    }

    /// Reads 64 hex characters. Prefers the systemd credential directory.
    pub fn load(path: Option<&str>) -> anyhow::Result<Self> {
        let path = match std::env::var("CREDENTIALS_DIRECTORY") {
            Ok(dir) => format!("{dir}/secret.key"),
            Err(_) => path
                .context("set VIDCUE_SECRET_KEY_FILE (64 hex characters, root-only)")?
                .to_owned(),
        };
        let text = std::fs::read_to_string(&path).with_context(|| format!("read {path}"))?;
        let bytes = HEXLOWER
            .decode(text.trim().to_ascii_lowercase().as_bytes())
            .context("secret key must be hex")?;
        let master: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("secret key must be 32 bytes (64 hex characters)"))?;
        ensure!(master != [0; 32], "secret key must not be all zero");
        Ok(Self::from_master(master))
    }

    pub fn generate_hex() -> String {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        HEXLOWER.encode(&bytes)
    }

    /// Encrypts `plain` bound to `user`, so a sealed seed cannot be moved to another row.
    pub fn seal(&self, user: Uuid, plain: &[u8]) -> Vec<u8> {
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let cipher = Aes256Gcm::new_from_slice(&self.enc).expect("32-byte key");
        let sealed = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plain,
                    aad: user.as_bytes(),
                },
            )
            .expect("AES-GCM encryption does not fail for small inputs");
        [SEALED_V1.as_slice(), &nonce, &sealed].concat()
    }

    pub fn open(&self, user: Uuid, blob: &[u8]) -> anyhow::Result<Vec<u8>> {
        if !Self::is_sealed(blob) {
            bail!("value is not sealed");
        }
        let (nonce, sealed) = blob[2..].split_at(12);
        let cipher = Aes256Gcm::new_from_slice(&self.enc).expect("32-byte key");
        cipher
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: sealed,
                    aad: user.as_bytes(),
                },
            )
            .map_err(|_| anyhow::anyhow!("sealed value does not open with this server key"))
    }

    pub fn is_sealed(blob: &[u8]) -> bool {
        blob.len() > 2 + 12 + 16 && blob.starts_with(SEALED_V1)
    }

    /// Keyed hash of a normalized recovery code, bound to its user.
    pub fn recovery_hash(&self, user: Uuid, normalized_code: &str) -> Vec<u8> {
        let mut mac =
            <Hmac<Sha256> as Mac>::new_from_slice(&self.mac).expect("HMAC accepts any key length");
        mac.update(user.as_bytes());
        mac.update(normalized_code.as_bytes());
        mac.finalize().into_bytes().to_vec()
    }
}

/// Seals authenticator seeds still stored in plain form (rows written before this change).
pub async fn seal_existing(db: &deadpool_postgres::Pool, key: &ServerKey) -> anyhow::Result<u64> {
    let mut client = db.get().await?;
    let tx = client.transaction().await?;
    let mut sealed = 0;
    for row in tx
        .query(
            "SELECT id, totp_secret, totp_pending FROM users FOR UPDATE",
            &[],
        )
        .await?
    {
        let user: Uuid = row.get(0);
        let secret: Vec<u8> = row.get(1);
        let pending: Option<Vec<u8>> = row.get(2);
        let seal = |v: Vec<u8>| {
            if ServerKey::is_sealed(&v) {
                v
            } else {
                key.seal(user, &v)
            }
        };
        let new_secret = seal(secret.clone());
        let new_pending = pending.clone().map(seal);
        if new_secret != secret || new_pending != pending {
            tx.execute(
                "UPDATE users SET totp_secret = $2, totp_pending = $3 WHERE id = $1",
                &[&user, &new_secret, &new_pending],
            )
            .await?;
            sealed += 1;
        }
    }
    for row in tx
        .query(
            "SELECT token_hash, user_id, totp_secret FROM account_resets WHERE totp_secret IS NOT NULL FOR UPDATE",
            &[],
        )
        .await?
    {
        let secret: Vec<u8> = row.get(2);
        if !ServerKey::is_sealed(&secret) {
            let user: Uuid = row.get(1);
            tx.execute(
                "UPDATE account_resets SET totp_secret = $2 WHERE token_hash = $1",
                &[&row.get::<_, Vec<u8>>(0), &key.seal(user, &secret)],
            )
            .await?;
            sealed += 1;
        }
    }
    tx.commit().await?;
    Ok(sealed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_round_trip_is_bound_to_the_user() {
        let key = ServerKey::from_master([3; 32]);
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        let sealed = key.seal(a, b"12345678901234567890");
        assert!(ServerKey::is_sealed(&sealed));
        assert!(!ServerKey::is_sealed(b"12345678901234567890"));
        assert_eq!(key.open(a, &sealed).unwrap(), b"12345678901234567890");
        assert!(key.open(b, &sealed).is_err(), "moved to another user");
        assert!(
            ServerKey::from_master([4; 32]).open(a, &sealed).is_err(),
            "other key"
        );
        assert_ne!(sealed, key.seal(a, b"12345678901234567890"), "random nonce");
    }

    #[test]
    fn recovery_hashes_depend_on_key_and_user() {
        let key = ServerKey::from_master([3; 32]);
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        assert_eq!(
            key.recovery_hash(a, "ABCDE12345"),
            key.recovery_hash(a, "ABCDE12345")
        );
        assert_ne!(
            key.recovery_hash(a, "ABCDE12345"),
            key.recovery_hash(b, "ABCDE12345")
        );
        assert_ne!(
            key.recovery_hash(a, "ABCDE12345"),
            ServerKey::from_master([4; 32]).recovery_hash(a, "ABCDE12345")
        );
    }

    #[test]
    fn generated_keys_load() {
        let dir = std::env::temp_dir().join(format!("vidcue-key-{}", Uuid::new_v4()));
        std::fs::write(&dir, ServerKey::generate_hex()).unwrap();
        assert!(ServerKey::load(dir.to_str()).is_ok());
        std::fs::write(&dir, "00".repeat(32)).unwrap();
        assert!(ServerKey::load(dir.to_str()).is_err());
        std::fs::write(&dir, "abc").unwrap();
        assert!(ServerKey::load(dir.to_str()).is_err());
        let _ = std::fs::remove_file(dir);
    }
}
