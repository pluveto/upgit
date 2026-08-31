use std::error::Error;
use std::path::Path;

use upgit_uploaders::RecipeCatalog;

const SAMPLE: &str = include_str!("../../../config.sample.toml");

pub fn run(dest: Option<&Path>) -> Result<(), Box<dyn Error>> {
    let dest = dest.unwrap_or(Path::new("config.toml"));
    if dest.exists() {
        return Err(format!(
            "{} already exists; edit it in place (recipes are in ./recipes or next to the binary)",
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
    let recipes_dir = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.join("recipes"))
        .unwrap_or_else(|| Path::new("recipes").to_path_buf());
    let extracted = RecipeCatalog::extract_to(&recipes_dir)?;
    println!(
        "Wrote {} and recipes/ ({} bundled, {} new files).",
        dest.display(),
        RecipeCatalog::ids().count(),
        extracted
    );
    println!(
        "GitHub is the default. Fill [uploaders.github] pat/username/repo/branch and run: upgit FILE"
    );
    println!("Qiniu is optional for CN CDN: fill AK/SK (not a web upload token).");
    Ok(())
}
