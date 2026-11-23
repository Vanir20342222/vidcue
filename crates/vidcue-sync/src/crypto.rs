//! End-to-end account key (PROTOCOL.md §7.6): the secrets envelope, the key-relay sealed
//! box and the fingerprint users compare. The server never sees the key or plaintext.
use anyhow::{Context, Result, bail, ensure};
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, Payload},
};
use data_encoding::{BASE64, BASE64URL_NOPAD};
use rand::RngCore;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

/// The 32-byte account key `K`.
pub type AccountKey = [u8; 32];

/// Register key of the key check in the `secrets` collection.
pub const KEYCHECK: &str = "keycheck";
const KEYCHECK_PLAINTEXT: &[u8] = b"vidcue-keycheck-v1";
/// Encoded envelopes must stay below the server's 64 KiB value limit.
pub const MAX_SECRET_PLAINTEXT: usize = 46_000;
const RELAY_INFO: &[u8] = b"vidcue key relay v1";
const RELAY_VERSION: u8 = 0x01;
/// `0x01 || E (32) || nonce (24) || ciphertext + tag (48)`.
pub const SEALED_LEN: usize = 1 + 32 + 24 + 48;

fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes
}

/// A fresh account key from the OS random generator.
pub fn new_account_key() -> AccountKey {
    random()
}

/// Standard base64 with padding, as stored under the `e2e_key` secret name.
pub fn encode_key(key: &AccountKey) -> String {
    BASE64.encode(key)
}

pub fn decode_key(text: &str) -> Result<AccountKey> {
    BASE64
        .decode(text.trim().as_bytes())
        .ok()
        .and_then(|bytes| AccountKey::try_from(bytes.as_slice()).ok())
        .context("The stored account key is invalid")
}

fn aad(register: &str) -> Vec<u8> {
    format!("vidcue/secrets/v1/{register}").into_bytes()
}

/// Encrypts `plaintext` for the `secrets` register `register` (§7.6.2).
pub fn seal_secret(key: &AccountKey, register: &str, plaintext: &[u8]) -> Result<Value> {
    ensure!(
        plaintext.len() <= MAX_SECRET_PLAINTEXT,
        "session too large to sync"
    );
    let nonce: [u8; 24] = random();
    let ciphertext = XChaCha20Poly1305::new(key.into())
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: &aad(register),
            },
        )
        .map_err(|_| anyhow::anyhow!("Encryption failed"))?;
    Ok(json!({
        "v": 1,
        "alg": "xchacha20poly1305",
        "nonce": BASE64.encode(&nonce),
        "ciphertext": BASE64.encode(&ciphertext),
    }))
}

/// Decrypts an envelope; fails for another key, another register or any tampering.
pub fn open_secret(key: &AccountKey, register: &str, envelope: &Value) -> Result<Vec<u8>> {
    ensure!(
        envelope["v"] == 1 && envelope["alg"] == "xchacha20poly1305",
        "Unsupported secret envelope"
    );
    let field = |name: &str| {
        envelope[name]
            .as_str()
            .and_then(|s| BASE64.decode(s.as_bytes()).ok())
            .with_context(|| format!("Secret envelope has no valid {name}"))
    };
    let nonce = field("nonce")?;
    ensure!(nonce.len() == 24, "Secret envelope has no valid nonce");
    let ciphertext = field("ciphertext")?;
    XChaCha20Poly1305::new(key.into())
        .decrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &ciphertext,
                aad: &aad(register),
            },
        )
        .map_err(|_| anyhow::anyhow!("The secret does not open with this account key"))
}

/// The `secrets/keycheck` register value for `key`.
pub fn keycheck(key: &AccountKey) -> Result<Value> {
    seal_secret(key, KEYCHECK, KEYCHECK_PLAINTEXT)
}

/// Whether `envelope` is the key check of `key`.
pub fn keycheck_opens(key: &AccountKey, envelope: &Value) -> bool {
    open_secret(key, KEYCHECK, envelope).is_ok_and(|plain| plain == KEYCHECK_PLAINTEXT)
}

fn relay_key(shared: &[u8; 32], salt: &[u8]) -> Result<[u8; 32]> {
    let mut okm = [0u8; 32];
    hkdf::Hkdf::<Sha256>::new(Some(salt), shared)
        .expand(RELAY_INFO, &mut okm)
        .map_err(|_| anyhow::anyhow!("Key derivation failed"))?;
    Ok(okm)
}

/// The requesting side of the key relay (§7.6.3). The secret stays in memory only.
pub struct RelayRequest {
    secret: StaticSecret,
    public: [u8; 32],
}

impl Default for RelayRequest {
    fn default() -> Self {
        Self::new()
    }
}

