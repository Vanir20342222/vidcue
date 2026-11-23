//! Playback health log: devices report sanitized outcome categories (never free text,
//! URLs, ids, titles, cookies or error messages) so a user and the admin can see how
//! well playback works.

use crate::api::{ApiError, ApiResult, AppState, Device};
use crate::stats::{DAY_MS, day_string};
use crate::throttle::Rule;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use deadpool_postgres::Pool;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

pub const KINDS: [&str; 5] = ["resolve", "start", "stall", "error", "download"];
/// Kinds whose reports are outcomes, so an `ok` share means something.
pub const RATED_KINDS: [&str; 3] = ["resolve", "start", "download"];
pub const CATEGORIES: [&str; 23] = [
    "ok",
    "network_timeout",
    "network_unreachable",
    "tls",
    "http_403",
    "http_404",
    "http_429",
    "http_5xx",
    "extractor_failed",
    "js_challenge_failed",
    "signature_failed",
    "format_unavailable",
    "members_only",
    "age_restricted",
    "geo_blocked",
    "drm",
    "live_unsupported",
    "decoder_error",
    "audio_output_error",
    "offline_file_missing",
    "storage_full",
    "cancelled",
    "unknown",
];
pub const PLATFORMS: [&str; 2] = ["android", "desktop"];
pub const MAX_REPORTS_PER_REQUEST: usize = 100;
/// Accepted reports per user in any rolling 24 hours.
pub const MAX_REPORTS_PER_DAY: i64 = 2000;
pub const MAX_REPORT_AGE_MS: i64 = 30 * DAY_MS;
pub const MAX_REPORT_AHEAD_MS: i64 = 5 * 60 * 1000;
pub const MAX_DURATION_MS: i64 = 600_000;
pub const MAX_VERSION_CHARS: usize = 40;
pub const RETENTION_MS: i64 = 90 * DAY_MS;
pub const MAX_SUMMARY_DAYS: i64 = 90;
pub const TOP_CATEGORIES: i64 = 10;
pub const TOP_VERSIONS: i64 = 20;
/// Report uploads per device.
pub const HEALTH_POST: Rule = Rule {
    limit: 30,
    window_ms: 60 * 60 * 1000,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/health/reports", post(post_reports))
        .route("/api/health/summary", get(own_summary))
        .route("/api/admin/health", get(admin_summary))
}

/// One report exactly as the client sends it; any other field rejects it.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub kind: String,
    pub category: String,
    pub client_version: String,
    pub platform: String,
    pub at: i64,
    #[serde(default)]
    pub duration_ms: Option<i64>,
}

pub fn version_ok(version: &str) -> bool {
    (1..=MAX_VERSION_CHARS).contains(&version.len())
        && version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'+' | b'-'))
}

