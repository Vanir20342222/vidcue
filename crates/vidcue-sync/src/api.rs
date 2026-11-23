//! HTTP client for every server endpoint the app uses (PROTOCOL.md §2, §5, §6, B1/B2).
//!
//! Bodies and headers are never logged. Errors carry the server's `{"error"}` message,
//! which the contract keeps free of tokens, cookies and request URLs.
use crate::protocol::{HealthReport, KeyRequest, PullPage, PushOp, PushResponse, SharedPull};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::time::Duration;
use url::Url;
use uuid::Uuid;
use vidcue_core::Media;

pub const TIMEOUT: Duration = Duration::from_secs(20);
pub const MAX_BODY: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApiError {
    /// 401: the device token is invalid or revoked.
    Unauthorized,
    /// Any other HTTP error status with the server's message.
    Http { status: u16, message: String },
    /// No HTTP response (DNS, TLS, timeout, connection reset).
    Network(String),
}

impl ApiError {
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Unauthorized => Some(401),
            Self::Http { status, .. } => Some(*status),
            Self::Network(_) => None,
        }
    }
    /// Transient: retry later with backoff.
    pub fn transient(&self) -> bool {
        match self {
            Self::Network(_) => true,
            Self::Http { status, .. } => *status >= 500 || *status == 408,
            Self::Unauthorized => false,
        }
    }
    /// The server does not know this endpoint: an older server without the feature.
    pub fn not_found(&self) -> bool {
        self.status() == Some(404)
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthorized => f.write_str("This device was signed out of the server"),
            Self::Http { message, .. } => f.write_str(message),
            Self::Network(message) => write!(f, "Server unreachable: {message}"),
        }
    }
}

impl std::error::Error for ApiError {}

pub type ApiResult<T = Value> = Result<T, ApiError>;

#[derive(Clone)]
pub struct Api {
    pub base: Url,
    pub token: Option<String>,
    pub http: reqwest::Client,
}

impl std::fmt::Debug for Api {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Api")
            .field("base", &self.base.as_str())
            .field("token", &self.token.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

fn client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(TIMEOUT)
        .connect_timeout(Duration::from_secs(10))
        // Never forward the bearer token anywhere else.
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("vidcue-sync/", env!("CARGO_PKG_VERSION")))
        .build()?)
}

fn network(error: reqwest::Error) -> ApiError {
    let error = error.without_url();
    let text = if error.is_timeout() {
        "timed out".to_owned()
    } else if error.is_connect() {
        "could not connect".to_owned()
    } else {
        // reqwest's chain describes DNS/TLS causes without the URL (removed above).
        let mut text = error.to_string();
        let mut source = std::error::Error::source(&error);
        while let Some(cause) = source {
            text = format!("{text}: {cause}");
            source = cause.source();
        }
        text.chars().take(200).collect()
    };
    ApiError::Network(text)
}

impl Api {
    /// `origin` must already be canonical (see [`crate::pairing::server_origin`]).
    pub fn new(origin: &str, token: Option<String>) -> anyhow::Result<Self> {
        let base = Url::parse(origin)?;
        Ok(Self {
            base,
            token,
            http: client()?,
        })
    }

    pub fn with_token(&self, token: String) -> Self {
        Self {
            base: self.base.clone(),
            token: Some(token),
            http: self.http.clone(),
        }
    }

    fn url(&self, path: &str, query: &[(&str, String)]) -> Url {
        let mut url = self.base.clone();
        url.set_path(path);
        if !query.is_empty() {
            url.query_pairs_mut()
                .extend_pairs(query.iter().map(|(k, v)| (*k, v.as_str())));
        }
        url
    }

    /// One request; returns the JSON body (or `null` for an empty body).
    pub async fn call(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<&Value>,
    ) -> ApiResult {
        let mut request = self.http.request(method, self.url(path, query));
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(body);
        }
        let mut response = request.send().await.map_err(network)?;
        let status = response.status();
        let mut bytes = Vec::new();
        if response
            .content_length()
            .is_some_and(|n| n > MAX_BODY as u64)
        {
            return Err(ApiError::Http {
                status: status.as_u16(),
                message: "The server response is too large".into(),
            });
        }
        while let Some(chunk) = response.chunk().await.map_err(network)? {
            if bytes.len() + chunk.len() > MAX_BODY {
                return Err(ApiError::Http {
                    status: status.as_u16(),
                    message: "The server response is too large".into(),
                });
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Option<Value> = serde_json::from_slice(&bytes).ok();
        if status == StatusCode::UNAUTHORIZED && self.token.is_some() {
            return Err(ApiError::Unauthorized);
        }
        if !status.is_success() {
            let message = value
                .as_ref()
                .and_then(|v| v["error"].as_str())
                .map(|m| m.chars().take(300).collect::<String>())
                .unwrap_or_else(|| match status.as_u16() {
                    404 => "This server does not support this feature".into(),
                    429 => "Too many requests, try again later".into(),
                    s if s >= 500 => "The server is unavailable, try again later".into(),
                    s => format!("The server refused the request ({s})"),
                });
            return Err(ApiError::Http {
                status: status.as_u16(),
                message,
            });
        }
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        value.ok_or_else(|| ApiError::Http {
            status: status.as_u16(),
            message: "The server sent an invalid response".into(),
        })
    }

    async fn typed<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<&Value>,
    ) -> ApiResult<T> {
        let value = self.call(method, path, query, body).await?;
        serde_json::from_value(value).map_err(|_| ApiError::Http {
            status: 200,
            message: "The server sent an unexpected response".into(),
        })
    }

