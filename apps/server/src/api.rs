//! HTTP and WebSocket API: state, authentication, devices, pairing, sync and admin.

use crate::auth;
use crate::secrets::ServerKey;
use crate::sync;
use crate::throttle::{self, Rule};
use axum::extract::connect_info::ConnectInfo;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Extension, FromRequestParts, Path, Query, Request, State};
use axum::http::request::Parts;
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use data_encoding::BASE32_NOPAD;
use deadpool_postgres::Pool;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{Semaphore, broadcast};
use uuid::Uuid;

pub const INVITE_PREFIX: &str = "vci_";
pub const DEVICE_PREFIX: &str = "vcd_";
pub const TICKET_PREFIX: &str = "vct_";
const PAIRING_TTL_MS: i64 = 10 * 60 * 1000;
const PENDING_SIGNUP_TTL_MS: i64 = 60 * 60 * 1000;
const TICKET_TTL_MS: i64 = 30 * 1000;
/// Device tokens unused for this long stop working.
pub const DEVICE_IDLE_MS: i64 = 180 * 24 * 3_600_000;
pub const MAX_DEVICES_PER_USER: i64 = 20;
const MAX_OPEN_PAIRINGS: i64 = 5;
const MAX_SOCKETS_PER_DEVICE: usize = 3;
const MAX_WS_MESSAGE: usize = 16 * 1024;

/// Argon2 uses ~19 MiB per hash; at most this many run at once so a login flood cannot
/// exhaust memory. Requests that wait longer than `HASH_WAIT` get 503.
static HASHING: Semaphore = Semaphore::const_new(4);
const HASH_WAIT: Duration = Duration::from_secs(5);

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

pub struct Settings {
    /// Public base URL without trailing slash, e.g. `https://vidcue.example.org`.
    pub public_url: String,
    pub key: ServerKey,
    /// Reverse proxies whose `X-Real-IP` is trusted.
    pub trusted_proxies: Vec<IpAddr>,
    /// Refuse connections from anything but the trusted proxies and loopback.
    pub proxies_only: bool,
}

#[derive(Clone)]
pub struct AppState(pub Arc<Inner>);

pub struct Inner {
    pub db: Pool,
    pub public_url: String,
    pub key: ServerKey,
    trusted_proxies: Vec<IpAddr>,
    proxies_only: bool,
    pub hub: Hub,
    /// Single-use WebSocket tickets: hash → (device, expiry).
    tickets: Mutex<HashMap<Vec<u8>, (Uuid, i64)>>,
    /// Serialises sync pushes per user before a database connection is taken.
    push_locks: Mutex<HashMap<Uuid, Arc<tokio::sync::Mutex<()>>>>,
    /// Overridable clock for tests.
    pub clock: fn() -> i64,
}

impl AppState {
    pub fn new(db: Pool, settings: Settings) -> Self {
        Self(Arc::new(Inner {
            db,
            public_url: settings.public_url.trim_end_matches('/').to_owned(),
            key: settings.key,
            trusted_proxies: settings.trusted_proxies,
            proxies_only: settings.proxies_only,
            hub: Hub::default(),
            tickets: Mutex::default(),
            push_locks: Mutex::default(),
            clock: now_ms,
        }))
    }
    pub(crate) fn now(&self) -> i64 {
        (self.0.clock)()
    }
    /// Records an attempt on every key, or refuses with 429 when one is over its limit.
    pub(crate) async fn attempt(
        &self,
        limits: &[(Rule, &str)],
        message: &str,
    ) -> ApiResult<Vec<i64>> {
        throttle::attempt(&self.0.db, limits, self.now())
            .await?
            .ok_or_else(|| ApiError(StatusCode::TOO_MANY_REQUESTS, message.into()))
    }
    /// Removes the attempt rows of a request that succeeded.
    pub(crate) async fn forgive(&self, ids: &[i64]) -> ApiResult<()> {
        Ok(throttle::forgive(&self.0.db, ids).await?)
    }
    fn push_lock(&self, user: Uuid) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.0.push_locks.lock().expect("push locks");
        if locks.len() > 1000 {
            locks.retain(|_, l| Arc::strong_count(l) > 1);
        }
        locks.entry(user).or_default().clone()
    }
}

/// Runs a password hash or verification with bounded concurrency.
pub(crate) async fn argon<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> ApiResult<T> {
    let _permit = tokio::time::timeout(HASH_WAIT, HASHING.acquire())
        .await
        .map_err(|_| {
            ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "Server busy, try again".into(),
            )
        })?
        .map_err(anyhow::Error::from)?;
    Ok(tokio::task::spawn_blocking(work)
        .await
        .map_err(anyhow::Error::from)?)
}

/// Client address as seen through the trusted reverse proxy.
#[derive(Clone, Copy, Debug)]
pub struct ClientIp(pub IpAddr);

