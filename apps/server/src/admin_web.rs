//! Administration: JSON endpoints for the app and a small server-rendered `/admin` page.
//!
//! The page uses no JavaScript: plain forms, a session cookie scoped to `/admin`, a
//! per-session CSRF token, Origin / Sec-Fetch-Site checks and a CSP that only allows the
//! page's own stylesheet (pinned by hash). It never shows device tokens, authenticator
//! secrets or recovery codes.

use crate::api::{
    self, ApiError, ApiResult, AppState, ClientIp, Device, argon, qr_svg, reauth, revoke_tree,
    second_factor,
};
use crate::auth;
use crate::throttle;
use axum::extract::rejection::{FormRejection, JsonRejection, PathRejection};
use axum::extract::{Extension, Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Json, Router};
use deadpool_postgres::Pool;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

pub const COOKIE: &str = "vidcue_admin";
pub const SESSION_PREFIX: &str = "vca_";
pub const SESSION_IDLE_MS: i64 = 30 * 60 * 1000;
pub const SESSION_MAX_MS: i64 = 8 * 3_600_000;
pub const MAX_SESSIONS_PER_ADMIN: i64 = 3;
/// Days of playback health shown on the page.
pub const HEALTH_DAYS: i64 = 7;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/users/{id}/devices", get(json_devices))
        .route("/api/admin/users/{id}/devices/revoke", post(json_revoke))
        .route("/api/admin/users/{id}/disable", post(json_disable))
        .route("/api/admin/users/{id}/enable", post(json_enable))
        .route("/admin", get(dashboard))
        .route("/admin/login", post(login))
        .route("/admin/logout", post(logout))
        .route("/admin/invites", post(web_invite))
        .route("/admin/users/{id}/reset", post(web_reset))
        .route("/admin/users/{id}/revoke", post(web_revoke))
        .route("/admin/users/{id}/disable", post(web_disable))
        .route("/admin/users/{id}/enable", post(web_enable))
}

fn not_found() -> ApiError {
    ApiError(StatusCode::NOT_FOUND, "No such user".into())
}

fn conflict(msg: &str) -> ApiError {
    ApiError(StatusCode::CONFLICT, msg.into())
}

// ---- Shared actions ----

async fn user_state(db: &Pool, user: Uuid) -> ApiResult<Option<bool>> {
    Ok(db
        .get()
        .await?
        .query_opt(
            "SELECT disabled_at IS NOT NULL FROM users WHERE id = $1 AND active",
            &[&user],
        )
        .await?
        .map(|r| r.get(0)))
}

/// Revokes one device tree, or every device of `user` when `device` is `None`.
pub async fn revoke_devices(
    state: &AppState,
    user: Uuid,
    device: Option<Uuid>,
) -> ApiResult<Vec<Uuid>> {
    if user_state(&state.0.db, user).await?.is_none() {
        return Err(not_found());
    }
    if let Some(device) = device {
        let revoked = revoke_tree(state, user, device).await?;
        if revoked.is_empty() {
            return Err(ApiError(StatusCode::NOT_FOUND, "No such device".into()));
        }
        return Ok(revoked);
    }
    let roots: Vec<Uuid> = state
        .0
        .db
        .get()
        .await?
        .query(
            "SELECT id FROM devices WHERE user_id = $1 AND revoked_at IS NULL ORDER BY created_at",
            &[&user],
        )
        .await?
        .iter()
        .map(|r| r.get(0))
        .collect();
    let mut revoked = Vec::new();
    for root in roots {
        if !revoked.contains(&root) {
            revoked.extend(revoke_tree(state, user, root).await?);
        }
    }
    // "Sign out everywhere" also ends the user's /admin cookie sessions.
    end_sessions(&state.0.db.get().await?, user).await?;
    state
        .0
        .hub
        .publish(user, json!({ "type": "devices_revoked" }));
    Ok(revoked)
}

/// Ends every /admin cookie session of `user`. Runs with device revocation, password
/// and authenticator changes, account resets and disabling.
pub async fn end_sessions(
    client: &impl deadpool_postgres::GenericClient,
    user: Uuid,
) -> Result<u64, tokio_postgres::Error> {
    client
        .execute("DELETE FROM admin_sessions WHERE user_id = $1", &[&user])
        .await
}

