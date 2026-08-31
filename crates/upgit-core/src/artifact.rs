use std::path::{Path, PathBuf};

use thiserror::Error;

/// A local file that has already passed size checks. Name + size, optional path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    name: String,
    size: u64,
    path: Option<PathBuf>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ArtifactError {
    #[error("file size is zero")]
    ZeroSize,
    #[error("file size {size} is larger than limit {limit}")]
    OverLimit { size: u64, limit: u64 },
    #[error("cannot read artifact: {0}")]
    Io(String),
}

impl Artifact {
    pub fn from_name_and_size(
        name: &str,
        size: u64,
        size_limit: Option<u64>,
    ) -> Result<Self, ArtifactError> {
        if size == 0 {
            return Err(ArtifactError::ZeroSize);
        }
        if let Some(limit) = size_limit {
            if size > limit {
                return Err(ArtifactError::OverLimit { size, limit });
            }
        }
        Ok(Self {
            name: name.to_string(),
            size,
            path: None,
        })
    }

    pub fn from_path(
        path: impl AsRef<Path>,
        size_limit: Option<u64>,
    ) -> Result<Self, ArtifactError> {
        let path = path.as_ref();
        let metadata = std::fs::metadata(path).map_err(|e| ArtifactError::Io(e.to_string()))?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| ArtifactError::Io("path has no valid file name".to_string()))?;
        Ok(Self::from_name_and_size(name, metadata.len(), size_limit)?.with_path(path))
    }

    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn file_name(&self) -> &str {
        &self.name
    }

    /// `"logo.png"` → `"logo"`.
    pub fn stem(&self) -> &str {
        match dot_index(&self.name) {
            Some(i) => &self.name[..i],
            None => &self.name,
        }
    }

    /// `"logo.png"` → `".png"` (leading dot included).
    pub fn ext(&self) -> &str {
        match dot_index(&self.name) {
            Some(i) => &self.name[i..],
            None => "",
        }
    }

    pub fn size(&self) -> u64 {
        self.size
    }
}

fn dot_index(name: &str) -> Option<usize> {
    match name.rfind('.') {
        Some(i) if i > 0 => Some(i),
        _ => None,
    }
}
