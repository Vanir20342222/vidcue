//! Server origins and the QR pairing URI (PROTOCOL.md §7.1, §7.6.4).
use crate::crypto::{AccountKey, decode_key_url, encode_key_url};
use anyhow::{Context, Result, bail, ensure};
use url::Url;

pub const MAX_URI_LEN: usize = 512;
/// Hosts that may use plain `http` (local development and the Android emulator).
const LOCAL_HOSTS: &[&str] = &["localhost", "127.0.0.1", "10.0.2.2"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pairing {
    /// Canonical server origin.
    pub server: String,
    /// Upper-case pairing code.
    pub code: String,
    /// The account key from the `#k=` fragment, when the showing device added it.
    pub key: Option<AccountKey>,
}

/// Checks a server address and returns its canonical origin (`https://host[:port]`).
pub fn server_origin(raw: &str) -> Result<String> {
    let raw = raw.trim();
    ensure!(
        !raw.is_empty() && raw.len() <= 256,
        "Enter the server address"
    );
    let url = Url::parse(raw)
        .ok()
        .context("The server address is invalid")?;
    let host = url.host_str().context("The server address has no host")?;
    match url.scheme() {
        "https" => {}
        "http" if LOCAL_HOSTS.contains(&host) => {}
        "http" => bail!("The server must use https"),
        _ => bail!("The server address must start with https://"),
    }
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "The server address must not contain a user name or password"
    );
    ensure!(
        matches!(url.path(), "" | "/") && url.query().is_none() && url.fragment().is_none(),
        "Use only the server address, without a path"
    );
    Ok(url.origin().ascii_serialization())
}

/// The host shown on the pairing confirmation sheet.
pub fn origin_host(origin: &str) -> String {
    Url::parse(origin)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Appends the account key as `#k=…` to a server-issued pairing URI (done locally only).
pub fn with_key(uri: &str, key: &AccountKey) -> Result<String> {
    let parsed = parse_pairing_uri(uri)?;
    ensure!(
        parsed.key.is_none(),
        "The pairing link already carries a key"
    );
    Ok(format!("{uri}#k={}", encode_key_url(key)))
}

pub fn parse_pairing_uri(raw: &str) -> Result<Pairing> {
    let invalid = || anyhow::anyhow!("This is not a Vidcue pairing link");
    let raw = raw.trim();
    ensure!(!raw.is_empty() && raw.len() <= MAX_URI_LEN, invalid());
    let url = Url::parse(raw).map_err(|_| invalid())?;
    ensure!(url.scheme() == "vidcue", invalid());
    ensure!(url.host_str() == Some("pair"), invalid());
    ensure!(
        matches!(url.path(), "" | "/")
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none(),
        invalid()
    );
    let (mut server, mut code) = (None, None);
    for (name, value) in url.query_pairs() {
        let slot = match name.as_ref() {
            "server" => &mut server,
            "code" => &mut code,
            _ => bail!(invalid()),
        };
        ensure!(slot.is_none(), invalid());
        *slot = Some(value.into_owned());
    }
    let server = server_origin(&server.ok_or_else(invalid)?)?;
    let code = code.ok_or_else(invalid)?.to_ascii_uppercase();
    ensure!(
        code.len() == 10
            && code
                .bytes()
                .all(|b| b.is_ascii_uppercase() || (b'2'..=b'7').contains(&b)),
        "The pairing code is invalid"
    );
    let key = match url.fragment() {
        None => None,
        Some(fragment) => {
            let encoded = fragment.strip_prefix("k=").ok_or_else(invalid)?;
            ensure!(encoded.len() == 43, "The pairing key is invalid");
            Some(decode_key_url(encoded)?)
        }
    };
    Ok(Pairing { server, code, key })
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE";

    #[test]
    fn origins_follow_the_https_and_local_http_rules() {
        assert_eq!(
            server_origin("https://Vidcue.Example.org/").unwrap(),
            "https://vidcue.example.org"
        );
        assert_eq!(
            server_origin("https://vidcue.example.org:8443").unwrap(),
            "https://vidcue.example.org:8443"
        );
        assert_eq!(
            server_origin("http://10.0.2.2:8080").unwrap(),
            "http://10.0.2.2:8080"
        );
        assert!(server_origin("http://127.0.0.1:1").is_ok());
        assert!(server_origin("http://localhost").is_ok());
        for bad in [
            "",
            "vidcue.example.org",
            "http://vidcue.example.org",
            "https://user:pw@vidcue.example.org",
            "https://vidcue.example.org/api",
            "https://vidcue.example.org/?x=1",
            "https://vidcue.example.org/#f",
            "ftp://vidcue.example.org",
            "http://192.168.1.2",
        ] {
            assert!(server_origin(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn pairing_uri_parse_table() {
        let ok = "vidcue://pair?server=https%3A%2F%2Fvidcue.example.org&code=abcdefgh23";
        let parsed = parse_pairing_uri(ok).unwrap();
        assert_eq!(parsed.server, "https://vidcue.example.org");
        assert_eq!(parsed.code, "ABCDEFGH23");
        assert_eq!(parsed.key, None);
        let keyed = parse_pairing_uri(&format!("{ok}#k={KEY}")).unwrap();
        assert_eq!(keyed.key, Some([1u8; 32]));
        assert_eq!(with_key(ok, &[1u8; 32]).unwrap(), format!("{ok}#k={KEY}"));
        assert!(with_key(&format!("{ok}#k={KEY}"), &[1u8; 32]).is_err());
        for bad in [
            // missing fields
            "vidcue://pair?server=https%3A%2F%2Fvidcue.example.org".to_owned(),
            "vidcue://pair?code=ABCDEFGH23".to_owned(),
            // duplicate or unknown fields
            format!("{ok}&code=ABCDEFGH23"),
            format!("{ok}&extra=1"),
            // bad scheme or host
            "https://pair?server=https%3A%2F%2Fa.example&code=ABCDEFGH23".to_owned(),
            "vidcue://join?server=https%3A%2F%2Fa.example&code=ABCDEFGH23".to_owned(),
            "vidcue://pair/x?server=https%3A%2F%2Fa.example&code=ABCDEFGH23".to_owned(),
            // http for a non-local host
            "vidcue://pair?server=http%3A%2F%2Fa.example&code=ABCDEFGH23".to_owned(),
            // bad codes
            "vidcue://pair?server=https%3A%2F%2Fa.example&code=ABCDEFGH1".to_owned(),
            "vidcue://pair?server=https%3A%2F%2Fa.example&code=ABCDEFGH18".to_owned(),
            // bad key length or fragment
            format!("{ok}#k=AQEB"),
            format!("{ok}#k={KEY}AA"),
            format!("{ok}#x={KEY}"),
            // too long
            format!("{ok}&{}", "x".repeat(600)),
        ] {
            assert!(parse_pairing_uri(&bad).is_err(), "{bad}");
        }
        assert!(
            parse_pairing_uri("vidcue://pair?server=http%3A%2F%2F10.0.2.2%3A8080&code=ABCDEFGH23")
                .is_ok()
        );
        assert_eq!(
            origin_host("https://vidcue.example.org:8443"),
            "vidcue.example.org"
        );
    }
}
