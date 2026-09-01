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

/// Remote http(s) URL. The temp download is held so it outlives upload.
pub struct UrlSource {
    url: String,
    size_limit: Option<u64>,
    hold: Option<NamedTempFile>,
}

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_REDIRECTS: u32 = 10;

impl UrlSource {
    pub fn new(url: String, size_limit: Option<u64>) -> Self {
        Self {
            url: url.trim().to_string(),
            size_limit,
            hold: None,
        }
    }
}

impl Source for UrlSource {
    fn artifacts(&mut self) -> Result<Vec<Artifact>, Box<dyn Error>> {
        let agent = ureq::AgentBuilder::new()
            .timeout(DOWNLOAD_TIMEOUT)
            .redirects(DOWNLOAD_REDIRECTS)
            .user_agent(concat!("upgit/", env!("CARGO_PKG_VERSION")))
            .build();
        let resp = match agent.get(&self.url).call() {
            Ok(resp) => resp,
            Err(ureq::Error::Status(code, _)) => {
                return Err(download_failed(
                    &self.url,
                    format!("HTTP {code}"),
                    Some("the URL must be a reachable http(s) file"),
                ));
            }
            Err(e) => return Err(download_failed(&self.url, http_what(e), None)),
        };

        let content_type = resp.header("Content-Type").map(str::to_string);
        let content_disposition = resp.header("Content-Disposition").map(str::to_string);
        if let Some(limit) = self.size_limit {
            if let Some(len) = resp
                .header("Content-Length")
                .and_then(|s| s.trim().parse::<u64>().ok())
            {
                if len > limit {
                    return Err(over_download_limit(&self.url, len, limit));
                }
            }
        }

        let name = resolve_download_name(
            content_disposition.as_deref(),
            &self.url,
            content_type.as_deref(),
        );
        let mut builder = tempfile::Builder::new();
        builder.prefix("upgit-url-");
        if let Some(suffix) = temp_suffix(&name) {
            builder.suffix(suffix);
        }
        let mut file = builder
            .tempfile()
            .map_err(|e| format!("cannot create a temp file for the download: {e}"))?;
        let mut reader = resp.into_reader();
        copy_limited(&self.url, &mut reader, &mut file, self.size_limit)?;
        file.flush()
            .map_err(|e| format!("cannot write download: {e}"))?;
        let size = file
            .as_file()
            .metadata()
            .map_err(|e| format!("cannot read download: {e}"))?
            .len();
        let artifact = Artifact::from_name_and_size(&name, size, self.size_limit)
            .map_err(|e| annotate_artifact(&self.url, e))?
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

fn looks_like_http_url(operand: &str) -> bool {
    strip_http_scheme(operand.trim()).is_some()
}

fn strip_http_scheme(s: &str) -> Option<&str> {
    if starts_with_ignore_ascii_case(s, "https://") {
        Some(&s["https://".len()..])
    } else if starts_with_ignore_ascii_case(s, "http://") {
        Some(&s["http://".len()..])
    } else {
        None
    }
}

fn starts_with_ignore_ascii_case(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len() && s.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

fn resolve_download_name(cd: Option<&str>, url: &str, content_type: Option<&str>) -> String {
    if let Some(header) = cd {
        if let Some(name) = filename_from_content_disposition(header) {
            return name;
        }
    }
    if let Some(name) = filename_from_url_path(url) {
        return name;
    }
    let mut name = String::from("download");
    if let Some(ct) = content_type {
        if let Some(ext) = extension_from_content_type(ct) {
            name.push_str(ext);
        }
    }
    name
}

fn filename_from_url_path(url: &str) -> Option<String> {
    let rest = strip_http_scheme(url.trim())?;
    let rest = rest.split_once('#').map(|(a, _)| a).unwrap_or(rest);
    let rest = rest.split_once('?').map(|(a, _)| a).unwrap_or(rest);
    let path = path_after_host(rest);
    if path.is_empty() || path.ends_with('/') {
        return None;
    }
    let segment = path.split('/').next_back().filter(|s| !s.is_empty())?;
    let decoded = percent_decode(segment)?;
    let decoded = decoded.trim();
    sane_filename(decoded).map(str::to_string)
}

fn path_after_host(rest: &str) -> &str {
    if let Some(rest) = rest.strip_prefix('[') {
        return rest
            .split_once(']')
            .map(|(_, after)| after.split_once('/').map(|(_, p)| p).unwrap_or(""))
            .unwrap_or("");
    }
    rest.split_once('/').map(|(_, p)| p).unwrap_or("")
}

fn filename_from_content_disposition(header: &str) -> Option<String> {
    if let Some(value) = header_param(header, "filename*") {
        if let Some(name) = decode_ext_value(&value) {
            let name = name.trim();
            if let Some(name) = sane_filename(name) {
                return Some(name.to_string());
            }
        }
    }
    let value = header_param(header, "filename")?;
    let decoded = percent_decode(&value).unwrap_or(value);
    let decoded = decoded.trim();
    sane_filename(decoded).map(str::to_string)
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
        return Some(parse_token_or_quoted(&header[j..]));
    }
    None
}

fn parse_token_or_quoted(s: &str) -> String {
    let s = s.trim_start();
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
        out
    } else {
        s.split(';').next().unwrap_or("").trim().to_string()
    }
}

fn decode_ext_value(v: &str) -> Option<String> {
    let mut parts = v.splitn(3, '\'');
    let charset = parts.next()?;
    let _lang = parts.next()?;
    let value = parts.next()?;
    if !charset.is_empty() && !charset.eq_ignore_ascii_case("utf-8") {
        return None;
    }
    percent_decode(value)
}

fn extension_from_content_type(ct: &str) -> Option<&'static str> {
    let mime = ct.split(';').next()?.trim().to_ascii_lowercase();
    match mime.as_str() {
        "image/png" => Some(".png"),
        "image/jpeg" | "image/jpg" => Some(".jpeg"),
        "image/gif" => Some(".gif"),
        "image/webp" => Some(".webp"),
        "image/svg+xml" => Some(".svg"),
        _ => None,
    }
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

fn temp_suffix(name: &str) -> Option<&str> {
    let i = name.rfind('.')?;
    if i == 0 || i + 1 == name.len() {
        return None;
    }
    let ext = &name[i + 1..];
    if ext.len() <= 16 && ext.bytes().all(|b| b.is_ascii_alphanumeric()) {
        Some(&name[i..])
    } else {
        None
    }
}

fn copy_limited<R: Read, W: Write>(
    url: &str,
    reader: &mut R,
    writer: &mut W,
    size_limit: Option<u64>,
) -> Result<u64, Box<dyn Error>> {
    let mut buf = [0u8; 8192];
    let mut total = 0u64;
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("cannot download {url}: cannot read download: {e}"))?;
        if n == 0 {
            break;
        }
        total = total.saturating_add(n as u64);
        if let Some(limit) = size_limit {
            if total > limit {
                return Err(over_download_limit(url, total, limit));
            }
        }
        writer
            .write_all(&buf[..n])
            .map_err(|e| format!("cannot download {url}: cannot write download: {e}"))?;
    }
    Ok(total)
}

