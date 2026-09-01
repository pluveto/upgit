use std::time::SystemTime;

use thiserror::Error;

use crate::artifact::Artifact;
use crate::key_policy::{KeyPolicy, KeyPolicyError};
use crate::link_policy::LinkPolicy;
use crate::locator::{Locator, PublicUrl};
use crate::uploader::{UploadError, Uploader};

#[derive(Debug, Error)]
pub enum PublishError {
    #[error(transparent)]
    Key(#[from] KeyPolicyError),
    #[error(transparent)]
    Upload(#[from] UploadError),
}

/// Orchestrates naming, upload, and link rewrite. Holds its collaborators;
/// the uploader is passed in as the recipient of the upload message.
pub struct Publisher {
    namer: KeyPolicy,
    linker: LinkPolicy,
}

impl Publisher {
    pub fn new(namer: KeyPolicy, linker: LinkPolicy) -> Self {
        Self { namer, linker }
    }

    pub fn publish(
        &self,
        uploader: &dyn Uploader,
        artifact: &Artifact,
        at: SystemTime,
    ) -> Result<PublicUrl, PublishError> {
        Ok(self.publish_with_raw(uploader, artifact, at)?.1)
    }

    /// Locator before `[link]` replacements, and the rewritten public URL.
    pub fn publish_with_raw(
        &self,
        uploader: &dyn Uploader,
        artifact: &Artifact,
        at: SystemTime,
    ) -> Result<(Locator, PublicUrl), PublishError> {
        let key = self.namer.apply(artifact, at)?;
        let locator = uploader.upload(artifact, &key)?;
        let url = self.linker.apply(&locator);
        Ok((locator, url))
    }
}
