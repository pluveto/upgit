use std::fs;
use std::path::PathBuf;

use super::{RecipeRefresh, UpdateError};

/// A `recipes/` directory next to the binary or the platform config.
///
/// It answers whether an on-disk file is still stock relative to this
/// installation's bundled text.
pub struct RecipeFolder {
    dir: PathBuf,
    stock: Vec<(String, String)>,
}

impl RecipeFolder {
    pub fn with_stock(dir: PathBuf, stock: &[(&str, &str)]) -> Self {
        Self {
            dir,
            stock: stock
                .iter()
                .map(|(id, text)| ((*id).to_string(), (*text).to_string()))
                .collect(),
        }
    }

    pub fn refresh(&self, incoming: &[(String, Vec<u8>)]) -> Result<RecipeRefresh, UpdateError> {
        if !self.dir.is_dir() {
            return Ok(RecipeRefresh::default());
        }
        let mut out = RecipeRefresh::default();
        for (id, new_bytes) in incoming {
            let path = self.dir.join(format!("{id}.toml"));
            if !path.is_file() {
                continue;
            }
            let disk = fs::read(&path)?;
            let old = self
                .stock
                .iter()
                .find(|(oid, _)| oid == id)
                .map(|(_, text)| text.as_bytes());
            let is_stock = old == Some(disk.as_slice()) || disk.as_slice() == new_bytes.as_slice();
            if is_stock {
                if disk.as_slice() != new_bytes.as_slice() {
                    fs::write(&path, new_bytes)?;
                    out.updated.push(id.clone());
                }
            } else {
                out.kept_custom.push(id.clone());
            }
        }
        Ok(out)
    }
}
