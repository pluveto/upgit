use std::error::Error;
use std::path::{Path, PathBuf};

use upgit_uploaders::RecipeCatalog;

const TEMPLATE: &str = include_str!("../../../config.github.toml");

pub fn run(dest: Option<&Path>) -> Result<(), Box<dyn Error>> {
    let dest = match dest {
        Some(path) => path.to_path_buf(),
        None => default_config_path()?,
    };
    if dest.exists() {
        return Err(format!("{} already exists; edit it in place", dest.display()).into());
    }
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(&dest, TEMPLATE)?;
    let recipes_dir = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.join("recipes"))
        .unwrap_or_else(|| PathBuf::from("recipes"));
    RecipeCatalog::extract_to(&recipes_dir)?;
    let shown = dest.canonicalize().unwrap_or(dest);
    println!("Wrote {}", shown.display());
    println!("Open that file and fill [uploaders.github]: pat, username, repo, branch.");
    println!("The repository must be public. Create a PAT: https://github.com/settings/tokens");
    println!("Then run: upgit FILE");
    Ok(())
}

fn default_config_path() -> Result<PathBuf, Box<dyn Error>> {
    upgit::platform_config_file()
        .ok_or_else(|| "cannot determine config directory; pass a path: upgit init PATH".into())
}
