use std::collections::HashMap;

use thiserror::Error;

use crate::uploader::Uploader;

/// Runtime lookup of uploaders by id. The binary must not `match` on id.
#[derive(Default)]
pub struct Registry {
    uploaders: HashMap<String, Box<dyn Uploader + Send + Sync>>,
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("unknown uploader `{id}` (configured: {known})")]
    Unknown { id: String, known: String },
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, id: impl Into<String>, uploader: Box<dyn Uploader + Send + Sync>) {
        self.uploaders.insert(id.into(), uploader);
    }

    pub fn get(&self, id: &str) -> Result<&dyn Uploader, RegistryError> {
        match self.uploaders.get(id) {
            Some(uploader) => Ok(uploader.as_ref()),
            None => {
                let mut known: Vec<&str> = self.uploaders.keys().map(String::as_str).collect();
                known.sort_unstable();
                let known = if known.is_empty() {
                    "none".to_string()
                } else {
                    known.join(", ")
                };
                Err(RegistryError::Unknown {
                    id: id.to_string(),
                    known,
                })
            }
        }
    }
}
