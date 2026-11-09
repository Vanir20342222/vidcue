use anyhow::{Context, Result, bail, ensure};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json::json;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;
use url::Url;
use vidcue_providers::cinecat::{
    AutoResolution, Backend, DownloadOptions, Media, Resolution, WebSources,
};

#[derive(Parser)]
#[command(
    about = "Standalone Cinecat backend and live qualification tool (independent of Android)"
)]
struct Cli {
    #[arg(long, default_value = "https://edge.cinecat.eu", global = true)]
    api: Url,
    #[arg(long, default_value_t = 2, global = true)]
    concurrency: usize,
    /// Disable the ordinary web providers and use only the sources API.
    #[arg(long, global = true)]
    api_only: bool,
    #[arg(
        long,
        default_value = "https://zcdn2.2wwlgkfo-31a.workers.dev",
        global = true,
        help = "Primary ZCDN origin (provider name 'zcdn')"
    )]
    zcdn: Url,
    #[arg(
        long,
        default_value = "https://zcdn1.2wwlgkfo-31a.workers.dev",
        global = true,
        help = "Independent backup ZCDN origin (provider name 'zcdn1')"
    )]
    zcdn1: Url,
    #[arg(
        long,
        default_value = "https://zcdn3.2wwlgkfo-31a.workers.dev",
        global = true,
        help = "Independent backup ZCDN origin (provider name 'zcdn3')"
    )]
    zcdn3: Url,
    #[arg(long, default_value = "https://cdn.hls.lol", global = true)]
    aphrodite: Url,
    #[command(subcommand)]
    command: Command,
}

#[derive(Args)]
struct Selection {
    #[arg(long)]
    tmdb: u64,
    #[arg(long, requires = "episode")]
    season: Option<u32>,
    #[arg(long, requires = "season")]
    episode: Option<u32>,
}
impl Selection {
    fn media(&self) -> Media {
        Media {
            tmdb: self.tmdb,
            season: self.season,
            episode: self.episode,
        }
    }
}