fn over_download_limit(url: &str, size: u64, limit: u64) -> Box<dyn Error> {
    download_failed(
        url,
        ArtifactError::OverLimit { size, limit },
        Some("pass --size-limit BYTES (0 = unlimited); default is 5MiB"),
    )
}

fn annotate_artifact(url: &str, err: ArtifactError) -> Box<dyn Error> {
    match &err {
        ArtifactError::OverLimit { size, limit } => over_download_limit(url, *size, *limit),
        _ => download_failed(url, err, None),
    }
}

fn download_failed(url: &str, what: impl std::fmt::Display, hint: Option<&str>) -> Box<dyn Error> {
    match hint {
        Some(hint) if !hint.is_empty() => {
            format!("cannot download {url}: {what}\nhint: {hint}").into()
        }
        _ => format!("cannot download {url}: {what}").into(),
    }
}

fn http_what(err: ureq::Error) -> String {
    match err {
        ureq::Error::Status(code, _) => format!("HTTP {code}"),
        ureq::Error::Transport(t) => {
            let mut detail = t
                .message()
                .map(str::to_string)
                .unwrap_or_else(|| t.kind().to_string());
            let mut src = Error::source(&t);
            while let Some(s) = src {
                detail = s.to_string();
                src = s.source();
            }
            detail
        }
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
            for operand in &cli.files {
                if looks_like_http_url(operand) {
                    sources.push(Box::new(UrlSource::new(operand.clone(), size_limit)));
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

    use clap::Parser;

    fn parse_cli(args: &[&str]) -> Cli {
        let mut all = vec!["upgit"];
        all.extend_from_slice(args);
        Cli::try_parse_from(&all).expect("cli")
    }

    fn serve_http(
        status_line: &'static str,
        headers: &'static [(&'static str, &'static str)],
        body: &'static [u8],
    ) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
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
            let mut resp = format!("{status_line}\r\n");
            for (k, v) in headers {
                resp.push_str(&format!("{k}: {v}\r\n"));
            }
            if !headers
                .iter()
                .any(|(k, _)| k.eq_ignore_ascii_case("content-length"))
            {
                resp.push_str(&format!("Content-Length: {}\r\n", body.len()));
            }
            resp.push_str("Connection: close\r\n\r\n");
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.write_all(body);
            let _ = stream.flush();
        });
        (format!("http://{addr}"), handle)
    }

    #[test]
    fn detects_http_and_https_operands() {
        assert!(looks_like_http_url("https://cdn.example.com/a.png"));
        assert!(looks_like_http_url("HTTP://cdn.example.com/a.png"));
        assert!(looks_like_http_url("  Https://cdn.example.com/a.png"));
        assert!(looks_like_http_url("http://127.0.0.1:8080/a.png"));
        assert!(!looks_like_http_url("logo.png"));
        assert!(!looks_like_http_url("./logo.png"));
        assert!(!looks_like_http_url("file:///tmp/a.png"));
        assert!(!looks_like_http_url("ftp://cdn.example.com/a.png"));
        assert!(!looks_like_http_url("http:/missing-slash"));
        assert!(!looks_like_http_url(""));
        assert!(!looks_like_http_url("C:\\Users\\a.png"));
    }

    #[test]
    fn filename_from_url_path_uses_last_segment() {
        assert_eq!(
            filename_from_url_path("https://cdn.example.com/dir/photo.png?x=1#y"),
            Some("photo.png".into())
        );
        assert_eq!(
            filename_from_url_path("HTTP://cdn.example.com/foo%20bar.png"),
            Some("foo bar.png".into())
        );
        assert_eq!(filename_from_url_path("https://cdn.example.com/"), None);
        assert_eq!(filename_from_url_path("https://cdn.example.com/dir/"), None);
        assert_eq!(
            filename_from_url_path("https://[::1]:443/a.png"),
            Some("a.png".into())
        );
        assert_eq!(
            filename_from_url_path("https://cdn.example.com/%2e%2e"),
            None
        );
        assert_eq!(
            filename_from_url_path("https://cdn.example.com/foo%2Fbar.png"),
            None
        );
    }

    #[test]
    fn filename_from_content_disposition_if_sane() {
        assert_eq!(
            filename_from_content_disposition("attachment; filename=\"photo.png\""),
            Some("photo.png".into())
        );
        assert_eq!(
            filename_from_content_disposition("inline; filename=photo.png"),
            Some("photo.png".into())
        );
        assert_eq!(
            filename_from_content_disposition(
                "attachment; filename=\"ignore.png\"; filename*=UTF-8''n%C3%A4me.png"
            ),
            Some("näme.png".into())
        );
        assert_eq!(
            filename_from_content_disposition("attachment; filename=\"../../etc/passwd\""),
            None
        );
        assert_eq!(
            filename_from_content_disposition("attachment; filename=\"a/b.png\""),
            None
        );
    }

    #[test]
    fn download_name_falls_back_to_content_type() {
        assert_eq!(
            resolve_download_name(None, "https://cdn.example.com/", Some("image/png")),
            "download.png"
        );
        assert_eq!(
            resolve_download_name(None, "https://cdn.example.com/", Some("image/jpeg")),
            "download.jpeg"
        );
        assert_eq!(
            resolve_download_name(None, "https://cdn.example.com/", Some("image/svg+xml")),
            "download.svg"
        );
        assert_eq!(
            resolve_download_name(None, "https://cdn.example.com/", Some("text/html")),
            "download"
        );
        assert_eq!(
            resolve_download_name(
                Some("attachment; filename=\"from-header.webp\""),
                "https://cdn.example.com/from-url.png",
                Some("image/png")
            ),
            "from-header.webp"
        );
    }

    #[test]
    fn url_source_names_artifact_from_path() {
        let (base, server) = serve_http(
            "HTTP/1.1 200 OK",
            &[("Content-Type", "image/png")],
            b"png-bytes",
        );
        let url = format!("{base}/dir/logo.png");
        let mut source = UrlSource::new(url, Some(1024));
        let artifacts = source.artifacts().expect("download");
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].file_name(), "logo.png");
        assert_eq!(artifacts[0].size(), 9);
        let path = artifacts[0].path().expect("path");
        assert_eq!(std::fs::read(path).expect("read"), b"png-bytes");
        server.join().ok();
    }

    #[test]
    fn url_source_rejects_over_size_limit_from_content_length() {
        static BODY: [u8; 100] = [0u8; 100];
        let (base, server) = serve_http(
            "HTTP/1.1 200 OK",
            &[("Content-Type", "image/png"), ("Content-Length", "100")],
            &BODY,
        );
        let url = format!("{base}/big.png");
        let mut source = UrlSource::new(url.clone(), Some(50));
        let err = source.artifacts().expect_err("over limit");
        let msg = err.to_string();
        assert!(
            msg.contains("larger than limit") || msg.contains("size"),
            "got {msg}"
        );
        assert!(msg.contains(&url), "got {msg}");
        assert!(msg.contains("hint:"), "got {msg}");
        assert!(!msg.contains('<'), "must not dump HTML, got {msg}");
        server.join().ok();
    }

    #[test]
    fn url_source_http_error_does_not_dump_html() {
        let (base, server) = serve_http(
            "HTTP/1.1 404 Not Found",
            &[("Content-Type", "text/html")],
            b"<html>secret-html-dump</html>",
        );
        let url = format!("{base}/missing.png");
        let mut source = UrlSource::new(url, Some(1024));
        let err = source.artifacts().expect_err("404");
        let msg = err.to_string();
        assert!(msg.contains("HTTP 404"), "got {msg}");
        assert!(!msg.contains("secret-html-dump"), "got {msg}");
        assert!(!msg.contains("<html"), "got {msg}");
        server.join().ok();
    }

    #[test]
    fn intake_mixes_local_path_and_url() {
        let dir = tempfile::tempdir().expect("tempdir");
        let local = dir.path().join("local.png");
        std::fs::write(&local, b"1234").expect("write");
        let (base, server) = serve_http(
            "HTTP/1.1 200 OK",
            &[("Content-Type", "image/png")],
            b"remote",
        );
        let url = format!("{base}/remote.png");
        let local_s = local.to_string_lossy().into_owned();
        let cli = parse_cli(&[&local_s, &url]);
        let mut intake = Intake::from_cli(&cli, Some(1024)).expect("intake");
        let artifacts = intake.collect().expect("collect");
        assert_eq!(artifacts.len(), 2);
        assert_eq!(artifacts[0].file_name(), "local.png");
        assert_eq!(artifacts[0].size(), 4);
        assert_eq!(artifacts[1].file_name(), "remote.png");
        assert_eq!(artifacts[1].size(), 6);
        server.join().ok();
    }
}