async fn network_guard(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map_or(IpAddr::V4(Ipv4Addr::LOCALHOST), |c| c.0.ip());
    let trusted = state.0.trusted_proxies.contains(&peer);
    if state.0.proxies_only && !trusted && !peer.is_loopback() {
        return ApiError(StatusCode::FORBIDDEN, "Use the public HTTPS address".into())
            .into_response();
    }
    let client = trusted
        .then(|| {
            req.headers()
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<IpAddr>().ok())
        })
        .flatten()
        .unwrap_or(peer);
    req.extensions_mut().insert(ClientIp(client));
    next.run(req).await
}

/// Per-user fan-out of change notifications, plus a count of open sockets per device.
#[derive(Default)]
pub struct Hub {
    channels: Mutex<HashMap<Uuid, broadcast::Sender<Value>>>,
    sockets: Arc<Mutex<HashMap<Uuid, usize>>>,
}

pub struct SocketSlot {
    device: Uuid,
    sockets: Arc<Mutex<HashMap<Uuid, usize>>>,
}

impl Drop for SocketSlot {
    fn drop(&mut self) {
        let mut sockets = self.sockets.lock().expect("hub sockets");
        if let Some(n) = sockets.get_mut(&self.device) {
            *n -= 1;
            if *n == 0 {
                sockets.remove(&self.device);
            }
        }
    }
}

impl Hub {
    pub fn subscribe(&self, user: Uuid) -> broadcast::Receiver<Value> {
        let mut map = self.channels.lock().expect("hub lock");
        map.retain(|_, tx| tx.receiver_count() > 0);
        map.entry(user)
            .or_insert_with(|| broadcast::channel(64).0)
            .subscribe()
    }
    pub fn publish(&self, user: Uuid, event: Value) {
        if let Some(tx) = self.channels.lock().expect("hub lock").get(&user) {
            let _ = tx.send(event);
        }
    }
    fn reserve_socket(&self, device: Uuid) -> Option<SocketSlot> {
        let mut sockets = self.sockets.lock().expect("hub sockets");
        let n = sockets.entry(device).or_default();
        if *n >= MAX_SOCKETS_PER_DEVICE {
            return None;
        }
        *n += 1;
        Some(SocketSlot {
            device,
            sockets: self.sockets.clone(),
        })
    }
}

pub struct ApiError(pub StatusCode, pub String);

impl ApiError {
    pub(crate) fn bad(msg: impl Into<String>) -> Self {
        Self(StatusCode::BAD_REQUEST, msg.into())
    }
    pub(crate) fn unauthorized(msg: impl Into<String>) -> Self {
        Self(StatusCode::UNAUTHORIZED, msg.into())
    }
    pub(crate) fn forbidden() -> Self {
        Self(StatusCode::FORBIDDEN, "Admins only".into())
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!("internal error: {e:#}");
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error".into(),
        )
    }
}
impl From<tokio_postgres::Error> for ApiError {
    fn from(e: tokio_postgres::Error) -> Self {
        anyhow::Error::from(e).into()
    }
}
impl From<deadpool_postgres::PoolError> for ApiError {
    fn from(e: deadpool_postgres::PoolError) -> Self {
        anyhow::Error::from(e).into()
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

pub(crate) type ApiResult<T = Json<Value>> = Result<T, ApiError>;

/// An authenticated, non-revoked, recently used device of an active user.
#[derive(Clone, Debug)]
pub struct Device {
    pub user: Uuid,
    pub device: Uuid,
    pub username: String,
    pub is_admin: bool,
}

const DEVICE_LOOKUP: &str = "UPDATE devices d SET last_seen = CASE WHEN d.last_seen < $2::BIGINT - 60000 THEN $2::BIGINT ELSE d.last_seen END
     FROM users u
     WHERE {WHERE} AND d.revoked_at IS NULL AND d.last_seen > $2::BIGINT - $3::BIGINT AND u.id = d.user_id AND u.active
     RETURNING d.id, d.user_id, u.username, u.is_admin";

async fn find_device(
    state: &AppState,
    condition: &str,
    param: &(dyn tokio_postgres::types::ToSql + Sync),
) -> ApiResult<Option<Device>> {
    let sql = DEVICE_LOOKUP.replace("{WHERE}", condition);
    let row = state
        .0
        .db
        .get()
        .await?
        .query_opt(&sql, &[param, &state.now(), &DEVICE_IDLE_MS])
        .await?;
    Ok(row.map(|row| Device {
        user: row.get("user_id"),
        device: row.get("id"),
        username: row.get("username"),
        is_admin: row.get("is_admin"),
    }))
}

pub(crate) async fn device_for_token(state: &AppState, token: &str) -> ApiResult<Device> {
    find_device(state, "d.token_hash = $1", &auth::token_hash(token))
        .await?
        .ok_or_else(|| ApiError::unauthorized("Invalid or revoked device token"))
}

async fn device_by_id(state: &AppState, id: Uuid) -> ApiResult<Option<Device>> {
    find_device(state, "d.id = $1", &id).await
}

fn bearer(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}

impl FromRequestParts<AppState> for Device {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let token =
            bearer(&parts.headers).ok_or_else(|| ApiError::unauthorized("Missing bearer token"))?;
        device_for_token(state, token).await
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route(
            "/join",
            get(|| async { crate::web::page_response(crate::web::join_page()) }),
        )
        .route("/api/signup", post(signup))
        .route("/api/signup/confirm", post(signup_confirm))
        .route("/api/login", post(login))
        .route("/api/me", get(me))
        .route("/api/devices", get(devices))
        .route("/api/devices/{id}", delete(revoke_device))
        .route("/api/pairing", post(pairing_start))
        .route("/api/pairing/claim", post(pairing_claim))
        .route("/api/sync/push", post(sync_push))
        .route("/api/sync/pull", get(sync_pull))
        .route("/api/ws/ticket", post(ws_ticket))
        .route("/api/ws", get(ws))
        .route("/api/admin/invites", post(admin_invite))
        .route("/api/admin/users", get(admin_users))
        .merge(crate::account::routes())
        .merge(crate::keys::routes())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            network_guard,
        ))
        .with_state(state)
}

