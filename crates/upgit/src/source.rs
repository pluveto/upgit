use std::error::Error;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use tempfile::NamedTempFile;
use upgit::Cli;
use upgit_core::{Artifact, ArtifactError};

pub const DEFAULT_SIZE_LIMIT: u64 = 5 * 1024 * 1024;

/// Something that can yield artifacts. File paths and clipboard are different objects.
pub trait Source {
    fn artifacts(&mut self) -> Result<Vec<Artifact>, Box<dyn Error>>;
}

pub struct FileSource {
    paths: Vec<String>,
    size_limit: Option<u64>,
}

impl FileSource {
    pub fn new(paths: Vec<String>, size_limit: Option<u64>) -> Self {
        Self { paths, size_limit }
    }
}

impl Source for FileSource {
    fn artifacts(&mut self) -> Result<Vec<Artifact>, Box<dyn Error>> {
        let mut out = Vec::new();
        for path in &self.paths {
            out.push(Artifact::from_path(path, self.size_limit).map_err(map_artifact_err)?);
        }
        Ok(out)
    }
}

/// MIME type from a Content-Type header. Answers its own image extension.
struct ContentType<'a> {
    essence: &'a str,
}

impl<'a> ContentType<'a> {
    fn from_header(header: &'a str) -> Self {
        Self {
            essence: header.split(';').next().unwrap_or("").trim(),
        }
    }

    fn image_extension(&self) -> Option<&'static str> {
        if self.essence.eq_ignore_ascii_case("image/png") {
            Some(".png")
        } else if self.essence.eq_ignore_ascii_case("image/jpeg")
            || self.essence.eq_ignore_ascii_case("image/jpg")
        {
            Some(".jpeg")
        } else if self.essence.eq_ignore_ascii_case("image/gif") {
            Some(".gif")
        } else if self.essence.eq_ignore_ascii_case("image/webp") {
            Some(".webp")
        } else if self.essence.eq_ignore_ascii_case("image/svg+xml") {
            Some(".svg")
        } else {
            None
        }
    }
}

/// Remote http(s) URL. The temp download is held so it outlives upload.
pub struct UrlSource {
    url: String,
    size_limit: Option<u64>,
    agent: ureq::Agent,
    hold: Option<NamedTempFile>,
}

impl UrlSource {
    pub fn new(url: String, size_limit: Option<u64>, agent: ureq::Agent) -> Self {
        Self {
            url: url.trim().to_string(),
            size_limit,
            agent,
            hold: None,
        }
    }

    fn accepts(operand: &str) -> bool {
        let s = operand.trim().as_bytes();
        (s.len() >= 8 && s[..8].eq_ignore_ascii_case(b"https://"))
            || (s.len() >= 7 && s[..7].eq_ignore_ascii_case(b"http://"))
    }

    fn failed(&self, what: impl std::fmt::Display, hint: Option<&str>) -> Box<dyn Error> {
        match hint {
            Some(hint) if !hint.is_empty() => {
                format!("cannot download {}: {what}\nhint: {hint}", self.url).into()
            }
            _ => format!("cannot download {}: {what}", self.url).into(),
        }
    }

    fn get(&self) -> Result<ureq::Response, Box<dyn Error>> {
        match self.agent.get(&self.url).call() {
            Ok(resp) => Ok(resp),
            Err(ureq::Error::Status(code, _)) => Err(self.failed(
                format!("HTTP {code}"),
                Some("the URL must be a reachable http(s) file"),
            )),
            Err(ureq::Error::Transport(t)) => {
                let mut detail = t
                    .message()
                    .map(str::to_string)
                    .unwrap_or_else(|| t.kind().to_string());
                let mut src = Error::source(&t);
                while let Some(s) = src {
                    detail = s.to_string();
                    src = s.source();
                }
                Err(self.failed(detail, None))
            }
        }
    }

