//! Listening and watching statistics and the yearly review, computed from the user's own
//! synced `history` events. Nothing here reads another user's data.

use crate::api::{ApiError, ApiResult, AppState, Device};
use crate::throttle::Rule;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use deadpool_postgres::Pool;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

pub const DAY_MS: i64 = 24 * 3_600_000;
/// Longest period one `GET /api/stats` may cover.
pub const MAX_PERIOD_MS: i64 = 366 * DAY_MS;
/// Plays shorter than this are accidental starts and do not count.
pub const MIN_PLAY_SECONDS: f64 = 5.0;
/// Upper bound for one play when the media length is unknown.
pub const MAX_PLAY_SECONDS: f64 = 6.0 * 3600.0;
/// Time zone offsets accepted, in minutes east of UTC.
pub const MAX_TZ_OFFSET_MIN: i32 = 840;
pub const FIRST_WRAPPED_YEAR: i32 = 2020;
/// A cached yearly review is recomputed when older than this.
pub const WRAPPED_REFRESH_MS: i64 = 6 * 3_600_000;
/// A past year's review computed this long after the year ended is final.
pub const WRAPPED_FINAL_AFTER_MS: i64 = 7 * DAY_MS;
pub const PERIOD_TOP: usize = 10;
pub const WRAPPED_TOP: usize = 5;
/// Statistics computations per user (each reads the whole period of history).
pub const STATS_COMPUTE: Rule = Rule {
    limit: 60,
    window_ms: 10 * 60 * 1000,
};

pub const KINDS: [&str; 7] = [
    "video", "song", "playlist", "album", "artist", "channel", "station",
];

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/stats", get(period))
        .route("/api/stats/wrapped", get(wrapped))
}

// ---- Calendar arithmetic (proleptic Gregorian, H. Hinnant's algorithms) ----

/// Days since 1970-01-01 of a civil date.
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Civil date of a day count since 1970-01-01.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (
        if month <= 2 {
            yoe + era * 400 + 1
        } else {
            yoe + era * 400
        },
        month,
        day,
    )
}

pub fn day_string(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Local day number of a UTC millisecond time in a zone `tz_offset_min` east of UTC.
pub fn local_day(at: i64, tz_offset_min: i32) -> i64 {
    (at + i64::from(tz_offset_min) * 60_000).div_euclid(DAY_MS)
}

/// UTC millisecond time at which local `year`-01-01 00:00 begins.
pub fn year_start_ms(year: i32, tz_offset_min: i32) -> i64 {
    days_from_civil(i64::from(year), 1, 1) * DAY_MS - i64::from(tz_offset_min) * 60_000
}

// ---- Input ----

/// One counted play, already clamped.
#[derive(Clone, Debug, PartialEq)]
pub struct Play {
    pub url: String,
    pub title: String,
    pub creator: String,
    pub thumbnail: String,
    pub kind: &'static str,
    pub at: i64,
    pub seconds: f64,
    pub finished: bool,
}

impl Play {
    /// Reads a synced history event. Events without `at`, without an online `url` or
    /// shorter than [`MIN_PLAY_SECONDS`] are not plays.
    pub fn from_event(value: &Value) -> Option<Self> {
        let at = value.get("at")?.as_f64()?;
        if !at.is_finite() || at.abs() > 1e15 {
            return None;
        }
        let url = value.get("url")?.as_str()?;
        vidcue_core::http_url(url).ok()?;
        let seconds = value.get("seconds").and_then(Value::as_f64).unwrap_or(0.0);
        if !seconds.is_finite() || seconds < MIN_PLAY_SECONDS {
            return None;
        }
        let duration = value
            .get("duration")
            .and_then(Value::as_f64)
            .filter(|d| d.is_finite() && *d > 0.0);
        let seconds = seconds.min(duration.unwrap_or(MAX_PLAY_SECONDS).min(MAX_PLAY_SECONDS));
        let text = |field: &str| {
            value
                .get(field)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let kind = value.get("kind").and_then(Value::as_str).unwrap_or("video");
        Some(Self {
            url: url.to_owned(),
            title: text("title"),
            creator: text("creator"),
            thumbnail: text("thumbnail"),
            kind: KINDS
                .iter()
                .find(|k| **k == kind)
                .copied()
                .unwrap_or("video"),
            at: at as i64,
            seconds,
            finished: value.get("finished").and_then(Value::as_bool) == Some(true),
        })
    }
}

/// Loads the user's plays with `from <= at < to`.
pub async fn load_plays(db: &Pool, user: Uuid, from: i64, to: i64) -> anyhow::Result<Vec<Play>> {
    let rows = db
        .get()
        .await?
        .query(
            "SELECT value FROM sync_records
             WHERE user_id = $1 AND collection = 'history' AND value IS NOT NULL
               AND (value->>'at')::numeric >= ($2::BIGINT)::numeric
               AND (value->>'at')::numeric < ($3::BIGINT)::numeric",
            &[&user, &from, &to],
        )
        .await?;
    let mut plays: Vec<Play> = rows
        .iter()
        .filter_map(|r| Play::from_event(&r.get::<_, Value>(0)))
        .collect();
    plays.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.url.cmp(&b.url)));
    Ok(plays)
}