pub(crate) fn device_name(name: Option<&str>) -> String {
    let name: String = vidcue_core::clean_text(name.unwrap_or_default().trim())
        .replace('\n', " ")
        .chars()
        .take(64)
        .collect();
    if name.is_empty() {
        "Device".into()
    } else {
        name
    }
}

pub fn qr_svg(data: &str) -> String {
    qrcode::QrCode::new(data.as_bytes())
        .map(|code| {
            code.render::<qrcode::render::svg::Color>()
                .min_dimensions(220, 220)
                .quiet_zone(true)
                .build()
        })
        .unwrap_or_default()
}

pub(crate) async fn issue_device(
    db: &Pool,
    user: Uuid,
    name: &str,
    now: i64,
    created_by: Option<Uuid>,
) -> ApiResult<(Uuid, String)> {
    let client = db.get().await?;
    let active: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM devices WHERE user_id = $1 AND revoked_at IS NULL",
            &[&user],
        )
        .await?
        .get(0);
    if active >= MAX_DEVICES_PER_USER {
        return Err(ApiError(
            StatusCode::CONFLICT,
            format!("At most {MAX_DEVICES_PER_USER} devices; remove one first"),
        ));
    }
    let id = Uuid::new_v4();
    let token = auth::new_token(DEVICE_PREFIX);
    client
        .execute(
            "INSERT INTO devices (id, user_id, name, token_hash, created_at, last_seen, created_by)
             VALUES ($1, $2, $3, $4, $5, $5, $6)",
            &[
                &id,
                &user,
                &name,
                &auth::token_hash(&token),
                &now,
                &created_by,
            ],
        )
        .await?;
    Ok((id, token))
}

/// Creates an invite and returns its signup link. The token is in the URL fragment, so
/// browsers never send it to the server or the proxy logs.
pub async fn create_invite(
    db: &Pool,
    public_url: &str,
    created_by: Option<Uuid>,
    is_admin: bool,
    hours: i64,
    now: i64,
) -> anyhow::Result<(String, i64)> {
    let token = auth::new_token(INVITE_PREFIX);
    let expires = now + hours.clamp(1, 24 * 30) * 3_600_000;
    db.get()
        .await?
        .execute(
            "INSERT INTO invites (token_hash, created_by, is_admin, created_at, expires_at)
             VALUES ($1, $2, $3, $4, $5)",
            &[
                &auth::token_hash(&token),
                &created_by,
                &is_admin,
                &now,
                &expires,
            ],
        )
        .await?;
    Ok((
        format!("{}/join#{token}", public_url.trim_end_matches('/')),
        expires,
    ))
}

/// Verifies an authenticator or recovery code for `user` and consumes it.
pub(crate) async fn second_factor(
    state: &AppState,
    user: Uuid,
    code: &str,
    now: i64,
) -> ApiResult<bool> {
    if auth::is_recovery_code(code) {
        return Ok(
            crate::account::use_recovery_code(&state.0.db, &state.0.key, user, code, now).await?,
        );
    }
    let client = state.0.db.get().await?;
    let row = client
        .query_one(
            "SELECT totp_secret, totp_last_step FROM users WHERE id = $1",
            &[&user],
        )
        .await?;
    let secret = state.0.key.open(user, &row.get::<_, Vec<u8>>(0))?;
    let Some(step) = auth::verify_totp(
        &secret,
        code,
        (now / 1000) as u64,
        row.get::<_, i64>(1) as u64,
    ) else {
        return Ok(false);
    };
    // Conditional update closes the race where two requests reuse one code.
    Ok(client
        .execute(
            "UPDATE users SET totp_last_step = $2 WHERE id = $1 AND totp_last_step < $2",
            &[&user, &(step as i64)],
        )
        .await?
        == 1)
}

