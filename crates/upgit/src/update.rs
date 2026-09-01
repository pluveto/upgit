//! Self-update from GitHub Releases.
//!
//! Replaces the running binary and refreshes **stock** recipes (on-disk files
//! that still match the previous bundled text). Never writes `config.toml`,
//! `history.log`, or `upgit.log`.

use std::error::Error;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

use semver::Version;
use serde::Deserialize;
use thiserror::Error;
use upgit_uploaders::RecipeCatalog;

use crate::migrate;
use crate::{env_config_search_paths, platform_config_file};

const RELEASES_URL: &str = "https://api.github.com/repos/pluveto/upgit/releases?per_page=100";

/// 0.2 Go builds are not installable through this command.
fn min_supported() -> Version {
    Version::new(0, 3, 0)
}

pub struct Opts {
    pub channel: Channel,
    pub dry_run: bool,
    pub force: bool,
    pub apply_migrations: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Stable,
    Beta,
    Alpha,
}

impl Channel {
    pub fn from_flags(beta: bool, alpha: bool) -> Self {
        if alpha {
            Channel::Alpha
        } else if beta {
            Channel::Beta
        } else {
            Channel::Stable
        }
    }

    pub fn allows(self, version: &Version) -> bool {
        match pre_ident(version) {
            None => true,
            Some(id) => match self {
                Channel::Stable => false,
                Channel::Beta => eq_ci(id, "beta") || eq_ci(id, "rc"),
                Channel::Alpha => true,
            },
        }
    }
}

impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Channel::Stable => f.write_str("stable"),
            Channel::Beta => f.write_str("beta"),
            Channel::Alpha => f.write_str("alpha"),
        }
    }
}

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error(
        "no release zip for this platform ({os}/{arch}); download from https://github.com/pluveto/upgit/releases"
    )]
    UnsupportedPlatform { os: String, arch: String },
    #[error("no {channel} GitHub release with asset {asset}")]
    NoRelease { channel: Channel, asset: String },
    #[error("GitHub releases: HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("cannot reach GitHub releases: {0}")]
    Network(String),
    #[error("cannot parse GitHub releases JSON: {0}")]
    Json(String),
    #[error("release zip is missing `{0}`")]
    MissingBinary(String),
    #[error("release zip: {0}")]
    Zip(String),
    #[error(
        "latest {channel} release {selected} is older than {current}; upgit update only moves forward. Download an older zip from https://github.com/pluveto/upgit/releases"
    )]
    TooOld {
        channel: Channel,
        selected: Version,
        current: Version,
    },
    #[error("new binary failed verification: {0}")]
    Verify(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Whether `update` may replace the running binary with `selected`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallPlan {
    /// `selected < current`. Never install; `--force` does not override.
    TooOld,
    /// Same version, no `--force`.
    UpToDate,
    /// Same version with `--force` (repair).
    Reinstall,
    /// `selected > current`.
    Upgrade,
}

pub fn install_plan(current: &Version, selected: &Version, force: bool) -> InstallPlan {
    if selected < current {
        InstallPlan::TooOld
    } else if selected == current {
        if force {
            InstallPlan::Reinstall
        } else {
            InstallPlan::UpToDate
        }
    } else {
        InstallPlan::Upgrade
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    pub tag: String,
    pub asset_url: String,
}

#[derive(Debug, Deserialize)]
pub struct GhRelease {
    pub tag_name: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
pub struct GhAsset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct RecipeRefresh {
    pub updated: Vec<String>,
    pub kept_custom: Vec<String>,
}

pub struct Extracted {
    pub binary: PathBuf,
    pub recipes: Vec<(String, Vec<u8>)>,
}

pub fn run(opts: Opts) -> Result<(), Box<dyn Error>> {
    if opts.apply_migrations {
        return apply_pending_migrations();
    }

    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let asset = release_asset_name(os, arch).ok_or_else(|| UpdateError::UnsupportedPlatform {
        os: os.to_string(),
        arch: arch.to_string(),
    })?;
    let current = Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|e| format!("internal version {}: {e}", env!("CARGO_PKG_VERSION")))?;

    let agent = http_agent();
    let json = fetch_releases(&agent)?;
    let parsed = releases_from_json(&json)?;
    let selected = select_release(&parsed, opts.channel, asset).ok_or(UpdateError::NoRelease {
        channel: opts.channel,
        asset: asset.to_string(),
    })?;

    println!("current: {current}");
    println!("latest:  {} ({})", selected.version, opts.channel);

    match install_plan(&current, &selected.version, opts.force) {
        InstallPlan::TooOld => {
            return Err(UpdateError::TooOld {
                channel: opts.channel,
                selected: selected.version,
                current,
            }
            .into());
        }
        InstallPlan::UpToDate => {
            println!("already up to date");
            return Ok(());
        }
        InstallPlan::Reinstall | InstallPlan::Upgrade => {}
    }
    let reinstall = selected.version == current;
    if opts.dry_run {
        let verb = if reinstall { "reinstall" } else { "update" };
        println!(
            "dry-run: would {verb} {current} -> {} ({})",
            selected.version, opts.channel
        );
        return Ok(());
    }

    let verb = if reinstall {
        "reinstalling"
    } else {
        "updating"
    };
    println!(
        "{verb} {current} -> {} ({})",
        selected.version, opts.channel
    );
    let tmp = tempfile::tempdir()?;
    let zip_path = tmp.path().join("release.zip");
    download(&agent, &selected.asset_url, &zip_path)?;
    let unpacked = tmp.path().join("unpacked");
    fs::create_dir(&unpacked)?;
    let extracted = extract_release_zip(&zip_path, &unpacked, packed_binary_name())?;

    let exe = std::env::current_exe()?;
    let exe_dir = exe
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let replacement = replace_executable(&exe, &extracted.binary)?;
    if let Err(e) = confirm_new_binary(&exe, &selected.version) {
        restore_replacement(&replacement)?;
        return Err(e.into());
    }
    discard_backup(&replacement)?;
    println!("replaced {}", exe.display());

    let old_stock = RecipeCatalog::embedded();
    for dir in recipe_dirs(&exe_dir) {
        let report = refresh_stock_recipes(&dir, old_stock, &extracted.recipes)?;
        for id in &report.updated {
            println!("updated recipe {id}.toml ({})", dir.display());
        }
        for id in &report.kept_custom {
            eprintln!("keeping customized recipe {id}.toml ({})", dir.display());
        }
    }

    match std::process::Command::new(&exe)
        .args(["update", "--apply-migrations"])
        .status()
    {
        Ok(st) if st.success() => {}
        Ok(st) => eprintln!("warning: config migration exited {st}"),
        Err(e) => eprintln!("warning: could not run config migration: {e}"),
    }
    Ok(())
}

fn apply_pending_migrations() -> Result<(), Box<dyn Error>> {
    for path in env_config_search_paths(None) {
        if path.is_file() {
            if migrate::apply_file(&path)? {
                println!("migrated {}", path.display());
            }
            break;
        }
    }
    Ok(())
}

pub fn packed_binary_name() -> &'static str {
    if cfg!(windows) {
        "upgit.exe"
    } else {
        "upgit"
    }
}

pub fn release_asset_name(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("linux", "x86_64") => Some("upgit_linux_amd64.zip"),
        ("linux", "aarch64") => Some("upgit_linux_arm64.zip"),
        ("linux", "x86") => Some("upgit_linux_386.zip"),
        ("linux", "arm") => Some("upgit_linux_arm.zip"),
        ("windows", "x86_64") => Some("upgit_win_amd64.zip"),
        ("windows", "aarch64") => Some("upgit_win_arm64.zip"),
        ("windows", "x86") => Some("upgit_win_386.zip"),
        ("macos", "x86_64") => Some("upgit_macos_amd64.zip"),
        ("macos", "aarch64") => Some("upgit_macos_arm64.zip"),
        _ => None,
    }
}

pub fn releases_from_json(json: &str) -> Result<Vec<GhRelease>, UpdateError> {
    serde_json::from_str(json).map_err(|e| UpdateError::Json(e.to_string()))
}