/// Disables `target`: refused for oneself and for the last enabled admin. Signs out
/// everywhere and drops the user's open pairings, resets, key requests and admin sessions.
pub async fn disable_user(state: &AppState, actor: Uuid, target: Uuid) -> ApiResult<()> {
    if actor == target {
        return Err(conflict("You cannot disable your own account"));
    }
    let now = state.now();
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    // Locks every enabled admin, so two admins cannot disable each other at once.
    let admins: Vec<Uuid> = tx
        .query(
            "SELECT id FROM users WHERE is_admin AND active AND disabled_at IS NULL ORDER BY id FOR UPDATE",
            &[],
        )
        .await?
        .iter()
        .map(|r| r.get(0))
        .collect();
    if !admins.contains(&actor) {
        return Err(ApiError::forbidden());
    }
    let exists = tx
        .query_opt(
            "SELECT 1 FROM users WHERE id = $1 AND active FOR UPDATE",
            &[&target],
        )
        .await?
        .is_some();
    if !exists {
        return Err(not_found());
    }
    if admins.contains(&target) && admins.len() < 2 {
        return Err(conflict("Keep at least one enabled admin"));
    }
    tx.execute(
        "UPDATE users SET disabled_at = $2 WHERE id = $1 AND disabled_at IS NULL",
        &[&target, &now],
    )
    .await?;
    tx.execute(
        "UPDATE devices SET revoked_at = $2 WHERE user_id = $1 AND revoked_at IS NULL",
        &[&target, &now],
    )
    .await?;
    end_sessions(&tx, target).await?;
    for table in ["pairings", "account_resets", "key_requests"] {
        tx.execute(
            &format!("DELETE FROM {table} WHERE user_id = $1"),
            &[&target],
        )
        .await?;
    }
    tx.commit().await?;
    state
        .0
        .hub
        .publish(target, json!({ "type": "devices_revoked" }));
    Ok(())
}

pub async fn enable_user(state: &AppState, target: Uuid) -> ApiResult<()> {
    let updated = state
        .0
        .db
        .get()
        .await?
        .execute(
            "UPDATE users SET disabled_at = NULL WHERE id = $1 AND active",
            &[&target],
        )
        .await?;
    if updated == 0 {
        return Err(not_found());
    }
    Ok(())
}

// ---- JSON API ----

fn admin(device: &Device) -> ApiResult<()> {
    if device.is_admin {
        Ok(())
    } else {
        Err(ApiError::forbidden())
    }
}

fn path_id(path: Result<Path<Uuid>, PathRejection>) -> ApiResult<Uuid> {
    path.map(|Path(id)| id)
        .map_err(|_| ApiError::bad("The user id must be a UUID"))
}

fn json_body<T>(body: Result<Json<T>, JsonRejection>) -> ApiResult<T> {
    body.map(|Json(b)| b)
        .map_err(|_| ApiError::bad("Invalid request body"))
}

async fn json_devices(
    State(state): State<AppState>,
    device: Device,
    path: Result<Path<Uuid>, PathRejection>,
) -> ApiResult {
    admin(&device)?;
    let user = path_id(path)?;
    if user_state(&state.0.db, user).await?.is_none() {
        return Err(not_found());
    }
    let devices: Vec<Value> = state
        .0
        .db
        .get()
        .await?
        .query(
            "SELECT id, name, created_at, last_seen, created_by FROM devices
             WHERE user_id = $1 AND revoked_at IS NULL ORDER BY created_at",
            &[&user],
        )
        .await?
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<_, Uuid>("id"),
                "name": r.get::<_, String>("name"),
                "created_at": r.get::<_, i64>("created_at"),
                "last_seen": r.get::<_, i64>("last_seen"),
                "paired_by": r.get::<_, Option<Uuid>>("created_by"),
            })
        })
        .collect();
    Ok(Json(json!({ "devices": devices })))
}

#[derive(Deserialize)]
struct RevokeBody {
    password: String,
    #[serde(default)]
    device_id: Option<Uuid>,
}

async fn json_revoke(
    State(state): State<AppState>,
    device: Device,
    path: Result<Path<Uuid>, PathRejection>,
    body: Result<Json<RevokeBody>, JsonRejection>,
) -> ApiResult {
    admin(&device)?;
    let (user, body) = (path_id(path)?, json_body(body)?);
    reauth(&state, &device, &body.password, None).await?;
    let revoked = revoke_devices(&state, user, body.device_id).await?;
    Ok(Json(json!({ "revoked": revoked })))
}

#[derive(Deserialize)]
struct PasswordBody {
    password: String,
}

async fn json_disable(
    State(state): State<AppState>,
    device: Device,
    path: Result<Path<Uuid>, PathRejection>,
    body: Result<Json<PasswordBody>, JsonRejection>,
) -> ApiResult {
    admin(&device)?;
    let (user, body) = (path_id(path)?, json_body(body)?);
    reauth(&state, &device, &body.password, None).await?;
    disable_user(&state, device.user, user).await?;
    Ok(Json(json!({ "disabled": true })))
}

async fn json_enable(
    State(state): State<AppState>,
    device: Device,
    path: Result<Path<Uuid>, PathRejection>,
    body: Result<Json<PasswordBody>, JsonRejection>,
) -> ApiResult {
    admin(&device)?;
    let (user, body) = (path_id(path)?, json_body(body)?);
    reauth(&state, &device, &body.password, None).await?;
    enable_user(&state, user).await?;
    Ok(Json(json!({ "disabled": false })))
}

// ---- Web page: rendering ----