/// Re-checks the password (and optionally a second factor) of a signed-in user before a
/// sensitive change, so a stolen device token alone is not enough.
pub(crate) async fn reauth(
    state: &AppState,
    device: &Device,
    password: &str,
    code: Option<&str>,
) -> ApiResult<()> {
    let key = format!("reauth:{}", device.user);
    let ids = state
        .attempt(
            &[(throttle::REAUTH, &key)],
            "Too many failed attempts, wait 15 minutes",
        )
        .await?;
    let stored: String = state
        .0
        .db
        .get()
        .await?
        .query_one(
            "SELECT password_hash FROM users WHERE id = $1",
            &[&device.user],
        )
        .await?
        .get(0);
    let password = password.to_owned();
    if !argon(move || auth::verify_password(&password, &stored)).await? {
        return Err(ApiError::unauthorized("Wrong password"));
    }
    if let Some(code) = code
        && !second_factor(state, device.user, code, state.now()).await?
    {
        return Err(ApiError::unauthorized(
            "Wrong authenticator or recovery code",
        ));
    }
    state.forgive(&ids).await
}

#[derive(Deserialize)]
struct SignupBody {
    invite: String,
    username: String,
    password: String,
}

async fn signup(
    State(state): State<AppState>,
    Extension(ip): Extension<ClientIp>,
    Json(body): Json<SignupBody>,
) -> ApiResult {
    let now = state.now();
    let username = auth::normalize_username(&body.username).map_err(ApiError::bad)?;
    if let Some(problem) = auth::password_problem(&body.password) {
        return Err(ApiError::bad(problem));
    }
    let ip_key = format!("invite:{}", ip.0);
    let ids = state
        .attempt(
            &[(throttle::CODES_IP, &ip_key)],
            "Too many attempts, try again later",
        )
        .await?;
    let invite_hash = auth::token_hash(&body.invite);
    let invite_ok = |row: &tokio_postgres::Row| {
        row.get::<_, i64>("expires_at") > now
            && !row.get::<_, Option<bool>>("used_active").unwrap_or(false)
    };
    const INVITE: &str =
        "SELECT i.token_hash, i.is_admin, i.expires_at, i.used_by, u.active AS used_active
         FROM invites i LEFT JOIN users u ON u.id = i.used_by WHERE i.token_hash = $1";
    // Check the invite before the expensive hash, so junk requests cost nothing.
    let valid = state
        .0
        .db
        .get()
        .await?
        .query_opt(INVITE, &[&invite_hash])
        .await?
        .is_some_and(|r| invite_ok(&r));
    if !valid {
        return Err(ApiError::bad(
            "This invite link is invalid, used or expired",
        ));
    }
    let password = body.password.clone();
    let hash = argon(move || auth::hash_password(&password)).await??;
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    let invite = tx
        .query_opt(&format!("{INVITE} FOR UPDATE OF i"), &[&invite_hash])
        .await?
        .filter(|r| invite_ok(r))
        .ok_or_else(|| ApiError::bad("This invite link is invalid, used or expired"))?;
    // An unfinished signup on this invite, or an abandoned one holding the name, is discarded.
    if let Some(previous) = invite.get::<_, Option<Uuid>>("used_by") {
        tx.execute(
            "DELETE FROM users WHERE id = $1 AND NOT active",
            &[&previous],
        )
        .await?;
    }
    tx.execute(
        "DELETE FROM users WHERE username = $1 AND NOT active AND created_at < $2",
        &[&username, &(now - PENDING_SIGNUP_TTL_MS)],
    )
    .await?;
    let taken: bool = tx
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM users WHERE username = $1)",
            &[&username],
        )
        .await?
        .get(0);
    if taken {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "This username is taken".into(),
        ));
    }
    let id = Uuid::new_v4();
    let secret = auth::new_totp_secret();
    tx.execute(
        "INSERT INTO users (id, username, password_hash, totp_secret, is_admin, active, created_at)
         VALUES ($1, $2, $3, $4, $5, FALSE, $6)",
        &[
            &id,
            &username,
            &hash,
            &state.0.key.seal(id, &secret),
            &invite.get::<_, bool>("is_admin"),
            &now,
        ],
    )
    .await?;
    tx.execute(
        "UPDATE invites SET used_by = $1, used_at = $2 WHERE token_hash = $3",
        &[&id, &now, &invite.get::<_, Vec<u8>>("token_hash")],
    )
    .await?;
    tx.commit().await?;
    drop(client);
    state.forgive(&ids).await?;
    let uri = auth::totp_uri("Vidcue", &username, &secret);
    Ok(Json(json!({
        "signup_id": id,
        "username": username,
        "totp_uri": uri,
        "totp_secret": BASE32_NOPAD.encode(&secret),
        "totp_qr_svg": qr_svg(&uri),
    })))
}

#[derive(Deserialize)]
struct ConfirmBody {
    invite: String,
    signup_id: Uuid,
    code: String,
    /// When set (signup from the app), the response carries a device token.
    device_name: Option<String>,
}

