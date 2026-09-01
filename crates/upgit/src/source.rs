use std::error::Error;
use std::io::Write;
use std::path::Path;

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
            sources.push(Box::new(FileSource::new(cli.files.clone(), size_limit)));
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