pub fn select_release(releases: &[GhRelease], channel: Channel, asset: &str) -> Option<Release> {
    let min = min_supported();
    releases
        .iter()
        .filter(|r| !r.draft)
        .filter_map(|r| {
            let version = parse_tag(&r.tag_name)?;
            if !is_supported(&version, &min) {
                return None;
            }
            if !channel.allows(&version) {
                return None;
            }
            let url = r
                .assets
                .iter()
                .find(|a| a.name == asset)
                .map(|a| a.browser_download_url.clone())?;
            Some(Release {
                version,
                tag: r.tag_name.clone(),
                asset_url: url,
            })
        })
        .max_by(|a, b| a.version.cmp(&b.version))
}

fn is_supported(version: &Version, floor: &Version) -> bool {
    Version::new(version.major, version.minor, version.patch)
        >= Version::new(floor.major, floor.minor, floor.patch)
}

fn parse_tag(tag: &str) -> Option<Version> {
    let tag = tag.strip_prefix('v').unwrap_or(tag);
    Version::parse(tag).ok()
}

fn pre_ident(version: &Version) -> Option<&str> {
    let pre = version.pre.as_str();
    if pre.is_empty() {
        None
    } else {
        pre.split('.').next()
    }
}

fn eq_ci(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(120))
        .user_agent(&format!("upgit/{}", env!("CARGO_PKG_VERSION")))
        .build()
}