async fn signup_confirm(
    State(state): State<AppState>,
    Extension(ip): Extension<ClientIp>,
    Json(body): Json<ConfirmBody>,
) -> ApiResult {
    let now = state.now();
    let (key, ip_key) = (
        format!("confirm:{}", body.signup_id),
        format!("confirm:{}", ip.0),
    );
    let ids = state
        .attempt(
            &[(throttle::CONFIRM, &key), (throttle::CODES_IP, &ip_key)],
            "Too many attempts, start again",
        )
        .await?;
    let client = state.0.db.get().await?;
    let row = client
        .query_opt(
            "SELECT u.totp_secret, u.username FROM users u JOIN invites i ON i.used_by = u.id
             WHERE u.id = $1 AND NOT u.active AND i.token_hash = $2 AND u.created_at > $3",
            &[
                &body.signup_id,
                &auth::token_hash(&body.invite),
                &(now - PENDING_SIGNUP_TTL_MS),
            ],
        )
        .await?
        .ok_or_else(|| ApiError::bad("Signup expired, open the invite link again"))?;
    let secret = state
        .0
        .key
        .open(body.signup_id, &row.get::<_, Vec<u8>>("totp_secret"))?;
    let Some(step) = auth::verify_totp(&secret, &body.code, (now / 1000) as u64, 0) else {
        return Err(ApiError::bad("Wrong authenticator code"));
    };
    client
        .execute(
            "UPDATE users SET active = TRUE, totp_last_step = $2 WHERE id = $1",
            &[&body.signup_id, &(step as i64)],
        )
        .await?;
    drop(client);
    state.forgive(&ids).await?;
    let recovery_codes =
        crate::account::new_recovery_codes(&state.0.db, &state.0.key, body.signup_id).await?;
    let mut out = json!({
        "user_id": body.signup_id,
        "username": row.get::<_, String>("username"),
        "recovery_codes": recovery_codes,
    });
    if body.device_name.is_some() {
        let (device, token) = issue_device(
            &state.0.db,
            body.signup_id,
            &device_name(body.device_name.as_deref()),
            now,
            None,
        )
        .await?;
        out["device_id"] = json!(device);
        out["token"] = json!(token);
    }
    Ok(Json(out))
}

#[derive(Deserialize)]
struct LoginBody {
    username: String,
    password: String,
    code: String,
    device_name: Option<String>,
}

async fn login(
    State(state): State<AppState>,
    Extension(ip): Extension<ClientIp>,
    Json(body): Json<LoginBody>,
) -> ApiResult {
    let now = state.now();
    let username = auth::normalize_username(&body.username).unwrap_or_default();
    let (ip_key, user_ip_key, user_key) = (
        format!("login-ip:{}", ip.0),
        format!("login:{username}:{}", ip.0),
        format!("login:{username}"),
    );
    let mut ids = state
        .attempt(
            &[
                (throttle::LOGIN_IP, &ip_key),
                (throttle::LOGIN_USER_IP, &user_ip_key),
                (throttle::LOGIN_USER, &user_key),
            ],
            "Too many failed logins, wait 15 minutes",
        )
        .await?;
    let row = state
        .0
        .db
        .get()
        .await?
        .query_opt(
            "SELECT id, password_hash FROM users WHERE username = $1 AND active",
            &[&username],
        )
        .await?;
    let password = body.password.clone();
    let stored = row.as_ref().map(|r| r.get::<_, String>("password_hash"));
    let password_ok = argon(move || match stored {
        Some(stored) => auth::verify_password(&password, &stored),
        None => {
            auth::dummy_verify(&password);
            false
        }
    })
    .await?;
    let refused = || ApiError::unauthorized("Wrong username, password or authenticator code");
    let Some(user) = row.filter(|_| password_ok).map(|r| r.get::<_, Uuid>("id")) else {
        return Err(refused());
    };
    // Second-factor guesses are limited per user even when spread over many addresses.
    let factor_key = format!("factor:{user}");
    ids.extend(
        state
            .attempt(
                &[(throttle::SECOND_FACTOR, &factor_key)],
                "Too many wrong authenticator codes, wait an hour or use a recovery code later",
            )
            .await?,
    );
    if !second_factor(&state, user, &body.code, now).await? {
        return Err(refused());
    }
    state.forgive(&ids).await?;
    throttle::clear(&state.0.db, &user_ip_key).await?;
    let (device, token) = issue_device(
        &state.0.db,
        user,
        &device_name(body.device_name.as_deref()),
        now,
        None,
    )
    .await?;
    Ok(Json(
        json!({ "user_id": user, "device_id": device, "token": token }),
    ))
}

async fn me(device: Device) -> ApiResult {
    Ok(Json(json!({
        "user_id": device.user,
        "username": device.username,
        "is_admin": device.is_admin,
        "device_id": device.device,
    })))
}