/// Parses and checks one report; `None` rejects it.
pub fn validate(value: &Value, now: i64) -> Option<Report> {
    let report: Report = serde_json::from_value(value.clone()).ok()?;
    let ok = KINDS.contains(&report.kind.as_str())
        && CATEGORIES.contains(&report.category.as_str())
        && PLATFORMS.contains(&report.platform.as_str())
        && version_ok(&report.client_version)
        && (now - MAX_REPORT_AGE_MS..=now + MAX_REPORT_AHEAD_MS).contains(&report.at)
        && report
            .duration_ms
            .is_none_or(|d| (0..=MAX_DURATION_MS).contains(&d));
    ok.then_some(report)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportsBody {
    reports: Vec<Value>,
}

async fn post_reports(
    State(state): State<AppState>,
    device: Device,
    body: Result<Json<ReportsBody>, JsonRejection>,
) -> ApiResult {
    let Json(body) = body.map_err(|_| ApiError::bad("Body must be {\"reports\": [...]}"))?;
    if body.reports.len() > MAX_REPORTS_PER_REQUEST {
        return Err(ApiError::bad(format!(
            "Send at most {MAX_REPORTS_PER_REQUEST} reports per request"
        )));
    }
    let key = format!("health-post:{}", device.device);
    state
        .attempt(
            &[(HEALTH_POST, &key)],
            "Too many health uploads, try again later",
        )
        .await?;
    let now = state.now();
    let valid: Vec<Report> = body
        .reports
        .iter()
        .filter_map(|r| validate(r, now))
        .collect();
    let mut rejected = body.reports.len() - valid.len();
    let accepted = store(&state.0.db, device.user, device.device, valid, now).await?;
    rejected += accepted.1;
    Ok(Json(
        json!({ "accepted": accepted.0, "rejected": rejected }),
    ))
}

/// Stores reports within the user's daily cap. Returns (stored, refused by the cap).
async fn store(
    db: &Pool,
    user: Uuid,
    device: Uuid,
    mut reports: Vec<Report>,
    now: i64,
) -> anyhow::Result<(usize, usize)> {
    if reports.is_empty() {
        return Ok((0, 0));
    }
    let mut client = db.get().await?;
    let tx = client.transaction().await?;
    // Parallel uploads of one user must not both pass the cap.
    tx.execute(
        "SELECT pg_advisory_xact_lock(hashtextextended('health:' || $1::text, 0))",
        &[&user.to_string()],
    )
    .await?;
    let recent: i64 = tx
        .query_one(
            "SELECT COUNT(*) FROM health_reports WHERE user_id = $1 AND received_at > $2",
            &[&user, &(now - DAY_MS)],
        )
        .await?
        .get(0);
    let room = (MAX_REPORTS_PER_DAY - recent).max(0) as usize;
    let over = reports.len().saturating_sub(room);
    reports.truncate(room);
    if !reports.is_empty() {
        let column = |f: fn(&Report) -> String| reports.iter().map(f).collect::<Vec<_>>();
        let kinds = column(|r| r.kind.clone());
        let categories = column(|r| r.category.clone());
        let versions = column(|r| r.client_version.clone());
        let platforms = column(|r| r.platform.clone());
        let ats: Vec<i64> = reports.iter().map(|r| r.at).collect();
        let durations: Vec<Option<i32>> = reports
            .iter()
            .map(|r| r.duration_ms.map(|d| d as i32))
            .collect();
        tx.execute(
            "INSERT INTO health_reports
                (user_id, device_id, kind, category, client_version, platform, at, duration_ms, received_at)
             SELECT $1, $2, k, c, v, p, a, d, $9
             FROM UNNEST($3::TEXT[], $4::TEXT[], $5::TEXT[], $6::TEXT[], $7::BIGINT[], $8::INT[])
                  AS t(k, c, v, p, a, d)",
            &[
                &user,
                &device,
                &kinds,
                &categories,
                &versions,
                &platforms,
                &ats,
                &durations,
                &now,
            ],
        )
        .await?;
    }
    tx.commit().await?;
    Ok((reports.len(), over))
}

#[derive(Deserialize)]
struct DaysQuery {
    days: Option<i64>,
}

fn days(query: Result<Query<DaysQuery>, QueryRejection>) -> ApiResult<i64> {
    let days = query
        .map_err(|_| ApiError::bad("days must be an integer"))?
        .0
        .days
        .unwrap_or(7);
    if !(1..=MAX_SUMMARY_DAYS).contains(&days) {
        return Err(ApiError::bad(format!(
            "days must be between 1 and {MAX_SUMMARY_DAYS}"
        )));
    }
    Ok(days)
}

fn rate(ok: i64, total: i64) -> Value {
    if total == 0 {
        Value::Null
    } else {
        json!(((ok as f64 / total as f64) * 1000.0).round() / 1000.0)
    }
}

/// HealthSummary of §6.2 over the last `days` days, for one user or (admin) everyone.
pub async fn summary(db: &Pool, user: Option<Uuid>, days: i64, now: i64) -> anyhow::Result<Value> {
    let since = now - days * DAY_MS;
    let client = db.get().await?;
    let scope = "at >= $1 AND ($2::UUID IS NULL OR user_id = $2)";
    let by_kind_rows = client
        .query(
            &format!(
                "SELECT kind, COUNT(*), COUNT(*) FILTER (WHERE category = 'ok')
                 FROM health_reports WHERE {scope} GROUP BY kind"
            ),
            &[&since, &user],
        )
        .await?;
    let mut total = 0i64;
    let mut by_kind = serde_json::Map::new();
    for kind in KINDS {
        let row = by_kind_rows.iter().find(|r| r.get::<_, String>(0) == kind);
        let (n, ok): (i64, i64) = row.map_or((0, 0), |r| (r.get(1), r.get(2)));
        total += n;
        let entry = if RATED_KINDS.contains(&kind) {
            json!({"total": n, "ok": ok, "ok_rate": rate(ok, n)})
        } else {
            json!({"total": n})
        };
        by_kind.insert(kind.into(), entry);
    }
    let top = client
        .query(
            &format!(
                "SELECT kind, category, COUNT(*) AS n FROM health_reports
                 WHERE {scope} AND category <> 'ok'
                 GROUP BY kind, category ORDER BY n DESC, kind, category LIMIT {TOP_CATEGORIES}"
            ),
            &[&since, &user],
        )
        .await?
        .iter()
        .map(|r| {
            json!({"kind": r.get::<_, String>(0), "category": r.get::<_, String>(1), "count": r.get::<_, i64>(2)})
        })
        .collect::<Vec<_>>();
    let start = client
        .query_one(
            &format!(
                "SELECT percentile_disc(0.5) WITHIN GROUP (ORDER BY duration_ms),
                        percentile_disc(0.9) WITHIN GROUP (ORDER BY duration_ms)
                 FROM health_reports
                 WHERE {scope} AND kind = 'start' AND category = 'ok' AND duration_ms IS NOT NULL"
            ),
            &[&since, &user],
        )
        .await?;
    let by_day = client
        .query(
            &format!(
                "SELECT floor(at::float8 / {DAY_MS})::BIGINT AS d, COUNT(*),
                        COUNT(*) FILTER (WHERE category = 'ok')
                 FROM health_reports WHERE {scope} GROUP BY d ORDER BY d"
            ),
            &[&since, &user],
        )
        .await?
        .iter()
        .map(|r| {
            json!({"day": day_string(r.get(0)), "total": r.get::<_, i64>(1), "ok": r.get::<_, i64>(2)})
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "days": days,
        "total": total,
        "by_kind": by_kind,
        "top_categories": top,
        "start_ms": {"p50": start.get::<_, Option<i32>>(0), "p90": start.get::<_, Option<i32>>(1)},
        "by_day": by_day,
    }))
}