    pub async fn get(&self, path: &str) -> ApiResult {
        self.call(Method::GET, path, &[], None).await
    }
    pub async fn post(&self, path: &str, body: &Value) -> ApiResult {
        self.call(Method::POST, path, &[], Some(body)).await
    }
    pub async fn put(&self, path: &str, body: &Value) -> ApiResult {
        self.call(Method::PUT, path, &[], Some(body)).await
    }
    pub async fn patch(&self, path: &str, body: &Value) -> ApiResult {
        self.call(Method::PATCH, path, &[], Some(body)).await
    }
    pub async fn delete(&self, path: &str) -> ApiResult {
        self.call(Method::DELETE, path, &[], None).await
    }

    // ---- B1/B2 account and sync ----

    pub async fn login(
        &self,
        username: &str,
        password: &str,
        code: &str,
        device_name: &str,
    ) -> ApiResult {
        self.post(
            "/api/login",
            &json!({"username": username, "password": password, "code": code, "device_name": device_name}),
        )
        .await
    }
    pub async fn me(&self) -> ApiResult {
        self.get("/api/me").await
    }
    pub async fn devices(&self) -> ApiResult {
        self.get("/api/devices").await
    }
    pub async fn revoke_device(&self, id: Uuid) -> ApiResult {
        self.delete(&format!("/api/devices/{id}")).await
    }
    pub async fn pairing_start(&self, password: &str) -> ApiResult {
        self.post("/api/pairing", &json!({"password": password}))
            .await
    }
    pub async fn pairing_claim(&self, code: &str, device_name: &str) -> ApiResult {
        self.post(
            "/api/pairing/claim",
            &json!({"code": code, "device_name": device_name}),
        )
        .await
    }
    pub async fn push(&self, ops: &[PushOp]) -> ApiResult<PushResponse> {
        self.typed(
            Method::POST,
            "/api/sync/push",
            &[],
            Some(&json!({"ops": ops})),
        )
        .await
    }
    pub async fn pull(&self, since: i64, limit: i64) -> ApiResult<PullPage> {
        self.typed(
            Method::GET,
            "/api/sync/pull",
            &[("since", since.to_string()), ("limit", limit.to_string())],
            None,
        )
        .await
    }
    pub async fn ws_ticket(&self) -> ApiResult {
        self.post("/api/ws/ticket", &json!({})).await
    }
    pub async fn recovery_status(&self) -> ApiResult {
        self.get("/api/account/recovery-codes").await
    }
    pub async fn recovery_regenerate(&self, password: &str, code: &str) -> ApiResult {
        self.post(
            "/api/account/recovery-codes",
            &json!({"password": password, "code": code}),
        )
        .await
    }
    pub async fn change_password(
        &self,
        current_password: &str,
        code: &str,
        new_password: &str,
        sign_out_others: bool,
    ) -> ApiResult {
        self.post(
            "/api/account/password",
            &json!({
                "current_password": current_password,
                "code": code,
                "new_password": new_password,
                "sign_out_others": sign_out_others,
            }),
        )
        .await
    }
    pub async fn totp_start(&self, password: &str, code: &str) -> ApiResult {
        self.post(
            "/api/account/totp",
            &json!({"password": password, "code": code}),
        )
        .await
    }
    pub async fn totp_confirm(&self, code: &str) -> ApiResult {
        self.post("/api/account/totp/confirm", &json!({"code": code}))
            .await
    }
    pub async fn key_request_create(&self, public_key_b64: &str) -> ApiResult {
        self.post("/api/keys/requests", &json!({"public_key": public_key_b64}))
            .await
    }
    pub async fn key_requests(&self) -> ApiResult<Vec<KeyRequest>> {
        let value = self.get("/api/keys/requests").await?;
        serde_json::from_value(value["requests"].clone()).map_err(|_| ApiError::Http {
            status: 200,
            message: "The server sent an unexpected response".into(),
        })
    }
    pub async fn key_request(&self, id: Uuid) -> ApiResult {
        self.get(&format!("/api/keys/requests/{id}")).await
    }
    pub async fn key_grant(&self, id: Uuid, sealed_b64: &str) -> ApiResult {
        self.post(
            &format!("/api/keys/requests/{id}/grant"),
            &json!({"sealed_key": sealed_b64}),
        )
        .await
    }
    /// B5 capability discovery (no auth).
    pub async fn features(&self) -> ApiResult {
        self.get("/api/features").await
    }

