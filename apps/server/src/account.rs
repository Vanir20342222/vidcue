//! Account maintenance and recovery: recovery codes, password and authenticator changes,
//! and admin-issued reset links.

use crate::api::{ApiError, ApiResult, AppState, ClientIp, Device, argon, qr_svg, reauth};
use crate::auth;
use crate::secrets::ServerKey;
use crate::throttle;
use axum::extract::{Extension, Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use data_encoding::BASE32_NOPAD;
use deadpool_postgres::Pool;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

pub const RESET_PREFIX: &str = "vcr_";
pub const RECOVERY_CODE_COUNT: usize = 10;
const RESET_TTL_MS: i64 = 48 * 3_600_000;
const PENDING_TOTP_TTL_MS: i64 = 30 * 60 * 1000;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/reset",
            get(|| async { crate::web::page_response(crate::web::reset_page()) }),
        )
        .route("/api/reset", post(reset_start))
        .route("/api/reset/confirm", post(reset_confirm))
        .route("/api/account/password", post(change_password))
        .route("/api/account/totp", post(totp_start))
        .route("/api/account/totp/confirm", post(totp_confirm))
        .route(
            "/api/account/recovery-codes",
            get(recovery_status).post(recovery_regenerate),
        )
        .route("/api/admin/users/{id}/reset", post(admin_reset))
}

/// Replaces all recovery codes of a user and returns the new ones (shown once).
pub async fn new_recovery_codes(
    db: &Pool,
    key: &ServerKey,
    user: Uuid,
) -> anyhow::Result<Vec<String>> {
    let codes: Vec<String> = (0..RECOVERY_CODE_COUNT)
        .map(|_| auth::new_recovery_code())
        .collect();
    let mut client = db.get().await?;
    let tx = client.transaction().await?;
    tx.execute("DELETE FROM recovery_codes WHERE user_id = $1", &[&user])
        .await?;
    for code in &codes {
        tx.execute(
            "INSERT INTO recovery_codes (user_id, code_hash) VALUES ($1, $2)",
            &[
                &user,
                &key.recovery_hash(user, &auth::normalize_recovery_code(code)),
            ],
        )
        .await?;
    }
    tx.commit().await?;
    Ok(codes)
}

/// Consumes a recovery code; returns whether it was valid and unused.
pub async fn use_recovery_code(
    db: &Pool,
    key: &ServerKey,
    user: Uuid,
    code: &str,
    now: i64,
) -> anyhow::Result<bool> {
    let hash = key.recovery_hash(user, &auth::normalize_recovery_code(code));
    Ok(db
        .get()
        .await?
        .execute(
            "UPDATE recovery_codes SET used_at = $3 WHERE user_id = $1 AND code_hash = $2 AND used_at IS NULL",
            &[&user, &hash, &now],
        )
        .await?
        == 1)
}

/// Returns a reset link; the token is in the URL fragment, out of proxy logs.
pub async fn create_reset_link(
    db: &Pool,
    public_url: &str,
    user: Uuid,
    now: i64,
) -> anyhow::Result<(String, i64)> {
    let token = auth::new_token(RESET_PREFIX);
    let expires = now + RESET_TTL_MS;
    let client = db.get().await?;
    // Only the newest link works.
    client
        .execute(
            "DELETE FROM account_resets WHERE user_id = $1 AND used_at IS NULL",
            &[&user],
        )
        .await?;
    client
        .execute(
            "INSERT INTO account_resets (token_hash, user_id, created_at, expires_at) VALUES ($1, $2, $3, $4)",
            &[&auth::token_hash(&token), &user, &now, &expires],
        )
        .await?;
    Ok((
        format!("{}/reset#{token}", public_url.trim_end_matches('/')),
        expires,
    ))
}

pub async fn user_id(db: &Pool, username: &str) -> anyhow::Result<Option<Uuid>> {
    let name = auth::normalize_username(username).unwrap_or_default();
    Ok(db
        .get()
        .await?
        .query_opt(
            "SELECT id FROM users WHERE username = $1 AND active",
            &[&name],
        )
        .await?
        .map(|r| r.get(0)))
}

async fn hash(password: &str) -> ApiResult<String> {
    if let Some(problem) = auth::password_problem(password) {
        return Err(ApiError::bad(problem));
    }
    let password = password.to_owned();
    Ok(argon(move || auth::hash_password(&password)).await??)
}

/// Password plus a current authenticator or recovery code.
#[derive(Deserialize)]
struct Proof {
    password: String,
    code: String,
}

#[derive(Deserialize)]
struct PasswordBody {
    current_password: String,
    code: String,
    new_password: String,
    /// Revoke every other device of the account.
    #[serde(default)]
    sign_out_others: bool,
}

