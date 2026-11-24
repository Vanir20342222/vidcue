//! Self-hosted Vidcue backend: accounts, device pairing, recovery and sync.

mod account;
mod admin_web;
mod api;
mod auth;
mod db;
mod health;
mod jam;
mod keys;
mod presence;
mod recs;
mod secrets;
mod social;
mod stats;
mod sync;
#[cfg(test)]
mod tests;
mod throttle;
mod web;

use anyhow::Context;
use clap::{Parser, Subcommand};
use std::net::{IpAddr, SocketAddr};

#[derive(Parser)]
#[command(name = "vidcue-server", version, about)]
struct Cli {
    /// PostgreSQL connection string.
    #[arg(long, env = "DATABASE_URL")]
    database_url: Option<String>,
    /// Public HTTPS base URL used in invite and pairing links.
    #[arg(
        long,
        env = "VIDCUE_PUBLIC_URL",
        default_value = "http://127.0.0.1:8080"
    )]
    public_url: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run migrations and serve the API.
    Serve {
        #[arg(long, env = "VIDCUE_BIND", default_value = "127.0.0.1:8080")]
        bind: SocketAddr,
        /// File with the 64-hex-character server key (a systemd credential named
        /// `secret.key` takes precedence).
        #[arg(long, env = "VIDCUE_SECRET_KEY_FILE")]
        secret_key_file: Option<String>,
        /// Reverse proxies whose X-Real-IP header is trusted (comma separated).
        #[arg(long, env = "VIDCUE_TRUSTED_PROXIES", value_delimiter = ',')]
        trusted_proxies: Vec<IpAddr>,
        /// Refuse direct connections from anything but the trusted proxies and loopback.
        #[arg(long, env = "VIDCUE_PROXIES_ONLY", default_value_t = false)]
        proxies_only: bool,
    },
    /// Run database migrations only.
    Migrate,
    /// Print a one-time signup link.
    Invite {
        /// The new account becomes an administrator.
        #[arg(long)]
        admin: bool,
        /// Hours until the link expires (1 to 720).
        #[arg(long, default_value_t = 72)]
        hours: i64,
    },
    /// Print a 48-hour link that lets a user set a new password and authenticator.
    /// Completing it signs out all of that user's devices.
    ResetLink { username: String },
    /// Print a new random server key (store it root-only, never in the database).
    GenKey,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        .init();
    let cli = Cli::parse();
    if let Command::GenKey = cli.command {
        println!("{}", secrets::ServerKey::generate_hex());
        return Ok(());
    }
    let db = db::connect(cli.database_url.as_deref().context("set DATABASE_URL")?)?;
    db::migrate(&db).await.context("run migrations")?;
    match cli.command {
        Command::GenKey => unreachable!(),
        Command::Migrate => println!("Migrations are up to date."),
        Command::Invite { admin, hours } => {
            let (url, expires) =
                api::create_invite(&db, &cli.public_url, None, admin, hours, api::now_ms()).await?;
            println!("{url}");
            eprintln!(
                "Expires in {} hours (Unix ms {expires}).",
                hours.clamp(1, 720)
            );
        }
        Command::ResetLink { username } => {
            let user = account::user_id(&db, &username)
                .await?
                .with_context(|| format!("no active user {username:?}"))?;
            let (url, _) =
                account::create_reset_link(&db, &cli.public_url, user, api::now_ms()).await?;
            println!("{url}");
        }
        Command::Serve {
            bind,
            secret_key_file,
            trusted_proxies,
            proxies_only,
        } => {
            let key = secrets::ServerKey::load(secret_key_file.as_deref())?;
            let sealed = secrets::seal_existing(&db, &key).await?;
            if sealed > 0 {
                tracing::info!("encrypted {sealed} stored authenticator secrets");
            }
            let state = api::AppState::new(
                db,
                api::Settings {
                    public_url: cli.public_url,
                    key,
                    trusted_proxies,
                    proxies_only,
                },
            );
            api::spawn_maintenance(state.clone());
            let app = api::router(state);
            let listener = tokio::net::TcpListener::bind(bind).await?;
            tracing::info!("listening on {bind}");
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(shutdown())
            .await?;
        }
    }
    Ok(())
}

async fn shutdown() {
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("install SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term.recv() => {}
    }
    tracing::info!("shutting down");
}
