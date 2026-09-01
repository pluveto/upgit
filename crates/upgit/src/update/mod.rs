//! Self-update from GitHub Releases.
//!
//! The running [`Installation`] becomes a [`github::Release`]. It never writes
//! `config.toml`, `history.log`, or `upgit.log`.

use std::error::Error;
use std::io;

use semver::Version;
use thiserror::Error;

mod binary;
mod github;
mod installation;
mod recipes;

use github::{Github, Package};
use installation::Installation;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    TooOld,
    UpToDate,
    Reinstall,
    Upgrade,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct RecipeRefresh {
    pub updated: Vec<String>,
    pub kept_custom: Vec<String>,
}

/// Orchestrates one `upgit update`: ask GitHub, then tell this installation to become that release.
pub struct Updater {
    install: Installation,
    github: Github,
    channel: Channel,
    dry_run: bool,
    force: bool,
    apply_migrations: bool,
}

impl Updater {
    pub fn new(opts: Opts) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            install: Installation::current()?,
            github: Github::new(),
            channel: opts.channel,
            dry_run: opts.dry_run,
            force: opts.force,
            apply_migrations: opts.apply_migrations,
        })
    }

    pub fn run(self) -> Result<(), Box<dyn Error>> {
        if self.apply_migrations {
            return self.install.apply_migrations();
        }

        let asset = Installation::platform_asset()?;
        let index = self.github.fetch_index()?;
        let release = index
            .latest(self.channel, asset)
            .ok_or(UpdateError::NoRelease {
                channel: self.channel,
                asset: asset.to_string(),
            })?;

        println!("current: {}", self.install.version());
        println!("latest:  {} ({})", release.version(), self.channel);

        match self.install.plan(&release, self.force) {
            Plan::TooOld => {
                return Err(UpdateError::TooOld {
                    channel: self.channel,
                    selected: release.version().clone(),
                    current: self.install.version().clone(),
                }
                .into());
            }
            Plan::UpToDate => {
                println!("already up to date");
                return Ok(());
            }
            Plan::Reinstall | Plan::Upgrade => {}
        }

        let reinstall = release.version() == self.install.version();
        if self.dry_run {
            let verb = if reinstall { "reinstall" } else { "update" };
            println!(
                "dry-run: would {verb} {} -> {} ({})",
                self.install.version(),
                release.version(),
                self.channel
            );
            return Ok(());
        }

        let verb = if reinstall {
            "reinstalling"
        } else {
            "updating"
        };
        println!(
            "{verb} {} -> {} ({})",
            self.install.version(),
            release.version(),
            self.channel
        );

        let tmp = tempfile::tempdir()?;
        let zip_path = tmp.path().join("release.zip");
        self.github.download(release.asset_url(), &zip_path)?;
        let unpacked = tmp.path().join("unpacked");
        std::fs::create_dir(&unpacked)?;
        let package = Package::from_zip(&zip_path, &unpacked)?;

        self.install.become_package(&package, release.version())?;
        println!("replaced {}", self.install.exe().display());

        for (dir, report) in self.install.refresh_recipes(&package)? {
            for id in &report.updated {
                println!("updated recipe {id}.toml ({})", dir.display());
            }
            for id in &report.kept_custom {
                eprintln!("keeping customized recipe {id}.toml ({})", dir.display());
            }
        }

        self.install.nudge_new_binary_migrations()?;
        Ok(())
    }
}

pub fn run(opts: Opts) -> Result<(), Box<dyn Error>> {
    Updater::new(opts)?.run()
}

#[cfg(test)]
mod tests {
    use super::binary::BinarySwap;
    use super::github::{release_asset_name, Package, Release, ReleaseIndex};
    use super::installation::Installation;
    use super::recipes::RecipeFolder;
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::Path;
    use zip::write::SimpleFileOptions;

    fn index(rows: &[(&str, bool, &[&str])]) -> ReleaseIndex {
        ReleaseIndex::from_tags(rows)
    }

    fn latest_tag(rows: &[(&str, bool, &[&str])], channel: Channel) -> Option<String> {
        index(rows)
            .latest(channel, "upgit_linux_amd64.zip")
            .map(|r| r.tag().to_string())
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
        let rows = [
            ("v0.2.25", false, &["upgit_linux_amd64.zip"][..]),
            ("v0.3.0-alpha.3", false, &["upgit_linux_amd64.zip"]),
            ("v0.3.0-beta.3", false, &["upgit_linux_amd64.zip"]),
            ("v0.3.0", false, &["upgit_linux_amd64.zip"]),
        ];
        assert_eq!(
            latest_tag(&rows, Channel::Stable).as_deref(),
            Some("v0.3.0")
        );
    }

    #[test]
    fn beta_prefers_newer_stable_over_older_beta() {
        let rows = [
            ("v0.3.0-beta.3", false, &["upgit_linux_amd64.zip"][..]),
            ("v0.3.0", false, &["upgit_linux_amd64.zip"]),
        ];
        assert_eq!(latest_tag(&rows, Channel::Beta).as_deref(), Some("v0.3.0"));
    }