/// HTML-escapes text for element content and quoted attribute values.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// Constant-time comparison of two secrets.
pub fn same_secret(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

const STYLE: &str = r#"
:root { --bg:#0f0f12; --card:#1a1a20; --text:#f2f2f5; --muted:#a0a0ab; --accent:#ff3d5a; --border:#2c2c35; --error:#ff7a8a; --ok:#4ad295; }
@media (prefers-color-scheme: light) { :root { --bg:#f6f6f8; --card:#fff; --text:#16161a; --muted:#5d5d68; --border:#dcdce3; --error:#c0182f; --ok:#0f7a4d; } }
* { box-sizing:border-box; }
body { margin:0; background:var(--bg); color:var(--text); font:15px/1.5 system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; padding:16px; }
main { max-width:960px; margin:0 auto; display:grid; gap:16px; }
main.narrow { max-width:380px; margin-top:12vh; }
section { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:18px 20px; overflow-x:auto; }
header { display:flex; justify-content:space-between; align-items:center; gap:12px; }
h1 { margin:0; font-size:22px; } h2 { margin:0 0 10px; font-size:16px; color:var(--muted); font-weight:600; }
table { width:100%; border-collapse:collapse; font-size:14px; }
th, td { text-align:left; padding:8px 6px; border-bottom:1px solid var(--border); vertical-align:top; }
th { color:var(--muted); font-weight:500; }
label { display:block; font-size:13px; color:var(--muted); margin:10px 0 4px; }
label.check { display:flex; gap:8px; align-items:center; }
input { width:100%; padding:10px 12px; border-radius:10px; border:1px solid var(--border); background:transparent; color:var(--text); font:inherit; }
input[type=checkbox] { width:auto; }
button { margin-top:12px; padding:9px 16px; border:0; border-radius:999px; background:var(--accent); color:#fff; font:600 14px system-ui, sans-serif; cursor:pointer; }
button.quiet { background:transparent; color:var(--text); border:1px solid var(--border); margin:0; }
details { margin:2px 0; } summary { cursor:pointer; color:var(--muted); }
details form { display:grid; gap:2px; max-width:320px; padding:6px 0 10px; }
.notice { padding:12px 14px; border-radius:12px; border:1px solid var(--border); }
.notice.error { color:var(--error); } .notice.ok { color:var(--ok); }
.link { word-break:break-all; font:13px ui-monospace, monospace; color:var(--text); }
.qr { background:#fff; border-radius:12px; padding:8px; width:fit-content; margin-top:10px; }
.qr svg { display:block; width:180px; height:180px; }
.tag { font-size:12px; padding:2px 8px; border-radius:999px; border:1px solid var(--border); color:var(--muted); }
"#;

/// `sha256-…` source expression of the page's only stylesheet.
pub fn style_hash() -> &'static str {
    static HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HASH.get_or_init(|| {
        use sha2::Digest;
        let digest = sha2::Sha256::digest(STYLE.as_bytes());
        format!("sha256-{}", data_encoding::BASE64.encode(&digest))
    })
}

fn csp() -> String {
    format!(
        "default-src 'none'; style-src '{}'; img-src data:; form-action 'self'; base-uri 'none'; frame-ancestors 'none'",
        style_hash()
    )
}

fn document(main_class: &str, body: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <meta name=\"robots\" content=\"noindex\">\n<title>Vidcue admin</title>\n\
         <style>{STYLE}</style>\n</head>\n<body>\n<main class=\"{main_class}\">\n{body}\n</main>\n</body>\n</html>\n"
    )
}

fn cookie(token: &str, max_age: i64) -> String {
    format!("{COOKIE}={token}; Path=/admin; HttpOnly; Secure; SameSite=Strict; Max-Age={max_age}")
}

fn respond(status: StatusCode, html: String, set_cookie: Option<String>) -> Response {
    let mut res = (status, axum::response::Html(html)).into_response();
    let headers = res.headers_mut();
    for (name, value) in [
        (header::CACHE_CONTROL, "no-store".to_owned()),
        (header::REFERRER_POLICY, "no-referrer".into()),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
        (header::CONTENT_SECURITY_POLICY, csp()),
    ] {
        headers.insert(name, HeaderValue::from_str(&value).expect("ASCII header"));
    }
    if let Some(cookie) = set_cookie
        && let Ok(value) = HeaderValue::from_str(&cookie)
    {
        headers.insert(header::SET_COOKIE, value);
    }
    res
}

fn login_page(status: StatusCode, error: Option<&str>, clear_cookie: bool) -> Response {
    let error = error.map_or(String::new(), |e| {
        format!("<p class=\"notice error\" role=\"alert\">{}</p>", escape(e))
    });
    let body = format!(
        "<section>\n<h1>Vidcue admin</h1>\n{error}\n<form method=\"post\" action=\"/admin/login\">\n\
         <label for=\"username\">Username</label><input id=\"username\" name=\"username\" autocomplete=\"username\" autocapitalize=\"none\" spellcheck=\"false\" required>\n\
         <label for=\"password\">Password</label><input id=\"password\" name=\"password\" type=\"password\" autocomplete=\"current-password\" required>\n\
         <label for=\"code\">Code</label><input id=\"code\" name=\"code\" autocomplete=\"one-time-code\" inputmode=\"numeric\" required>\n\
         <button type=\"submit\">Sign in</button>\n</form>\n</section>"
    );
    respond(
        status,
        document("narrow", &body),
        clear_cookie.then(|| cookie("", 0)),
    )
}

fn message_page(status: StatusCode, text: &str) -> Response {
    let body = format!(
        "<section><h1>Vidcue admin</h1><p class=\"notice error\" role=\"alert\">{}</p>\
         <p><a href=\"/admin\">Back</a></p></section>",
        escape(text)
    );
    respond(status, document("narrow", &body), None)
}

/// When a time in ms is shown.
fn when(ms: Option<i64>) -> String {
    ms.map_or("—".into(), |ms| {
        let minutes = ms.div_euclid(60_000);
        let day = minutes.div_euclid(24 * 60);
        let rem = minutes.rem_euclid(24 * 60);
        format!(
            "{} {:02}:{:02}",
            crate::stats::day_string(day),
            rem / 60,
            rem % 60
        )
    })
}

/// Outcome of an action, shown above the dashboard. `link` is shown once with its QR.
struct Notice {
    ok: bool,
    text: String,
    link: Option<String>,
}

fn password_field(id: &str) -> String {
    format!(
        "<label for=\"{id}\">Password</label><input id=\"{id}\" name=\"password\" type=\"password\" autocomplete=\"current-password\" required>"
    )
}

fn action_form(session: &Session, action: &str, label: &str, fields: &str) -> String {
    format!(
        "<form method=\"post\" action=\"{}\"><input type=\"hidden\" name=\"csrf\" value=\"{}\">{fields}<button type=\"submit\">{}</button></form>",
        escape(action),
        escape(&session.csrf),
        escape(label)
    )
}

fn health_section(health: &Value) -> String {
    let mut rows = String::new();
    if let Some(kinds) = health["by_kind"].as_object() {
        for (kind, v) in kinds {
            let rate = v["ok_rate"]
                .as_f64()
                .map_or("—".into(), |r| format!("{:.1}%", r * 100.0));
            let ok = v["ok"].as_i64().map_or("—".into(), |n| n.to_string());
            rows.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape(kind),
                v["total"].as_i64().unwrap_or(0),
                ok,
                rate
            ));
        }
    }
    let mut problems = String::new();
    for p in health["top_categories"].as_array().into_iter().flatten() {
        problems.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape(p["kind"].as_str().unwrap_or_default()),
            escape(p["category"].as_str().unwrap_or_default()),
            p["count"].as_i64().unwrap_or(0)
        ));
    }
    let mut versions = String::new();
    for v in health["by_version"].as_array().into_iter().flatten() {
        let rate = v["ok_rate"]
            .as_f64()
            .map_or("—".into(), |r| format!("{:.1}%", r * 100.0));
        versions.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{rate}</td></tr>",
            escape(v["client_version"].as_str().unwrap_or_default()),
            v["total"].as_i64().unwrap_or(0)
        ));
    }
    let ms = |p: &str| {
        health["start_ms"][p]
            .as_i64()
            .map_or("—".into(), |n| format!("{n} ms"))
    };
    format!(
        "<section>\n<h2>Playback health · {HEALTH_DAYS} days · {} reports · {} users · start p50 {} p90 {}</h2>\n\
         <table><thead><tr><th>Kind</th><th>Total</th><th>OK</th><th>OK rate</th></tr></thead><tbody>{rows}</tbody></table>\n\
         <table><thead><tr><th>Kind</th><th>Problem</th><th>Count</th></tr></thead><tbody>{problems}</tbody></table>\n\
         <table><thead><tr><th>Version</th><th>Total</th><th>OK rate</th></tr></thead><tbody>{versions}</tbody></table>\n</section>",
        health["total"].as_i64().unwrap_or(0),
        health["users_reporting"].as_i64().unwrap_or(0),
        ms("p50"),
        ms("p90"),
    )
}

