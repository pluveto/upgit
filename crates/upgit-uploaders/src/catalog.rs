use std::path::{Path, PathBuf};

use crate::recipe::{HttpRecipe, RecipeError};

/// Bundled HTTP recipes: on disk next to the binary, then compiled-in copies.
pub struct RecipeCatalog;

impl RecipeCatalog {
    pub fn embedded() -> &'static [(&'static str, &'static str)] {
        &[
            ("smms", include_str!("../../../recipes/smms.toml")),
            ("imgur", include_str!("../../../recipes/imgur.toml")),
            ("catbox", include_str!("../../../recipes/catbox.toml")),
            (
                "cloudinary",
                include_str!("../../../recipes/cloudinary.toml"),
            ),
            ("easyimage", include_str!("../../../recipes/easyimage.toml")),
            ("lskypro", include_str!("../../../recipes/lskypro.toml")),
            ("lskypro2", include_str!("../../../recipes/lskypro2.toml")),
            ("hello", include_str!("../../../recipes/hello.toml")),
            ("niupic", include_str!("../../../recipes/niupic.toml")),
            ("imgurlorg", include_str!("../../../recipes/imgurlorg.toml")),
            ("imgbb", include_str!("../../../recipes/imgbb.toml")),
            ("chevereto", include_str!("../../../recipes/chevereto.toml")),
        ]
    }

    pub fn ids() -> impl Iterator<Item = &'static str> {
        Self::embedded().iter().map(|(id, _)| *id)
    }

    pub fn contains(id: &str) -> bool {
        Self::embedded().iter().any(|(known, _)| *known == id)
    }

    pub fn load(id: &str) -> Result<HttpRecipe, RecipeError> {
        if let Some(text) = Self::read_text(id) {
            return HttpRecipe::from_toml(&text);
        }
        Err(RecipeError::Message(format!(
            "unknown recipe `{id}` (bundled: {})",
            Self::ids().collect::<Vec<_>>().join(", ")
        )))
    }

    pub fn extract_to(dir: &Path) -> std::io::Result<usize> {
        std::fs::create_dir_all(dir)?;
        let mut n = 0;
        for (id, text) in Self::embedded() {
            let path = dir.join(format!("{id}.toml"));
            if path.exists() {
                continue;
            }
            std::fs::write(&path, text)?;
            n += 1;
        }
        Ok(n)
    }

    fn read_text(id: &str) -> Option<String> {
        for dir in Self::search_dirs() {
            let path = dir.join(format!("{id}.toml"));
            if let Ok(text) = std::fs::read_to_string(path) {
                return Some(text);
            }
        }
        Self::embedded()
            .iter()
            .find(|(known, _)| *known == id)
            .map(|(_, text)| (*text).to_string())
    }

    fn search_dirs() -> Vec<PathBuf> {
        let mut dirs = vec![PathBuf::from("recipes")];
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                dirs.push(parent.join("recipes"));
            }
        }
        dirs
    }
}
