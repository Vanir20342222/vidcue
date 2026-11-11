//! Opt-out check for a newer desktop release on GitHub, and the user-started updater.
//!
//! The check only reads the public releases list. Nothing is downloaded or installed
//! until the user presses "Update now". On Linux that rebuilds the release tag from
//! source with the same `install.sh` the user installed with; on Windows it downloads
//! the release's `VidcueSetup-*.exe`, checks it against GitHub's SHA-256 digest and
//! runs it silently over the existing installation.
use anyhow::{Context, Result, bail, ensure};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

/// Canonical repository. Releases also mirror to Vanir20342222/ytui for older installs.
pub const REPOSITORY: &str = "Vanir20342222/vidcue";
pub const API_BASE: &str = "https://api.github.com";
/// Release assets are only fetched from these repositories' download URLs.
const ASSET_PREFIXES: [&str; 2] = [
    "https://github.com/Vanir20342222/vidcue/releases/download/",
    "https://github.com/Vanir20342222/ytui/releases/download/",
];
/// An installer is a few tens of MB; refuse anything absurd rather than fill the disk.
const MAX_ASSET_BYTES: u64 = 1 << 30;

/// A file attached to a release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
    /// Lower-case hex SHA-256 from GitHub's `digest` field, when GitHub reports one.
    pub sha256: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    pub version: String,
    pub name: String,
    pub url: String,
    /// First few non-empty lines of the release notes.
    pub notes: Vec<String>,
    pub assets: Vec<Asset>,
}

impl Release {
    /// The Windows installer attached to this release (`VidcueSetup-<version>.exe`).
    pub fn windows_installer(&self) -> Option<&Asset> {
        self.assets
            .iter()
            .find(|a| a.name.starts_with("VidcueSetup-") && a.name.ends_with(".exe"))
    }
}

