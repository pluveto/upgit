/// Location returned by an uploader (raw host URL).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locator(String);

impl Locator {
    pub fn new(url: impl Into<String>) -> Self {
        Self(url.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// URL after [`crate::LinkPolicy`] rewriting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicUrl(String);

impl PublicUrl {
    pub(crate) fn new(url: impl Into<String>) -> Self {
        Self(url.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