async fn dashboard_page(
    state: &AppState,
    session: &Session,
    status: StatusCode,
    notice: Option<Notice>,
) -> ApiResult<Response> {
    let rows = state
        .0
        .db
        .get()
        .await?
        .query(
            "SELECT u.id, u.username, u.is_admin, u.disabled_at,
                    COUNT(d.id) FILTER (WHERE d.revoked_at IS NULL) AS devices,
                    MAX(d.last_seen) FILTER (WHERE d.revoked_at IS NULL) AS last_seen
             FROM users u LEFT JOIN devices d ON d.user_id = u.id
             WHERE u.active GROUP BY u.id ORDER BY u.username",
            &[],
        )
        .await?;
    let mut users = String::new();
    for r in &rows {
        let id: Uuid = r.get("id");
        let disabled = r.get::<_, Option<i64>>("disabled_at").is_some();
        let base = format!("/admin/users/{id}");
        let toggle = if disabled {
            action_form(
                session,
                &format!("{base}/enable"),
                "Enable",
                &password_field(&format!("en-{id}")),
            )
        } else {
            action_form(
                session,
                &format!("{base}/disable"),
                "Disable",
                &password_field(&format!("di-{id}")),
            )
        };
        let reset = if disabled {
            String::new()
        } else {
            action_form(
                session,
                &format!("{base}/reset"),
                "Reset link",
                &format!(
                    "{}<label for=\"rc-{id}\">Code</label><input id=\"rc-{id}\" name=\"code\" autocomplete=\"one-time-code\" required>",
                    password_field(&format!("rp-{id}"))
                ),
            )
        };
        let revoke = action_form(
            session,
            &format!("{base}/revoke"),
            "Sign out all devices",
            &password_field(&format!("rv-{id}")),
        );
        users.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td>\
             <td><details><summary>Manage</summary>{reset}{revoke}{toggle}</details></td></tr>",
            escape(r.get::<_, &str>("username")),
            if r.get::<_, bool>("is_admin") {
                "<span class=\"tag\">admin</span>"
            } else {
                ""
            },
            r.get::<_, i64>("devices"),
            when(r.get::<_, Option<i64>>("last_seen")),
            if disabled {
                "<span class=\"tag\">disabled</span>"
            } else {
                ""
            },
        ));
    }
    let notice = notice.map_or(String::new(), |n| {
        let link = n.link.map_or(String::new(), |link| {
            format!(
                "<p class=\"link\">{}</p><div class=\"qr\">{}</div>",
                escape(&link),
                qr_svg(&link)
            )
        });
        format!(
            "<section class=\"notice {}\" role=\"status\"><p>{}</p>{link}</section>",
            if n.ok { "ok" } else { "error" },
            escape(&n.text)
        )
    });
    let health = crate::health::admin_aggregate(&state.0.db, HEALTH_DAYS, state.now()).await?;
    let invite = action_form(
        session,
        "/admin/invites",
        "Create invite",
        &format!(
            "{}<label class=\"check\"><input type=\"checkbox\" name=\"admin\" value=\"on\"> Admin</label>\
             <label for=\"inv-hours\">Hours</label><input id=\"inv-hours\" name=\"hours\" type=\"number\" min=\"1\" max=\"720\" value=\"72\">",
            password_field("inv-pw")
        ),
    );
    let logout = format!(
        "<form method=\"post\" action=\"/admin/logout\"><input type=\"hidden\" name=\"csrf\" value=\"{}\"><button class=\"quiet\" type=\"submit\">Sign out</button></form>",
        escape(&session.csrf)
    );
    let body = format!(
        "<header><h1>Vidcue admin</h1><div><span class=\"tag\">{}</span> {logout}</div></header>\n{notice}\n\
         <section><h2>Users</h2><table><thead><tr><th>User</th><th></th><th>Devices</th><th>Last seen (UTC)</th><th></th><th></th></tr></thead>\
         <tbody>{users}</tbody></table></section>\n\
         <section><h2>Invite</h2><details><summary>New invite link</summary>{invite}</details></section>\n{}",
        escape(&session.username),
        health_section(&health),
    );
    Ok(respond(
        status,
        document("", &body),
        Some(cookie(&session.token, SESSION_IDLE_MS / 1000)),
    ))
}

