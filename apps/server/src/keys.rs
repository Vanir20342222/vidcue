//! Relay for the account's end-to-end key (encrypts synced provider sessions).
//!
//! QR pairing carries the key directly from device to device, so the server never sees
//! it. A device that signed in with a password instead asks for it here: it posts an
//! X25519 public key, a trusted device of the same account seals the key to that public
//! key and posts the sealed blob back. The server stores neither secret. Both devices
//! must show the same fingerprint of the public key and the user confirms it on the
//! granting device, so a server that swaps in its own public key is detected.

use crate::api::{ApiError, ApiResult, AppState, Device};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use data_encoding::BASE64;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

const REQUEST_TTL_MS: i64 = 15 * 60 * 1000;
const MAX_SEALED_BYTES: usize = 512;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/keys/requests", get(list).post(create))
        .route("/api/keys/requests/{id}", get(fetch))
        .route("/api/keys/requests/{id}/grant", post(grant))
}

fn decoded_len(b64: &str) -> Option<usize> {
    BASE64.decode(b64.trim().as_bytes()).ok().map(|b| b.len())
}

#[derive(Deserialize)]
struct CreateBody {
    /// Base64 X25519 public key (32 bytes).
    public_key: String,
}

async fn create(
    State(state): State<AppState>,
    device: Device,
    Json(body): Json<CreateBody>,
) -> ApiResult {
    if decoded_len(&body.public_key) != Some(32) {
        return Err(ApiError::bad("public_key must be 32 bytes of base64"));
    }
    let now = state.now();
    let id = Uuid::new_v4();
    let client = state.0.db.get().await?;
    // One open request per device.
    client
        .execute(
            "DELETE FROM key_requests WHERE device_id = $1 OR expires_at < $2",
            &[&device.device, &now],
        )
        .await?;
    client
        .execute(
            "INSERT INTO key_requests (id, user_id, device_id, public_key, created_at, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6)",
            &[
                &id,
                &device.user,
                &device.device,
                &body.public_key.trim(),
                &now,
                &(now + REQUEST_TTL_MS),
            ],
        )
        .await?;
    state.0.hub.publish(
        device.user,
        json!({ "type": "key_request", "id": id, "device_id": device.device }),
    );
    Ok(Json(
        json!({ "id": id, "expires_at": now + REQUEST_TTL_MS }),
    ))
}

/// Open requests from the user's other devices, for the granting screen.
async fn list(State(state): State<AppState>, device: Device) -> ApiResult {
    let rows = state
        .0
        .db
        .get()
        .await?
        .query(
            "SELECT r.id, r.device_id, d.name, r.public_key, r.created_at, r.expires_at
             FROM key_requests r JOIN devices d ON d.id = r.device_id
             WHERE r.user_id = $1 AND r.device_id <> $2 AND r.sealed_key IS NULL
               AND r.expires_at > $3 AND d.revoked_at IS NULL
             ORDER BY r.created_at",
            &[&device.user, &device.device, &state.now()],
        )
        .await?;
    let requests: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<_, Uuid>("id"),
                "device_id": r.get::<_, Uuid>("device_id"),
                "device_name": r.get::<_, String>("name"),
                "public_key": r.get::<_, String>("public_key"),
                "created_at": r.get::<_, i64>("created_at"),
                "expires_at": r.get::<_, i64>("expires_at"),
            })
        })
        .collect();
    Ok(Json(json!({ "requests": requests })))
}

#[derive(Deserialize)]
struct GrantBody {
    /// Base64 sealed box of the account key for the request's public key.
    sealed_key: String,
}

async fn grant(
    State(state): State<AppState>,
    device: Device,
    Path(id): Path<Uuid>,
    Json(body): Json<GrantBody>,
) -> ApiResult {
    if !decoded_len(&body.sealed_key).is_some_and(|n| (32..=MAX_SEALED_BYTES).contains(&n)) {
        return Err(ApiError::bad(
            "sealed_key must be base64 of 32 to 512 bytes",
        ));
    }
    let requester: Option<Uuid> = state
        .0
        .db
        .get()
        .await?
        .query_opt(
            "UPDATE key_requests SET sealed_key = $4, granted_by = $3
             WHERE id = $1 AND user_id = $2 AND device_id <> $3 AND sealed_key IS NULL AND expires_at > $5
             RETURNING device_id",
            &[&id, &device.user, &device.device, &body.sealed_key.trim(), &state.now()],
        )
        .await?
        .map(|r| r.get(0));
    let Some(requester) = requester else {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            "No open key request with this id".into(),
        ));
    };
    state.0.hub.publish(
        device.user,
        json!({ "type": "key_granted", "id": id, "device_id": requester }),
    );
    Ok(Json(json!({ "granted": id })))
}

/// The requesting device collects its sealed key once; the request is then deleted.
async fn fetch(State(state): State<AppState>, device: Device, Path(id): Path<Uuid>) -> ApiResult {
    let client = state.0.db.get().await?;
    let row = client
        .query_opt(
            "SELECT sealed_key FROM key_requests WHERE id = $1 AND device_id = $2 AND expires_at > $3",
            &[&id, &device.device, &state.now()],
        )
        .await?
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "No such key request".into()))?;
    let sealed: Option<String> = row.get(0);
    if sealed.is_some() {
        client
            .execute("DELETE FROM key_requests WHERE id = $1", &[&id])
            .await?;
    }
    Ok(Json(json!({ "id": id, "sealed_key": sealed })))
}