fn assets(release: &serde_json::Value) -> Vec<Asset> {
    release["assets"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|a| {
                    Some(Asset {
                        name: a["name"].as_str()?.into(),
                        url: a["browser_download_url"].as_str()?.into(),
                        size: a["size"].as_u64().unwrap_or(0),
                        sha256: a["digest"]
                            .as_str()
                            .and_then(|d| d.strip_prefix("sha256:"))
                            .filter(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()))
                            .map(str::to_ascii_lowercase),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Desktop release tags are `v1.2.3`, `desktop-v1.2.3` or `desktop-1.2.3` (with an
/// optional `-alpha.N` style suffix). Android tags (`android-…`) never match.
pub fn desktop_version(tag: &str) -> Option<&str> {
    let version = tag.strip_prefix("desktop-").unwrap_or(tag);
    let version = version.strip_prefix('v').unwrap_or(version);
    (tag.starts_with('v') || tag.starts_with("desktop-"))
        .then_some(version)
        .filter(|v| parse(v).is_some())
}

/// Newest desktop release that is newer than `current`. Pre-releases are offered only
/// while the running build is itself a pre-release, so stable users stay on stable.
pub fn newest(releases: &serde_json::Value, current: &str) -> Option<Release> {
    let current_pre = parse(current).is_some_and(|(_, pre)| !pre.is_empty());
    releases
        .as_array()?
        .iter()
        .filter(|r| !r["draft"].as_bool().unwrap_or(false))
        .filter(|r| current_pre || !r["prerelease"].as_bool().unwrap_or(false))
        .filter_map(|r| {
            let tag = r["tag_name"].as_str()?;
            let version = desktop_version(tag)?;
            (compare(version, current) == Some(Ordering::Greater)).then(|| Release {
                tag: tag.into(),
                version: version.into(),
                name: r["name"]
                    .as_str()
                    .filter(|n| !n.is_empty())
                    .unwrap_or(tag)
                    .into(),
                url: r["html_url"].as_str().unwrap_or_default().into(),
                notes: r["body"]
                    .as_str()
                    .unwrap_or_default()
                    .lines()
                    .map(|l| l.trim().trim_start_matches(['#', '-', '*', ' ']).trim())
                    .filter(|l| !l.is_empty())
                    .take(4)
                    .map(|l| l.chars().take(90).collect())
                    .collect(),
                assets: assets(r),
            })
        })
        .max_by(|a, b| compare(&a.version, &b.version).unwrap_or(Ordering::Equal))
}

/// Fetch the release list and return the newest desktop release above `current`.
pub async fn check(api_base: &str, proxy: Option<&str>, current: &str) -> Result<Option<Release>> {
    let response = client(proxy, "update check", 15)?
        .get(format!(
            "{api_base}/repos/{REPOSITORY}/releases?per_page=30"
        ))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .context("Cannot reach GitHub to check for updates")?;
    ensure!(
        response.status().is_success(),
        "GitHub answered {} to the update check",
        response.status()
    );
    let releases: serde_json::Value = response.json().await?;
    Ok(newest(&releases, current))
}

fn client(proxy: Option<&str>, purpose: &str, timeout: u64) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .user_agent(format!("vidcue/{} ({purpose})", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(timeout))
        // Release downloads redirect from github.com to GitHub's object storage.
        .redirect(reqwest::redirect::Policy::limited(5));
    if let Some(proxy) = proxy.filter(|p| !p.is_empty()) {
        builder = builder.proxy(reqwest::Proxy::all(proxy).context("Invalid proxy URL")?);
    }
    Ok(builder.build()?)
}

/// Downloads `asset` into `dir` and checks its SHA-256 against GitHub's digest. Only
/// assets of this project's releases with a reported digest are accepted. `progress`
/// counts the bytes received so far. Returns the downloaded file.
pub async fn download(
    asset: &Asset,
    dir: &Path,
    proxy: Option<&str>,
    progress: &AtomicU64,
) -> Result<PathBuf> {
    ensure!(
        ASSET_PREFIXES.iter().any(|p| asset.url.starts_with(p)),
        "Refusing a download from outside Vidcue's releases: {}",
        asset.url
    );
    download_from(asset, &asset.url, dir, proxy, progress).await
}

async fn download_from(
    asset: &Asset,
    url: &str,
    dir: &Path,
    proxy: Option<&str>,
    progress: &AtomicU64,
) -> Result<PathBuf> {
    ensure!(
        !asset.name.is_empty()
            && !asset.name.starts_with('.')
            && asset
                .name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')),
        "Refusing an unusual file name: {}",
        asset.name
    );
    let Some(expected) = asset.sha256.as_deref() else {
        bail!(
            "GitHub reports no checksum for {}, so it cannot be verified",
            asset.name
        );
    };
    ensure!(
        asset.size <= MAX_ASSET_BYTES,
        "{} is unexpectedly large",
        asset.name
    );
    progress.store(0, AtomicOrdering::Relaxed);
    let mut response = client(proxy, "update download", 30 * 60)?
        .get(url)
        .send()
        .await
        .context("Cannot reach GitHub to download the update")?;
    ensure!(
        response.status().is_success(),
        "GitHub answered {} to the download",
        response.status()
    );
    std::fs::create_dir_all(dir)?;
    let path = dir.join(&asset.name);
    let partial = dir.join(format!("{}.part", asset.name));
    let mut file = std::fs::File::create(&partial)
        .with_context(|| format!("Cannot write {}", partial.display()))?;
    let mut hasher = Sha256::new();
    let mut received = 0u64;
    let result: Result<()> = async {
        while let Some(chunk) = response
            .chunk()
            .await
            .context("The download was interrupted")?
        {
            received += chunk.len() as u64;
            ensure!(
                received <= MAX_ASSET_BYTES,
                "{} is unexpectedly large",
                asset.name
            );
            hasher.update(&chunk);
            std::io::Write::write_all(&mut file, &chunk)?;
            progress.store(received, AtomicOrdering::Relaxed);
        }
        file.sync_all()?;
        let actual = hex::encode(hasher.finalize());
        ensure!(
            actual == expected,
            "The downloaded {} does not match its published checksum",
            asset.name
        );
        Ok(())
    }
    .await;
    drop(file);
    if let Err(e) = result {
        let _ = std::fs::remove_file(&partial);
        return Err(e);
    }
    std::fs::rename(&partial, &path)?;
    Ok(path)
}

/// Arguments for running a downloaded Inno Setup installer over the current installation:
/// no wizard pages, Vidcue closed if still running, and the new version started afterwards
/// (`/RELAUNCH=1`, read by `tools/windows/vidcue.iss`).
pub const WINDOWS_INSTALLER_ARGS: [&str; 6] = [
    "/SILENT",
    "/SUPPRESSMSGBOXES",
    "/NORESTART",
    "/CLOSEAPPLICATIONS",
    "/NORESTARTAPPLICATIONS",
    "/RELAUNCH=1",
];

/// Shell program run by "Update now": clone the release tag and run its installer, which
/// builds with cargo and atomically replaces the vidcue launchers (keeping
/// `vidcue.previous` for `tools/rollback-install.sh`).
const INSTALL_SCRIPT: &str = r#"set -euo pipefail
tag="$1"
command -v git >/dev/null || { echo 'git is required to update vidcue.' >&2; exit 1; }
command -v cargo >/dev/null || { echo 'cargo (Rust) is required to update vidcue.' >&2; exit 1; }
dir="$(mktemp -d "${TMPDIR:-/tmp}/vidcue-update.XXXXXX")"
trap 'rm -rf -- "$dir"' EXIT
echo "Downloading vidcue $tag…"
git clone --quiet --depth 1 --branch "$tag" -- "https://github.com/$2.git" "$dir/src"
echo "Building and installing (this can take a few minutes)…"
bash "$dir/src/install.sh"
"#;

/// The updater process for `release`. Tags come from GitHub, so only plain tag
/// characters are accepted, and they are passed as arguments, never spliced into code.
pub fn install_command(release: &Release) -> Result<std::process::Command> {
    ensure!(
        !release.tag.starts_with('-')
            && release
                .tag
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')),
        "Refusing an unusual release tag: {}",
        release.tag
    );
    let mut command = std::process::Command::new("bash");
    command
        .arg("-c")
        .arg(INSTALL_SCRIPT)
        .arg("vidcue-update")
        .arg(&release.tag)
        .arg(REPOSITORY);
    Ok(command)
}

type Version<'a> = ([u64; 3], Vec<&'a str>);

fn parse(version: &str) -> Option<Version<'_>> {
    let version = version.split('+').next()?;
    let (core, pre) = version.split_once('-').unwrap_or((version, ""));
    let mut numbers = core.split('.').map(|n| n.parse::<u64>().ok());
    let core = [numbers.next()??, numbers.next()??, numbers.next()??];
    if numbers.next().is_some() {
        return None;
    }
    let pre: Vec<_> = pre.split('.').filter(|p| !p.is_empty()).collect();
    Some((core, pre))
}

/// Semantic-version precedence (`0.2.0-alpha.9 < 0.2.0-alpha.12 < 0.2.0 < 0.2.1`).
pub fn compare(a: &str, b: &str) -> Option<Ordering> {
    let ((a_core, a_pre), (b_core, b_pre)) = (parse(a)?, parse(b)?);
    Some(
        a_core
            .cmp(&b_core)
            .then_with(|| match (a_pre.is_empty(), b_pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => {
                    for (x, y) in a_pre.iter().zip(&b_pre) {
                        let order = match (x.parse::<u64>(), y.parse::<u64>()) {
                            (Ok(x), Ok(y)) => x.cmp(&y),
                            (Ok(_), Err(_)) => Ordering::Less,
                            (Err(_), Ok(_)) => Ordering::Greater,
                            (Err(_), Err(_)) => x.cmp(y),
                        };
                        if order != Ordering::Equal {
                            return order;
                        }
                    }
                    a_pre.len().cmp(&b_pre.len())
                }
            }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn versions_follow_semver_precedence() {
        let ordered = [
            "0.1.9",
            "0.2.0-alpha.2",
            "0.2.0-alpha.12",
            "0.2.0-beta",
            "0.2.0-beta.1",
            "0.2.0-rc.1",
            "0.2.0",
            "0.2.1",
            "0.10.0",
            "1.0.0",
        ];
        for pair in ordered.windows(2) {
            assert_eq!(compare(pair[0], pair[1]), Some(Ordering::Less), "{pair:?}");
            assert_eq!(
                compare(pair[1], pair[0]),
                Some(Ordering::Greater),
                "{pair:?}"
            );
        }
        assert_eq!(compare("1.2.3+build", "1.2.3"), Some(Ordering::Equal));
        assert_eq!(compare("1.2", "1.2.0"), None);
        assert_eq!(compare("x.y.z", "1.0.0"), None);
    }

    #[test]
    fn only_desktop_tags_are_versions() {
        assert_eq!(desktop_version("v0.3.0"), Some("0.3.0"));
        assert_eq!(
            desktop_version("desktop-v0.3.0-alpha.1"),
            Some("0.3.0-alpha.1")
        );
        assert_eq!(desktop_version("desktop-0.3.0"), Some("0.3.0"));
        assert_eq!(desktop_version("android-0.2.0-alpha.12-mobile.10"), None);
        assert_eq!(
            desktop_version("android-preview-0.2.0-alpha.12-mobile.15"),
            None
        );
        assert_eq!(desktop_version("0.3.0"), None);
        assert_eq!(desktop_version("vnext"), None);
    }

    fn releases() -> serde_json::Value {
        json!([
            {"tag_name": "android-preview-9.9.9", "prerelease": true, "draft": false},
            {"tag_name": "v0.4.0", "draft": true, "prerelease": false},
            {"tag_name": "v0.3.0-beta.1", "prerelease": true, "draft": false, "name": "Beta"},
            {"tag_name": "v0.2.1", "prerelease": false, "draft": false, "name": "Vidcue 0.2.1",
             "html_url": "https://example.invalid/r/0.2.1",
             "body": "## What's new\n\n- Colourful themes\n- Update popup\n\n* Faster search\n- Four\n- Five",
             "assets": [
                {"name": "Vidcue-portable.zip", "size": 3,
                 "browser_download_url": "https://github.com/Vanir20342222/vidcue/releases/download/v0.2.1/Vidcue-portable.zip",
                 "digest": "sha256:BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"},
                {"name": "VidcueSetup-0.2.1.exe", "size": 3,
                 "browser_download_url": "https://github.com/Vanir20342222/vidcue/releases/download/v0.2.1/VidcueSetup-0.2.1.exe",
                 "digest": "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"},
                {"name": "no-digest.bin", "size": 1, "browser_download_url": "https://x.invalid/a", "digest": "md5:abc"},
                {"size": 1}
             ]},
            {"tag_name": "v0.2.0-alpha.11", "prerelease": true, "draft": false}
        ])
    }

    #[test]
    fn prerelease_builds_see_newer_prereleases_but_never_drafts_or_android() {
        let found = newest(&releases(), "0.2.0-alpha.12").unwrap();
        assert_eq!(found.tag, "v0.3.0-beta.1");
        assert_eq!(found.name, "Beta");
    }

    #[test]
    fn stable_builds_see_only_stable_releases_with_short_notes() {
        let found = newest(&releases(), "0.2.0").unwrap();
        assert_eq!(found.tag, "v0.2.1");
        assert_eq!(found.version, "0.2.1");
        assert_eq!(found.url, "https://example.invalid/r/0.2.1");
        assert_eq!(
            found.notes,
            [
                "What's new",
                "Colourful themes",
                "Update popup",
                "Faster search"
            ]
        );
        assert_eq!(
            found.assets.len(),
            3,
            "an asset without name or URL is skipped"
        );
        assert_eq!(found.assets[2].sha256, None, "only sha256 digests count");
        let setup = found.windows_installer().unwrap();
        assert_eq!(setup.name, "VidcueSetup-0.2.1.exe");
        assert_eq!(setup.sha256.as_deref(), Some(ABC_SHA256));
        assert_eq!(
            found.assets[0].sha256.as_deref(),
            Some(ABC_SHA256),
            "lower-cased"
        );
        assert_eq!(newest(&releases(), "0.2.1"), None);
        assert_eq!(newest(&json!({"message": "rate limited"}), "0.2.0"), None);
    }

    #[tokio::test]
    async fn check_reads_the_release_list_from_the_api() {
        let (base, server) = crate::test_http::serve(|target| {
            assert_eq!(target, "/repos/Vanir20342222/vidcue/releases?per_page=30");
            (
                200,
                "Content-Type: application/json\r\n".into(),
                releases().to_string().into_bytes(),
            )
        })
        .await;
        let found = check(&base, None, "0.2.0").await.unwrap().unwrap();
        assert_eq!(found.tag, "v0.2.1");
        server.abort();
        let (base, server) =
            crate::test_http::serve(|_| (403, String::new(), b"{}".to_vec())).await;
        assert!(check(&base, None, "0.2.0").await.is_err());
        server.abort();
    }

    /// SHA-256 of "abc".
    const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    fn setup_asset() -> Asset {
        newest(&releases(), "0.2.0")
            .unwrap()
            .windows_installer()
            .unwrap()
            .clone()
    }

    #[tokio::test]
    async fn download_keeps_a_file_only_when_its_checksum_matches() {
        let dir = tempfile::tempdir().unwrap();
        let asset = setup_asset();
        let progress = AtomicU64::new(0);
        let (base, server) =
            crate::test_http::serve(|_| (200, String::new(), b"abc".to_vec())).await;
        let path = download_from(
            &asset,
            &format!("{base}/setup"),
            dir.path(),
            None,
            &progress,
        )
        .await
        .unwrap();
        assert_eq!(path, dir.path().join("VidcueSetup-0.2.1.exe"));
        assert_eq!(std::fs::read(&path).unwrap(), b"abc");
        assert_eq!(progress.load(AtomicOrdering::Relaxed), 3);
        server.abort();

        std::fs::remove_file(&path).unwrap();
        let (base, server) =
            crate::test_http::serve(|_| (200, String::new(), b"abd".to_vec())).await;
        let error = download_from(
            &asset,
            &format!("{base}/setup"),
            dir.path(),
            None,
            &progress,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("checksum"), "{error}");
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            0,
            "a tampered download is removed"
        );
        server.abort();

        let (base, server) = crate::test_http::serve(|_| (404, String::new(), Vec::new())).await;
        assert!(
            download_from(
                &asset,
                &format!("{base}/setup"),
                dir.path(),
                None,
                &progress
            )
            .await
            .is_err()
        );
        server.abort();
    }

    #[tokio::test]
    async fn download_refuses_foreign_urls_odd_names_and_missing_checksums() {
        let dir = tempfile::tempdir().unwrap();
        let progress = AtomicU64::new(0);
        let mut asset = setup_asset();
        asset.url = "https://example.invalid/VidcueSetup.exe".into();
        let error = download(&asset, dir.path(), None, &progress)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("outside"), "{error}");
        asset.url = "https://github.com/someone/vidcue/releases/download/x/VidcueSetup.exe".into();
        assert!(download(&asset, dir.path(), None, &progress).await.is_err());

        for bad in ["../VidcueSetup.exe", "a/b.exe", ".hidden", "", "a b.exe"] {
            let mut asset = setup_asset();
            asset.name = bad.into();
            let error = download_from(&asset, "http://127.0.0.1:9/", dir.path(), None, &progress)
                .await
                .unwrap_err();
            assert!(error.to_string().contains("file name"), "{bad}: {error}");
        }
        let mut asset = setup_asset();
        asset.sha256 = None;
        let error = download_from(&asset, "http://127.0.0.1:9/", dir.path(), None, &progress)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("no checksum"), "{error}");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn install_command_passes_the_tag_as_an_argument_and_rejects_odd_tags() {
        let mut release = newest(&releases(), "0.2.0").unwrap();
        let command = install_command(&release).unwrap();
        let args: Vec<_> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(command.get_program(), "bash");
        assert_eq!(args[0], "-c");
        assert_eq!(&args[2..], ["vidcue-update", "v0.2.1", REPOSITORY]);
        for bad in ["--upload-pack=x", "v1;rm -rf ~", "v1 $(id)", "v1\nx"] {
            release.tag = bad.into();
            assert!(install_command(&release).is_err(), "{bad}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn updater_clones_the_tag_runs_its_installer_and_cleans_up() {
        use std::os::unix::fs::PermissionsExt;
        let t = tempfile::tempdir().unwrap();
        let (bin, log, tmp) = (
            t.path().join("bin"),
            t.path().join("log"),
            t.path().join("tmp"),
        );
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&tmp).unwrap();
        let tool = |name: &str, body: &str| {
            let path = bin.join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        };
        // The fake git records its arguments and "clones" an installer into the target.
        tool(
            "git",
            &format!(
                "echo \"git $*\" >> {log}\nfor dest; do :; done\nmkdir -p \"$dest\"\n\
                 printf 'echo installer ran >> {log}\\n' > \"$dest/install.sh\"",
                log = log.display()
            ),
        );
        tool("cargo", "exit 0");
        // Only these tools are on PATH, so a cargo installed on the machine cannot leak in.
        for name in ["bash", "sh", "mktemp", "rm", "mkdir"] {
            let found = ["/usr/bin", "/bin"]
                .iter()
                .map(|dir| std::path::Path::new(dir).join(name))
                .find(|p| p.exists())
                .unwrap_or_else(|| panic!("{name} is required for this test"));
            std::os::unix::fs::symlink(found, bin.join(name)).unwrap();
        }
        let release = newest(&releases(), "0.2.0").unwrap();
        let status = install_command(&release)
            .unwrap()
            .env("PATH", &bin)
            .env("TMPDIR", &tmp)
            .status()
            .unwrap();
        assert!(status.success());
        let log = std::fs::read_to_string(&log).unwrap();
        assert!(
            log.contains(
                "--depth 1 --branch v0.2.1 -- https://github.com/Vanir20342222/vidcue.git"
            ),
            "{log}"
        );
        assert!(log.ends_with("installer ran\n"), "{log}");
        assert_eq!(
            std::fs::read_dir(&tmp).unwrap().count(),
            0,
            "temporary clone removed"
        );
        // Without cargo the updater stops before cloning anything.
        std::fs::remove_file(bin.join("cargo")).unwrap();
        let status = install_command(&release)
            .unwrap()
            .env("PATH", &bin)
            .env("TMPDIR", &tmp)
            .status()
            .unwrap();
        assert!(!status.success());
    }
}