    fn artifact_name(&self, cd: Option<&str>, content_type: Option<&str>) -> String {
        if let Some(name) = cd.and_then(|h| self.name_from_disposition(h)) {
            return name;
        }
        if let Some(name) = self.name_from_url() {
            return name;
        }
        let mut name = String::from("download");
        if let Some(ext) =
            content_type.and_then(|ct| ContentType::from_header(ct).image_extension())
        {
            name.push_str(ext);
        }
        name
    }

    fn name_from_url(&self) -> Option<String> {
        let s = self.url.trim();
        let rest = if s.len() >= 8 && s.as_bytes()[..8].eq_ignore_ascii_case(b"https://") {
            &s[8..]
        } else if s.len() >= 7 && s.as_bytes()[..7].eq_ignore_ascii_case(b"http://") {
            &s[7..]
        } else {
            return None;
        };
        let rest = rest.split_once('#').map(|(a, _)| a).unwrap_or(rest);
        let rest = rest.split_once('?').map(|(a, _)| a).unwrap_or(rest);
        let path = if let Some(rest) = rest.strip_prefix('[') {
            rest.split_once(']')
                .map(|(_, after)| after.split_once('/').map(|(_, p)| p).unwrap_or(""))
                .unwrap_or("")
        } else {
            rest.split_once('/').map(|(_, p)| p).unwrap_or("")
        };
        if path.is_empty() || path.ends_with('/') {
            return None;
        }
        let segment = path.split('/').next_back().filter(|s| !s.is_empty())?;
        let decoded = Self::percent_decode(segment)?;
        Self::sane_filename(decoded.trim()).map(str::to_string)
    }

    fn name_from_disposition(&self, header: &str) -> Option<String> {
        if let Some(value) = Self::header_param(header, "filename*") {
            if let Some(name) = Self::decode_ext_value(&value) {
                if let Some(name) = Self::sane_filename(name.trim()) {
                    return Some(name.to_string());
                }
            }
        }
        let value = Self::header_param(header, "filename")?;
        let decoded = Self::percent_decode(&value).unwrap_or(value);
        Self::sane_filename(decoded.trim()).map(str::to_string)
    }

    fn header_param(header: &str, name: &str) -> Option<String> {
        let lower = header.to_ascii_lowercase();
        let key = name.to_ascii_lowercase();
        let bytes = header.as_bytes();
        let mut i = 0;
        while let Some(pos) = lower[i..].find(&key) {
            let start = i + pos;
            if start > 0 {
                let prev = bytes[start - 1];
                if prev != b';' && !prev.is_ascii_whitespace() {
                    i = start + 1;
                    continue;
                }
            }
            let mut j = start + key.len();
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j >= bytes.len() || bytes[j] != b'=' {
                i = start + 1;
                continue;
            }
            j += 1;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            let s = header[j..].trim_start();
            if let Some(rest) = s.strip_prefix('"') {
                let mut out = String::new();
                let mut chars = rest.chars();
                while let Some(c) = chars.next() {
                    if c == '\\' {
                        if let Some(n) = chars.next() {
                            out.push(n);
                        }
                    } else if c == '"' {
                        break;
                    } else {
                        out.push(c);
                    }
                }
                return Some(out);
            }
            return Some(s.split(';').next().unwrap_or("").trim().to_string());
        }
        None
    }

    fn decode_ext_value(v: &str) -> Option<String> {
        let mut parts = v.splitn(3, '\'');
        let charset = parts.next()?;
        let _lang = parts.next()?;
        let value = parts.next()?;
        if !charset.is_empty() && !charset.eq_ignore_ascii_case("utf-8") {
            return None;
        }
        Self::percent_decode(value)
    }

    fn sane_filename(name: &str) -> Option<&str> {
        if name.is_empty() || name == "." || name == ".." || name.len() > 255 {
            return None;
        }
        if name.bytes().all(|b| b == b'.') {
            return None;
        }
        let bad = [b'/', b'\\', 0, b':', b'*', b'?', b'"', b'<', b'>', b'|'];
        if name
            .bytes()
            .any(|b| bad.contains(&b) || b.is_ascii_control())
        {
            return None;
        }
        Some(name)
    }

