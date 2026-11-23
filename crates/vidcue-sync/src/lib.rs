//! Client side of the self-hosted Vidcue backend (docs/backend/PROTOCOL.md §7).
//!
//! Pure Rust without Android code so a future desktop app can reuse it. The app
//! supplies secret sealing ([`store::SecretStore`]) and its local library
//! ([`engine::LocalStores`]); this crate owns the wire protocol, the persisted outbox,
//! the end-to-end account key, the WebSocket connection and the Jam clock.
pub mod api;
pub mod clock;
pub mod crypto;
pub mod engine;
pub mod filters;
pub mod fractional;
pub mod health;
pub mod outbox;
pub mod pairing;
pub mod protocol;
pub mod qr;
pub mod store;
pub mod ws;

pub use api::{Api, ApiError};
pub use engine::{Change, LocalStores, SyncEngine};
pub use protocol::SessionInfo;

/// Current Unix time in milliseconds.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// `±20 %` jitter around `base`, from the OS random generator.
pub(crate) fn jitter(base: std::time::Duration) -> std::time::Duration {
    use rand::Rng;
    let factor = rand::rngs::OsRng.gen_range(0.8..=1.2);
    base.mul_f64(factor)
}
