//! WebSocket client (PROTOCOL.md §7.5): one connection per signed-in device, reconnects
//! with backoff, exposes server events, and sends client messages only to servers that
//! announce `realtime`.
use crate::protocol::{ClientMessage, ServerEvent};
use anyhow::{Context, Result, bail};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::Duration,
};
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio_tungstenite::{
    Connector,
    tungstenite::{self, Message, client::IntoClientRequest, protocol::WebSocketConfig},
};
use uuid::Uuid;

pub const OUTGOING_CAPACITY: usize = 64;
pub const EVENT_CAPACITY: usize = 256;
pub const REPLY_TIMEOUT: Duration = Duration::from_secs(10);
pub const MAX_BACKOFF: Duration = Duration::from_secs(60);
/// A connection that stayed up this long resets the backoff.
pub const STABLE_AFTER: Duration = Duration::from_secs(60);
const MAX_MESSAGE: usize = 4 * 1024 * 1024;
pub const OFFLINE: &str = "device offline";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WsState {
    Connecting,
    Connected,
    Disconnected,
    /// 401 on upgrade or this device was revoked: no reconnect.
    SignedOut,
    Closed,
}

struct Outgoing {
    frame: String,
}

#[derive(Default)]
struct Shared {
    features: Mutex<Vec<String>>,
    pending: Mutex<HashMap<u32, oneshot::Sender<Result<Value, String>>>>,
    next_rid: AtomicU32,
    connected: AtomicBool,
}

