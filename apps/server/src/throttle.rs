//! Attempt throttling stored in PostgreSQL.
//!
//! An attempt is recorded *before* the slow check (password hash, code comparison)
//! runs, under a per-key advisory lock, so a burst of parallel requests cannot all pass
//! the limit check before the first failure is written. A successful attempt deletes
//! its own rows again; failures stay until the window passes. Counters survive restarts.

use deadpool_postgres::Pool;

#[derive(Clone, Copy, Debug)]
pub struct Rule {
    pub limit: i64,
    pub window_ms: i64,
}

const MINUTE: i64 = 60 * 1000;

/// Logins from one client address, over all usernames.
pub const LOGIN_IP: Rule = Rule {
    limit: 10,
    window_ms: 15 * MINUTE,
};
/// Logins for one username from one address: the normal typo budget.
pub const LOGIN_USER_IP: Rule = Rule {
    limit: 5,
    window_ms: 15 * MINUTE,
};
/// Logins for one username from anywhere. High, so one address cannot lock a user out.
pub const LOGIN_USER: Rule = Rule {
    limit: 100,
    window_ms: 60 * MINUTE,
};
/// Wrong second factors after a correct password, per user: bounds authenticator
/// brute force by someone who knows the password.
pub const SECOND_FACTOR: Rule = Rule {
    limit: 5,
    window_ms: 60 * MINUTE,
};
/// Invite, pairing and reset tokens/codes per client address.
pub const CODES_IP: Rule = Rule {
    limit: 30,
    window_ms: 10 * MINUTE,
};
/// Code confirmations for one signup or reset.
pub const CONFIRM: Rule = Rule {
    limit: 10,
    window_ms: 10 * MINUTE,
};
/// Password re-checks for sensitive changes from a signed-in device, per user.
pub const REAUTH: Rule = Rule {
    limit: 5,
    window_ms: 15 * MINUTE,
};

/// Records one attempt against every `(rule, key)`. Returns the row ids to forgive on
/// success, or `None` when any key is over its limit (nothing is recorded then).
pub async fn attempt(
    db: &Pool,
    limits: &[(Rule, &str)],
    now: i64,
) -> anyhow::Result<Option<Vec<i64>>> {
    let mut client = db.get().await?;
    let tx = client.transaction().await?;
    let mut keys: Vec<&str> = limits.iter().map(|(_, k)| *k).collect();
    keys.sort_unstable();
    keys.dedup();
    // Lock in a fixed order so two requests with overlapping keys cannot deadlock.
    for key in &keys {
        tx.execute(
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 42))",
            &[key],
        )
        .await?;
    }
    for (rule, key) in limits {
        let count: i64 = tx
            .query_one(
                "SELECT COUNT(*) FROM auth_failures WHERE key = $1 AND at > $2",
                &[key, &(now - rule.window_ms)],
            )
            .await?
            .get(0);
        if count >= rule.limit {
            return Ok(None);
        }
    }
    let mut ids = Vec::with_capacity(limits.len());
    for (_, key) in limits {
        let id: i64 = tx
            .query_one(
                "INSERT INTO auth_failures (key, at) VALUES ($1, $2) RETURNING id",
                &[key, &now],
            )
            .await?
            .get(0);
        ids.push(id);
    }
    tx.commit().await?;
    Ok(Some(ids))
}

/// Removes the rows of a successful attempt.
pub async fn forgive(db: &Pool, ids: &[i64]) -> anyhow::Result<()> {
    if !ids.is_empty() {
        db.get()
            .await?
            .execute("DELETE FROM auth_failures WHERE id = ANY($1)", &[&ids])
            .await?;
    }
    Ok(())
}

pub async fn clear(db: &Pool, key: &str) -> anyhow::Result<()> {
    db.get()
        .await?
        .execute("DELETE FROM auth_failures WHERE key = $1", &[&key])
        .await?;
    Ok(())
}