    // ---- B4 social ----

    pub async fn friends(&self) -> ApiResult {
        self.get("/api/friends").await
    }
    pub async fn friend_request(&self, username: &str) -> ApiResult {
        self.post("/api/friends/requests", &json!({"username": username}))
            .await
    }
    pub async fn friend_accept(&self, user: Uuid) -> ApiResult {
        self.post(&format!("/api/friends/{user}/accept"), &json!({}))
            .await
    }
    pub async fn friend_remove(&self, user: Uuid) -> ApiResult {
        self.delete(&format!("/api/friends/{user}")).await
    }
    pub async fn friends_activity(&self) -> ApiResult {
        self.get("/api/friends/activity").await
    }
    pub async fn social_settings(&self) -> ApiResult {
        self.get("/api/social/settings").await
    }
    pub async fn set_social_settings(&self, share_activity: &str) -> ApiResult {
        self.put(
            "/api/social/settings",
            &json!({"share_activity": share_activity}),
        )
        .await
    }
    pub async fn inbox(&self, unread_only: bool) -> ApiResult {
        let query = if unread_only {
            vec![("unread_only", "true".to_owned())]
        } else {
            Vec::new()
        };
        self.call(Method::GET, "/api/inbox", &query, None).await
    }
    pub async fn inbox_send(&self, to: Uuid, media: &Media, note: &str) -> ApiResult {
        self.post(
            "/api/inbox",
            &json!({"to": to, "media": media, "note": note}),
        )
        .await
    }
    pub async fn inbox_read(&self, id: Uuid) -> ApiResult {
        self.post(&format!("/api/inbox/{id}/read"), &json!({}))
            .await
    }
    pub async fn inbox_delete(&self, id: Uuid) -> ApiResult {
        self.delete(&format!("/api/inbox/{id}")).await
    }
    pub async fn shared_playlists(&self) -> ApiResult {
        self.get("/api/shared-playlists").await
    }
    pub async fn shared_create(&self, id: Uuid, title: &str) -> ApiResult {
        self.post("/api/shared-playlists", &json!({"id": id, "title": title}))
            .await
    }
    pub async fn shared_rename(&self, id: Uuid, title: &str) -> ApiResult {
        self.patch(
            &format!("/api/shared-playlists/{id}"),
            &json!({"title": title}),
        )
        .await
    }
    pub async fn shared_delete(&self, id: Uuid) -> ApiResult {
        self.delete(&format!("/api/shared-playlists/{id}")).await
    }
    pub async fn shared_add_member(&self, id: Uuid, user: Uuid) -> ApiResult {
        self.post(
            &format!("/api/shared-playlists/{id}/members"),
            &json!({"user_id": user}),
        )
        .await
    }
    pub async fn shared_remove_member(&self, id: Uuid, user: Uuid) -> ApiResult {
        self.delete(&format!("/api/shared-playlists/{id}/members/{user}"))
            .await
    }
    /// `ops`: `{id, entry_id, value, clock}`.
    pub async fn shared_push(&self, id: Uuid, ops: &[Value]) -> ApiResult<PushResponse> {
        self.typed(
            Method::POST,
            &format!("/api/shared-playlists/{id}/push"),
            &[],
            Some(&json!({"ops": ops})),
        )
        .await
    }
    pub async fn shared_pull(&self, id: Uuid, since: i64, limit: i64) -> ApiResult<SharedPull> {
        self.typed(
            Method::GET,
            &format!("/api/shared-playlists/{id}/pull"),
            &[("since", since.to_string()), ("limit", limit.to_string())],
            None,
        )
        .await
    }

    // ---- B4 Jam ----