impl RelayRequest {
    pub fn new() -> Self {
        Self::from_secret(random())
    }
    /// Deterministic construction for test vectors.
    pub fn from_secret(bytes: [u8; 32]) -> Self {
        let secret = StaticSecret::from(bytes);
        let public = PublicKey::from(&secret).to_bytes();
        Self { secret, public }
    }
    pub fn public(&self) -> [u8; 32] {
        self.public
    }
    /// Standard base64 of the public key, as posted to `/api/keys/requests`.
    pub fn public_b64(&self) -> String {
        BASE64.encode(&self.public)
    }
    pub fn fingerprint(&self) -> String {
        fingerprint(&self.public)
    }
    /// Opens a granted sealed key. The caller must still check it against `keycheck`.
    pub fn open(&self, sealed_b64: &str) -> Result<AccountKey> {
        let sealed = BASE64
            .decode(sealed_b64.trim().as_bytes())
            .ok()
            .context("key did not match")?;
        ensure!(
            sealed.len() == SEALED_LEN && sealed[0] == RELAY_VERSION,
            "key did not match"
        );
        let ephemeral: [u8; 32] = sealed[1..33].try_into().expect("32 bytes");
        let nonce = &sealed[33..57];
        let ciphertext = &sealed[57..];
        let shared = self.secret.diffie_hellman(&PublicKey::from(ephemeral));
        ensure!(shared.was_contributory(), "key did not match");
        let salt = [ephemeral.as_slice(), self.public.as_slice()].concat();
        let okm = relay_key(shared.as_bytes(), &salt)?;
        let key = XChaCha20Poly1305::new((&okm).into())
            .decrypt(
                XNonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad: &salt,
                },
            )
            .map_err(|_| anyhow::anyhow!("key did not match"))?;
        AccountKey::try_from(key.as_slice())
            .ok()
            .context("key did not match")
    }
}

/// Seals the account key to a requester's public key (granting side of §7.6.3).
pub fn seal_for(requester_b64: &str, key: &AccountKey) -> Result<String> {
    seal_for_with(requester_b64, key, random(), random())
}

fn seal_for_with(
    requester_b64: &str,
    key: &AccountKey,
    ephemeral: [u8; 32],
    nonce: [u8; 24],
) -> Result<String> {
    let requester = decode_public(requester_b64)?;
    let secret = StaticSecret::from(ephemeral);
    let public = PublicKey::from(&secret).to_bytes();
    let shared = secret.diffie_hellman(&PublicKey::from(requester));
    if !shared.was_contributory() {
        bail!("The requesting device sent an invalid key");
    }
    let salt = [public.as_slice(), requester.as_slice()].concat();
    let okm = relay_key(shared.as_bytes(), &salt)?;
    let ciphertext = XChaCha20Poly1305::new((&okm).into())
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: key,
                aad: &salt,
            },
        )
        .map_err(|_| anyhow::anyhow!("Encryption failed"))?;
    let mut sealed = Vec::with_capacity(SEALED_LEN);
    sealed.push(RELAY_VERSION);
    sealed.extend_from_slice(&public);
    sealed.extend_from_slice(&nonce);
    sealed.extend_from_slice(&ciphertext);
    debug_assert_eq!(sealed.len(), SEALED_LEN);
    Ok(BASE64.encode(&sealed))
}

/// Decodes a standard-base64 X25519 public key.
pub fn decode_public(text: &str) -> Result<[u8; 32]> {
    BASE64
        .decode(text.trim().as_bytes())
        .ok()
        .and_then(|bytes| <[u8; 32]>::try_from(bytes.as_slice()).ok())
        .context("The device key is invalid")
}

/// Twelve digits in four groups of three (§7.6.5).
pub fn fingerprint(public_key: &[u8; 32]) -> String {
    let digest = Sha256::new()
        .chain_update(b"vidcue-fingerprint-v1")
        .chain_update(public_key)
        .finalize();
    let n = u64::from_be_bytes(digest[..8].try_into().expect("8 bytes")) % 1_000_000_000_000;
    let digits = format!("{n:012}");
    format!(
        "{} {} {} {}",
        &digits[0..3],
        &digits[3..6],
        &digits[6..9],
        &digits[9..12]
    )
}

/// `base64url` without padding, as carried in the pairing URI fragment (`#k=`).
pub fn encode_key_url(key: &AccountKey) -> String {
    BASE64URL_NOPAD.encode(key)
}

