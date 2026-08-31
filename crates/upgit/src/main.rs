use std::error::Error;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use clap::Parser;
use upgit::{Cli, Output};
use upgit_core::{publish, Artifact, KeyPolicy, LinkPolicy, Registry};
use upgit_uploaders::{install, AppConfig};

const DEFAULT_NAMING: &str = "{year}/{month}/{stem}_{unix}{ext}";
const DEFAULT_SIZE_LIMIT: u64 = 5 * 1024 * 1024;

struct Inputs {
    artifacts: Vec<Artifact>,
    _temp: Vec<tempfile::NamedTempFile>,
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) = run(cli) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    if cli.files.is_empty() && !cli.clipboard && !cli.clipboard_files {
        return Err(
            "no files to upload; pass FILE arguments, --clipboard, or --clipboard-files".into(),
        );
    }

    let config = load_config(&cli)?;
    let mut registry = Registry::new();
    install(&mut registry, config.uploaders.clone())?;

    let uploader_id = cli
        .uploader
        .as_deref()
        .filter(|id| !id.is_empty())
        .or(config.default.as_deref().filter(|id| !id.is_empty()))
        .ok_or("no uploader configured; pass --uploader ID or set `default` in config.toml")?;
    let uploader = registry.get(uploader_id)?;

    let inputs = resolve_artifacts(&cli)?;
    let key_policy = key_policy_from_config(&config);
    let link_policy = LinkPolicy::from_pairs(
        config
            .link
            .iter()
            .map(|(from, to)| (from.clone(), to.clone())),
    );
    let now = SystemTime::now();

    let mut urls = Vec::new();
    for artifact in &inputs.artifacts {
        let url = publish(uploader, artifact, &key_policy, &link_policy, now)?;
        urls.push(url.as_str().to_string());
    }
    emit(cli.output, &urls)
}

fn load_config(cli: &Cli) -> Result<AppConfig, Box<dyn Error>> {
    if let Some(path) = cli.config.as_deref() {
        return read_config(Path::new(path));
    }
    for path in config_candidates() {
        if path.is_file() {
            return read_config(&path);
        }
    }
    Ok(AppConfig::default())
}

fn read_config(path: &Path) -> Result<AppConfig, Box<dyn Error>> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read config {}: {e}", path.display()))?;
    Ok(AppConfig::from_toml(&text)?)
}

fn config_candidates() -> Vec<PathBuf> {
    let mut out = vec![PathBuf::from("config.toml")];
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        out.push(PathBuf::from(xdg).join("upgit").join("config.toml"));
    } else if let Ok(home) = std::env::var("HOME") {
        out.push(
            PathBuf::from(home)
                .join(".config")
                .join("upgit")
                .join("config.toml"),
        );
    }
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        out.push(exe_dir.join("config.toml"));
    }
    out
}

fn key_policy_from_config(config: &AppConfig) -> KeyPolicy {
    let template = config
        .naming
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_NAMING);
    let policy = KeyPolicy::template(template);
    match config.hmac_key.as_deref().filter(|s| !s.is_empty()) {
        Some(key) => policy.with_hmac(
            key,
            config
                .hmac_format
                .as_deref()
                .unwrap_or("{year}_{month}_{day}_{unix}{ext}"),
            config.hmac_len,
        ),
        None => policy,
    }
}

fn resolve_artifacts(cli: &Cli) -> Result<Inputs, Box<dyn Error>> {
    let mut artifacts = Vec::new();
    let mut temp = Vec::new();
    for file in &cli.files {
        artifacts.push(Artifact::from_path(file, Some(DEFAULT_SIZE_LIMIT))?);
    }
    if cli.clipboard {
        let (artifact, file) = clipboard_image()?;
        artifacts.push(artifact);
        temp.push(file);
    }
    if cli.clipboard_files {
        artifacts.extend(clipboard_files()?);
    }
    if artifacts.is_empty() {
        return Err(
            "no files to upload; pass FILE arguments, --clipboard, or --clipboard-files".into(),
        );
    }
    Ok(Inputs {
        artifacts,
        _temp: temp,
    })
}

fn clipboard_image() -> Result<(Artifact, tempfile::NamedTempFile), Box<dyn Error>> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("clipboard is unavailable: {e}"))?;
    let image = clipboard
        .get_image()
        .map_err(|e| format!("no image on the clipboard: {e}"))?;
    let png = rgba_to_png(image.width, image.height, &image.bytes)?;
    let mut file = tempfile::Builder::new()
        .prefix("upgit-clipboard-")
        .suffix(".png")
        .tempfile()
        .map_err(|e| format!("cannot create a temp file for the clipboard image: {e}"))?;
    file.write_all(&png)
        .map_err(|e| format!("cannot write clipboard image: {e}"))?;
    file.flush()?;
    let artifact = Artifact::from_path(file.path(), Some(DEFAULT_SIZE_LIMIT))?;
    Ok((artifact, file))
}

fn clipboard_files() -> Result<Vec<Artifact>, Box<dyn Error>> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("clipboard is unavailable: {e}"))?;
    let text = clipboard
        .get_text()
        .map_err(|e| format!("clipboard does not contain a file list: {e}"))?;
    let mut artifacts = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let path = decode_clipboard_path(line);
        if !Path::new(&path).exists() {
            return Err(format!("clipboard file does not exist: {path}").into());
        }
        artifacts.push(Artifact::from_path(&path, Some(DEFAULT_SIZE_LIMIT))?);
    }
    if artifacts.is_empty() {
        return Err("clipboard does not contain a file list".into());
    }
    Ok(artifacts)
}

fn decode_clipboard_path(line: &str) -> String {
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

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn rgba_to_png(width: usize, height: usize, bytes: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
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

fn emit(output: Output, urls: &[String]) -> Result<(), Box<dyn Error>> {
    let text = urls.join("\n");
    if output == Output::Clipboard {
        let mut clipboard =
            arboard::Clipboard::new().map_err(|e| format!("clipboard is unavailable: {e}"))?;
        clipboard
            .set_text(&text)
            .map_err(|e| format!("cannot copy URL to clipboard: {e}"))?;
    } else {
        println!("{text}");
    }
    Ok(())
}