async fn change_password(
    State(state): State<AppState>,
    device: Device,
    Json(body): Json<PasswordBody>,
) -> ApiResult {
    if let Some(problem) = auth::password_problem(&body.new_password) {
        return Err(ApiError::bad(problem));
    }
    reauth(&state, &device, &body.current_password, Some(&body.code)).await?;
    let new_hash = hash(&body.new_password).await?;
    let client = state.0.db.get().await?;
    client
        .execute(
            "UPDATE users SET password_hash = $2 WHERE id = $1",
            &[&device.user, &new_hash],
        )
        .await?;
    let mut revoked = 0;
    if body.sign_out_others {
        revoked = client
            .execute(
                "UPDATE devices SET revoked_at = $3 WHERE user_id = $1 AND id <> $2 AND revoked_at IS NULL",
                &[&device.user, &device.device, &state.now()],
            )
            .await?;
        state.0.hub.publish(
            device.user,
            json!({ "type": "devices_revoked", "keep": device.device }),
        );
    }
    Ok(Json(json!({ "changed": true, "revoked_devices": revoked })))
}

async fn totp_start(
    State(state): State<AppState>,
    device: Device,
    Json(body): Json<Proof>,
) -> ApiResult {
    reauth(&state, &device, &body.password, Some(&body.code)).await?;
    let secret = auth::new_totp_secret();
    state
        .0
        .db
        .get()
        .await?
        .execute(
            "UPDATE users SET totp_pending = $2, totp_pending_at = $3 WHERE id = $1",
            &[
                &device.user,
                &state.0.key.seal(device.user, &secret),
                &state.now(),
            ],
        )
        .await?;
    let uri = auth::totp_uri("Vidcue", &device.username, &secret);
    Ok(Json(json!({
        "totp_uri": uri,
        "totp_secret": BASE32_NOPAD.encode(&secret),
        "totp_qr_svg": qr_svg(&uri),
    })))
}

#[derive(Deserialize)]
struct CodeOnly {
    code: String,
}

async fn totp_confirm(
    State(state): State<AppState>,
    device: Device,
    Json(body): Json<CodeOnly>,
) -> ApiResult {
    let now = state.now();
    let key = format!("totp:{}", device.user);
    let ids = state
        .attempt(
            &[(throttle::CONFIRM, &key)],
            "Too many attempts, start again",
        )
        .await?;
    let row = state
        .0
        .db
        .get()
        .await?
        .query_opt(
            "SELECT totp_pending FROM users WHERE id = $1 AND totp_pending IS NOT NULL AND totp_pending_at > $2",
            &[&device.user, &(now - PENDING_TOTP_TTL_MS)],
        )
        .await?
        .ok_or_else(|| ApiError::bad("No authenticator change in progress"))?;
    let sealed: Vec<u8> = row.get(0);
    let secret = state.0.key.open(device.user, &sealed)?;
    let Some(step) = auth::verify_totp(&secret, &body.code, (now / 1000) as u64, 0) else {
        return Err(ApiError::bad("Wrong authenticator code"));
    };
    state
        .0
        .db
        .get()
        .await?
        .execute(
            "UPDATE users SET totp_secret = totp_pending, totp_last_step = $2,
                totp_pending = NULL, totp_pending_at = NULL WHERE id = $1",
            &[&device.user, &(step as i64)],
        )
        .await?;
    state.forgive(&ids).await?;
    Ok(Json(json!({ "changed": true })))
}

async fn recovery_status(State(state): State<AppState>, device: Device) -> ApiResult {
    let remaining: i64 = state
        .0
        .db
        .get()
        .await?
        .query_one(
            "SELECT COUNT(*) FROM recovery_codes WHERE user_id = $1 AND used_at IS NULL",
            &[&device.user],
        )
        .await?
        .get(0);
    Ok(Json(json!({ "remaining": remaining })))
}

async fn recovery_regenerate(
    State(state): State<AppState>,
    device: Device,
    Json(body): Json<Proof>,
) -> ApiResult {
    reauth(&state, &device, &body.password, Some(&body.code)).await?;
    let codes = new_recovery_codes(&state.0.db, &state.0.key, device.user).await?;
    Ok(Json(json!({ "recovery_codes": codes })))
}

async fn admin_reset(
    State(state): State<AppState>,
    device: Device,
    Path(id): Path<Uuid>,
    Json(body): Json<Proof>,
) -> ApiResult {
    if !device.is_admin {
        return Err(ApiError::forbidden());
    }
    reauth(&state, &device, &body.password, Some(&body.code)).await?;
    let exists: bool = state
        .0
        .db
        .get()
        .await?
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM users WHERE id = $1 AND active)",
            &[&id],
        )
        .await?
        .get(0);
    if !exists {
        return Err(ApiError(
            axum::http::StatusCode::NOT_FOUND,
            "No such user".into(),
        ));
    }
    let (url, expires) =
        create_reset_link(&state.0.db, &state.0.public_url, id, state.now()).await?;
    Ok(Json(
        json!({ "url": url, "qr_svg": qr_svg(&url), "expires_at": expires }),
    ))
}

#[derive(Deserialize)]
struct ResetBody {
    token: String,
    password: String,
}

