use crate::artifact::Artifact;
use crate::locator::Locator;
use crate::object_key::ObjectKey;
use thiserror::Error;

/// An object that accepts one message: put this artifact at this key.
pub trait Uploader {
    fn upload(&self, artifact: &Artifact, key: &ObjectKey) -> Result<Locator, UploadError>;
}

impl std::fmt::Debug for dyn Uploader + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("dyn Uploader")
    }
}

#[derive(Debug, Error)]
pub enum UploadError {
    #[error("{0}")]
    Message(String),
}

impl UploadError {
    pub fn message(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }
}