#[derive(Subcommand)]
enum Command {
    Sources,
    Resolve {
        #[command(flatten)]
        selection: Selection,
        /// Omit for automatic failover: default provider first, backups next.
        #[arg(long)]
        provider: Option<String>,
        /// Explicit failover order (comma separated); default is web providers then API order.
        #[arg(long, value_delimiter = ',')]
        order: Vec<String>,
        /// Skip real-media verification during automatic failover.
        #[arg(long)]
        no_verify: bool,
        /// Emits short-lived URLs/headers. Treat stdout as private; never commit it.
        #[arg(long)]
        show_urls: bool,
    },
    /// Independently checks every advertised provider and samples real media bytes.
    Check {
        #[command(flatten)]
        selection: Selection,
        #[arg(long, default_value_t = 6)]
        segments: usize,
        /// Sanitized report, created exclusively (will not overwrite a file).
        #[arg(long)]
        report: Option<PathBuf>,
    },
    /// Downloads a complete relocatable HLS bundle or progressive file.
    Download {
        #[command(flatten)]
        selection: Selection,
        /// Omit for automatic failover: default provider first, backups next.
        #[arg(long)]
        provider: Option<String>,
        /// Explicit failover order (comma separated); default is web providers then API order.
        #[arg(long, value_delimiter = ',')]
        order: Vec<String>,
        /// Skip real-media verification during automatic failover.
        #[arg(long)]
        no_verify: bool,
        #[arg(long, default_value_t = 0)]
        stream: usize,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = 1080)]
        max_height: u32,
        #[arg(long, value_delimiter = ',')]
        subtitle_languages: Vec<String>,
        #[arg(long, default_value_t = 21474836480_u64)]
        max_bytes: u64,
        /// Clearly marked qualification excerpt, rounded up to whole HLS segments.
        #[arg(long)]
        sample_seconds: Option<f64>,
    },
    /// Searches Granite or a caller-selected Wyzie-compatible subtitle API.
    Subtitles {
        #[command(flatten)]
        selection: Selection,
        #[arg(long)]
        endpoint: Option<Url>,
        #[arg(long, value_enum, default_value = "granite")]
        service: SubtitleService,
        #[arg(long)]
        language: Option<String>,
        /// Downloads the selected UTF-8 SRT/WebVTT results; folder must be new.
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        show_urls: bool,
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Randomized live sample: picks titles from the built-in pool, resolves
    /// with automatic failover and verification, downloads a short excerpt,
    /// and exits nonzero unless at least `--min-pass` testcases pass.
    Sample {
        #[arg(long, default_value_t = 15)]
        count: usize,
        #[arg(long, default_value_t = 0)]
        seed: u64,
        #[arg(long, default_value_t = 14)]
        min_pass: usize,
        /// Pause between testcases to stay under per-origin rate limits.
        #[arg(long, default_value_t = 3000)]
        delay_ms: u64,
        /// Disable the short excerpt download (resolve+probe only).
        #[arg(long)]
        no_excerpt: bool,
        #[arg(long, default_value_t = 10.0)]
        excerpt_seconds: f64,
        /// Sanitized JSON report, created exclusively (never overwrites).
        #[arg(long)]
        report: Option<PathBuf>,
        /// Read `tmdb [label]` lines from a file instead of the built-in pool.
        #[arg(long, conflicts_with = "count")]
        from_file: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum SubtitleService {
    Granite,
    Wyzie,
}

fn summary(resolution: &Resolution) -> serde_json::Value {
    json!({"provider":resolution.provider,"elapsed_ms":resolution.elapsed_ms,"streams":resolution.streams.iter().map(|s|json!({"kind":s.kind,"subtitle_count":s.subtitles.len()})).collect::<Vec<_>>()})
}

/// Automatic failover: default provider first, then backups in order.
async fn auto_resolve(
    backend: &Backend,
    media: &Media,
    order: Vec<String>,
    no_verify: bool,
    cancel: &CancellationToken,
) -> Result<AutoResolution> {
    let order = (!order.is_empty()).then_some(order);
    backend
        .resolve_auto(media, order.as_deref(), !no_verify, cancel)
        .await
}

fn failover_summary(auto: &AutoResolution, show_urls: bool) -> Result<serde_json::Value> {
    let mut value = if show_urls {
        serde_json::to_value(&auto.resolution)?
    } else {
        summary(&auto.resolution)
    };
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "failover".into(),
            json!(auto
                .attempts
                .iter()
                .map(|a| json!({"provider":a.provider,"succeeded":a.succeeded,"elapsed_ms":a.elapsed_ms,"error":a.error}))
                .collect::<Vec<_>>()),
        );
    }
    Ok(value)
}

/// Built-in pool of public TMDB titles used for randomized live sampling.
/// Every entry was verified resolvable through the backend when the pool was
/// curated (2026-11-09); the sample measures that the backend keeps serving
/// them, not TMDB catalog completeness. Titles without a ready upstream build
/// are intentionally excluded, because the backend cannot force them to play.
const POOL: &[PoolEntry] = &[
    PoolEntry {
        label: "Fight Club",
        tmdb: 550,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Inception",
        tmdb: 27205,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "The Matrix",
        tmdb: 603,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Interstellar",
        tmdb: 157336,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Parasite",
        tmdb: 496243,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Avengers Endgame",
        tmdb: 299534,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Dune",
        tmdb: 438631,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Dune Part Two",
        tmdb: 693134,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Oppenheimer",
        tmdb: 872585,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Barbie",
        tmdb: 346698,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Spider-Man No Way Home",
        tmdb: 634649,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "The Godfather",
        tmdb: 238,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Spirited Away",
        tmdb: 129,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Mad Max Fury Road",
        tmdb: 76341,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Everything Everywhere All At Once",
        tmdb: 545611,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Top Gun Maverick",
        tmdb: 361743,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Metropolis",
        tmdb: 19,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Nosferatu",
        tmdb: 1091,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "The Terminator",
        tmdb: 218,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Jurassic Park",
        tmdb: 329,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Toy Story",
        tmdb: 862,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Home Alone",
        tmdb: 771,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Project Hail Mary",
        tmdb: 687163,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "The Dark Knight",
        tmdb: 155,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Resident Evil",
        tmdb: 1423191,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Apocalypse Z",
        tmdb: 1507696,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Hope",
        tmdb: 1058424,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Spider-Man Brand New Day",
        tmdb: 969681,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "The Odyssey",
        tmdb: 1368337,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Heart of the Beast",
        tmdb: 1263337,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "A Story of Derry",
        tmdb: 1813355,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "The Hotel New Hampshire",
        tmdb: 11308,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Coco",
        tmdb: 354912,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "The Mongoose",
        tmdb: 1294189,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Verity",
        tmdb: 1283515,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Fall 2 Deadpoint",
        tmdb: 1101412,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "La Bola Negra",
        tmdb: 1422041,
        season: None,
        episode: None,
    },
    PoolEntry {
        label: "Breaking Bad S1E1",
        tmdb: 1396,
        season: Some(1),
        episode: Some(1),
    },
    PoolEntry {
        label: "Friends S1E1",
        tmdb: 1668,
        season: Some(1),
        episode: Some(1),
    },
];

