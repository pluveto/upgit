use std::time::SystemTime;

use thiserror::Error;

use crate::artifact::Artifact;
use crate::key_policy::{KeyPolicy, KeyPolicyError};
use crate::link_policy::LinkPolicy;
use crate::locator::{Locator, PublicUrl};
use crate::object_key::ObjectKey;

/// Upload one artifact to a precomputed object key. No renaming or token minting.
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

#[derive(Debug, Error)]
pub enum PublishError {
    #[error(transparent)]
    Key(#[from] KeyPolicyError),
    #[error(transparent)]
    Upload(#[from] UploadError),
}

/// Compute the object key, upload, then rewrite the locator into a public URL.
pub fn publish(
    uploader: &dyn Uploader,
    artifact: &Artifact,
    key_policy: &KeyPolicy,
    link_policy: &LinkPolicy,
    at: SystemTime,
) -> Result<PublicUrl, PublishError> {
    let key = key_policy.apply(artifact, at)?;
    let locator = uploader.upload(artifact, &key)?;
    Ok(link_policy.apply(&locator))
}