// ---- Aggregation ----

#[derive(Default)]
struct Totals {
    plays: u64,
    seconds: f64,
}

struct ItemTotals {
    totals: Totals,
    latest: Play,
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

/// Seconds desc, then plays desc, then the name for a stable order.
fn rank<'a, T>(
    items: impl Iterator<Item = (&'a str, T)>,
    key: impl Fn(&T) -> (f64, u64),
) -> Vec<(&'a str, T)> {
    let mut items: Vec<(&str, T)> = items.collect();
    items.sort_by(|(an, a), (bn, b)| {
        let (asec, aplays) = key(a);
        let (bsec, bplays) = key(b);
        bsec.total_cmp(&asec)
            .then(bplays.cmp(&aplays))
            .then_with(|| an.cmp(bn))
    });
    items
}

fn by_item<'a>(plays: impl Iterator<Item = &'a Play>) -> HashMap<&'a str, ItemTotals> {
    let mut items: HashMap<&str, ItemTotals> = HashMap::new();
    for play in plays {
        let entry = items.entry(&play.url).or_insert_with(|| ItemTotals {
            totals: Totals::default(),
            latest: play.clone(),
        });
        entry.totals.plays += 1;
        entry.totals.seconds += play.seconds;
        if play.at >= entry.latest.at {
            entry.latest = play.clone();
        }
    }
    items
}

fn top_items<'a>(plays: impl Iterator<Item = &'a Play>, n: usize) -> Vec<Value> {
    let items = by_item(plays);
    rank(items.iter().map(|(k, v)| (*k, v)), |v| {
        (v.totals.seconds, v.totals.plays)
    })
    .into_iter()
    .take(n)
    .map(|(url, v)| {
        json!({
            "url": url,
            "title": v.latest.title,
            "creator": v.latest.creator,
            "thumbnail": v.latest.thumbnail,
            "kind": v.latest.kind,
            "plays": v.totals.plays,
            "seconds": round1(v.totals.seconds),
        })
    })
    .collect()
}