async fn devices(State(state): State<AppState>, device: Device) -> ApiResult {
    let rows = state
        .0
        .db
        .get()
        .await?
        .query(
            "SELECT id, name, created_at, last_seen, created_by FROM devices
             WHERE user_id = $1 AND revoked_at IS NULL ORDER BY created_at",
            &[&device.user],
        )
        .await?;
    let list: Vec<Value> = rows
        .iter()
        .map(|r| {
            let id: Uuid = r.get("id");
            json!({
                "id": id,
                "name": r.get::<_, String>("name"),
                "created_at": r.get::<_, i64>("created_at"),
                "last_seen": r.get::<_, i64>("last_seen"),
                "paired_by": r.get::<_, Option<Uuid>>("created_by"),
                "current": id == device.device,
            })
        })
        .collect();
    Ok(Json(json!({ "devices": list })))
}

/// Revokes a device and every device it paired, directly or indirectly.
pub(crate) async fn revoke_tree(state: &AppState, user: Uuid, root: Uuid) -> ApiResult<Vec<Uuid>> {
    let revoked: Vec<Uuid> = state
        .0
        .db
        .get()
        .await?
        .query(
            "WITH RECURSIVE tree AS (
                SELECT id FROM devices WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL
                UNION
                SELECT d.id FROM devices d JOIN tree t ON d.created_by = t.id WHERE d.revoked_at IS NULL
             )
             UPDATE devices SET revoked_at = $3 WHERE id IN (SELECT id FROM tree) RETURNING id",
            &[&root, &user, &state.now()],
        )
        .await?
        .iter()
        .map(|r| r.get(0))
        .collect();
    for id in &revoked {
        state
            .0
            .hub
            .publish(user, json!({ "type": "device_revoked", "device_id": id }));
    }
    Ok(revoked)
}

async fn revoke_device(
    State(state): State<AppState>,
    device: Device,
    Path(id): Path<Uuid>,
) -> ApiResult {
    let revoked = revoke_tree(&state, device.user, id).await?;
    if revoked.is_empty() {
        return Err(ApiError(StatusCode::NOT_FOUND, "No such device".into()));
    }
    Ok(Json(json!({ "revoked": revoked })))
}

#[derive(Deserialize)]
struct PasswordOnly {
    password: String,
}

async fn pairing_start(
    State(state): State<AppState>,
    device: Device,
    Json(body): Json<PasswordOnly>,
) -> ApiResult {
    reauth(&state, &device, &body.password, None).await?;
    let now = state.now();
    let client = state.0.db.get().await?;
    client
        .execute(
            "DELETE FROM pairings WHERE user_id = $1 AND (used_at IS NOT NULL OR expires_at < $2)",
            &[&device.user, &now],
        )
        .await?;
    let open: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM pairings WHERE user_id = $1",
            &[&device.user],
        )
        .await?
        .get(0);
    if open >= MAX_OPEN_PAIRINGS {
        return Err(ApiError(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many open pairing codes, wait 10 minutes".into(),
        ));
    }
    let code = auth::new_short_code();
    let expires = now + PAIRING_TTL_MS;
    client
        .execute(
            "INSERT INTO pairings (code_hash, user_id, created_by_device, expires_at) VALUES ($1, $2, $3, $4)",
            &[&auth::token_hash(&code), &device.user, &device.device, &expires],
        )
        .await?;
    let uri = format!(
        "vidcue://pair?server={}&code={code}",
        crate::web::query_escape(&state.0.public_url)
    );
    Ok(Json(
        json!({ "code": code, "uri": uri, "qr_svg": qr_svg(&uri), "expires_at": expires }),
    ))
}

#[derive(Deserialize)]
struct ClaimBody {
    code: String,
    device_name: Option<String>,
}

async fn pairing_claim(
    State(state): State<AppState>,
    Extension(ip): Extension<ClientIp>,
    Json(body): Json<ClaimBody>,
) -> ApiResult {
    let now = state.now();
    let ip_key = format!("pairing:{}", ip.0);
    let ids = state
        .attempt(
            &[(throttle::CODES_IP, &ip_key)],
            "Too many attempts, try again later",
        )
        .await?;
    let code = body.code.trim().to_ascii_uppercase();
    let claimed = state
        .0
        .db
        .get()
        .await?
        .query_opt(
            "UPDATE pairings p SET used_at = $2 FROM users u
             WHERE p.code_hash = $1 AND p.used_at IS NULL AND p.expires_at > $2 AND u.id = p.user_id AND u.active
               AND EXISTS (SELECT 1 FROM devices d WHERE d.id = p.created_by_device AND d.revoked_at IS NULL)
             RETURNING p.user_id, p.created_by_device",
            &[&auth::token_hash(&code), &now],
        )
        .await?;
    let Some(claimed) = claimed else {
        return Err(ApiError::bad(
            "This pairing code is invalid, used or expired",
        ));
    };
    state.forgive(&ids).await?;
    let (user, parent): (Uuid, Uuid) = (claimed.get(0), claimed.get(1));
    let (device, token) = issue_device(
        &state.0.db,
        user,
        &device_name(body.device_name.as_deref()),
        now,
        Some(parent),
    )
    .await?;
    state.0.hub.publish(
        user,
        json!({ "type": "device_added", "device_id": device, "paired_by": parent }),
    );
    Ok(Json(
        json!({ "user_id": user, "device_id": device, "token": token }),
    ))
}