/// Step 1 of a reset link: choose a new password, receive a new authenticator secret.
async fn reset_start(
    State(state): State<AppState>,
    Extension(ip): Extension<ClientIp>,
    Json(body): Json<ResetBody>,
) -> ApiResult {
    let now = state.now();
    if let Some(problem) = auth::password_problem(&body.password) {
        return Err(ApiError::bad(problem));
    }
    let ip_key = format!("reset:{}", ip.0);
    let ids = state
        .attempt(
            &[(throttle::CODES_IP, &ip_key)],
            "Too many attempts, try again later",
        )
        .await?;
    let token_hash = auth::token_hash(&body.token);
    const VALID: &str =
        "SELECT r.user_id, u.username FROM account_resets r JOIN users u ON u.id = r.user_id
         WHERE r.token_hash = $1 AND r.used_at IS NULL AND r.expires_at > $2";
    // Check the link before the expensive hash, so junk requests cost nothing.
    let Some(row) = state
        .0
        .db
        .get()
        .await?
        .query_opt(VALID, &[&token_hash, &now])
        .await?
    else {
        return Err(ApiError::bad("This reset link is invalid, used or expired"));
    };
    let (user, username): (Uuid, String) = (row.get(0), row.get(1));
    let new_hash = hash(&body.password).await?;
    let secret = auth::new_totp_secret();
    let updated = state
        .0
        .db
        .get()
        .await?
        .execute(
            "UPDATE account_resets SET password_hash = $3, totp_secret = $4
             WHERE token_hash = $1 AND used_at IS NULL AND expires_at > $2",
            &[
                &token_hash,
                &now,
                &new_hash,
                &state.0.key.seal(user, &secret),
            ],
        )
        .await?;
    if updated != 1 {
        return Err(ApiError::bad("This reset link is invalid, used or expired"));
    }
    state.forgive(&ids).await?;
    let uri = auth::totp_uri("Vidcue", &username, &secret);
    Ok(Json(json!({
        "username": username,
        "totp_uri": uri,
        "totp_secret": BASE32_NOPAD.encode(&secret),
        "totp_qr_svg": qr_svg(&uri),
    })))
}

#[derive(Deserialize)]
struct ResetConfirmBody {
    token: String,
    code: String,
}

/// Step 2: a valid code applies the new password and authenticator, signs out every
/// device and issues fresh recovery codes.
async fn reset_confirm(
    State(state): State<AppState>,
    Extension(ip): Extension<ClientIp>,
    Json(body): Json<ResetConfirmBody>,
) -> ApiResult {
    let now = state.now();
    let token_hash = auth::token_hash(&body.token);
    let key = format!(
        "reset-confirm:{}",
        data_encoding::HEXLOWER.encode(&token_hash[..8])
    );
    let ip_key = format!("reset:{}", ip.0);
    let ids = state
        .attempt(
            &[(throttle::CONFIRM, &key), (throttle::CODES_IP, &ip_key)],
            "Too many attempts, open the link again",
        )
        .await?;
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    let row = tx
        .query_opt(
            "SELECT user_id, password_hash, totp_secret FROM account_resets
             WHERE token_hash = $1 AND used_at IS NULL AND expires_at > $2
               AND password_hash IS NOT NULL AND totp_secret IS NOT NULL
             FOR UPDATE",
            &[&token_hash, &now],
        )
        .await?
        .ok_or_else(|| ApiError::bad("This reset link is invalid, used or expired"))?;
    let user: Uuid = row.get("user_id");
    let sealed: Vec<u8> = row.get("totp_secret");
    let secret = state.0.key.open(user, &sealed)?;
    let Some(step) = auth::verify_totp(&secret, &body.code, (now / 1000) as u64, 0) else {
        return Err(ApiError::bad("Wrong authenticator code"));
    };
    tx.execute(
        "UPDATE users SET password_hash = $2, totp_secret = $3, totp_last_step = $4,
            totp_pending = NULL, totp_pending_at = NULL WHERE id = $1",
        &[
            &user,
            &row.get::<_, String>("password_hash"),
            &sealed,
            &(step as i64),
        ],
    )
    .await?;
    let revoked = tx
        .execute(
            "UPDATE devices SET revoked_at = $2 WHERE user_id = $1 AND revoked_at IS NULL",
            &[&user, &now],
        )
        .await?;
    tx.execute(
        "UPDATE account_resets SET used_at = $2 WHERE token_hash = $1",
        &[&token_hash, &now],
    )
    .await?;
    // A reset also lifts lockouts on the account.
    tx.execute(
        "DELETE FROM auth_failures
         WHERE starts_with(key, (SELECT 'login:' || username || ':' FROM users WHERE id = $1))
            OR key = (SELECT 'login:' || username FROM users WHERE id = $1)
            OR key = 'factor:' || $1::text OR key = 'reauth:' || $1::text",
        &[&user],
    )
    .await?;
    tx.commit().await?;
    drop(client);
    state.forgive(&ids).await?;
    state
        .0
        .hub
        .publish(user, json!({ "type": "devices_revoked" }));
    let codes = new_recovery_codes(&state.0.db, &state.0.key, user).await?;
    Ok(Json(
        json!({ "reset": true, "revoked_devices": revoked, "recovery_codes": codes }),
    ))
}