    #[test]
    fn beta_picks_newer_beta_over_stable() {
        let rows = [
            ("v0.3.0", false, &["upgit_linux_amd64.zip"][..]),
            ("v0.3.1-beta.1", false, &["upgit_linux_amd64.zip"]),
        ];
        assert_eq!(
            latest_tag(&rows, Channel::Beta).as_deref(),
            Some("v0.3.1-beta.1")
        );
        assert_eq!(
            latest_tag(&rows, Channel::Stable).as_deref(),
            Some("v0.3.0")
        );
    }

    #[test]
    fn alpha_includes_all_prereleases_but_picks_newest() {
        let rows = [
            ("v0.3.0-alpha.3", false, &["upgit_linux_amd64.zip"][..]),
            ("v0.3.0-beta.3", false, &["upgit_linux_amd64.zip"]),
        ];
        assert_eq!(
            latest_tag(&rows, Channel::Alpha).as_deref(),
            Some("v0.3.0-beta.3")
        );
        assert_eq!(
            latest_tag(&rows, Channel::Beta).as_deref(),
            Some("v0.3.0-beta.3")
        );
        assert_eq!(latest_tag(&rows, Channel::Stable), None);
    }

    #[test]
    fn skips_release_without_platform_asset() {
        let rows = [
            ("v0.3.1", false, &["upgit_win_amd64.zip"][..]),
            ("v0.3.0", false, &["upgit_linux_amd64.zip"]),
        ];
        assert_eq!(
            latest_tag(&rows, Channel::Stable).as_deref(),
            Some("v0.3.0")
        );
    }

    #[test]
    fn skips_drafts() {
        let rows = [
            ("v0.4.0", true, &["upgit_linux_amd64.zip"][..]),
            ("v0.3.0", false, &["upgit_linux_amd64.zip"]),
        ];
        assert_eq!(
            latest_tag(&rows, Channel::Stable).as_deref(),
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
    fn package_takes_binary_and_recipes_not_config() {
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
        let package = Package::from_zip_named(&zip_path, &unpacked, "upgit").expect("extract");
        assert_eq!(fs::read(package.binary()).expect("bin"), b"new-bin");
        assert!(!unpacked.join("config.toml").exists());
        assert_eq!(
            package.recipes(),
            &[("smms".to_string(), b"new-smms".to_vec())]
        );
        assert!(!unpacked.join("escape.toml").exists());
    }

    #[test]
    fn recipe_folder_updates_stock_keeps_custom_skips_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let recipes = dir.path().join("recipes");
        fs::create_dir(&recipes).expect("recipes");
        fs::write(recipes.join("smms.toml"), "old-stock").expect("smms");
        fs::write(recipes.join("gitee.toml"), "user-edit").expect("gitee");
        fs::write(recipes.join("extra.toml"), "mine").expect("extra");
        let folder = RecipeFolder::with_stock(
            recipes.clone(),
            &[("smms", "old-stock"), ("gitee", "old-gitee")],
        );
        let new = vec![
            ("smms".to_string(), b"new-stock".to_vec()),
            ("gitee".to_string(), b"new-gitee".to_vec()),
            ("imgur".to_string(), b"new-imgur".to_vec()),
        ];
        let report = folder.refresh(&new).expect("refresh");
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
    fn installation_never_moves_backward_even_with_force() {
        let install = Installation::at(
            Path::new("upgit").to_path_buf(),
            Version::parse("0.4.0-beta.1").unwrap(),
        );
        let stable = Release::at("0.3.0");
        assert_eq!(install.plan(&stable, false), Plan::TooOld);
        assert_eq!(install.plan(&stable, true), Plan::TooOld);
    }

    #[test]
    fn installation_force_only_reinstalls_same_version() {
        let v = Version::parse("0.3.0").unwrap();
        let install = Installation::at(Path::new("upgit").to_path_buf(), v.clone());
        let same = Release::at("0.3.0");
        let newer = Release::at("0.3.1");
        assert_eq!(install.plan(&same, false), Plan::UpToDate);
        assert_eq!(install.plan(&same, true), Plan::Reinstall);
        assert_eq!(install.plan(&newer, false), Plan::Upgrade);
        assert_eq!(install.plan(&newer, true), Plan::Upgrade);
    }

    #[test]
    fn binary_swap_keeps_backup_until_commit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let dest = dir.path().join("upgit");
        let src = dir.path().join("new");
        fs::write(&dest, b"old").expect("old");
        fs::write(&src, b"new").expect("new");
        let swap = BinarySwap::install(&dest, &src).expect("replace");
        assert_eq!(fs::read(swap.current()).expect("read"), b"new");
        assert_eq!(fs::read(swap.backup()).expect("backup"), b"old");
        swap.commit().expect("commit");
        assert_eq!(fs::read(&dest).expect("read"), b"new");
        assert!(!dir.path().join("upgit.old").exists());
    }

    #[test]
    fn binary_swap_restore_puts_old_bytes_back() {
        let dir = tempfile::tempdir().expect("tempdir");
        let dest = dir.path().join("upgit");
        let src = dir.path().join("new");
        fs::write(&dest, b"old").expect("old");
        fs::write(&src, b"new").expect("new");
        let swap = BinarySwap::install(&dest, &src).expect("replace");
        swap.restore().expect("restore");
        assert_eq!(fs::read(&dest).expect("read"), b"old");
        assert!(!dir.path().join("upgit.old").exists());
    }
}
