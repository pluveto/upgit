use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

/// A journal of successful uploads (`history.log` and `upgit.log`), or a no-op.
pub struct History {
    sink: Sink,
}

enum Sink {
    Silent,
    Files(FileJournal),
}

struct FileJournal {
    dir: PathBuf,
    uploader_id: String,
}

#[derive(Serialize)]
struct HistoryLine<'a> {
    time: String,
    #[serde(rename = "rawUrl")]
    raw_url: &'a str,
    url: &'a str,
}

impl History {
    pub fn silent() -> Self {
        Self { sink: Sink::Silent }
    }

    pub fn files(dir: impl Into<PathBuf>, uploader_id: impl Into<String>) -> Self {
        Self {
            sink: Sink::Files(FileJournal {
                dir: dir.into(),
                uploader_id: uploader_id.into(),
            }),
        }
    }

    /// Record one upload: locator before rewrite, locator after, object key, shown URL.
    pub fn record(&self, raw: &str, url: &str, key: &str, shown: &str) -> io::Result<()> {
        match &self.sink {
            Sink::Silent => Ok(()),
            Sink::Files(journal) => journal.record(raw, url, key, shown),
        }
    }
}

impl FileJournal {
    fn record(&self, raw: &str, url: &str, key: &str, shown: &str) -> io::Result<()> {
        self.append_history(raw, url)?;
        self.append_upload_log(key, shown)?;
        Ok(())
    }

    fn append_history(&self, raw: &str, url: &str) -> io::Result<()> {
        let path = self.dir.join("history.log");
        Self::ensure_parent(&path)?;
        let line = serde_json::to_string(&HistoryLine {
            time: Self::timestamp(),
            raw_url: raw,
            url,
        })
        .map_err(io::Error::other)?;
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{line}")?;
        Ok(())
    }

    fn append_upload_log(&self, key: &str, shown: &str) -> io::Result<()> {
        let path = self.dir.join("upgit.log");
        Self::ensure_parent(&path)?;
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(
            file,
            "{} [INFO ] uploader: {} key: {key} url: {shown}",
            Self::timestamp(),
            self.uploader_id
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