/// Handle to the background connection task. Dropping it closes the connection.
pub struct WsClient {
    tx: mpsc::Sender<Outgoing>,
    events: broadcast::Sender<ServerEvent>,
    state: watch::Receiver<WsState>,
    shared: Arc<Shared>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for WsClient {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// `wss://host/api/ws` (or `ws://` for local http origins).
pub fn ws_url(origin: &str) -> Result<String> {
    let mut url = url::Url::parse(origin)?;
    let scheme = match url.scheme() {
        "https" => "wss",
        "http" => "ws",
        _ => bail!("Unsupported server address"),
    };
    url.set_scheme(scheme)
        .map_err(|_| anyhow::anyhow!("Unsupported server address"))?;
    url.set_path("/api/ws");
    Ok(url.to_string())
}

fn tls() -> Result<Connector> {
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_root_certificates(roots)
    .with_no_client_auth();
    Ok(Connector::Rustls(Arc::new(config)))
}

enum Ended {
    /// Reconnect after backoff; `true` when the session was stable.
    Retry(bool),
    SignedOut,
    /// The handle was dropped.
    Closed,
}

impl WsClient {
    /// Starts connecting in the background on the current Tokio runtime.
    pub fn connect(origin: &str, token: &str, device_id: Uuid) -> Result<Self> {
        let url = ws_url(origin)?;
        let connector = if url.starts_with("wss:") {
            tls()?
        } else {
            Connector::Plain
        };
        let (tx, rx) = mpsc::channel(OUTGOING_CAPACITY);
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        let (state_tx, state) = watch::channel(WsState::Connecting);
        let shared = Arc::new(Shared::default());
        let task = tokio::spawn(run(
            url,
            token.to_owned(),
            device_id,
            connector,
            rx,
            events.clone(),
            state_tx,
            shared.clone(),
        ));
        Ok(Self {
            tx,
            events,
            state,
            shared,
            task,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.events.subscribe()
    }
    pub fn state(&self) -> watch::Receiver<WsState> {
        self.state.clone()
    }
    pub fn connected(&self) -> bool {
        self.shared.connected.load(Ordering::SeqCst)
    }
    /// Features from the last `hello`.
    pub fn features(&self) -> Vec<String> {
        self.shared
            .features
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
    pub fn realtime(&self) -> bool {
        self.connected() && self.features().iter().any(|f| f == "realtime")
    }

    /// Sends without waiting. Presence and Jam pings are dropped silently when the
    /// socket is down or full; returns whether the frame was queued.
    pub fn send(&self, message: &ClientMessage) -> bool {
        if !self.realtime() {
            return false;
        }
        self.tx
            .try_send(Outgoing {
                frame: message.frame(None),
            })
            .is_ok()
    }

    /// Sends with a `rid` and waits for the `ack`, `error` or reply (`pong`,
    /// `presence_snapshot`). Fails with "device offline" when it cannot be delivered.
    pub async fn request(&self, message: &ClientMessage) -> Result<Value> {
        if !self.realtime() {
            bail!(OFFLINE);
        }
        let rid = self.shared.next_rid.fetch_add(1, Ordering::SeqCst) % 0x7fff_ffff + 1;
        let (reply_tx, reply_rx) = oneshot::channel();
        self.shared
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(rid, reply_tx);
        let queued = self
            .tx
            .try_send(Outgoing {
                frame: message.frame(Some(rid)),
            })
            .is_ok();
        let forget = || {
            self.shared
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&rid);
        };
        if !queued {
            forget();
            bail!(OFFLINE);
        }
        match tokio::time::timeout(REPLY_TIMEOUT, reply_rx).await {
            Ok(Ok(Ok(value))) => Ok(value),
            Ok(Ok(Err(code))) if code == "not_found" => bail!(OFFLINE),
            Ok(Ok(Err(code))) => bail!(match code.as_str() {
                "rate_limited" => "Too many requests, try again in a moment",
                "invalid" => "This item cannot be sent",
                "too_large" => "Too many items to send",
                _ => "The server refused the request",
            }),
            _ => {
                forget();
                bail!(OFFLINE)
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run(
    url: String,
    token: String,
    device_id: Uuid,
    connector: Connector,
    mut rx: mpsc::Receiver<Outgoing>,
    events: broadcast::Sender<ServerEvent>,
    state: watch::Sender<WsState>,
    shared: Arc<Shared>,
) {
    let mut backoff = Duration::from_secs(1);
    loop {
        let _ = state.send(WsState::Connecting);
        let ended = match open(&url, &token, connector.clone()).await {
            Ok(stream) => session(stream, device_id, &mut rx, &events, &state, &shared).await,
            Err(Unauthorized(true)) => Ended::SignedOut,
            Err(Unauthorized(false)) => Ended::Retry(false),
        };
        shared.connected.store(false, Ordering::SeqCst);
        fail_pending(&shared);
        match ended {
            Ended::SignedOut => {
                let _ = state.send(WsState::SignedOut);
                return;
            }
            Ended::Closed => {
                let _ = state.send(WsState::Closed);
                return;
            }
            Ended::Retry(stable) => {
                let _ = state.send(WsState::Disconnected);
                if stable {
                    backoff = Duration::from_secs(1);
                }
                tokio::time::sleep(crate::jitter(backoff)).await;
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
        }
    }
}

/// Connection failure; `true` when the server refused the token (401).
struct Unauthorized(bool);

type Stream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn open(url: &str, token: &str, connector: Connector) -> Result<Stream, Unauthorized> {
    let request = (|| -> Result<_> {
        let mut request = url.into_client_request()?;
        request.headers_mut().insert(
            "Authorization",
            format!("Bearer {token}").parse().context("invalid token")?,
        );
        Ok(request)
    })()
    .map_err(|_| Unauthorized(true))?;
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE))
        .max_frame_size(Some(MAX_MESSAGE));
    match tokio::time::timeout(
        Duration::from_secs(20),
        tokio_tungstenite::connect_async_tls_with_config(
            request,
            Some(config),
            true,
            Some(connector),
        ),
    )
    .await
    {
        Ok(Ok((stream, _))) => Ok(stream),
        Ok(Err(tungstenite::Error::Http(response))) if response.status() == 401 => {
            Err(Unauthorized(true))
        }
        _ => Err(Unauthorized(false)),
    }
}

fn fail_pending(shared: &Shared) {
    for (_, reply) in shared
        .pending
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .drain()
    {
        let _ = reply.send(Err("offline".into()));
    }
}

fn resolve(shared: &Shared, rid: Option<u32>, reply: Result<Value, String>) {
    if let Some(rid) = rid
        && let Some(sender) = shared
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&rid)
    {
        let _ = sender.send(reply);
    }
}

async fn session(
    stream: Stream,
    device_id: Uuid,
    rx: &mut mpsc::Receiver<Outgoing>,
    events: &broadcast::Sender<ServerEvent>,
    state: &watch::Sender<WsState>,
    shared: &Shared,
) -> Ended {
    let started = tokio::time::Instant::now();
    let stable = || started.elapsed() >= STABLE_AFTER;
    let (mut sink, mut source) = stream.split();
    // Frames queued while disconnected are stale (presence, pings): drop them.
    while rx.try_recv().is_ok() {}
    loop {
        tokio::select! {
            incoming = source.next() => {
                let text = match incoming {
                    Some(Ok(Message::Text(text))) => text,
                    Some(Ok(Message::Ping(payload))) => {
                        if sink.send(Message::Pong(payload)).await.is_err() {
                            return Ended::Retry(stable());
                        }
                        continue;
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return Ended::Retry(stable()),
                    Some(Ok(_)) => continue,
                };
                let raw: Value = serde_json::from_str(text.as_str()).unwrap_or(Value::Null);
                let event = ServerEvent::from_value(raw.clone());
                match &event {
                    ServerEvent::Hello { features, .. } => {
                        *shared
                            .features
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner) = features.clone();
                        shared.connected.store(true, Ordering::SeqCst);
                        let _ = state.send(WsState::Connected);
                    }
                    ServerEvent::Ack { rid } => resolve(shared, Some(*rid), Ok(raw.clone())),
                    ServerEvent::Error { rid, code, .. } => resolve(shared, *rid, Err(code.clone())),
                    ServerEvent::Pong { rid, .. } => resolve(shared, *rid, Ok(raw.clone())),
                    ServerEvent::PresenceSnapshot { rid, .. } => resolve(shared, *rid, Ok(raw.clone())),
                    _ => {}
                }
                // Replies to this client's own requests are not app events.
                let reply = match &event {
                    ServerEvent::Ack { .. } | ServerEvent::Pong { .. } => true,
                    ServerEvent::Error { rid, .. } | ServerEvent::PresenceSnapshot { rid, .. } => {
                        rid.is_some()
                    }
                    _ => false,
                };
                let revoked = match &event {
                    ServerEvent::DeviceRevoked { device_id: id } => *id == device_id,
                    ServerEvent::DevicesRevoked { keep } => *keep != Some(device_id),
                    _ => false,
                };
                if !reply {
                    let _ = events.send(event);
                }
                if revoked {
                    let _ = sink.close().await;
                    return Ended::SignedOut;
                }
            }
            outgoing = rx.recv() => {
                let Some(outgoing) = outgoing else {
                    let _ = sink.close().await;
                    return Ended::Closed;
                };
                if sink.send(Message::Text(outgoing.frame.into())).await.is_err() {
                    return Ended::Retry(stable());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_urls_follow_the_origin_scheme() {
        assert_eq!(
            ws_url("https://vidcue.example.org").unwrap(),
            "wss://vidcue.example.org/api/ws"
        );
        assert_eq!(
            ws_url("http://127.0.0.1:8080").unwrap(),
            "ws://127.0.0.1:8080/api/ws"
        );
        assert!(tls().is_ok());
    }
}
