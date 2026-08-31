use std::collections::HashMap;

use thiserror::Error;

use crate::publish::Uploader;

/// Runtime lookup of uploaders by id. The binary must not `match` on id.
#[derive(Default)]
pub struct Registry {
    uploaders: HashMap<String, Box<dyn Uploader>>,
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("unknown uploader `{id}` (known: {known})")]
    Unknown { id: String, known: String },
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, id: impl Into<String>, uploader: Box<dyn Uploader>) {
        self.uploaders.insert(id.into(), uploader);
    }

    pub fn get(&self, id: &str) -> Result<&dyn Uploader, RegistryError> {
        match self.uploaders.get(id) {
            Some(uploader) => Ok(uploader.as_ref()),
            None => {
                let mut known: Vec<&str> = self.uploaders.keys().map(String::as_str).collect();
                known.sort_unstable();
                Err(RegistryError::Unknown {
                    id: id.to_string(),
                    known: known.join(", "),
                })
            }
        }
    }
}
