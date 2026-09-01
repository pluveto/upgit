use std::error::Error;
use std::path::{Path, PathBuf};

use semver::Version;
use upgit_uploaders::RecipeCatalog;

use super::binary::BinarySwap;
use super::github::{release_asset_name, Package, Release};
use super::recipes::RecipeFolder;
use super::{Plan, RecipeRefresh, UpdateError};
use crate::migrate::ConfigFile;
use crate::{env_config_search_paths, platform_config_file};

/// This machine's upgit: the running binary, its version, and sidecar files.
pub struct Installation {
    exe: PathBuf,
    version: Version,
}

impl Installation {
    pub fn current() -> Result<Self, Box<dyn Error>> {
        let exe = std::env::current_exe()?;
        let version = Version::parse(env!("CARGO_PKG_VERSION"))
            .map_err(|e| format!("internal version {}: {e}", env!("CARGO_PKG_VERSION")))?;
        Ok(Self { exe, version })
    }

    pub fn version(&self) -> &Version {
        &self.version
    }

    pub fn exe(&self) -> &Path {
        &self.exe
    }

    pub fn platform_asset() -> Result<&'static str, UpdateError> {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        release_asset_name(os, arch).ok_or_else(|| UpdateError::UnsupportedPlatform {
            os: os.to_string(),
            arch: arch.to_string(),
        })
    }

    pub fn plan(&self, release: &Release, force: bool) -> Plan {
        if release.version() < &self.version {
            Plan::TooOld
        } else if release.version() == &self.version {
            if force {
                Plan::Reinstall
            } else {
                Plan::UpToDate
            }
        } else {
            Plan::Upgrade
        }
    }

    /// Replace this binary with `package`, confirm `--version`, then refresh stock recipes.
    pub fn become_package(&self, package: &Package, expected: &Version) -> Result<(), UpdateError> {
        let swap = BinarySwap::install(&self.exe, package.binary())?;
        if let Err(e) = self.confirm_version(expected) {
            swap.restore()?;
            return Err(e);
        }
        swap.commit()?;
        Ok(())
    }

    fn confirm_version(&self, expected: &Version) -> Result<(), UpdateError> {
        let output = std::process::Command::new(&self.exe)
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
        let ver = expected.to_string();
        if !text.contains(&ver) {
            return Err(UpdateError::Verify(format!(
                "--version output {text:?} does not contain {ver}"
            )));
        }
        Ok(())
    }

    pub fn refresh_recipes(
        &self,
        package: &Package,
    ) -> Result<Vec<(PathBuf, RecipeRefresh)>, UpdateError> {
        let mut reports = Vec::new();
        for dir in self.recipe_dirs() {
            let folder = RecipeFolder::with_stock(dir.clone(), RecipeCatalog::embedded());
            let report = folder.refresh(package.recipes())?;
            reports.push((dir, report));
        }
        Ok(reports)
    }

    fn recipe_dirs(&self) -> Vec<PathBuf> {
        let exe_dir = self
            .exe
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
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

    pub fn apply_migrations(&self) -> Result<(), Box<dyn Error>> {
        for path in env_config_search_paths(None) {
            if path.is_file() {
                if ConfigFile::migrate_path(&path)? {
                    println!("migrated {}", path.display());
                }
                break;
            }
        }
        Ok(())
    }

    pub fn nudge_new_binary_migrations(&self) -> Result<(), UpdateError> {
        match std::process::Command::new(&self.exe)
            .args(["update", "--apply-migrations"])
            .status()
        {
            Ok(st) if st.success() => Ok(()),
            Ok(st) => {
                eprintln!("warning: config migration exited {st}");
                Ok(())
            }
            Err(e) => {
                eprintln!("warning: could not run config migration: {e}");
                Ok(())
            }
        }
    }
}

#[cfg(test)]
impl Installation {
    pub fn at(exe: PathBuf, version: Version) -> Self {
        Self { exe, version }
    }
}