    fn percent_decode(s: &str) -> Option<String> {
        let bytes = s.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' {
                if i + 2 >= bytes.len() {
                    return None;
                }
                let hi = from_hex(bytes[i + 1])?;
                let lo = from_hex(bytes[i + 2])?;
                out.push((hi << 4) | lo);
                i += 3;
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }
        String::from_utf8(out).ok()
    }

    fn write_limited<R: Read, W: Write>(
        &self,
        reader: &mut R,
        writer: &mut W,
    ) -> Result<(), Box<dyn Error>> {
        let mut buf = [0u8; 8192];
        let mut total = 0u64;
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| self.failed(format!("cannot read download: {e}"), None))?;
            if n == 0 {
                break;
            }
            total = total.saturating_add(n as u64);
            if let Some(limit) = self.size_limit {
                if total > limit {
                    return Err(self.failed(
                        ArtifactError::OverLimit { size: total, limit },
                        Some("pass --size-limit BYTES (0 = unlimited); default is 5MiB"),
                    ));
                }
            }
            writer
                .write_all(&buf[..n])
                .map_err(|e| self.failed(format!("cannot write download: {e}"), None))?;
        }
        Ok(())
    }
}

impl Source for UrlSource {
    fn artifacts(&mut self) -> Result<Vec<Artifact>, Box<dyn Error>> {
        let resp = self.get()?;
        let content_type = resp.header("Content-Type").map(str::to_string);
        let content_disposition = resp.header("Content-Disposition").map(str::to_string);
        if let Some(limit) = self.size_limit {
            if let Some(len) = resp
                .header("Content-Length")
                .and_then(|s| s.trim().parse::<u64>().ok())
            {
                if len > limit {
                    return Err(self.failed(
                        ArtifactError::OverLimit { size: len, limit },
                        Some("pass --size-limit BYTES (0 = unlimited); default is 5MiB"),
                    ));
                }
            }
        }

        let name = self.artifact_name(content_disposition.as_deref(), content_type.as_deref());
        let mut builder = tempfile::Builder::new();
        builder.prefix("upgit-url-");
        if let Some(i) = name.rfind('.') {
            if i > 0 && i + 1 < name.len() {
                let ext = &name[i + 1..];
                if ext.len() <= 16 && ext.bytes().all(|b| b.is_ascii_alphanumeric()) {
                    builder.suffix(&name[i..]);
                }
            }
        }
        let mut file = builder
            .tempfile()
            .map_err(|e| format!("cannot create a temp file for the download: {e}"))?;
        self.write_limited(&mut resp.into_reader(), &mut file)?;
        file.flush()
            .map_err(|e| format!("cannot write download: {e}"))?;
        let size = file
            .as_file()
            .metadata()
            .map_err(|e| format!("cannot read download: {e}"))?
            .len();
        let artifact = Artifact::from_name_and_size(&name, size, self.size_limit)
            .map_err(|e| match &e {
                ArtifactError::OverLimit { size, limit } => self.failed(
                    ArtifactError::OverLimit {
                        size: *size,
                        limit: *limit,
                    },
                    Some("pass --size-limit BYTES (0 = unlimited); default is 5MiB"),
                ),
                _ => self.failed(e, None),
            })?
            .with_path(file.path());
        self.hold = Some(file);
        Ok(vec![artifact])
    }
}

pub struct ClipboardImageSource {
    size_limit: Option<u64>,
    hold: Option<NamedTempFile>,
}

impl ClipboardImageSource {
    pub fn new(size_limit: Option<u64>) -> Self {
        Self {
            size_limit,
            hold: None,
        }
    }

    fn encode_png(width: usize, height: usize, bytes: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        let expected = width.saturating_mul(height).saturating_mul(4);
        if bytes.len() != expected {
            return Err(format!(
                "clipboard image has unexpected size ({} bytes, expected {expected})",
                bytes.len()
            )
            .into());
        }
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, width as u32, height as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header()?;
            writer.write_image_data(bytes)?;
        }
        Ok(out)
    }
}

