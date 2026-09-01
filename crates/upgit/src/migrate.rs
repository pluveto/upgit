//! Forward-only config rewrites for 0.3 and later.
//!
//! Invariant: a newer binary must still `install_into` the previous keys
//! (serde aliases). A [`Step`] may rewrite the file as cleanup; it is not
//! required for the new binary to run. `schema` is stamped only when a step
//! actually rewrites. Users never see this key in help.
//!
//! Schema 1 is the 0.3 config (`schema` key absent). `upgit update` never
//! replaces `config.toml` with the release-zip template. Steps run when the
//! new binary loads config (and via hidden `update --apply-migrations`).
//! There is no reverse chain: `update` does not install an older release.

use std::path::Path;

use thiserror::Error;

/// Schema version this binary understands. 1 = 0.3 config as shipped.
pub const CURRENT_SCHEMA: u32 = 1;

/// Ordered rewrites. Each step's `to` is the schema after it runs.
pub static MIGRATIONS: &[Step] = &[];

/// One config rewrite, applied only when the file's schema is below `to`.
pub struct Step {
    pub to: u32,
    pub name: &'static str,
    pub apply: fn(&str) -> Result<String, String>,
}

#[derive(Debug, Error)]
pub enum MigrateError {
    #[error("config migration `{name}` failed: {message}")]
    Step { name: String, message: String },
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

/// Result of applying migrations to a config body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub text: String,
    pub schema: u32,
    pub changed: bool,
}

/// Apply [`MIGRATIONS`] up to [`CURRENT_SCHEMA`].
pub fn apply_to_text(text: &str) -> Result<Outcome, MigrateError> {
    apply_steps(text, MIGRATIONS, CURRENT_SCHEMA)
}

/// Read `path`, rewrite if needed, write only when the text changed.
///
/// Returns `true` if the file was written.
pub fn apply_file(path: &Path) -> Result<bool, MigrateError> {
    let text = std::fs::read_to_string(path).map_err(|source| MigrateError::Read {
        path: path.display().to_string(),
        source,
    })?;
    let outcome = apply_to_text(&text)?;
    if !outcome.changed {
        return Ok(false);
    }
    std::fs::write(path, &outcome.text).map_err(|source| MigrateError::Write {
        path: path.display().to_string(),
        source,
    })?;
    Ok(true)
}

/// Apply `steps` whose `to` is in `(schema, current]`.
///
/// A file from a newer binary (`schema > current`) is left unchanged.
pub fn apply_steps(text: &str, steps: &[Step], current: u32) -> Result<Outcome, MigrateError> {
    let mut schema = read_schema(text);
    if schema > current {
        return Ok(Outcome {
            text: text.to_string(),
            schema,
            changed: false,
        });
    }
    let mut body = text.to_string();
    let mut changed = false;
    for step in steps {
        if step.to <= schema || step.to > current {
            continue;
        }
        body = (step.apply)(&body).map_err(|message| MigrateError::Step {
            name: step.name.to_string(),
            message,
        })?;
        schema = step.to;
        body = set_schema(&body, schema);
        changed = true;
    }
    Ok(Outcome {
        text: body,
        schema,
        changed,
    })
}

/// Top-level `schema = N` before the first table. Missing → 1.
pub fn read_schema(text: &str) -> u32 {
    for line in text.lines() {
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

fn parse_schema_line(trimmed: &str) -> Option<u32> {
    if trimmed.starts_with('#') {
        return None;
    }
    let rest = trimmed.strip_prefix("schema")?.trim_start();
    let rest = rest.strip_prefix('=')?.trim();
    rest.parse().ok()
}

/// Set or insert a top-level `schema = N` assignment. Preserves other lines.
pub fn set_schema(text: &str, n: u32) -> String {
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
        assert_eq!(read_schema("default = \"github\"\n"), 1);
        assert_eq!(read_schema("# schema = 9\n[uploaders.github]\n"), 1);
    }

    #[test]
    fn reads_top_level_schema() {
        assert_eq!(read_schema("schema = 2\ndefault = \"github\"\n"), 2);
    }

    #[test]
    fn empty_migrations_do_not_rewrite_schema_1() {
        let text = "default = \"github\"\n";
        let out = apply_to_text(text).expect("apply");
        assert!(!out.changed);
        assert_eq!(out.text, text);
        assert_eq!(out.schema, 1);
    }

    #[test]
    fn newer_schema_is_left_alone() {
        let text = "schema = 9\ndefault = \"github\"\n";
        let out = apply_steps(text, &[], 1).expect("apply");
        assert!(!out.changed);
        assert_eq!(out.text, text);
        assert_eq!(out.schema, 9);
    }

    #[test]
    fn step_rewrites_and_stamps_schema() {
        let step = Step {
            to: 2,
            name: "rename-default-uploader",
            apply: |text| Ok(text.replace("default_uploader", "default")),
        };
        let text = "# keep me\ndefault_uploader = \"github\"\n";
        let out = apply_steps(text, &[step], 2).expect("apply");
        assert!(out.changed);
        assert_eq!(out.schema, 2);
        assert!(out.text.contains("schema = 2"));
        assert!(out.text.contains("# keep me"));
        assert!(out.text.contains("default = \"github\""));
        assert!(!out.text.contains("default_uploader"));
    }

    #[test]
    fn apply_file_skips_write_when_unchanged() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "default = \"github\"\n").expect("write");
        let before = std::fs::metadata(&path)
            .expect("meta")
            .modified()
            .expect("mtime");
        assert!(!apply_file(&path).expect("apply"));
        let after = std::fs::metadata(&path)
            .expect("meta")
            .modified()
            .expect("mtime");
        assert_eq!(before, after);
    }

    #[test]
    fn migrations_chain_reaches_current_schema() {
        if MIGRATIONS.is_empty() {
            assert_eq!(CURRENT_SCHEMA, 1);
            return;
        }
        let mut seen = 1u32;
        for step in MIGRATIONS {
            assert!(
                step.to > seen,
                "migration `{}` to {} is not after {seen}",
                step.name,
                step.to
            );
            seen = step.to;
        }
        assert_eq!(seen, CURRENT_SCHEMA);
    }
}