    pub async fn jam_create(&self, body: &Value) -> ApiResult {
        self.post("/api/jam", body).await
    }
    pub async fn jam_current(&self) -> ApiResult {
        self.get("/api/jam").await
    }
    pub async fn jam_room(&self, room: Uuid) -> ApiResult {
        self.get(&format!("/api/jam/{room}")).await
    }
    pub async fn jam_update(&self, room: Uuid, body: &Value) -> ApiResult {
        self.patch(&format!("/api/jam/{room}"), body).await
    }
    pub async fn jam_end(&self, room: Uuid) -> ApiResult {
        self.delete(&format!("/api/jam/{room}")).await
    }
    pub async fn jam_invite(&self, room: Uuid, users: &[Uuid]) -> ApiResult {
        self.post(
            &format!("/api/jam/{room}/invite"),
            &json!({"user_ids": users}),
        )
        .await
    }
    /// `join`, `decline` or `leave`.
    pub async fn jam_member_action(&self, room: Uuid, action: &str) -> ApiResult {
        debug_assert!(matches!(action, "join" | "decline" | "leave"));
        self.post(&format!("/api/jam/{room}/{action}"), &json!({}))
            .await
    }
    pub async fn jam_kick(&self, room: Uuid, user: Uuid) -> ApiResult {
        self.delete(&format!("/api/jam/{room}/members/{user}"))
            .await
    }
    pub async fn jam_queue_add(&self, room: Uuid, media: &Media) -> ApiResult {
        self.post(&format!("/api/jam/{room}/queue"), &json!({"media": media}))
            .await
    }
    pub async fn jam_queue_remove(&self, room: Uuid, item: Uuid) -> ApiResult {
        self.delete(&format!("/api/jam/{room}/queue/{item}")).await
    }
    pub async fn jam_vote(&self, room: Uuid, item: Uuid, up: bool) -> ApiResult {
        self.post(
            &format!("/api/jam/{room}/queue/{item}/vote"),
            &json!({"up": up}),
        )
        .await
    }
    pub async fn jam_playback(&self, room: Uuid, action: &Value) -> ApiResult {
        self.post(&format!("/api/jam/{room}/playback"), action)
            .await
    }

    // ---- B5 personal ----

    pub async fn health_post(&self, reports: &[HealthReport]) -> ApiResult {
        self.post("/api/health/reports", &json!({"reports": reports}))
            .await
    }
    pub async fn health_summary(&self, days: u32) -> ApiResult {
        self.call(
            Method::GET,
            "/api/health/summary",
            &[("days", days.to_string())],
            None,
        )
        .await
    }
    pub async fn stats(&self, from: i64, to: i64, tz_offset_min: i32) -> ApiResult {
        self.call(
            Method::GET,
            "/api/stats",
            &[
                ("from", from.to_string()),
                ("to", to.to_string()),
                ("tz_offset_min", tz_offset_min.to_string()),
            ],
            None,
        )
        .await
    }
    pub async fn stats_wrapped(&self, year: i32, tz_offset_min: i32) -> ApiResult {
        self.call(
            Method::GET,
            "/api/stats/wrapped",
            &[
                ("year", year.to_string()),
                ("tz_offset_min", tz_offset_min.to_string()),
            ],
            None,
        )
        .await
    }
    pub async fn recs(&self) -> ApiResult {
        self.get("/api/recs").await
    }
    pub async fn recs_settings(&self) -> ApiResult {
        self.get("/api/recs/settings").await
    }
    pub async fn set_recs_settings(&self, body: &Value) -> ApiResult {
        self.put("/api/recs/settings", body).await
    }
    pub async fn recs_refresh(&self) -> ApiResult {
        self.post("/api/recs/refresh", &json!({})).await
    }

    // ---- admin ----

    pub async fn admin_users(&self) -> ApiResult {
        self.get("/api/admin/users").await
    }
    pub async fn admin_invite(&self, password: &str, admin: bool, hours: i64) -> ApiResult {
        self.post(
            "/api/admin/invites",
            &json!({"password": password, "admin": admin, "hours": hours}),
        )
        .await
    }
    pub async fn admin_reset(&self, user: Uuid, password: &str, code: &str) -> ApiResult {
        self.post(
            &format!("/api/admin/users/{user}/reset"),
            &json!({"password": password, "code": code}),
        )
        .await
    }
    pub async fn admin_devices(&self, user: Uuid) -> ApiResult {
        self.get(&format!("/api/admin/users/{user}/devices")).await
    }
    pub async fn admin_revoke(
        &self,
        user: Uuid,
        password: &str,
        device: Option<Uuid>,
    ) -> ApiResult {
        self.post(
            &format!("/api/admin/users/{user}/devices/revoke"),
            &json!({"password": password, "device_id": device}),
        )
        .await
    }
    /// `disable` or `enable`.
    pub async fn admin_set_enabled(&self, user: Uuid, password: &str, enabled: bool) -> ApiResult {
        let action = if enabled { "enable" } else { "disable" };
        self.post(
            &format!("/api/admin/users/{user}/{action}"),
            &json!({"password": password}),
        )
        .await
    }
    pub async fn admin_health(&self, days: u32) -> ApiResult {
        self.call(
            Method::GET,
            "/api/admin/health",
            &[("days", days.to_string())],
            None,
        )
        .await
    }
}
