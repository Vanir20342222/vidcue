//! Aphrodite's public v2 client-session protocol. Session material stays in memory.
use super::{Backend, CancellationToken, JSON_LIMIT, Result, Url};
use anyhow::{Context, ensure};
use reqwest::header::HeaderMap;
use ring::{
    aead, hmac,
    rand::{SecureRandom, SystemRandom},
};
use serde::Deserialize;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

// Public client bootstrap material distributed in Cinecat's web player.
// This identifies the public client; it is shared by anonymous site visitors.
const PUBLIC_CLIENT_KEY: &str = "0a1bde0c410e79eeba2af6f3354ac394fba43ea593839be4dd7cd5a3222ff9d9";

fn timestamp() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

fn nonce() -> Result<String> {
    let mut bytes = [0; 8];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| anyhow::anyhow!("Could not generate provider nonce"))?;
    Ok(hex::encode(bytes))
}

fn sign(key: &[u8], message: &str) -> String {
    hex::encode(hmac::sign(
        &hmac::Key::new(hmac::HMAC_SHA256, key),
        message.as_bytes(),
    ))
}

#[derive(Deserialize)]
struct Session {
    sid: String,
    skey: String,
    exp: u64,
    now: u64,
}

fn decode_session(key: &[u8], ts: u64, nonce: &str, envelope: &str) -> Result<Session> {
    let derived = hex::decode(sign(key, &format!("aphrodite-seal-v2|{ts}|{nonce}")))?;
    let key = aead::UnboundKey::new(&aead::AES_256_GCM, &derived)
        .map_err(|_| anyhow::anyhow!("Invalid provider session key"))?;
    let key = aead::LessSafeKey::new(key);
    let mut bytes = hex::decode(envelope).context("Invalid provider session envelope")?;
    ensure!(bytes.len() >= 28, "Truncated provider session envelope");
    let iv: [u8; 12] = bytes[..12].try_into()?;
    let plaintext = key
        .open_in_place(
            aead::Nonce::assume_unique_for_key(iv),
            aead::Aad::empty(),
            &mut bytes[12..],
        )
        .map_err(|_| anyhow::anyhow!("Provider session authentication failed"))?;
    serde_json::from_slice(plaintext).context("Invalid provider session")
}

impl Backend {
    pub(super) async fn aphrodite_headers(
        &self,
        origin: &Url,
        path: &str,
        cancel: &CancellationToken,
    ) -> Result<HeaderMap> {
        let key = hex::decode(PUBLIC_CLIENT_KEY)?;
        let ts = timestamp()?;
        let bootstrap_nonce = nonce()?;
        let body = json!({
            "c": "z", "ts": ts, "n": bootstrap_nonce,
            "s": sign(&key, &format!("v2:z:{ts}:{bootstrap_nonce}")),
        });
        let request = self.client.post(origin.join("/content/index")?).json(&body);
        let response = tokio::select! {
            _ = cancel.cancelled() => anyhow::bail!("Cancelled"),
            response = tokio::time::timeout(std::time::Duration::from_secs(45), request.send()) =>
                response.map_err(|_| anyhow::anyhow!("Provider session timeout"))?
                    .map_err(|_| anyhow::anyhow!("Provider session request failed"))?,
        };
        ensure!(
            response.status().is_success(),
            "Provider session HTTP {}",
            response.status().as_u16()
        );
        let bytes = self.collect(response, JSON_LIMIT, cancel).await?;
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).context("Invalid provider session response")?;
        let session = decode_session(
            &key,
            ts,
            &bootstrap_nonce,
            value["d"]
                .as_str()
                .context("Missing provider session envelope")?,
        )?;
        ensure!(
            !session.sid.is_empty() && session.exp > session.now,
            "Invalid or expired provider session"
        );
        let session_key = hex::decode(session.skey).context("Invalid provider signing key")?;
        ensure!(
            session_key.len() == 32,
            "Invalid provider signing key length"
        );
        let request_ts = session.now;
        let request_nonce = nonce()?;
        let tag = sign(
            &session_key,
            &format!("v2:{}:{path}:{request_ts}:{request_nonce}", session.sid),
        );
        let mut headers = HeaderMap::new();
        for (name, value) in [
            ("x-az-id", session.sid),
            ("x-az-ts", request_ts.to_string()),
            ("x-az-nonce", request_nonce),
            ("x-az-tag", tag),
        ] {
            headers.insert(
                reqwest::header::HeaderName::from_static(name),
                value
                    .parse()
                    .map_err(|_| anyhow::anyhow!("Invalid provider session header"))?,
            );
        }
        Ok(headers)
    }
}

#[cfg(test)]
pub(super) fn fixture_response(request: &str) -> Option<String> {
    let (head, body) = request.split_once("\r\n\r\n").unwrap();
    let target = head
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap();
    let client_key = hex::decode(PUBLIC_CLIENT_KEY).unwrap();
    if target == "/content/index" {
        assert!(head.starts_with("POST "));
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        let ts = value["ts"].as_u64().unwrap();
        let nonce = value["n"].as_str().unwrap();
        assert_eq!(value["c"], "z");
        assert_eq!(value["s"], sign(&client_key, &format!("v2:z:{ts}:{nonce}")));
        let session = json!({"sid":"fixture-session", "skey":hex::encode([7;32]),
            "now":ts, "exp":ts+120});
        let derived = hex::decode(sign(
            &client_key,
            &format!("aphrodite-seal-v2|{ts}|{nonce}"),
        ))
        .unwrap();
        let key =
            aead::LessSafeKey::new(aead::UnboundKey::new(&aead::AES_256_GCM, &derived).unwrap());
        let mut encrypted = serde_json::to_vec(&session).unwrap();
        key.seal_in_place_append_tag(
            aead::Nonce::assume_unique_for_key([1; 12]),
            aead::Aad::empty(),
            &mut encrypted,
        )
        .unwrap();
        let mut envelope = vec![1; 12];
        envelope.extend(encrypted);
        return Some(json!({"d":hex::encode(envelope)}).to_string());
    }
    if target.starts_with("/content/") {
        let headers: std::collections::HashMap<_, _> = head
            .lines()
            .skip(1)
            .filter_map(|line| line.split_once(':'))
            .map(|(k, v)| (k.to_ascii_lowercase(), v.trim()))
            .collect();
        assert_eq!(headers.get("x-az-id"), Some(&"fixture-session"));
        let ts = headers["x-az-ts"];
        let nonce = headers["x-az-nonce"];
        assert_eq!(nonce.len(), 16);
        assert_eq!(
            headers["x-az-tag"],
            sign(
                &[7; 32],
                &format!("v2:fixture-session:{target}:{ts}:{nonce}")
            )
        );
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_session_envelopes_are_rejected() {
        for envelope in ["", "xx", "000000", &"00".repeat(28)] {
            assert!(decode_session(&[1; 32], 1, "nonce", envelope).is_err());
        }
    }
}