// ---- Web page: sessions and request checks ----

struct Session {
    token: String,
    user: Uuid,
    username: String,
    csrf: String,
}

impl Session {
    /// `reauth` only needs the user; the page has no device of its own.
    fn actor(&self) -> Device {
        Device {
            user: self.user,
            device: Uuid::nil(),
            username: self.username.clone(),
            is_admin: true,
        }
    }
}

fn session_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, value)| *name == COOKIE && value.starts_with(SESSION_PREFIX))
        .map(|(_, value)| value.to_owned())
}

/// The valid session of an enabled admin, touching its idle timer.
async fn session(state: &AppState, headers: &HeaderMap) -> ApiResult<Option<Session>> {
    let Some(token) = session_token(headers) else {
        return Ok(None);
    };
    let now = state.now();
    let row = state
        .0
        .db
        .get()
        .await?
        .query_opt(
            "UPDATE admin_sessions s SET last_used = $2 FROM users u
             WHERE s.token_hash = $1 AND s.expires_at > $2 AND s.last_used > $2 - $3::BIGINT
               AND u.id = s.user_id AND u.active AND u.is_admin AND u.disabled_at IS NULL
             RETURNING s.user_id, s.csrf, u.username",
            &[&auth::token_hash(&token), &now, &SESSION_IDLE_MS],
        )
        .await?;
    Ok(row.map(|r| Session {
        token,
        user: r.get(0),
        csrf: r.get(1),
        username: r.get(2),
    }))
}

