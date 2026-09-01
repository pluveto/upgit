use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Append one JSON history line: `{"time":"...","rawUrl":"...","url":"..."}`.
///
/// `raw` is the locator before `[link]` replacements; `url` is after them.
pub fn record_history(path: impl AsRef<Path>, raw: &str, url: &str) -> io::Result<()> {
    let path = path.as_ref();
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
pub fn record_upload_log(
    path: impl AsRef<Path>,
    uploader: &str,
    key: &str,
    url: &str,
) -> io::Result<()> {
    let path = path.as_ref();
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
