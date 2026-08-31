use thiserror::Error;

/// Remote object path: no leading or trailing slashes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectKey(String);

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ObjectKeyError {
    #[error("object key is empty")]
    Empty,
}

impl ObjectKey {
    pub fn parse(s: &str) -> Result<Self, ObjectKeyError> {
        let stripped = strip_key(s);
        if stripped.is_empty() {
            return Err(ObjectKeyError::Empty);
        }
        Ok(Self(stripped.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn strip_key(s: &str) -> &str {
    s.trim().trim_matches('/').trim()
}