/// Summary over every user plus reporting users and per-version outcome rates.
pub async fn admin_aggregate(db: &Pool, days: i64, now: i64) -> anyhow::Result<Value> {
    let mut out = summary(db, None, days, now).await?;
    let since = now - days * DAY_MS;
    let client = db.get().await?;
    let users: i64 = client
        .query_one(
            "SELECT COUNT(DISTINCT user_id) FROM health_reports WHERE at >= $1",
            &[&since],
        )
        .await?
        .get(0);
    let versions = client
        .query(
            &format!(
                "SELECT client_version, COUNT(*) AS n,
                        COUNT(*) FILTER (WHERE kind = ANY($2) AND category = 'ok'),
                        COUNT(*) FILTER (WHERE kind = ANY($2))
                 FROM health_reports WHERE at >= $1
                 GROUP BY client_version ORDER BY n DESC, client_version LIMIT {TOP_VERSIONS}"
            ),
            &[&since, &RATED_KINDS.as_slice()],
        )
        .await?
        .iter()
        .map(|r| {
            json!({
                "client_version": r.get::<_, String>(0),
                "total": r.get::<_, i64>(1),
                "ok_rate": rate(r.get(2), r.get(3)),
            })
        })
        .collect::<Vec<_>>();
    out["users_reporting"] = json!(users);
    out["by_version"] = json!(versions);
    Ok(out)
}

