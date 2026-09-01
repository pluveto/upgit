use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use md5::{Digest, Md5};
use thiserror::Error;

/// A local file that has already passed size checks. Name + size, optional path.
#[derive(Debug, Clone)]
pub struct Artifact {
    name: String,
    size: u64,
    path: Option<PathBuf>,
    content_digest: OnceLock<String>,
}

impl PartialEq for Artifact {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.size == other.size && self.path == other.path
    }
}

impl Eq for Artifact {}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ArtifactError {
    #[error("file size is zero")]
    ZeroSize,
    #[error("file size {size} is larger than limit {limit}")]
    OverLimit { size: u64, limit: u64 },
    #[error("cannot read artifact: {0}")]
    Io(String),
    #[error("artifact has no readable bytes")]
    NoContent,
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
            content_digest: OnceLock::new(),
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
        self.content_digest = OnceLock::new();
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

    /// MD5 hex of the file bytes. Same algorithm as `{fname_hash}`.
    pub fn content_digest(&self) -> Result<String, ArtifactError> {
        if let Some(hex) = self.content_digest.get() {
            return Ok(hex.clone());
        }
        let path = self.path.as_deref().ok_or(ArtifactError::NoContent)?;
        let bytes = std::fs::read(path).map_err(|e| ArtifactError::Io(e.to_string()))?;
        let hex = hex_lower(&Md5::digest(bytes));
        let _ = self.content_digest.set(hex.clone());
        Ok(hex)
    }
}

fn dot_index(name: &str) -> Option<usize> {
    match name.rfind('.') {
        Some(i) if i > 0 => Some(i),
        _ => None,
    }
}

pub(crate) fn hex_lower(bytes: &[u8]) -> String {
    const LUT: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(LUT[(b >> 4) as usize] as char);
        out.push(LUT[(b & 0x0f) as usize] as char);
    }
    out
}
