use std::fmt;

use crate::artifact::Artifact;
use crate::locator::Locator;
use crate::object_key::ObjectKey;

/// An object that accepts one message: put this artifact at this key.
pub trait Uploader {
    fn upload(&self, artifact: &Artifact, key: &ObjectKey) -> Result<Locator, UploadError>;
}

impl std::fmt::Debug for dyn Uploader + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("dyn Uploader")
    }
}

/// Normalized upload failure. Display is the one-line `what`; a non-empty `hint`
/// is printed on the next line. Callers must not put raw JSON/XML into `what`.
#[derive(Debug)]
pub struct UploadError {
    pub uploader: String,
    pub what: String,
    pub hint: String,
    pub status: Option<u16>,
}

impl fmt::Display for UploadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.what)?;
        if !self.hint.is_empty() {
            f.write_str("\nhint: ")?;
            f.write_str(&self.hint)?;
        }
        Ok(())
    }
}

impl std::error::Error for UploadError {}

impl UploadError {
    /// IO / internal failure: `what` only, no hint, no HTTP status.
    pub fn message(msg: impl Into<String>) -> Self {
        Self {
            uploader: String::new(),
            what: msg.into(),
            hint: String::new(),
            status: None,
        }
    }

    pub fn new(
        uploader: impl Into<String>,
        what: impl Into<String>,
        hint: impl Into<String>,
        status: Option<u16>,
    ) -> Self {
        Self {
            uploader: uploader.into(),
            what: what.into(),
            hint: hint.into(),
            status,
        }
    }
}
