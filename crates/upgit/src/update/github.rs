use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

use semver::Version;
use serde::Deserialize;

use super::{Channel, UpdateError};

const RELEASES_URL: &str = "https://api.github.com/repos/pluveto/upgit/releases?per_page=100";

fn min_supported() -> Version {
    Version::new(0, 3, 0)
}

/// GitHub Releases as an object: list, download.
pub struct Github {
    agent: ureq::Agent,
    releases_url: String,
}

impl Github {
    pub fn new() -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(120))
            .user_agent(&format!("upgit/{}", env!("CARGO_PKG_VERSION")))
            .build();
        let releases_url = std::env::var("UPGIT_RELEASES_URL")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| RELEASES_URL.to_string());
        Self {
            agent,
            releases_url,
        }
    }

    pub fn fetch_index(&self) -> Result<ReleaseIndex, UpdateError> {
        let json = self.get_text(&self.releases_url)?;
        ReleaseIndex::parse(&json)
    }

    pub fn download(&self, url: &str, dest: &Path) -> Result<(), UpdateError> {
        let resp = match self.authorized(self.agent.get(url)).call() {
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

    fn get_text(&self, url: &str) -> Result<String, UpdateError> {
        match self.authorized(self.agent.get(url)).call() {
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

    fn authorized(&self, mut req: ureq::Request) -> ureq::Request {
        req = req.set("Accept", "application/vnd.github+json");
        if let Some(token) = github_token() {
            req = req.set("Authorization", &format!("token {token}"));
        }
        req
    }
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

/// Parsed GitHub releases list. Answers "latest matching this channel and zip".
pub struct ReleaseIndex {
    listings: Vec<GhRelease>,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

impl ReleaseIndex {
    pub fn parse(json: &str) -> Result<Self, UpdateError> {
        let listings = serde_json::from_str(json).map_err(|e| UpdateError::Json(e.to_string()))?;
        Ok(Self { listings })
    }

    pub fn latest(&self, channel: Channel, asset: &str) -> Option<Release> {
        let min = min_supported();
        self.listings
            .iter()
            .filter(|r| !r.draft)
            .filter_map(|r| Release::from_listing(r, channel, asset, &min))
            .max_by(|a, b| a.version.cmp(&b.version))
    }
}

/// One published upgit zip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    version: Version,
    tag: String,
    asset_url: String,
}

impl Release {
    fn from_listing(
        listing: &GhRelease,
        channel: Channel,
        asset: &str,
        floor: &Version,
    ) -> Option<Self> {
        let version = parse_tag(&listing.tag_name)?;
        if !is_supported(&version, floor) {
            return None;
        }
        if !channel.allows(&version) {
            return None;
        }
        let url = listing
            .assets
            .iter()
            .find(|a| a.name == asset)
            .map(|a| a.browser_download_url.clone())?;
        Some(Self {
            version,
            tag: listing.tag_name.clone(),
            asset_url: url,
        })
    }

    pub fn version(&self) -> &Version {
        &self.version
    }

    #[cfg(test)]
    pub fn tag(&self) -> &str {
        &self.tag
    }

    pub fn asset_url(&self) -> &str {
        &self.asset_url
    }
}

fn is_supported(version: &Version, floor: &Version) -> bool {
    Version::new(version.major, version.minor, version.patch)
        >= Version::new(floor.major, floor.minor, floor.patch)
}

fn parse_tag(tag: &str) -> Option<Version> {
    let tag = tag.strip_prefix('v').unwrap_or(tag);
    Version::parse(tag).ok()
}

/// Unpacked release zip: binary + bundled recipes. Ignores `config.toml`.
pub struct Package {
    binary: PathBuf,
    recipes: Vec<(String, Vec<u8>)>,
}

impl Package {
    pub fn from_zip(zip_path: &Path, dest_dir: &Path) -> Result<Self, UpdateError> {
        Self::from_zip_named(zip_path, dest_dir, packed_binary_name())
    }

    pub fn from_zip_named(
        zip_path: &Path,
        dest_dir: &Path,
        binary_name: &str,
    ) -> Result<Self, UpdateError> {
        let file = File::open(zip_path)?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| UpdateError::Zip(e.to_string()))?;
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
            std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755))?;
        }
        Ok(Self { binary, recipes })
    }

    pub fn binary(&self) -> &Path {
        &self.binary
    }

    pub fn recipes(&self) -> &[(String, Vec<u8>)] {
        &self.recipes
    }
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

#[cfg(test)]
impl Release {
    pub fn at(version: &str) -> Self {
        let version = Version::parse(version).expect("version");
        Self {
            tag: format!("v{version}"),
            version,
            asset_url: "https://example.invalid/upgit.zip".to_string(),
        }
    }
}

#[cfg(test)]
impl ReleaseIndex {
    pub fn from_tags(rows: &[(&str, bool, &[&str])]) -> Self {
        Self {
            listings: rows
                .iter()
                .map(|(tag, draft, assets)| GhRelease {
                    tag_name: (*tag).to_string(),
                    draft: *draft,
                    assets: assets
                        .iter()
                        .map(|name| GhAsset {
                            name: (*name).to_string(),
                            browser_download_url: format!("https://example.invalid/{name}"),
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}