impl Source for ClipboardImageSource {
    fn artifacts(&mut self) -> Result<Vec<Artifact>, Box<dyn Error>> {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| explain_clipboard("clipboard is unavailable", e))?;
        let image = clipboard.get_image().map_err(clipboard_image_err)?;
        let png = Self::encode_png(image.width, image.height, &image.bytes)?;
        let mut file = tempfile::Builder::new()
            .prefix("upgit-clipboard-")
            .suffix(".png")
            .tempfile()
            .map_err(|e| format!("cannot create a temp file for the clipboard image: {e}"))?;
        file.write_all(&png)
            .map_err(|e| format!("cannot write clipboard image: {e}"))?;
        file.flush()?;
        let artifact =
            Artifact::from_path(file.path(), self.size_limit).map_err(map_artifact_err)?;
        self.hold = Some(file);
        Ok(vec![artifact])
    }
}

pub struct ClipboardFilesSource {
    size_limit: Option<u64>,
}

impl ClipboardFilesSource {
    pub fn new(size_limit: Option<u64>) -> Self {
        Self { size_limit }
    }

    #[cfg(not(windows))]
    fn decode_path(line: &str) -> String {
        let stripped = line.strip_prefix("file://").unwrap_or(line);
        let mut out = String::with_capacity(stripped.len());
        let bytes = stripped.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' && i + 2 < bytes.len() {
                if let (Some(hi), Some(lo)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                    out.push(char::from((hi << 4) | lo));
                    i += 3;
                    continue;
                }
            }
            out.push(bytes[i] as char);
            i += 1;
        }
        out
    }
}

impl Source for ClipboardFilesSource {
    fn artifacts(&mut self) -> Result<Vec<Artifact>, Box<dyn Error>> {
        let paths = read_clipboard_file_paths()?;
        let mut artifacts = Vec::new();
        for path in paths {
            let path = path.trim();
            if path.is_empty() || path.starts_with('#') {
                continue;
            }
            if !Path::new(path).exists() {
                return Err(format!("clipboard file does not exist: {path}").into());
            }
            artifacts.push(Artifact::from_path(path, self.size_limit).map_err(map_artifact_err)?);
        }
        if artifacts.is_empty() {
            return Err("clipboard does not contain a file list".into());
        }
        Ok(artifacts)
    }
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn map_artifact_err(err: ArtifactError) -> Box<dyn Error> {
    match &err {
        ArtifactError::OverLimit { .. } => {
            format!("{err}; pass --size-limit BYTES (0 = unlimited); default is 5MiB").into()
        }
        _ => err.into(),
    }
}

pub(crate) fn explain_clipboard(action: &str, err: impl std::fmt::Display) -> String {
    let msg = err.to_string();
    let mut out = format!("{action}: {msg}");
    if cfg!(target_os = "linux") && looks_like_missing_backend(&msg) {
        out.push_str("\nLinux needs `xclip` (X11) or `wl-clipboard` (Wayland).");
    }
    out
}

fn clipboard_image_err(err: impl std::fmt::Display) -> String {
    let msg = err.to_string();
    let mut out = format!("no image on the clipboard: {msg}");
    if cfg!(target_os = "linux") && looks_like_missing_backend(&msg) {
        out.push_str("\nLinux needs `xclip` (X11) or `wl-clipboard` (Wayland).");
    }
    out
}

fn looks_like_missing_backend(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("wayland")
        || m.contains("x11")
        || m.contains("unavailable")
        || m.contains("backend")
        || m.contains("display")
        || m.contains("not found")
        || m.contains("unknown")
        || m.contains("no provider")
        || m.contains("xclip")
        || m.contains("wl-clipboard")
}

#[cfg(windows)]
fn read_clipboard_file_paths() -> Result<Vec<String>, Box<dyn Error>> {
    use clipboard_win::{formats, get_clipboard};
    let files: Vec<String> = get_clipboard(formats::FileList)
        .map_err(|e| format!("clipboard does not contain a file list: {e}"))?;
    if files.is_empty() {
        return Err("clipboard does not contain a file list".into());
    }
    Ok(files)
}

#[cfg(not(windows))]
fn read_clipboard_file_paths() -> Result<Vec<String>, Box<dyn Error>> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| explain_clipboard("clipboard is unavailable", e))?;
    let text = clipboard
        .get_text()
        .map_err(|e| format!("clipboard does not contain a file list: {e}"))?;
    let mut paths = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        paths.push(ClipboardFilesSource::decode_path(line));
    }
    if paths.is_empty() {
        return Err("clipboard does not contain a file list".into());
    }
    Ok(paths)
}