async fn own_summary(
    State(state): State<AppState>,
    device: Device,
    query: Result<Query<DaysQuery>, QueryRejection>,
) -> ApiResult {
    let days = days(query)?;
    Ok(Json(
        summary(&state.0.db, Some(device.user), days, state.now()).await?,
    ))
}

async fn admin_summary(
    State(state): State<AppState>,
    device: Device,
    query: Result<Query<DaysQuery>, QueryRejection>,
) -> ApiResult {
    if !device.is_admin {
        return Err(ApiError::forbidden());
    }
    let days = days(query)?;
    Ok(Json(admin_aggregate(&state.0.db, days, state.now()).await?))
}

pub async fn maintenance(db: &Pool, now: i64) -> anyhow::Result<()> {
    db.get()
        .await?
        .execute(
            "DELETE FROM health_reports WHERE at < $1",
            &[&(now - RETENTION_MS)],
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000_000;

    fn report() -> Value {
        json!({"kind": "start", "category": "ok", "client_version": "0.2.0-beta.3",
               "platform": "android", "at": NOW, "duration_ms": 840})
    }

    fn with(field: &str, value: Value) -> Value {
        let mut r = report();
        r[field] = value;
        r
    }

    #[test]
    fn every_enum_value_is_accepted_and_unknown_ones_rejected() {
        assert!(validate(&report(), NOW).is_some());
        for kind in KINDS {
            assert!(
                validate(&with("kind", json!(kind)), NOW).is_some(),
                "{kind}"
            );
        }
        for category in CATEGORIES {
            assert!(
                validate(&with("category", json!(category)), NOW).is_some(),
                "{category}"
            );
        }
        for platform in PLATFORMS {
            assert!(validate(&with("platform", json!(platform)), NOW).is_some());
        }
        for (field, bad) in [
            ("kind", json!("seek")),
            ("kind", json!("Start")),
            ("category", json!("http_500")),
            ("category", json!("Sign in to confirm you're not a bot")),
            ("platform", json!("ios")),
            ("kind", json!(1)),
        ] {
            assert!(
                validate(&with(field, bad.clone()), NOW).is_none(),
                "{field} {bad}"
            );
        }
    }

    #[test]
    fn report_edges() {
        // Unknown fields (free text, URLs) reject the report.
        assert!(validate(&with("message", json!("https://x/?sig=1")), NOW).is_none());
        assert!(validate(&with("url", json!("https://youtu.be/x")), NOW).is_none());
        assert!(validate(&with("at", json!(NOW - MAX_REPORT_AGE_MS)), NOW).is_some());
        assert!(validate(&with("at", json!(NOW - MAX_REPORT_AGE_MS - 1)), NOW).is_none());
        assert!(validate(&with("at", json!(NOW + MAX_REPORT_AHEAD_MS)), NOW).is_some());
        assert!(validate(&with("at", json!(NOW + MAX_REPORT_AHEAD_MS + 1)), NOW).is_none());
        assert!(validate(&with("at", json!(1.5)), NOW).is_none());
        assert!(validate(&with("duration_ms", json!(0)), NOW).is_some());
        assert!(validate(&with("duration_ms", json!(MAX_DURATION_MS)), NOW).is_some());
        assert!(validate(&with("duration_ms", json!(MAX_DURATION_MS + 1)), NOW).is_none());
        assert!(validate(&with("duration_ms", json!(-1)), NOW).is_none());
        assert!(validate(&with("duration_ms", Value::Null), NOW).is_some());
        let mut no_duration = report();
        no_duration.as_object_mut().unwrap().remove("duration_ms");
        assert!(validate(&no_duration, NOW).is_some());
        for good in ["1", "0.2.0-beta.3", "1.0+build.7", &"a".repeat(40)] {
            assert!(version_ok(good), "{good}");
        }
        for bad in ["", "1.0 beta", "1/2", "ü", &"a".repeat(41)] {
            assert!(!version_ok(bad), "{bad}");
        }
        assert!(validate(&json!("start"), NOW).is_none());
    }
}