/// Refuses cross-site form posts: a present `Origin` must be the public origin, and a
/// present `Sec-Fetch-Site` must say `same-origin`.
fn same_origin(state: &AppState, headers: &HeaderMap) -> bool {
    let expected = reqwest::Url::parse(&state.0.public_url)
        .map(|u| u.origin().ascii_serialization())
        .unwrap_or_default();
    let origin_ok = headers
        .get(header::ORIGIN)
        .is_none_or(|o| o.to_str().is_ok_and(|o| o == expected));
    let site_ok = headers
        .get("sec-fetch-site")
        .is_none_or(|s| s.to_str().is_ok_and(|s| s == "same-origin"));
    origin_ok && site_ok
}

#[derive(Deserialize)]
struct ActionForm {
    #[serde(default)]
    csrf: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    code: String,
    #[serde(default)]
    admin: Option<String>,
    #[serde(default)]
    hours: Option<String>,
}

/// Session plus CSRF and origin checks for every POST except login.
async fn checked(
    state: &AppState,
    headers: &HeaderMap,
    form: &Result<Form<ActionForm>, FormRejection>,
) -> Result<Session, Box<Response>> {
    if !same_origin(state, headers) {
        return Err(Box::new(message_page(
            StatusCode::FORBIDDEN,
            "Request refused",
        )));
    }
    let session = match session(state, headers).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return Err(Box::new(login_page(
                StatusCode::FORBIDDEN,
                Some("Sign in again"),
                true,
            )));
        }
        Err(e) => return Err(Box::new(e.into_response())),
    };
    let csrf_ok = form
        .as_ref()
        .is_ok_and(|Form(f)| same_secret(&f.csrf, &session.csrf));
    if !csrf_ok {
        return Err(Box::new(message_page(
            StatusCode::FORBIDDEN,
            "Request refused",
        )));
    }
    Ok(session)
}

async fn dashboard(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let had_cookie = session_token(&headers).is_some();
    match session(&state, &headers).await {
        Ok(Some(session)) => dashboard_page(&state, &session, StatusCode::OK, None)
            .await
            .unwrap_or_else(IntoResponse::into_response),
        Ok(None) => login_page(StatusCode::OK, None, had_cookie),
        Err(e) => e.into_response(),
    }
}

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
    code: String,
}

async fn login(
    State(state): State<AppState>,
    Extension(ip): Extension<ClientIp>,
    headers: HeaderMap,
    form: Result<Form<LoginForm>, FormRejection>,
) -> Response {
    if !same_origin(&state, &headers) {
        return message_page(StatusCode::FORBIDDEN, "Request refused");
    }
    let Ok(Form(form)) = form else {
        return login_page(StatusCode::BAD_REQUEST, Some("Fill in every field"), false);
    };
    match login_inner(&state, ip, form).await {
        Ok(Some(token)) => {
            let mut res = respond(
                StatusCode::SEE_OTHER,
                String::new(),
                Some(cookie(&token, SESSION_IDLE_MS / 1000)),
            );
            res.headers_mut()
                .insert(header::LOCATION, HeaderValue::from_static("/admin"));
            res
        }
        Ok(None) => login_page(
            StatusCode::UNAUTHORIZED,
            Some("Wrong username, password or authenticator code"),
            false,
        ),
        Err(ApiError(status, msg)) => login_page(status, Some(&msg), false),
    }
}

/// Returns a new session token, or `None` for the generic failure.
async fn login_inner(state: &AppState, ip: ClientIp, form: LoginForm) -> ApiResult<Option<String>> {
    let now = state.now();
    let username = auth::normalize_username(&form.username).unwrap_or_default();
    let (ip_key, user_ip_key, user_key) = (
        // The same keys as /api/login: both entry points share one budget.
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
            "Too many failed sign-ins, wait 15 minutes",
        )
        .await?;
    let row = state
        .0
        .db
        .get()
        .await?
        .query_opt(
            "SELECT id, password_hash, is_admin FROM users
             WHERE username = $1 AND active AND disabled_at IS NULL",
            &[&username],
        )
        .await?;
    let stored = row.as_ref().map(|r| r.get::<_, String>("password_hash"));
    let password = form.password;
    let password_ok = argon(move || match stored {
        Some(stored) => auth::verify_password(&password, &stored),
        None => {
            auth::dummy_verify(&password);
            false
        }
    })
    .await?;
    // Non-admins fail exactly like a wrong password.
    let Some(user) = row
        .filter(|r| password_ok && r.get::<_, bool>("is_admin"))
        .map(|r| r.get::<_, Uuid>("id"))
    else {
        return Ok(None);
    };
    let factor_key = format!("factor:{user}");
    ids.extend(
        state
            .attempt(
                &[(throttle::SECOND_FACTOR, &factor_key)],
                "Too many wrong authenticator codes, wait an hour",
            )
            .await?,
    );
    if !second_factor(state, user, form.code.trim(), now).await? {
        return Ok(None);
    }
    state.forgive(&ids).await?;
    throttle::clear(&state.0.db, &user_ip_key).await?;
    let token = auth::new_token(SESSION_PREFIX);
    let csrf = auth::new_token("");
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    tx.execute(
        "INSERT INTO admin_sessions (token_hash, user_id, csrf, created_at, last_used, expires_at)
         VALUES ($1, $2, $3, $4, $4, $5)",
        &[
            &auth::token_hash(&token),
            &user,
            &csrf,
            &now,
            &(now + SESSION_MAX_MS),
        ],
    )
    .await?;
    tx.execute(
        "DELETE FROM admin_sessions WHERE user_id = $1 AND token_hash NOT IN
            (SELECT token_hash FROM admin_sessions WHERE user_id = $1
             ORDER BY created_at DESC, token_hash LIMIT $2)",
        &[&user, &MAX_SESSIONS_PER_ADMIN],
    )
    .await?;
    tx.commit().await?;
    Ok(Some(token))
}