#[derive(Deserialize)]
struct PushBody {
    ops: Vec<sync::Op>,
}

async fn sync_push(
    State(state): State<AppState>,
    device: Device,
    Json(body): Json<PushBody>,
) -> ApiResult {
    if body.ops.len() > sync::MAX_OPS_PER_PUSH {
        return Err(ApiError::bad(format!(
            "Send at most {} operations per push",
            sync::MAX_OPS_PER_PUSH
        )));
    }
    // Queue this user's pushes here, so one user's burst cannot hold every pooled
    // database connection while waiting for its own advisory lock.
    let lock = state.push_lock(device.user);
    let _guard = tokio::time::timeout(Duration::from_secs(10), lock.lock())
        .await
        .map_err(|_| {
            ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "Sync busy, try again".into(),
            )
        })?;
    let (results, latest) = sync::push(
        &state.0.db,
        device.user,
        device.device,
        &body.ops,
        state.now(),
    )
    .await?;
    if let Some(seq) = latest {
        state.0.hub.publish(
            device.user,
            json!({ "type": "changed", "seq": seq, "device_id": device.device }),
        );
    }
    let results: Vec<Value> = body
        .ops
        .iter()
        .zip(results)
        .map(|(op, r)| {
            let mut v = serde_json::to_value(r).unwrap_or_default();
            v["id"] = json!(op.id);
            v
        })
        .collect();
    Ok(Json(json!({ "results": results, "seq": latest })))
}

#[derive(Deserialize)]
struct PullQuery {
    #[serde(default)]
    since: i64,
    limit: Option<i64>,
}

async fn sync_pull(
    State(state): State<AppState>,
    device: Device,
    Query(q): Query<PullQuery>,
) -> ApiResult {
    let limit = q.limit.unwrap_or(sync::MAX_PULL).clamp(1, sync::MAX_PULL);
    let changes = sync::pull(&state.0.db, device.user, q.since, limit + 1).await?;
    let more = changes.len() as i64 > limit;
    let changes = &changes[..changes.len().min(limit as usize)];
    let next = changes.last().map_or(q.since, |c| c.seq);
    Ok(Json(
        json!({ "changes": changes, "next": next, "more": more }),
    ))
}

/// A 30-second single-use ticket for clients that cannot set headers on the WebSocket
/// upgrade. Unlike the device token it is harmless once it appears in a proxy log.
async fn ws_ticket(State(state): State<AppState>, device: Device) -> ApiResult {
    let now = state.now();
    let ticket = auth::new_token(TICKET_PREFIX);
    let mut tickets = state.0.tickets.lock().expect("tickets");
    tickets.retain(|_, (_, expires)| *expires > now);
    if tickets.len() > 10_000 {
        return Err(ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "Server busy, try again".into(),
        ));
    }
    tickets.insert(
        auth::token_hash(&ticket),
        (device.device, now + TICKET_TTL_MS),
    );
    Ok(Json(
        json!({ "ticket": ticket, "expires_in_ms": TICKET_TTL_MS }),
    ))
}

#[derive(Deserialize)]
struct WsQuery {
    ticket: Option<String>,
}

/// Change notifications. Authenticate with the bearer header or a `?ticket=` from
/// `POST /api/ws/ticket`; device tokens are never accepted in the URL.
async fn ws(
    State(state): State<AppState>,
    Query(q): Query<WsQuery>,
    headers: axum::http::HeaderMap,
    upgrade: WebSocketUpgrade,
) -> ApiResult<Response> {
    let device = match (bearer(&headers), q.ticket) {
        (Some(token), _) => device_for_token(&state, token).await?,
        (None, Some(ticket)) => {
            let now = state.now();
            let entry = state
                .0
                .tickets
                .lock()
                .expect("tickets")
                .remove(&auth::token_hash(&ticket))
                .filter(|(_, expires)| *expires > now);
            let device = match entry {
                Some((id, _)) => device_by_id(&state, id).await?,
                None => None,
            };
            device.ok_or_else(|| ApiError::unauthorized("Invalid or expired ticket"))?
        }
        (None, None) => return Err(ApiError::unauthorized("Missing bearer token or ticket")),
    };
    let slot = state.0.hub.reserve_socket(device.device).ok_or_else(|| {
        ApiError(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many open connections for this device".into(),
        )
    })?;
    Ok(upgrade
        .max_message_size(MAX_WS_MESSAGE)
        .max_frame_size(MAX_WS_MESSAGE)
        .on_upgrade(move |socket| ws_session(state, device, slot, socket)))
}