fn top_creators(plays: &[Play], n: usize) -> Vec<Value> {
    let mut creators: HashMap<&str, Vec<&Play>> = HashMap::new();
    for play in plays.iter().filter(|p| !p.creator.trim().is_empty()) {
        creators.entry(&play.creator).or_default().push(play);
    }
    let totals: Vec<(&str, Totals, String)> = creators
        .into_iter()
        .map(|(name, plays)| {
            let totals = Totals {
                plays: plays.len() as u64,
                seconds: plays.iter().map(|p| p.seconds).sum(),
            };
            let items = by_item(plays.into_iter());
            let thumbnail = rank(items.iter().map(|(k, v)| (*k, v)), |v| {
                (v.totals.seconds, v.totals.plays)
            })
            .first()
            .map(|(_, v)| v.latest.thumbnail.clone())
            .unwrap_or_default();
            (name, totals, thumbnail)
        })
        .collect();
    rank(totals.iter().map(|(n, t, th)| (*n, (t, th))), |(t, _)| {
        (t.seconds, t.plays)
    })
    .into_iter()
    .take(n)
    .map(|(name, (t, thumbnail))| {
        json!({
            "creator": name,
            "plays": t.plays,
            "seconds": round1(t.seconds),
            "thumbnail": thumbnail,
        })
    })
    .collect()
}

/// PeriodStats of §6.3 for plays already limited to `[from, to)`.
pub fn period_stats(plays: &[Play], from: i64, to: i64, tz_offset_min: i32) -> Value {
    let mut by_kind: BTreeMap<&str, Totals> = BTreeMap::new();
    let mut by_day: BTreeMap<i64, Totals> = BTreeMap::new();
    let mut by_hour = [0.0f64; 24];
    let mut total = Totals::default();
    let mut finished = 0u64;
    let mut distinct = HashSet::new();
    let shift = i64::from(tz_offset_min) * 60_000;
    for play in plays {
        total.plays += 1;
        total.seconds += play.seconds;
        finished += u64::from(play.finished);
        distinct.insert(play.url.as_str());
        let kind = by_kind.entry(play.kind).or_default();
        kind.plays += 1;
        kind.seconds += play.seconds;
        let day = by_day.entry(local_day(play.at, tz_offset_min)).or_default();
        day.plays += 1;
        day.seconds += play.seconds;
        let hour = (play.at + shift).rem_euclid(DAY_MS) / 3_600_000;
        by_hour[hour as usize] += play.seconds;
    }
    json!({
        "from": from,
        "to": to,
        "plays": total.plays,
        "seconds": round1(total.seconds),
        "distinct_items": distinct.len(),
        "finished": finished,
        "by_kind": by_kind.iter().map(|(k, t)| ((*k).to_owned(), json!({"plays": t.plays, "seconds": round1(t.seconds)}))).collect::<serde_json::Map<_, _>>(),
        "top_creators": top_creators(plays, PERIOD_TOP),
        "top_items": top_items(plays.iter(), PERIOD_TOP),
        "by_day": by_day.iter().map(|(d, t)| json!({"day": day_string(*d), "seconds": round1(t.seconds), "plays": t.plays})).collect::<Vec<_>>(),
        "by_hour": by_hour.map(round1),
    })
}

/// Wrapped of §6.3. `plays` are the year's plays; `new_creators` is counted separately
/// because it needs the whole history.
pub fn wrapped_stats(
    plays: &[Play],
    year: i32,
    tz_offset_min: i32,
    new_creators: i64,
    computed_at: i64,
) -> Value {
    let mut by_month = [0.0f64; 12];
    let mut by_day: BTreeMap<i64, f64> = BTreeMap::new();
    let (mut seconds, mut song_seconds) = (0.0, 0.0);
    let mut distinct = HashSet::new();
    for play in plays {
        seconds += play.seconds;
        if play.kind == "song" {
            song_seconds += play.seconds;
        }
        distinct.insert(play.url.as_str());
        let day = local_day(play.at, tz_offset_min);
        *by_day.entry(day).or_default() += play.seconds;
        let (_, month, _) = civil_from_days(day);
        by_month[month as usize - 1] += play.seconds;
    }
    let busiest = by_day
        .iter()
        .max_by(|a, b| a.1.total_cmp(b.1).then(b.0.cmp(a.0)))
        .map(|(d, s)| json!({"day": day_string(*d), "seconds": round1(*s)}));
    let (mut longest, mut run, mut previous) = (0u32, 0u32, None::<i64>);
    for day in by_day.keys() {
        run = if previous == Some(day - 1) {
            run + 1
        } else {
            1
        };
        longest = longest.max(run);
        previous = Some(*day);
    }
    let first = plays
        .iter()
        .min_by_key(|p| p.at)
        .map(|p| json!({"title": p.title, "creator": p.creator, "at": p.at}));
    json!({
        "year": year,
        "seconds": round1(seconds),
        "plays": plays.len(),
        "distinct_items": distinct.len(),
        "top_creators": top_creators(plays, WRAPPED_TOP),
        "top_items": top_items(plays.iter(), WRAPPED_TOP),
        "top_songs": top_items(plays.iter().filter(|p| p.kind == "song"), WRAPPED_TOP),
        "top_videos": top_items(plays.iter().filter(|p| p.kind == "video"), WRAPPED_TOP),
        "by_month": by_month.iter().enumerate().map(|(i, s)| json!({"month": i + 1, "seconds": round1(*s)})).collect::<Vec<_>>(),
        "busiest_day": busiest,
        "longest_streak_days": longest,
        "first_play": first,
        "new_creators": new_creators,
        "music_share": if seconds > 0.0 { (song_seconds / seconds * 1000.0).round() / 1000.0 } else { 0.0 },
        "computed_at": computed_at,
    })
}