/// Holds source objects so clipboard temp files outlive the upload.
pub struct Intake {
    sources: Vec<Box<dyn Source>>,
}

impl Intake {
    pub fn from_cli(cli: &Cli, size_limit: Option<u64>) -> Result<Self, Box<dyn Error>> {
        if cli.files.is_empty() && !cli.clipboard && !cli.clipboard_files {
            return Err(
                "no files to upload; pass FILE arguments, --clipboard, or --clipboard-files".into(),
            );
        }
        let mut sources: Vec<Box<dyn Source>> = Vec::new();
        if !cli.files.is_empty() {
            let mut download_agent = None;
            for operand in &cli.files {
                if UrlSource::accepts(operand) {
                    let agent = download_agent
                        .get_or_insert_with(|| {
                            let ua = cli
                                .user_agent
                                .as_deref()
                                .filter(|s| !s.is_empty())
                                .unwrap_or(concat!("upgit/", env!("CARGO_PKG_VERSION")));
                            ureq::AgentBuilder::new()
                                .timeout(Duration::from_secs(30))
                                .redirects(10)
                                .user_agent(ua)
                                .build()
                        })
                        .clone();
                    sources.push(Box::new(UrlSource::new(operand.clone(), size_limit, agent)));
                } else {
                    sources.push(Box::new(FileSource::new(vec![operand.clone()], size_limit)));
                }
            }
        }
        if cli.clipboard {
            sources.push(Box::new(ClipboardImageSource::new(size_limit)));
        }
        if cli.clipboard_files {
            sources.push(Box::new(ClipboardFilesSource::new(size_limit)));
        }
        Ok(Self { sources })
    }

    pub fn collect(&mut self) -> Result<Vec<Artifact>, Box<dyn Error>> {
        let mut artifacts = Vec::new();
        for source in &mut self.sources {
            artifacts.extend(source.artifacts()?);
        }
        if artifacts.is_empty() {
            return Err(
                "no files to upload; pass FILE arguments, --clipboard, or --clipboard-files".into(),
            );
        }
        Ok(artifacts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn url_source_downloads_with_injected_agent() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return String::new();
            };
            stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
            stream.set_write_timeout(Some(Duration::from_secs(2))).ok();
            let mut buf = Vec::new();
            let mut tmp = [0u8; 1024];
            loop {
                match stream.read(&mut tmp) {
                    Ok(0) => break,
                    Ok(n) => {
                        buf.extend_from_slice(&tmp[..n]);
                        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            let req = String::from_utf8_lossy(&buf);
            let ua = req
                .lines()
                .find(|line| line.to_ascii_lowercase().starts_with("user-agent:"))
                .and_then(|line| line.split_once(':'))
                .map(|(_, v)| v.trim().to_string())
                .unwrap_or_default();
            let body = b"png-bytes";
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.write_all(body);
            let _ = stream.flush();
            ua
        });
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(2))
            .user_agent("upgit-test/1")
            .build();
        let mut source = UrlSource::new(format!("http://{addr}/dir/logo.png"), Some(1024), agent);
        let artifacts = source.artifacts().expect("download");
        assert_eq!(artifacts[0].file_name(), "logo.png");
        assert_eq!(
            std::fs::read(artifacts[0].path().expect("path")).expect("read"),
            b"png-bytes"
        );
        let ua = server.join().expect("server");
        assert_eq!(ua, "upgit-test/1");
    }
}
