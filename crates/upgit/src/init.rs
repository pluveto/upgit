use std::error::Error;
use std::path::Path;

const SAMPLE: &str = include_str!("../../../config.sample.toml");

pub fn run(dest: Option<&Path>) -> Result<(), Box<dyn Error>> {
    let dest = dest.unwrap_or(Path::new("config.toml"));
    if dest.exists() {
        return Err(format!(
            "{} already exists; edit it in place (no extensions/ folder is needed)",
            dest.display()
        )
        .into());
    }
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(dest, SAMPLE)?;
    println!(
        "Wrote {}. Fill in [uploaders.qiniu] AK/SK (not a web upload token).",
        dest.display()
    );
    Ok(())
}
