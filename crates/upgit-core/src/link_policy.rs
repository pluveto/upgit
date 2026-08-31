use crate::locator::{Locator, PublicUrl};

/// Sequential substring replacements from an uploader locator to a public URL.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LinkPolicy {
    pairs: Vec<(String, String)>,
}

impl LinkPolicy {
    pub fn identity() -> Self {
        Self { pairs: Vec::new() }
    }

    pub fn from_pairs(pairs: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            pairs: pairs.into_iter().collect(),
        }
    }

    pub fn apply(&self, locator: &Locator) -> PublicUrl {
        let mut url = locator.as_str().to_string();
        for (from, to) in &self.pairs {
            url = url.replace(from, to);
        }
        PublicUrl::new(url)
    }
}
