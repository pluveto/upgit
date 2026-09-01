use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Journals a successful upload to `history.log` and `upgit.log`, or discards it.
///
/// `--no-log` constructs [`History::silent`]; the caller always sends `record`.
pub struct History {
    sink: Sink,
}

enum Sink {
    Silent,
    Files { dir: PathBuf, uploader_id: String },
}

impl History {
    pub fn silent() -> Self {
        Self { sink: Sink::Silent }
    }

    pub fn files(dir: impl Into<PathBuf>, uploader_id: impl Into<String>) -> Self {
        Self {
            sink: Sink::Files {
                dir: dir.into(),
                uploader_id: uploader_id.into(),
            },
        }
    }

    /// `raw` is the locator before `[link]` replacements; `url` is after them.
    /// `shown` is what the user sees (raw or rewritten, depending on `--raw`).
    pub fn record(&self, raw: &str, url: &str, key: &str, shown: &str) -> io::Result<()> {
        match &self.sink {
            Sink::Silent => Ok(()),
            Sink::Files { dir, uploader_id } => {
                append_history(&dir.join("history.log"), raw, url)?;
                append_upload_log(&dir.join("upgit.log"), uploader_id, key, shown)?;
                Ok(())
            }
        }
    }
}

/// Append one JSON history line: `{"time":"...","rawUrl":"...","url":"..."}`.
fn append_history(path: &Path, raw: &str, url: &str) -> io::Result<()> {
    ensure_parent(path)?;
    let line = serde_json::json!({
        "time": timestamp(),
        "rawUrl": raw,
        "url": url,
    });
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{line}")?;
    Ok(())
}

/// Append an info line with uploader, object key, and URL to `upgit.log`.
fn append_upload_log(path: &Path, uploader: &str, key: &str, url: &str) -> io::Result<()> {
    ensure_parent(path)?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(
        file,
        "{} [INFO ] uploader: {uploader} key: {key} url: {url}",
        timestamp()
    )?;
    Ok(())
}

fn ensure_parent(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

fn timestamp() -> String {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    chrono::DateTime::from_timestamp(dur.as_secs() as i64, dur.subsec_nanos())
        .unwrap_or(chrono::DateTime::UNIX_EPOCH)
        .to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn silent_writes_no_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        History::silent()
            .record(
                "https://raw.example/a",
                "https://cdn.example/a",
                "a.png",
                "https://cdn.example/a",
            )
            .expect("record");
        assert!(!dir.path().join("history.log").exists());
        assert!(!dir.path().join("upgit.log").exists());
        assert_eq!(dir.path().read_dir().expect("read_dir").count(), 0);
    }

    #[test]
    fn files_writes_history_and_upload_log() {
        let dir = tempfile::tempdir().expect("tempdir");
        History::files(dir.path(), "github")
            .record(
                "https://raw.example/a.png",
                "https://cdn.example/a.png",
                "2022/a.png",
                "https://cdn.example/a.png",
            )
            .expect("record");

        let history = std::fs::read_to_string(dir.path().join("history.log")).expect("history.log");
        let v: Value = serde_json::from_str(history.trim()).expect("json");
        assert_eq!(v["rawUrl"], "https://raw.example/a.png");
        assert_eq!(v["url"], "https://cdn.example/a.png");
        assert!(v["time"].as_str().is_some_and(|t| !t.is_empty()));

        let log = std::fs::read_to_string(dir.path().join("upgit.log")).expect("upgit.log");
        assert!(log.contains("uploader: github"));
        assert!(log.contains("key: 2022/a.png"));
        assert!(log.contains("url: https://cdn.example/a.png"));
    }
}