struct PoolEntry {
    label: &'static str,
    tmdb: u64,
    season: Option<u32>,
    episode: Option<u32>,
}

/// Owned testcase drawn either from the built-in pool or a caller list file.
#[derive(Clone)]
struct SampleEntry {
    label: String,
    tmdb: u64,
    season: Option<u32>,
    episode: Option<u32>,
}

/// Reads `tmdb [label]` lines (blank lines and `#` comments ignored) from a
/// caller-built list file, for sweeps larger than the built-in pool.
fn load_sample_entries(path: &std::path::Path) -> Result<Vec<SampleEntry>> {
    let text = std::fs::read_to_string(path).context("Sample list is unreadable")?;
    let mut entries = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(2, char::is_whitespace);
        let id: u64 = parts
            .next()
            .unwrap_or_default()
            .parse()
            .with_context(|| format!("Sample list line {} is not a TMDB ID", number + 1))?;
        ensure!(
            id > 0,
            "Sample list line {} has an invalid TMDB ID",
            number + 1
        );
        let label = parts
            .next()
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("tmdb:{id}"));
        entries.push(SampleEntry {
            label,
            tmdb: id,
            season: None,
            episode: None,
        });
    }
    ensure!(!entries.is_empty(), "Sample list is empty");
    ensure!(entries.len() <= 2048, "Sample list exceeds the entry limit");
    Ok(entries)
}

/// Deterministic xorshift64* so a seed always reproduces the same sample.
fn xorshift(state: &mut u64) -> u64 {
    *state ^= *state >> 12;
    *state ^= *state << 25;
    *state ^= *state >> 27;
    state.wrapping_mul(0x2545F4914F6CDD1D)
}

/// Seed 0 must still produce varied, reproducible picks.
fn select_pool(seed: u64, count: usize) -> Vec<&'static PoolEntry> {
    let mut state = seed | 1;
    let mut indices: Vec<usize> = (0..POOL.len()).collect();
    for i in (1..indices.len()).rev() {
        let j = (xorshift(&mut state) % (i as u64 + 1)) as usize;
        indices.swap(i, j);
    }
    indices.into_iter().take(count).map(|i| &POOL[i]).collect()
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!("{prefix}-{nanos}"))
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        // Errors from the library never contain remote URLs, headers or response bodies.
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    let mut backend = Backend::new(cli.api, cli.concurrency)?;
    if !cli.api_only {
        let primary = cli.zcdn;
        let mut sources = WebSources::new(primary.clone(), cli.aphrodite)?;
        if cli.zcdn1 != primary {
            sources = sources.with_zcdn_backup("zcdn1", cli.zcdn1)?;
        }
        if cli.zcdn3 != primary {
            sources = sources.with_zcdn_backup("zcdn3", cli.zcdn3)?;
        }
        backend = backend.with_web_sources(sources);
    }
    let cancel = CancellationToken::new();
    let signal_cancel = cancel.clone();
    let signal = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            signal_cancel.cancel();
        }
    });
    let result = execute(backend, cli.command, &cancel).await;
    signal.abort();
    result
}

