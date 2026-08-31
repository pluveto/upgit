use std::time::{SystemTime, UNIX_EPOCH};

use chrono::DateTime;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;

use crate::artifact::Artifact;
use crate::object_key::{ObjectKey, ObjectKeyError};

type HmacSha256 = Hmac<Sha256>;

/// Turns an [`Artifact`] name and a clock into an [`ObjectKey`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyPolicy {
    kind: KeyKind,
    hmac: Option<HmacSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum KeyKind {
    Template(String),
    KeepOriginal { dir: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HmacSpec {
    key: String,
    format: String,
    len: Option<usize>,
}

impl HmacSpec {
    fn digest(&self, material: &str) -> Result<String, KeyPolicyError> {
        let mut mac = HmacSha256::new_from_slice(self.key.as_bytes())
            .map_err(|_| KeyPolicyError::InvalidHmacKey)?;
        mac.update(material.as_bytes());
        let mut hex = hex_lower(&mac.finalize().into_bytes());
        if let Some(n) = self.len {
            hex.truncate(n.min(hex.len()));
        }
        Ok(hex)
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum KeyPolicyError {
    #[error("rename template is empty")]
    EmptyTemplate,
    #[error(transparent)]
    ObjectKey(#[from] ObjectKeyError),
    #[error("timestamp is before the Unix epoch")]
    InvalidTime,
    #[error("invalid HMAC key")]
    InvalidHmacKey,
}

impl KeyPolicy {
    pub fn template(template: impl Into<String>) -> Self {
        Self {
            kind: KeyKind::Template(template.into()),
            hmac: None,
        }
    }

    pub fn with_hmac(
        mut self,
        key: impl Into<String>,
        format: impl Into<String>,
        len: Option<usize>,
    ) -> Self {
        self.hmac = Some(HmacSpec {
            key: key.into(),
            format: format.into(),
            len,
        });
        self
    }

    pub fn keep_original_in(dir: impl Into<String>) -> Self {
        Self {
            kind: KeyKind::KeepOriginal { dir: dir.into() },
            hmac: None,
        }
    }

    pub fn apply(&self, artifact: &Artifact, at: SystemTime) -> Result<ObjectKey, KeyPolicyError> {
        match &self.kind {
            KeyKind::KeepOriginal { dir } => {
                let dir = dir.trim().trim_matches('/');
                let name = artifact.file_name();
                let joined = if dir.is_empty() {
                    name.to_string()
                } else {
                    format!("{dir}/{name}")
                };
                Ok(ObjectKey::parse(&joined)?)
            }
            KeyKind::Template(template) => {
                if template.trim().is_empty() {
                    return Err(KeyPolicyError::EmptyTemplate);
                }
                let fields = Fields::from_artifact(artifact, at)?;
                let hmac = match &self.hmac {
                    Some(spec) => {
                        let material = fields.interpolate(&spec.format, None);
                        Some(spec.digest(&material)?)
                    }
                    None => None,
                };
                let rendered = fields.interpolate(template, hmac.as_deref());
                Ok(ObjectKey::parse(&rendered)?)
            }
        }
    }
}

struct Fields<'a> {
    year: String,
    month: String,
    day: String,
    unix: String,
    stem: &'a str,
    ext: &'a str,
}

impl<'a> Fields<'a> {
    fn from_artifact(artifact: &'a Artifact, at: SystemTime) -> Result<Self, KeyPolicyError> {
        let duration = at
            .duration_since(UNIX_EPOCH)
            .map_err(|_| KeyPolicyError::InvalidTime)?;
        let datetime = DateTime::from_timestamp(duration.as_secs() as i64, 0)
            .ok_or(KeyPolicyError::InvalidTime)?;
        Ok(Self {
            year: datetime.format("%Y").to_string(),
            month: datetime.format("%m").to_string(),
            day: datetime.format("%d").to_string(),
            unix: duration.as_secs().to_string(),
            stem: artifact.stem(),
            ext: artifact.ext(),
        })
    }

    fn interpolate(&self, template: &str, hmac: Option<&str>) -> String {
        let mut out = template
            .replace("{year}", &self.year)
            .replace("{month}", &self.month)
            .replace("{day}", &self.day)
            .replace("{unix}", &self.unix)
            .replace("{stem}", self.stem)
            .replace("{ext}", self.ext);
        if let Some(hmac) = hmac {
            out = out.replace("{hmac}", hmac);
        }
        out
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const LUT: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(LUT[(b >> 4) as usize] as char);
        out.push(LUT[(b & 0x0f) as usize] as char);
    }
    out
}
