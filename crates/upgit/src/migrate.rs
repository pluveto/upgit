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

    #[cfg(test)]
    fn stamp_schema(&mut self, n: u32) {
        self.text = stamp_schema(&self.text, n);
    }

    #[cfg(test)]
    fn replace_all(&mut self, from: &str, to: &str) {
        self.text = self.text.replace(from, to);
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
fn stamp_schema(text: &str, n: u32) -> String {
    let assignment = format!("schema = {n}");
    let newline = if text.contains('\r') { "\r\n" } else { "\n" };
    let trailing_newline = text.ends_with('\n');
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();

    let mut header_end = lines.len();
    for (i, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with('[') {
            header_end = i;
            break;
        }
    }

    for line in lines.iter_mut().take(header_end) {
        if parse_schema_line(line.trim()).is_some() {
            *line = assignment;
            let mut out = lines.join(newline);
            if trailing_newline {
                out.push_str(newline);
            }
            return out;
        }
    }

    let mut insert_at = 0;
    for (i, line) in lines.iter().enumerate().take(header_end) {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            insert_at = i + 1;
        } else {
            break;
        }
    }
    lines.insert(insert_at, assignment);
    let mut out = lines.join(newline);
    if trailing_newline || text.is_empty() {
        out.push_str(newline);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_schema_is_version_1() {
        assert_eq!(ConfigFile::from_text("default = \"github\"\n").schema(), 1);
        assert_eq!(
            ConfigFile::from_text("# schema = 9\n[uploaders.github]\n").schema(),
            1
        );
    }

    #[test]
    fn reads_top_level_schema() {
        assert_eq!(
            ConfigFile::from_text("schema = 2\ndefault = \"github\"\n").schema(),
            2
        );
    }

    #[test]
    fn schema_1_is_already_current() {
        let mut cfg = ConfigFile::from_text("default = \"github\"\n");
        assert!(!cfg.migrate().expect("migrate"));
        assert_eq!(cfg.text(), "default = \"github\"\n");
        assert_eq!(cfg.schema(), 1);
    }

    #[test]
    fn newer_schema_is_left_alone() {
        let text = "schema = 9\ndefault = \"github\"\n";
        let mut cfg = ConfigFile::from_text(text);
        assert!(!cfg.migrate().expect("migrate"));
        assert_eq!(cfg.text(), text);
        assert_eq!(cfg.schema(), 9);
    }

    #[test]
    fn rewrite_message_renames_a_key_and_stamps_schema() {
        let mut cfg = ConfigFile::from_text("# keep me\ndefault_uploader = \"github\"\n");
        cfg.replace_all("default_uploader", "default");
        cfg.stamp_schema(2);
        assert_eq!(cfg.schema(), 2);
        assert!(cfg.text().contains("schema = 2"));
        assert!(cfg.text().contains("# keep me"));
        assert!(cfg.text().contains("default = \"github\""));
        assert!(!cfg.text().contains("default_uploader"));
    }

    #[test]
    fn migrate_path_skips_write_when_unchanged() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "default = \"github\"\n").expect("write");
        let before = std::fs::metadata(&path)
            .expect("meta")
            .modified()
            .expect("mtime");
        assert!(!ConfigFile::migrate_path(&path).expect("migrate"));
        let after = std::fs::metadata(&path)
            .expect("meta")
            .modified()
            .expect("mtime");
        assert_eq!(before, after);
    }

    #[test]
    fn this_binary_is_schema_1() {
        assert_eq!(CURRENT_SCHEMA, 1);
    }
}