fn github_token() -> Option<String> {
    for key in ["GITHUB_TOKEN", "UPGIT_TOKEN"] {
        if let Ok(v) = std::env::var(key) {
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

fn with_github_headers(mut req: ureq::Request) -> ureq::Request {
    req = req.set("Accept", "application/vnd.github+json");
    if let Some(token) = github_token() {
        req = req.set("Authorization", &format!("token {token}"));
    }
    req
}

fn fetch_releases(agent: &ureq::Agent) -> Result<String, UpdateError> {
    let url = std::env::var("UPGIT_RELEASES_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| RELEASES_URL.to_string());
    match with_github_headers(agent.get(&url)).call() {
        Ok(resp) => resp
            .into_string()
            .map_err(|e| UpdateError::Network(e.to_string())),
        Err(ureq::Error::Status(status, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            if matches!(status, 403 | 429) {
                return Err(UpdateError::Http {
                    status,
                    body: format!(
                        "{body} (set GITHUB_TOKEN or UPGIT_TOKEN if you are rate-limited)"
                    ),
                });
            }
            Err(UpdateError::Http { status, body })
        }
        Err(e) => Err(UpdateError::Network(e.to_string())),
    }
}

fn download(agent: &ureq::Agent, url: &str, dest: &Path) -> Result<(), UpdateError> {
    let resp = match with_github_headers(agent.get(url)).call() {
        Ok(resp) => resp,
        Err(ureq::Error::Status(status, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            return Err(UpdateError::Http { status, body });
        }
        Err(e) => return Err(UpdateError::Network(e.to_string())),
    };
    let mut file = File::create(dest)?;
    io::copy(&mut resp.into_reader(), &mut file)?;
    Ok(())
}

pub fn extract_release_zip(
    zip_path: &Path,
    dest_dir: &Path,
    binary_name: &str,
) -> Result<Extracted, UpdateError> {
    let file = File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| UpdateError::Zip(e.to_string()))?;
    let mut binary = None;
    let mut recipes = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| UpdateError::Zip(e.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        let Some(enclosed) = entry.enclosed_name() else {
            continue;
        };
        let name = enclosed.to_string_lossy().replace('\\', "/");
        if name == binary_name {
            let out = dest_dir.join(binary_name);
            let mut dest = File::create(&out)?;
            io::copy(&mut entry, &mut dest)?;
            binary = Some(out);
            continue;
        }
        if let Some(file_name) = name.strip_prefix("recipes/") {
            if file_name.ends_with(".toml") && !file_name.contains('/') {
                let id = file_name.trim_end_matches(".toml").to_string();
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf)?;
                recipes.push((id, buf));
            }
        }
    }
    let binary = binary.ok_or_else(|| UpdateError::MissingBinary(binary_name.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))?;
    }
    Ok(Extracted { binary, recipes })
}

pub struct Replacement {
    pub current: PathBuf,
    pub backup: PathBuf,
}

/// Swap in `new_bin` and keep the previous file at `*.old` until
/// [`discard_backup`] or [`restore_replacement`].
pub fn replace_executable(current: &Path, new_bin: &Path) -> Result<Replacement, UpdateError> {
    let backup = sibling(current, ".old");
    let _ = fs::remove_file(&backup);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::copy(current, &backup)?;
        let tmp = sibling(current, ".new");
        fs::copy(new_bin, &tmp)?;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
        fs::rename(&tmp, current)?;
    }
    #[cfg(windows)]
    {
        fs::rename(current, &backup)?;
        if let Err(e) = fs::copy(new_bin, current) {
            let _ = fs::rename(&backup, current);
            return Err(e.into());
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        fs::copy(current, &backup)?;
        fs::copy(new_bin, current)?;
    }
    Ok(Replacement {
        current: current.to_path_buf(),
        backup,
    })
}

fn confirm_new_binary(exe: &Path, version: &Version) -> Result<(), UpdateError> {
    let output = std::process::Command::new(exe)
        .arg("--version")
        .output()
        .map_err(|e| UpdateError::Verify(e.to_string()))?;
    if !output.status.success() {
        return Err(UpdateError::Verify(format!(
            "--version exited {}",
            output.status
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let text = format!("{stdout}{stderr}");
    let ver = version.to_string();
    if !text.contains(&ver) {
        return Err(UpdateError::Verify(format!(
            "--version output {text:?} does not contain {ver}"
        )));
    }
    Ok(())
}

pub fn restore_replacement(replacement: &Replacement) -> Result<(), UpdateError> {
    #[cfg(windows)]
    {
        let _ = fs::remove_file(&replacement.current);
    }
    fs::rename(&replacement.backup, &replacement.current)?;
    Ok(())
}

pub fn discard_backup(replacement: &Replacement) -> Result<(), UpdateError> {
    let _ = fs::remove_file(&replacement.backup);
    Ok(())
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_else(|| std::ffi::OsString::from("upgit"));
    name.push(suffix);
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(name),
        _ => PathBuf::from(name),
    }
}

/// Refresh bundled recipes that still match the previous stock text.
///
/// Missing files are not created (the new binary embeds them). Extra files in
/// the directory are left alone. User-edited stock files are kept and listed
/// in [`RecipeRefresh::kept_custom`].
pub fn refresh_stock_recipes(
    dir: &Path,
    old_stock: &[(&str, &str)],
    new_stock: &[(String, Vec<u8>)],
) -> Result<RecipeRefresh, UpdateError> {
    if !dir.is_dir() {
        return Ok(RecipeRefresh::default());
    }
    let mut out = RecipeRefresh::default();
    for (id, new_bytes) in new_stock {
        let path = dir.join(format!("{id}.toml"));
        if !path.is_file() {
            continue;
        }
        let disk = fs::read(&path)?;
        let old = old_stock
            .iter()
            .find(|(oid, _)| *oid == id)
            .map(|(_, text)| text.as_bytes());
        let is_stock = old == Some(disk.as_slice()) || disk.as_slice() == new_bytes.as_slice();
        if is_stock {
            if disk.as_slice() != new_bytes.as_slice() {
                fs::write(&path, new_bytes)?;
                out.updated.push(id.clone());
            }
        } else {
            out.kept_custom.push(id.clone());
        }
    }
    Ok(out)
}

fn recipe_dirs(exe_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut push = |p: PathBuf| {
        if !dirs.contains(&p) {
            dirs.push(p);
        }
    };
    push(exe_dir.join("recipes"));
    if let Some(cfg) = platform_config_file() {
        if let Some(parent) = cfg.parent() {
            push(parent.join("recipes"));
        }
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn release(tag: &str, assets: &[&str]) -> GhRelease {
        GhRelease {
            tag_name: tag.to_string(),
            draft: false,
            assets: assets
                .iter()
                .map(|name| GhAsset {
                    name: (*name).to_string(),
                    browser_download_url: format!("https://example.invalid/{name}"),
                })
                .collect(),
        }
    }

    fn catalog(releases: &[GhRelease], channel: Channel) -> Option<String> {
        select_release(releases, channel, "upgit_linux_amd64.zip").map(|r| r.tag)
    }

    #[test]
    fn asset_names_match_release_zips() {
        assert_eq!(
            release_asset_name("linux", "x86_64"),
            Some("upgit_linux_amd64.zip")
        );
        assert_eq!(
            release_asset_name("windows", "x86_64"),
            Some("upgit_win_amd64.zip")
        );
        assert_eq!(
            release_asset_name("macos", "aarch64"),
            Some("upgit_macos_arm64.zip")
        );
        assert_eq!(release_asset_name("linux", "powerpc"), None);
    }

    #[test]
    fn stable_skips_prereleases_and_0_2() {
        let releases = [
            release("v0.2.25", &["upgit_linux_amd64.zip"]),
            release("v0.3.0-alpha.3", &["upgit_linux_amd64.zip"]),
            release("v0.3.0-beta.3", &["upgit_linux_amd64.zip"]),
            release("v0.3.0", &["upgit_linux_amd64.zip"]),
        ];
        assert_eq!(
            catalog(&releases, Channel::Stable).as_deref(),
            Some("v0.3.0")
        );
    }

    #[test]
    fn beta_prefers_newer_stable_over_older_beta() {
        let releases = [
            release("v0.3.0-beta.3", &["upgit_linux_amd64.zip"]),
            release("v0.3.0", &["upgit_linux_amd64.zip"]),
        ];
        assert_eq!(catalog(&releases, Channel::Beta).as_deref(), Some("v0.3.0"));
    }

    #[test]
    fn beta_picks_newer_beta_over_stable() {
        let releases = [
            release("v0.3.0", &["upgit_linux_amd64.zip"]),
            release("v0.3.1-beta.1", &["upgit_linux_amd64.zip"]),
        ];
        assert_eq!(
            catalog(&releases, Channel::Beta).as_deref(),
            Some("v0.3.1-beta.1")
        );
        assert_eq!(
            catalog(&releases, Channel::Stable).as_deref(),
            Some("v0.3.0")
        );
    }

    #[test]
    fn alpha_includes_all_prereleases_but_picks_newest() {
        let releases = [
            release("v0.3.0-alpha.3", &["upgit_linux_amd64.zip"]),
            release("v0.3.0-beta.3", &["upgit_linux_amd64.zip"]),
        ];
        assert_eq!(
            catalog(&releases, Channel::Alpha).as_deref(),
            Some("v0.3.0-beta.3")
        );
        assert_eq!(
            catalog(&releases, Channel::Beta).as_deref(),
            Some("v0.3.0-beta.3")
        );
        assert_eq!(catalog(&releases, Channel::Stable), None);
    }

    #[test]
    fn skips_release_without_platform_asset() {
        let releases = [
            release("v0.3.1", &["upgit_win_amd64.zip"]),
            release("v0.3.0", &["upgit_linux_amd64.zip"]),
        ];
        assert_eq!(
            catalog(&releases, Channel::Stable).as_deref(),
            Some("v0.3.0")
        );
    }

    #[test]
    fn skips_drafts() {
        let mut draft = release("v0.4.0", &["upgit_linux_amd64.zip"]);
        draft.draft = true;
        let releases = [draft, release("v0.3.0", &["upgit_linux_amd64.zip"])];
        assert_eq!(
            catalog(&releases, Channel::Stable).as_deref(),
            Some("v0.3.0")
        );
    }

    #[test]
    fn channel_from_flags() {
        assert_eq!(Channel::from_flags(false, false), Channel::Stable);
        assert_eq!(Channel::from_flags(true, false), Channel::Beta);
        assert_eq!(Channel::from_flags(false, true), Channel::Alpha);
    }

    fn write_test_zip(path: &Path, files: &[(&str, &[u8])]) {
        let file = File::create(path).expect("zip file");
        let mut zip = zip::ZipWriter::new(file);
        let opts =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in files {
            zip.start_file(*name, opts).expect("start");
            zip.write_all(bytes).expect("write");
        }
        zip.finish().expect("finish");
    }

    #[test]
    fn extract_takes_binary_and_recipes_not_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        let zip_path = dir.path().join("rel.zip");
        write_test_zip(
            &zip_path,
            &[
                ("upgit", b"new-bin"),
                ("config.toml", b"PASTE_YOUR_TOKEN"),
                ("recipes/smms.toml", b"new-smms"),
                ("../escape.toml", b"nope"),
            ],
        );
        let unpacked = dir.path().join("out");
        fs::create_dir(&unpacked).expect("out");
        let extracted = extract_release_zip(&zip_path, &unpacked, "upgit").expect("extract");
        assert_eq!(fs::read(&extracted.binary).expect("bin"), b"new-bin");
        assert!(!unpacked.join("config.toml").exists());
        assert_eq!(
            extracted.recipes,
            vec![("smms".to_string(), b"new-smms".to_vec())]
        );
        assert!(!unpacked.join("escape.toml").exists());
    }

    #[test]
    fn refresh_updates_stock_keeps_custom_skips_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let recipes = dir.path().join("recipes");
        fs::create_dir(&recipes).expect("recipes");
        fs::write(recipes.join("smms.toml"), "old-stock").expect("smms");
        fs::write(recipes.join("gitee.toml"), "user-edit").expect("gitee");
        fs::write(recipes.join("extra.toml"), "mine").expect("extra");
        let old = [("smms", "old-stock"), ("gitee", "old-gitee")];
        let new = vec![
            ("smms".to_string(), b"new-stock".to_vec()),
            ("gitee".to_string(), b"new-gitee".to_vec()),
            ("imgur".to_string(), b"new-imgur".to_vec()),
        ];
        let report = refresh_stock_recipes(&recipes, &old, &new).expect("refresh");
        assert_eq!(report.updated, vec!["smms".to_string()]);
        assert_eq!(report.kept_custom, vec!["gitee".to_string()]);
        assert_eq!(
            fs::read_to_string(recipes.join("smms.toml")).unwrap(),
            "new-stock"
        );
        assert_eq!(
            fs::read_to_string(recipes.join("gitee.toml")).unwrap(),
            "user-edit"
        );
        assert_eq!(
            fs::read_to_string(recipes.join("extra.toml")).unwrap(),
            "mine"
        );
        assert!(!recipes.join("imgur.toml").exists());
    }

    #[test]
    fn install_plan_never_moves_backward_even_with_force() {
        let current = Version::parse("0.4.0-beta.1").unwrap();
        let stable = Version::parse("0.3.0").unwrap();
        assert_eq!(install_plan(&current, &stable, false), InstallPlan::TooOld);
        assert_eq!(install_plan(&current, &stable, true), InstallPlan::TooOld);
    }

    #[test]
    fn install_plan_force_only_reinstalls_same_version() {
        let v = Version::parse("0.3.0").unwrap();
        assert_eq!(install_plan(&v, &v, false), InstallPlan::UpToDate);
        assert_eq!(install_plan(&v, &v, true), InstallPlan::Reinstall);
        let newer = Version::parse("0.3.1").unwrap();
        assert_eq!(install_plan(&v, &newer, false), InstallPlan::Upgrade);
        assert_eq!(install_plan(&v, &newer, true), InstallPlan::Upgrade);
    }

    #[test]
    fn replace_keeps_backup_until_discarded() {
        let dir = tempfile::tempdir().expect("tempdir");
        let dest = dir.path().join("upgit");
        let src = dir.path().join("new");
        fs::write(&dest, b"old").expect("old");
        fs::write(&src, b"new").expect("new");
        let replacement = replace_executable(&dest, &src).expect("replace");
        assert_eq!(fs::read(&dest).expect("read"), b"new");
        assert_eq!(fs::read(&replacement.backup).expect("backup"), b"old");
        discard_backup(&replacement).expect("discard");
        assert!(!replacement.backup.exists());
        assert_eq!(fs::read(&dest).expect("read"), b"new");
    }

    #[test]
    fn restore_replacement_puts_old_bytes_back() {
        let dir = tempfile::tempdir().expect("tempdir");
        let dest = dir.path().join("upgit");
        let src = dir.path().join("new");
        fs::write(&dest, b"old").expect("old");
        fs::write(&src, b"new").expect("new");
        let replacement = replace_executable(&dest, &src).expect("replace");
        restore_replacement(&replacement).expect("restore");
        assert_eq!(fs::read(&dest).expect("read"), b"old");
        assert!(!replacement.backup.exists());
    }
}