async fn execute(backend: Backend, command: Command, cancel: &CancellationToken) -> Result<()> {
    match command {
        Command::Sources => println!(
            "{}",
            serde_json::to_string_pretty(&backend.providers(cancel).await?)?
        ),
        Command::Resolve {
            selection,
            provider,
            order,
            no_verify,
            show_urls,
        } => {
            let value = match provider {
                Some(provider) => {
                    let resolution = backend
                        .resolve(&provider, &selection.media(), cancel)
                        .await?;
                    if show_urls {
                        serde_json::to_value(&resolution)?
                    } else {
                        summary(&resolution)
                    }
                }
                None => failover_summary(
                    &auto_resolve(&backend, &selection.media(), order, no_verify, cancel).await?,
                    show_urls,
                )?,
            };
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
        Command::Check {
            selection,
            segments,
            report,
        } => {
            let reports = backend
                .check_all(&selection.media(), segments, cancel)
                .await?;
            let passed = reports.iter().all(|p| {
                p.resolved
                    && p.error.is_none()
                    && !p.network.is_empty()
                    && p.network.iter().all(|n| n.sustainable == Some(true))
            });
            let observed_unix_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis();
            let value = json!({"observed_unix_ms":observed_unix_ms,"backend_version":env!("CARGO_PKG_VERSION"),"tmdb":selection.tmdb,"all_providers_passed":passed,"scope":"configured web providers and sources API", "decoder_verified":false,"providers":reports});
            let text = serde_json::to_string_pretty(&value)?;
            if let Some(path) = report {
                use tokio::io::AsyncWriteExt;
                let mut output = tokio::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .await?;
                output.write_all(text.as_bytes()).await?;
                output.sync_all().await?;
            }
            println!("{text}");
            ensure!(
                passed,
                "One or more providers failed live qualification (see report)"
            );
        }
        Command::Download {
            selection,
            provider,
            order,
            no_verify,
            stream,
            output,
            max_height,
            subtitle_languages,
            max_bytes,
            sample_seconds,
        } => {
            ensure!(max_height > 0, "Max height must be positive");
            let resolution = match provider {
                Some(provider) => {
                    backend
                        .resolve(&provider, &selection.media(), cancel)
                        .await?
                }
                None => {
                    auto_resolve(&backend, &selection.media(), order, no_verify, cancel)
                        .await?
                        .resolution
                }
            };
            let mut source = resolution
                .streams
                .get(stream)
                .context("Stream index is out of range")?
                .clone();
            if !subtitle_languages.is_empty() && source.subtitles.is_empty() {
                let matches = backend
                    .search_granite_subtitles(
                        &Url::parse("https://sub.vdrk.site")?,
                        &selection.media(),
                        cancel,
                    )
                    .await?;
                for language in &subtitle_languages {
                    let caption = matches
                        .iter()
                        .find(|s| s.language.eq_ignore_ascii_case(language))
                        .with_context(|| format!("No external subtitle match for {language}"))?;
                    source.subtitles.push(caption.clone());
                }
            }
            let mut options = DownloadOptions::new(output);
            options.max_height = Some(max_height);
            options.subtitle_languages = subtitle_languages;
            options.max_bytes = max_bytes;
            options.sample_seconds = sample_seconds;
            println!(
                "{}",
                serde_json::to_string_pretty(&backend.download(&source, &options, cancel).await?)?
            );
        }
        Command::Sample {
            count,
            seed,
            min_pass,
            delay_ms,
            no_excerpt,
            excerpt_seconds,
            report,
            from_file,
        } => {
            let picks: Vec<SampleEntry> = match from_file {
                Some(path) => load_sample_entries(&path)?,
                None => {
                    ensure!(
                        (1..=POOL.len()).contains(&count),
                        "Sample count exceeds the pool size"
                    );
                    select_pool(seed, count)
                        .into_iter()
                        .map(|entry| SampleEntry {
                            label: entry.label.to_string(),
                            tmdb: entry.tmdb,
                            season: entry.season,
                            episode: entry.episode,
                        })
                        .collect()
                }
            };
            let count = picks.len();
            ensure!(
                (1..=count).contains(&min_pass),
                "Min pass must be between 1 and the sample count"
            );
            ensure!(
                no_excerpt || excerpt_seconds > 0.0,
                "Excerpt seconds must be positive"
            );
            let mut results = Vec::new();
            let mut passed = 0usize;
            for (index, entry) in picks.iter().enumerate() {
                ensure!(!cancel.is_cancelled(), "Cancelled");
                if index > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                let media = Media {
                    tmdb: entry.tmdb,
                    season: entry.season,
                    episode: entry.episode,
                };
                let mut result = json!({
                    "label": entry.label,
                    "tmdb": entry.tmdb,
                    "season": entry.season,
                    "episode": entry.episode,
                    "ok": false,
                    "provider": serde_json::Value::Null,
                    "error": serde_json::Value::Null,
                    "excerpt_bytes": serde_json::Value::Null,
                });
                let started = std::time::Instant::now();
                let outcome = backend.resolve_auto(&media, None, true, cancel).await;
                match outcome {
                    Err(error) => {
                        result["error"] = json!(error.to_string());
                    }
                    Ok(auto) => {
                        result["provider"] = json!(auto.resolution.provider);
                        if no_excerpt {
                            result["ok"] = json!(true);
                            passed += 1;
                        } else {
                            let output = unique_temp_dir("vidcue-cinecat-sample");
                            let mut options = DownloadOptions::new(output.clone());
                            options.max_height = Some(1080);
                            options.sample_seconds = Some(excerpt_seconds);
                            options.max_bytes = 512 * 1024 * 1024;
                            let source = auto.resolution.streams[0].clone();
                            match backend.download(&source, &options, cancel).await {
                                Ok(receipt) => {
                                    result["ok"] = json!(true);
                                    result["excerpt_bytes"] = json!(receipt.bytes);
                                    passed += 1;
                                    // Excerpts are throwaway qualification artifacts.
                                    let _ = std::fs::remove_dir_all(&output);
                                }
                                Err(error) => {
                                    result["error"] = json!(format!("excerpt: {}", error));
                                }
                            }
                        }
                    }
                }
                result["elapsed_ms"] = json!(started.elapsed().as_millis() as u64);
                println!(
                    "[{}/{}] {}: {}",
                    index + 1,
                    count,
                    entry.label,
                    if result["ok"] == json!(true) {
                        format!("PASS via {}", result["provider"])
                    } else {
                        format!("FAIL: {}", result["error"])
                    }
                );
                results.push(result);
            }
            let failed = count - passed;
            let value = json!({
                "observed_unix_ms": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_millis(),
                "backend_version": env!("CARGO_PKG_VERSION"),
                "seed": seed,
                "count": count,
                "passed": passed,
                "failed": failed,
                "min_pass": min_pass,
                "goal_met": passed >= min_pass,
                "scope": "auto failover resolve + verification + excerpt download",
                "results": results,
            });
            let text = serde_json::to_string_pretty(&value)?;
            if let Some(path) = report {
                use tokio::io::AsyncWriteExt;
                let mut output = tokio::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .await?;
                output.write_all(text.as_bytes()).await?;
                output.sync_all().await?;
            }
            println!(
                "Sample: {passed}/{count} passed (goal {min_pass}/{count}){}",
                if passed >= min_pass {
                    ""
                } else {
                    " — GOAL NOT MET"
                }
            );
            ensure!(
                passed >= min_pass,
                "Sample goal not met: {passed}/{count} passed, needed {min_pass}"
            );
        }
        Command::Subtitles {
            selection,
            endpoint,
            service,
            language,
            output,
            show_urls,
            limit,
        } => {
            let key = std::env::var("CINECAT_SUBTITLE_KEY").ok();
            let mut subtitles = match service {
                SubtitleService::Granite => {
                    backend
                        .search_granite_subtitles(
                            &endpoint.unwrap_or(Url::parse("https://sub.vdrk.site")?),
                            &selection.media(),
                            cancel,
                        )
                        .await?
                }
                SubtitleService::Wyzie => {
                    backend
                        .search_subtitles(
                            &endpoint.unwrap_or(Url::parse("https://sub.wyzie.io/search")?),
                            &selection.media(),
                            key.as_deref(),
                            cancel,
                        )
                        .await?
                }
            };
            if let Some(language) = language {
                subtitles.retain(|s| {
                    s.language.eq_ignore_ascii_case(&language)
                        || s.display.eq_ignore_ascii_case(&language)
                });
            }
            if let Some(limit) = limit {
                ensure!((1..=2048).contains(&limit), "Subtitle limit must be 1–2048");
                subtitles.truncate(limit);
            }
            if subtitles.is_empty() {
                bail!("No subtitles returned for the requested selection");
            }
            if let Some(output) = output {
                backend
                    .download_subtitles(&subtitles, &output, cancel)
                    .await?;
            }
            let value = if show_urls {
                serde_json::to_value(&subtitles)?
            } else {
                json!(
                    subtitles
                        .iter()
                        .map(
                            |s| json!({"language":s.language,"display":s.display,"format":s.format})
                        )
                        .collect::<Vec<_>>()
                )
            };
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_sampling_is_deterministic_unique_and_seed_sensitive() {
        let a = select_pool(0, 15);
        let b = select_pool(0, 15);
        assert_eq!(a.len(), 15);
        assert_eq!(b.len(), 15);
        for (x, y) in a.iter().zip(b.iter()) {
            assert!(std::ptr::eq(*x, *y), "Same seed must reproduce the sample");
        }
        let names: std::collections::HashSet<&str> = a.iter().map(|e| e.label).collect();
        assert_eq!(names.len(), 15, "No duplicate picks within one sample");
        let other = select_pool(42, 15);
        assert_ne!(
            a.iter().map(|e| e.tmdb).collect::<Vec<_>>(),
            other.iter().map(|e| e.tmdb).collect::<Vec<_>>(),
            "Different seeds must sample differently"
        );
        let full = select_pool(7, POOL.len());
        let unique: std::collections::HashSet<u64> = full.iter().map(|e| e.tmdb).collect();
        assert_eq!(unique.len(), POOL.len());
        // Media rules: TV entries carry both season and episode.
        for entry in &full {
            assert_eq!(
                entry.season.is_some(),
                entry.episode.is_some(),
                "{} has an inconsistent season/episode pair",
                entry.label
            );
        }
    }

    #[test]
    fn sample_lists_parse_ids_labels_comments_and_reject_garbage() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("list.txt");
        std::fs::write(
            &path,
            "# sweep list\n\
             550 Fight Club\n\
             \n\
             27205  Inception  \n\
             603\n",
        )
        .unwrap();
        let entries = load_sample_entries(&path).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].tmdb, 550);
        assert_eq!(entries[0].label, "Fight Club");
        assert_eq!(entries[1].tmdb, 27205);
        assert_eq!(entries[1].label, "Inception");
        assert_eq!(entries[2].tmdb, 603);
        assert_eq!(entries[2].label, "tmdb:603");
        assert!(
            entries
                .iter()
                .all(|e| e.season.is_none() && e.episode.is_none())
        );

        let bad = temp.path().join("bad.txt");
        std::fs::write(&bad, "not-an-id\n").unwrap();
        assert!(
            load_sample_entries(&bad)
                .err()
                .unwrap()
                .to_string()
                .contains("not a TMDB ID")
        );
        let zero = temp.path().join("zero.txt");
        std::fs::write(&zero, "0\n").unwrap();
        assert!(
            load_sample_entries(&zero)
                .err()
                .unwrap()
                .to_string()
                .contains("invalid TMDB ID")
        );
        let empty = temp.path().join("empty.txt");
        std::fs::write(&empty, "# only a comment\n\n").unwrap();
        assert!(
            load_sample_entries(&empty)
                .err()
                .unwrap()
                .to_string()
                .contains("empty")
        );
    }
}