async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    form: Result<Form<ActionForm>, FormRejection>,
) -> Response {
    let session = match checked(&state, &headers, &form).await {
        Ok(s) => s,
        Err(res) => return *res,
    };
    let deleted = async {
        state
            .0
            .db
            .get()
            .await?
            .execute(
                "DELETE FROM admin_sessions WHERE token_hash = $1",
                &[&auth::token_hash(&session.token)],
            )
            .await?;
        anyhow::Ok(())
    }
    .await;
    if let Err(e) = deleted {
        return ApiError::from(e).into_response();
    }
    let mut res = respond(StatusCode::SEE_OTHER, String::new(), Some(cookie("", 0)));
    res.headers_mut()
        .insert(header::LOCATION, HeaderValue::from_static("/admin"));
    res
}

/// Runs a checked action and renders the dashboard with its outcome.
async fn web_action<F, Fut>(
    state: AppState,
    headers: HeaderMap,
    form: Result<Form<ActionForm>, FormRejection>,
    action: F,
) -> Response
where
    F: FnOnce(AppState, Session, ActionForm) -> Fut,
    Fut: std::future::Future<Output = ApiResult<(String, Option<String>)>>,
{
    let session = match checked(&state, &headers, &form).await {
        Ok(s) => s,
        Err(res) => return *res,
    };
    let Ok(Form(form)) = form else {
        return message_page(StatusCode::BAD_REQUEST, "Invalid form");
    };
    let shown = Session {
        token: session.token.clone(),
        user: session.user,
        username: session.username.clone(),
        csrf: session.csrf.clone(),
    };
    let (status, notice) = match action(state.clone(), session, form).await {
        Ok((text, link)) => (
            StatusCode::OK,
            Notice {
                ok: true,
                text,
                link,
            },
        ),
        Err(ApiError(status, text)) => (
            status,
            Notice {
                ok: false,
                text,
                link: None,
            },
        ),
    };
    dashboard_page(&state, &shown, status, Some(notice))
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

fn web_target(path: Result<Path<Uuid>, PathRejection>) -> Result<Uuid, Box<Response>> {
    path.map(|Path(id)| id)
        .map_err(|_| Box::new(message_page(StatusCode::BAD_REQUEST, "No such user")))
}

async fn web_invite(
    State(state): State<AppState>,
    headers: HeaderMap,
    form: Result<Form<ActionForm>, FormRejection>,
) -> Response {
    web_action(state, headers, form, |state, session, form| async move {
        let hours = match form.hours.as_deref().map(str::trim) {
            None | Some("") => 72,
            Some(h) => h
                .parse::<i64>()
                .ok()
                .filter(|h| (1..=720).contains(h))
                .ok_or_else(|| ApiError::bad("Hours must be between 1 and 720"))?,
        };
        reauth(&state, &session.actor(), &form.password, None).await?;
        let (url, _) = api::create_invite(
            &state.0.db,
            &state.0.public_url,
            Some(session.user),
            form.admin.as_deref() == Some("on"),
            hours,
            state.now(),
        )
        .await?;
        Ok((
            format!("Invite link, valid for {hours} hours. Shown once."),
            Some(url),
        ))
    })
    .await
}

async fn web_reset(
    State(state): State<AppState>,
    headers: HeaderMap,
    path: Result<Path<Uuid>, PathRejection>,
    form: Result<Form<ActionForm>, FormRejection>,
) -> Response {
    let target = match web_target(path) {
        Ok(t) => t,
        Err(res) => return *res,
    };
    web_action(
        state,
        headers,
        form,
        move |state, session, form| async move {
            reauth(
                &state,
                &session.actor(),
                &form.password,
                Some(form.code.trim()),
            )
            .await?;
            match user_state(&state.0.db, target).await? {
                None => return Err(not_found()),
                Some(true) => return Err(conflict("Enable the user first")),
                Some(false) => {}
            }
            let (url, _) = crate::account::create_reset_link(
                &state.0.db,
                &state.0.public_url,
                target,
                state.now(),
            )
            .await?;
            Ok((
                "Reset link, valid for 48 hours. Shown once.".into(),
                Some(url),
            ))
        },
    )
    .await
}

async fn web_revoke(
    State(state): State<AppState>,
    headers: HeaderMap,
    path: Result<Path<Uuid>, PathRejection>,
    form: Result<Form<ActionForm>, FormRejection>,
) -> Response {
    let target = match web_target(path) {
        Ok(t) => t,
        Err(res) => return *res,
    };
    web_action(
        state,
        headers,
        form,
        move |state, session, form| async move {
            reauth(&state, &session.actor(), &form.password, None).await?;
            let revoked = revoke_devices(&state, target, None).await?;
            Ok((format!("Signed out {} devices.", revoked.len()), None))
        },
    )
    .await
}

async fn web_disable(
    State(state): State<AppState>,
    headers: HeaderMap,
    path: Result<Path<Uuid>, PathRejection>,
    form: Result<Form<ActionForm>, FormRejection>,
) -> Response {
    let target = match web_target(path) {
        Ok(t) => t,
        Err(res) => return *res,
    };
    web_action(
        state,
        headers,
        form,
        move |state, session, form| async move {
            reauth(&state, &session.actor(), &form.password, None).await?;
            disable_user(&state, session.user, target).await?;
            Ok(("User disabled and signed out everywhere.".into(), None))
        },
    )
    .await
}

async fn web_enable(
    State(state): State<AppState>,
    headers: HeaderMap,
    path: Result<Path<Uuid>, PathRejection>,
    form: Result<Form<ActionForm>, FormRejection>,
) -> Response {
    let target = match web_target(path) {
        Ok(t) => t,
        Err(res) => return *res,
    };
    web_action(
        state,
        headers,
        form,
        move |state, session, form| async move {
            reauth(&state, &session.actor(), &form.password, None).await?;
            enable_user(&state, target).await?;
            Ok((
                "User enabled. They sign in again on their devices.".into(),
                None,
            ))
        },
    )
    .await
}

pub async fn maintenance(db: &Pool, now: i64) -> anyhow::Result<()> {
    db.get()
        .await?
        .execute(
            "DELETE FROM admin_sessions WHERE expires_at < $1 OR last_used < $1 - $2::BIGINT",
            &[&now, &SESSION_IDLE_MS],
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_html() {
        assert_eq!(
            escape(r#"<script>alert("x") & 'y'</script>"#),
            "&lt;script&gt;alert(&quot;x&quot;) &amp; &#39;y&#39;&lt;/script&gt;"
        );
        assert_eq!(escape("plain café"), "plain café");
    }

    #[test]
    fn csp_hash_matches_the_inline_style() {
        let html = document("", "");
        let start = html.find("<style>").unwrap() + "<style>".len();
        let end = html.find("</style>").unwrap();
        use sha2::Digest;
        let digest = sha2::Sha256::digest(&html.as_bytes()[start..end]);
        let expected = format!("sha256-{}", data_encoding::BASE64.encode(&digest));
        assert_eq!(style_hash(), expected);
        assert_eq!(html.matches("<style>").count(), 1);
        assert!(!html.contains("<script"));
        let res = login_page(StatusCode::OK, Some("<b>"), false);
        let csp = res.headers()["content-security-policy"].to_str().unwrap();
        assert!(csp.contains(&format!("style-src '{expected}'")));
        assert!(csp.starts_with("default-src 'none'") && csp.contains("form-action 'self'"));
        assert!(!csp.contains("script-src") && !csp.contains("unsafe"));
        assert_eq!(res.headers()["cache-control"], "no-store");
        assert_eq!(res.headers()["referrer-policy"], "no-referrer");
        assert_eq!(res.headers()["x-content-type-options"], "nosniff");
    }

    #[test]
    fn csrf_compare_is_exact() {
        assert!(same_secret("abc", "abc"));
        assert!(!same_secret("abc", "abd"));
        assert!(!same_secret("abc", "ab"));
        assert!(!same_secret("", "a"));
        assert!(same_secret("", ""));
    }

    #[test]
    fn session_cookie_is_read_only_from_its_name() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("other=vca_x; vidcue_admin=vca_token"),
        );
        assert_eq!(session_token(&headers).as_deref(), Some("vca_token"));
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("vidcue_admin=nope"),
        );
        assert_eq!(session_token(&headers), None);
        let set = cookie("vca_t", 1800);
        for part in [
            "Path=/admin",
            "HttpOnly",
            "Secure",
            "SameSite=Strict",
            "Max-Age=1800",
        ] {
            assert!(set.contains(part), "{part}");
        }
    }

    #[test]
    fn qr_svg_needs_no_inline_style() {
        let svg = qr_svg("https://vidcue.test/join#vci_x");
        assert!(svg.contains("<svg") && !svg.contains("style="));
        assert_eq!(when(Some(0)), "1970-01-01 00:00");
        assert_eq!(when(None), "—");
    }
}
