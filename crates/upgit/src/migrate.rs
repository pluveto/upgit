//! Forward-only config document for 0.3 and later.
//!
//! A newer binary must still `install_into` the previous keys (serde aliases).
//! Rewrite is a message this document answers, not a pipeline of free
//! functions. `schema` is stamped only when a rewrite actually happens.
//! `upgit update` never replaces this file with the release-zip template.

use std::path::{Path, PathBuf};

use thiserror::Error;

/// Schema this binary understands. 1 = 0.3 config as shipped (`schema` absent).
pub const CURRENT_SCHEMA: u32 = 1;

#[derive(Debug, Error)]
pub enum MigrateError {
    #[error("cannot read config {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot write migrated config {path}: {source}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// A user config.toml. It knows its schema and whether this binary must rewrite it.
pub struct ConfigFile {
    path: Option<PathBuf>,
    text: String,
}

impl ConfigFile {
    pub fn load(path: &Path) -> Result<Self, MigrateError> {
        let text = std::fs::read_to_string(path).map_err(|source| MigrateError::Read {
            path: path.display().to_string(),
            source,
        })?;
        Ok(Self {
            path: Some(path.to_path_buf()),
            text,
        })
    }

    pub fn from_text(text: impl Into<String>) -> Self {
        Self {
            path: None,
            text: text.into(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Top-level `schema = N` before the first table. Missing → 1.
    pub fn schema(&self) -> u32 {
        for line in self.text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                break;
            }
            if let Some(n) = parse_schema_line(trimmed) {
                return n;
            }
        }
        1
    }

    /// Bring this document up to [`CURRENT_SCHEMA`]. No rewrite steps exist yet.
    ///
    /// A file from a newer binary (`schema > CURRENT_SCHEMA`) is left unchanged.
    /// Returns whether the text changed.
    pub fn migrate(&mut self) -> Result<bool, MigrateError> {
        if self.schema() > CURRENT_SCHEMA {
            return Ok(false);
        }
        Ok(false)
    }

    pub fn save(&self) -> Result<(), MigrateError> {
        let path = self.path.as_ref().ok_or_else(|| MigrateError::Write {
            path: "<memory>".to_string(),
            source: std::io::Error::other("config has no path"),
        })?;
        std::fs::write(path, &self.text).map_err(|source| MigrateError::Write {
            path: path.display().to_string(),
            source,
        })
    }

    /// Load, migrate, write only if the text changed. Returns whether it wrote.
    pub fn migrate_path(path: &Path) -> Result<bool, MigrateError> {
        let mut file = Self::load(path)?;
        if !file.migrate()? {
            return Ok(false);
        }
        file.save()?;
        Ok(true)
    }
}

fn parse_schema_line(trimmed: &str) -> Option<u32> {
    if trimmed.starts_with('#') {
        return None;
    }
    let rest = trimmed.strip_prefix("schema")?.trim_start();
    let rest = rest.strip_prefix('=')?.trim();
    rest.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_does_not_rewrite_current_or_newer_config() {
        let mut current = ConfigFile::from_text("default = \"github\"\n");
        assert!(!current.migrate().expect("migrate"));
        assert_eq!(current.text(), "default = \"github\"\n");

        let text = "schema = 9\ndefault = \"github\"\n";
        let mut newer = ConfigFile::from_text(text);
        assert!(!newer.migrate().expect("migrate"));
        assert_eq!(newer.text(), text);
    }
}