/// Creators whose first counted play ever falls in `[from, to)`.
async fn count_new_creators(db: &Pool, user: Uuid, from: i64, to: i64) -> anyhow::Result<i64> {
    Ok(db
        .get()
        .await?
        .query_one(
            "SELECT COUNT(*) FROM (
                SELECT value->>'creator' AS creator, MIN((value->>'at')::numeric) AS first
                FROM sync_records
                WHERE user_id = $1 AND collection = 'history' AND value IS NOT NULL
                  AND jsonb_typeof(value->'at') = 'number'
                  AND jsonb_typeof(value->'seconds') = 'number'
                  AND (value->>'seconds')::float8 >= $4
                  AND btrim(COALESCE(value->>'creator', '')) <> ''
                GROUP BY 1
             ) c WHERE c.first >= ($2::BIGINT)::numeric AND c.first < ($3::BIGINT)::numeric",
            &[&user, &from, &to, &MIN_PLAY_SECONDS],
        )
        .await?
        .get(0))
}

// ---- HTTP ----

fn tz(value: Option<i32>) -> ApiResult<i32> {
    let tz = value.unwrap_or(0);
    if !(-MAX_TZ_OFFSET_MIN..=MAX_TZ_OFFSET_MIN).contains(&tz) {
        return Err(ApiError::bad(format!(
            "tz_offset_min must be between -{MAX_TZ_OFFSET_MIN} and {MAX_TZ_OFFSET_MIN}"
        )));
    }
    Ok(tz)
}

async fn throttle(state: &AppState, user: Uuid) -> ApiResult<()> {
    let key = format!("stats:{user}");
    state
        .attempt(
            &[(STATS_COMPUTE, &key)],
            "Too many statistics requests, try again in a few minutes",
        )
        .await
        .map(|_| ())
}

#[derive(Deserialize)]
struct PeriodQuery {
    from: Option<i64>,
    to: Option<i64>,
    tz_offset_min: Option<i32>,
}

async fn period(
    State(state): State<AppState>,
    device: Device,
    query: Result<Query<PeriodQuery>, QueryRejection>,
) -> ApiResult {
    let Query(q) =
        query.map_err(|_| ApiError::bad("from, to and tz_offset_min must be integers"))?;
    let (Some(from), Some(to)) = (q.from, q.to) else {
        return Err(ApiError::bad("from and to are required"));
    };
    if from >= to || to.saturating_sub(from) > MAX_PERIOD_MS {
        return Err(ApiError::bad(
            "from must be before to, at most 366 days apart",
        ));
    }
    let tz = tz(q.tz_offset_min)?;
    throttle(&state, device.user).await?;
    let plays = load_plays(&state.0.db, device.user, from, to).await?;
    Ok(Json(period_stats(&plays, from, to, tz)))
}