pub fn decode_key_url(text: &str) -> Result<AccountKey> {
    BASE64URL_NOPAD
        .decode(text.as_bytes())
        .ok()
        .and_then(|bytes| AccountKey::try_from(bytes.as_slice()).ok())
        .context("The pairing key is invalid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_round_trips_and_binds_key_and_register() {
        let key = [7u8; 32];
        let envelope = seal_secret(&key, "youtube", b"{\"cookies\":[]}").unwrap();
        assert_eq!(envelope["v"], 1);
        assert_eq!(envelope["alg"], "xchacha20poly1305");
        assert_eq!(
            BASE64
                .decode(envelope["nonce"].as_str().unwrap().as_bytes())
                .unwrap()
                .len(),
            24
        );
        assert_eq!(
            open_secret(&key, "youtube", &envelope).unwrap(),
            b"{\"cookies\":[]}"
        );
        // AAD binding: the same envelope under another register name fails.
        assert!(open_secret(&key, "other", &envelope).is_err());
        assert!(open_secret(&[8u8; 32], "youtube", &envelope).is_err());
        let mut tampered = envelope.clone();
        let mut ct = BASE64
            .decode(envelope["ciphertext"].as_str().unwrap().as_bytes())
            .unwrap();
        ct[0] ^= 1;
        tampered["ciphertext"] = BASE64.encode(&ct).into();
        assert!(open_secret(&key, "youtube", &tampered).is_err());
        let too_big = vec![b'x'; MAX_SECRET_PLAINTEXT + 1];
        assert!(
            seal_secret(&key, "youtube", &too_big)
                .unwrap_err()
                .to_string()
                .contains("too large")
        );
        // The encoded maximum stays below the server's 64 KiB value limit.
        let max = seal_secret(&key, "youtube", &vec![b'x'; MAX_SECRET_PLAINTEXT]).unwrap();
        assert!(max.to_string().len() < 64 * 1024);
    }

    #[test]
    fn keycheck_opens_only_with_its_key() {
        let key = new_account_key();
        let check = keycheck(&key).unwrap();
        assert!(keycheck_opens(&key, &check));
        assert!(!keycheck_opens(&new_account_key(), &check));
        // A youtube secret is not a key check even under the right key.
        let other = seal_secret(&key, "youtube", KEYCHECK_PLAINTEXT).unwrap();
        assert!(!keycheck_opens(&key, &other));
    }

    #[test]
    fn sealed_box_round_trips_and_rejects_tampering() {
        let request = RelayRequest::from_secret([3u8; 32]);
        let key = [42u8; 32];
        let sealed = seal_for(&request.public_b64(), &key).unwrap();
        let raw = BASE64.decode(sealed.as_bytes()).unwrap();
        assert_eq!(raw.len(), 105);
        assert_eq!(raw[0], 1);
        assert_eq!(request.open(&sealed).unwrap(), key);
        for index in [0, 1, 40, 60, 104] {
            let mut bad = raw.clone();
            bad[index] ^= 0x80;
            assert!(request.open(&BASE64.encode(&bad)).is_err(), "byte {index}");
        }
        assert!(request.open(&BASE64.encode(&raw[..104])).is_err());
        // Another requester cannot open it.
        assert!(RelayRequest::new().open(&sealed).is_err());
    }

    #[test]
    fn sealed_box_vector_is_stable() {
        let request = RelayRequest::from_secret([1u8; 32]);
        let sealed =
            seal_for_with(&request.public_b64(), &[9u8; 32], [2u8; 32], [5u8; 24]).unwrap();
        // Same inputs, same bytes: the format is fixed by the contract.
        assert_eq!(
            sealed,
            seal_for_with(&request.public_b64(), &[9u8; 32], [2u8; 32], [5u8; 24]).unwrap()
        );
        assert_eq!(request.open(&sealed).unwrap(), [9u8; 32]);
    }

    #[test]
    fn all_zero_shared_secret_is_refused() {
        // The all-zero point (and other low-order points) yield an all-zero secret.
        let zero = BASE64.encode(&[0u8; 32]);
        assert!(seal_for(&zero, &[1u8; 32]).is_err());
        let request = RelayRequest::from_secret([4u8; 32]);
        let mut sealed = vec![1u8];
        sealed.extend_from_slice(&[0u8; 32 + 24 + 48]);
        assert!(request.open(&BASE64.encode(&sealed)).is_err());
    }

    #[test]
    fn fingerprint_has_twelve_digits_in_four_groups() {
        let fp = fingerprint(&[0u8; 32]);
        assert_eq!(fp.len(), 15);
        assert!(
            fp.split(' ')
                .all(|g| g.len() == 3 && g.bytes().all(|b| b.is_ascii_digit()))
        );
        assert_eq!(fp.split(' ').count(), 4);
        // Fixed vectors (computed independently with Python's hashlib).
        assert_eq!(fp, "746 136 825 322");
        assert_eq!(fingerprint(&[1u8; 32]), "797 227 684 471");
        assert_ne!(fingerprint(&[1u8; 32]), fp);
    }

    #[test]
    fn key_encodings_round_trip() {
        let key = new_account_key();
        assert_eq!(decode_key(&encode_key(&key)).unwrap(), key);
        let url = encode_key_url(&key);
        assert_eq!(url.len(), 43);
        assert_eq!(decode_key_url(&url).unwrap(), key);
        assert!(decode_key_url("short").is_err());
        assert!(decode_key("AAAA").is_err());
    }
}