async fn ws_session(state: AppState, device: Device, _slot: SocketSlot, mut socket: WebSocket) {
    let mut rx = state.0.hub.subscribe(device.user);
    let seq = sync::latest_seq(&state.0.db, device.user)
        .await
        .unwrap_or(0);
    let hello = json!({ "type": "hello", "seq": seq, "device_id": device.device });
    if socket
        .send(Message::Text(hello.to_string().into()))
        .await
        .is_err()
    {
        return;
    }
    let mut ticker = tokio::time::interval(Duration::from_secs(25));
    ticker.tick().await;
    loop {
        tokio::select! {
            event = rx.recv() => match event {
                Ok(event) => {
                    let revoked = (event["type"] == "device_revoked" && event["device_id"] == json!(device.device))
                        || (event["type"] == "devices_revoked" && event["keep"] != json!(device.device));
                    if revoked {
                        let _ = socket.send(Message::Close(None)).await;
                        return;
                    }
                    if socket.send(Message::Text(event.to_string().into())).await.is_err() { return; }
                }
                // Missed events: tell the device to pull everything newer than it has.
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let seq = sync::latest_seq(&state.0.db, device.user).await.unwrap_or(0);
                    let msg = json!({ "type": "changed", "seq": seq });
                    if socket.send(Message::Text(msg.to_string().into())).await.is_err() { return; }
                }
                Err(broadcast::error::RecvError::Closed) => return,
            },
            incoming = socket.recv() => match incoming {
                // The socket is server-to-client only; clients send nothing but control frames.
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                _ => return,
            },
            _ = ticker.tick() => {
                // Keeps proxies from timing out idle sockets and drops revoked devices.
                let active = matches!(device_by_id(&state, device.device).await, Ok(Some(_)));
                if !active || socket.send(Message::Ping(Vec::new().into())).await.is_err() {
                    let _ = socket.send(Message::Close(None)).await;
                    return;
                }
            }
        }
    }
}

#[derive(Deserialize)]
struct InviteBody {
    password: String,
    #[serde(default)]
    admin: bool,
    hours: Option<i64>,
}

async fn admin_invite(
    State(state): State<AppState>,
    device: Device,
    Json(body): Json<InviteBody>,
) -> ApiResult {
    if !device.is_admin {
        return Err(ApiError::forbidden());
    }
    reauth(&state, &device, &body.password, None).await?;
    let (url, expires) = create_invite(
        &state.0.db,
        &state.0.public_url,
        Some(device.user),
        body.admin,
        body.hours.unwrap_or(72),
        state.now(),
    )
    .await?;
    Ok(Json(
        json!({ "url": url, "qr_svg": qr_svg(&url), "expires_at": expires }),
    ))
}

async fn admin_users(State(state): State<AppState>, device: Device) -> ApiResult {
    if !device.is_admin {
        return Err(ApiError::forbidden());
    }
    let rows = state
        .0
        .db
        .get()
        .await?
        .query(
            "SELECT u.id, u.username, u.is_admin, u.created_at,
                    COUNT(d.id) FILTER (WHERE d.revoked_at IS NULL) AS devices,
                    MAX(d.last_seen) AS last_seen
             FROM users u LEFT JOIN devices d ON d.user_id = u.id
             WHERE u.active GROUP BY u.id ORDER BY u.created_at",
            &[],
        )
        .await?;
    let users: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<_, Uuid>("id"),
                "username": r.get::<_, String>("username"),
                "is_admin": r.get::<_, bool>("is_admin"),
                "created_at": r.get::<_, i64>("created_at"),
                "devices": r.get::<_, i64>("devices"),
                "last_seen": r.get::<_, Option<i64>>("last_seen"),
            })
        })
        .collect();
    Ok(Json(json!({ "users": users })))
}

/// Hourly cleanup of expired rows so no table grows without bound.
pub fn spawn_maintenance(state: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(3600));
        loop {
            tick.tick().await;
            if let Err(e) = maintenance(&state.0.db, state.now()).await {
                tracing::warn!("maintenance failed: {e:#}");
            }
        }
    });
}

pub async fn maintenance(db: &Pool, now: i64) -> anyhow::Result<()> {
    let day = 24 * 3_600_000;
    let client = db.get().await?;
    client
        .batch_execute(&format!(
            "DELETE FROM auth_failures WHERE at < {d1};
             DELETE FROM sync_applied WHERE applied_at < {d30};
             DELETE FROM pairings WHERE used_at IS NOT NULL OR expires_at < {now};
             DELETE FROM key_requests WHERE expires_at < {now};
             DELETE FROM account_resets WHERE used_at IS NOT NULL OR expires_at < {now};
             UPDATE devices SET revoked_at = {now} WHERE revoked_at IS NULL AND last_seen < {idle};
             DELETE FROM devices WHERE revoked_at < {d30};
             DELETE FROM users WHERE NOT active AND created_at < {d1};
             DELETE FROM invites WHERE used_by IS NULL AND expires_at < {d30};",
            d1 = now - day,
            d30 = now - 30 * day,
            idle = now - DEVICE_IDLE_MS,
        ))
        .await?;
    Ok(())
}
