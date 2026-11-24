//! PostgreSQL pool and embedded, ordered migrations.

use anyhow::Context;
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod, Runtime};
use std::time::Duration;

/// Applied in order, each once, inside one transaction with the schema record.
const MIGRATIONS: &[(i32, &str, &str)] = &[
    (
        1,
        "accounts_sync",
        include_str!("../migrations/0001_accounts_sync.sql"),
    ),
    (
        2,
        "recovery_throttle_keys",
        include_str!("../migrations/0002_recovery_throttle_keys.sql"),
    ),
    (
        3,
        "hardening",
        include_str!("../migrations/0003_hardening.sql"),
    ),
    (
        4,
        "social_realtime",
        include_str!("../migrations/0004_social_realtime.sql"),
    ),
    (
        5,
        "personal",
        include_str!("../migrations/0005_personal.sql"),
    ),
];

pub fn connect(url: &str) -> anyhow::Result<Pool> {
    let mut config: tokio_postgres::Config = url.parse().context("parse DATABASE_URL")?;
    // No single query may hold a connection for long.
    config.options("-c statement_timeout=15000 -c lock_timeout=10000");
    let manager = Manager::from_config(
        config,
        tokio_postgres::NoTls,
        ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        },
    );
    Pool::builder(manager)
        .max_size(16)
        .runtime(Runtime::Tokio1)
        .wait_timeout(Some(Duration::from_secs(5)))
        .create_timeout(Some(Duration::from_secs(5)))
        .recycle_timeout(Some(Duration::from_secs(5)))
        .build()
        .context("build PostgreSQL pool")
}

pub async fn migrate(pool: &Pool) -> anyhow::Result<()> {
    let mut client = pool.get().await.context("connect to PostgreSQL")?;
    let tx = client.transaction().await?;
    // Two server processes starting together must not both migrate.
    tx.execute("SELECT pg_advisory_xact_lock(7210452)", &[])
        .await?;
    tx.batch_execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INT PRIMARY KEY, name TEXT NOT NULL, applied_at TIMESTAMPTZ NOT NULL DEFAULT now())",
    )
    .await?;
    let applied: Vec<i32> = tx
        .query("SELECT version FROM schema_migrations", &[])
        .await?
        .iter()
        .map(|r| r.get(0))
        .collect();
    for (version, name, sql) in MIGRATIONS {
        if applied.contains(version) {
            continue;
        }
        tx.batch_execute(sql)
            .await
            .with_context(|| format!("migration {version} {name}"))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name) VALUES ($1, $2)",
            &[version, name],
        )
        .await?;
        tracing::info!("applied migration {version} {name}");
    }
    tx.commit().await?;
    Ok(())
}