#[derive(Deserialize)]
struct WrappedQuery {
    year: Option<i32>,
    tz_offset_min: Option<i32>,
}

async fn wrapped(
    State(state): State<AppState>,
    device: Device,
    query: Result<Query<WrappedQuery>, QueryRejection>,
) -> ApiResult {
    let Query(q) = query.map_err(|_| ApiError::bad("year and tz_offset_min must be integers"))?;
    let tz = tz(q.tz_offset_min)?;
    let now = state.now();
    let (current, _, _) = civil_from_days(local_day(now, tz));
    let year = q.year.ok_or_else(|| ApiError::bad("year is required"))?;
    if !(i64::from(FIRST_WRAPPED_YEAR)..=current).contains(&i64::from(year)) {
        return Err(ApiError::bad(format!(
            "year must be between {FIRST_WRAPPED_YEAR} and {current}"
        )));
    }
    let db = &state.0.db;
    let cached = db
        .get()
        .await?
        .query_opt(
            "SELECT computed_at, value FROM stats_wrapped
             WHERE user_id = $1 AND year = $2 AND tz_offset_min = $3",
            &[&device.user, &year, &tz],
        )
        .await?;
    let (from, to) = (year_start_ms(year, tz), year_start_ms(year + 1, tz));
    if let Some(row) = cached {
        let computed_at: i64 = row.get(0);
        let fresh =
            computed_at > now - WRAPPED_REFRESH_MS || computed_at >= to + WRAPPED_FINAL_AFTER_MS;
        if fresh {
            return Ok(Json(row.get(1)));
        }
    }
    throttle(&state, device.user).await?;
    let plays = load_plays(db, device.user, from, to).await?;
    let new_creators = count_new_creators(db, device.user, from, to).await?;
    let value = wrapped_stats(&plays, year, tz, new_creators, now);
    let mut client = db.get().await?;
    let tx = client.transaction().await?;
    // One cached review per user and year: asking with another offset replaces it, so
    // the table cannot grow with every possible offset.
    tx.execute(
        "DELETE FROM stats_wrapped WHERE user_id = $1 AND year = $2 AND tz_offset_min <> $3",
        &[&device.user, &year, &tz],
    )
    .await?;
    tx.execute(
        "INSERT INTO stats_wrapped (user_id, year, tz_offset_min, computed_at, value)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (user_id, year, tz_offset_min)
         DO UPDATE SET computed_at = EXCLUDED.computed_at, value = EXCLUDED.value",
        &[&device.user, &year, &tz, &now, &value],
    )
    .await?;
    tx.commit().await?;
    Ok(Json(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(url: &str, creator: &str, kind: &str, at: i64, seconds: f64) -> Value {
        json!({"url": url, "title": format!("T {url}"), "creator": creator, "kind": kind,
               "thumbnail": format!("{url}/t.jpg"), "at": at, "seconds": seconds})
    }

    #[test]
    fn calendar_round_trips() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(day_string(days_from_civil(2026, 11, 23)), "2026-11-23");
        assert_eq!(day_string(days_from_civil(2024, 2, 29)), "2024-02-29");
        assert_eq!(day_string(-1), "1969-12-31");
        for d in -1000..40_000 {
            let (y, m, dd) = civil_from_days(d);
            assert_eq!(days_from_civil(y, m, dd), d);
        }
        // 2026-01-01 00:00 at UTC+1 is 2025-12-31 23:00 UTC.
        assert_eq!(
            year_start_ms(2026, 60),
            days_from_civil(2026, 1, 1) * DAY_MS - 3_600_000
        );
    }

    #[test]
    fn plays_ignore_short_starts_and_clamp() {
        let base = days_from_civil(2026, 3, 1) * DAY_MS;
        assert!(Play::from_event(&event("https://youtu.be/a", "A", "song", base, 4.9)).is_none());
        assert!(Play::from_event(&json!({"url": "https://youtu.be/a", "seconds": 60})).is_none());
        assert!(Play::from_event(&event("file:///x", "A", "song", base, 60.0)).is_none());
        let mut e = event("https://youtu.be/a", "A", "song", base, 500.0);
        e["duration"] = json!(200);
        assert_eq!(Play::from_event(&e).unwrap().seconds, 200.0);
        let long = Play::from_event(&event("https://youtu.be/a", "A", "weird", base, 1e9)).unwrap();
        assert_eq!(long.seconds, MAX_PLAY_SECONDS);
        assert_eq!(long.kind, "video");
    }

    #[test]
    fn period_buckets_by_time_zone() {
        // 23:30 UTC on 2026-11-22 is 00:30 on 2026-11-23 at UTC+1.
        let at = days_from_civil(2026, 11, 22) * DAY_MS + 23 * 3_600_000 + 30 * 60_000;
        let plays: Vec<Play> = [
            event("https://youtu.be/a", "Ann", "song", at, 100.0),
            event("https://youtu.be/a", "Ann", "song", at + 1000, 100.0),
            event("https://youtu.be/b", "Bob", "video", at + 2000, 300.0),
        ]
        .iter()
        .filter_map(Play::from_event)
        .collect();
        let utc = period_stats(&plays, 0, at + DAY_MS, 0);
        assert_eq!(utc["by_day"][0]["day"], "2026-11-22");
        assert_eq!(utc["by_hour"][23], 500.0);
        let cet = period_stats(&plays, 0, at + DAY_MS, 60);
        assert_eq!(cet["by_day"][0]["day"], "2026-11-23");
        assert_eq!(cet["by_hour"][0], 500.0);
        assert_eq!(cet["plays"], 3);
        assert_eq!(cet["distinct_items"], 2);
        assert_eq!(
            cet["by_kind"]["song"],
            json!({"plays": 2, "seconds": 200.0})
        );
        // Seconds first: Bob's single long video outranks Ann's two songs.
        assert_eq!(cet["top_creators"][0]["creator"], "Bob");
        assert_eq!(cet["top_items"][1]["plays"], 2);
        assert_eq!(
            cet["top_creators"][1]["thumbnail"],
            "https://youtu.be/a/t.jpg"
        );
    }

    #[test]
    fn wrapped_streaks_months_and_empty_year() {
        let day = |m, d| days_from_civil(2026, m, d) * DAY_MS + 12 * 3_600_000;
        let plays: Vec<Play> = [
            event("https://youtu.be/a", "A", "song", day(1, 1), 60.0),
            event("https://youtu.be/b", "B", "video", day(1, 2), 60.0),
            event("https://youtu.be/c", "C", "song", day(1, 3), 600.0),
            event("https://youtu.be/c", "C", "song", day(5, 9), 30.0),
        ]
        .iter()
        .filter_map(Play::from_event)
        .collect();
        let w = wrapped_stats(&plays, 2026, 0, 2, 7);
        assert_eq!(w["longest_streak_days"], 3);
        assert_eq!(w["busiest_day"]["day"], "2026-01-03");
        assert_eq!(w["by_month"][0]["seconds"], 720.0);
        assert_eq!(w["by_month"][4], json!({"month": 5, "seconds": 30.0}));
        assert_eq!(w["first_play"]["creator"], "A");
        assert_eq!(w["top_songs"].as_array().unwrap().len(), 2);
        assert_eq!(w["top_videos"][0]["url"], "https://youtu.be/b");
        assert_eq!(w["music_share"], 0.92); // 690 of 750 seconds
        let empty = wrapped_stats(&[], 2025, 0, 0, 7);
        assert_eq!(empty["plays"], 0);
        assert_eq!(empty["music_share"], 0.0);
        assert_eq!(empty["by_month"].as_array().unwrap().len(), 12);
        assert!(empty["busiest_day"].is_null() && empty["first_play"].is_null());
    }
}
