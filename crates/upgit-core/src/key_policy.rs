use std::time::{SystemTime, UNIX_EPOCH};

use chrono::DateTime;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;
use xxhash_rust::xxh32::xxh32;

use crate::artifact::{Artifact, ArtifactError};
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
    #[error("naming template uses `{{hmac}}` but `hmac_key` is not set")]
    MissingHmacKey,
    #[error("naming template uses `{{content_hash}}` but the artifact has no readable bytes")]
    MissingContent,
    #[error("cannot read artifact content: {0}")]
    ContentIo(String),
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

    /// `content_hash_fallback` is inserted as `{content_hash*}` when the artifact
    /// has no readable bytes. It is never hashed. Required if the template (or
    /// `hmac_format`) uses those placeholders and content is missing.
    pub fn apply(
        &self,
        artifact: &Artifact,
        at: SystemTime,
        content_hash_fallback: Option<&str>,
    ) -> Result<ObjectKey, KeyPolicyError> {
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
                if self.hmac.is_none() && template.contains("{hmac}") {
                    return Err(KeyPolicyError::MissingHmacKey);
                }
                let uses_content_hash =
                    |s: &str| s.contains("{content_hash") || s.contains("{contenthash");
                let needs_content = uses_content_hash(template)
                    || self
                        .hmac
                        .as_ref()
                        .is_some_and(|spec| uses_content_hash(&spec.format));
                let xxh32_hex = |data: &[u8]| format!("{:08x}", xxh32(data, 0));
                let content_hash = if needs_content {
                    Some(resolve_content_hash(
                        artifact,
                        content_hash_fallback,
                        xxh32_hex,
                    )?)
                } else {
                    None
                };
                let fields = Fields::from_artifact(artifact, at, xxh32_hex, content_hash)?;
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

fn resolve_content_hash(
    artifact: &Artifact,
    fallback: Option<&str>,
    xxh32_hex: impl Fn(&[u8]) -> String,
) -> Result<String, KeyPolicyError> {
    match artifact.bytes() {
        Ok(Some(bytes)) if !bytes.is_empty() => Ok(xxh32_hex(&bytes)),
        Ok(_) => match fallback {
            Some(fb) => Ok(fb.to_string()),
            None => Err(KeyPolicyError::MissingContent),
        },
        Err(ArtifactError::Io(msg)) => Err(KeyPolicyError::ContentIo(msg)),
        Err(other) => Err(KeyPolicyError::ContentIo(other.to_string())),
    }
}

struct Fields<'a> {
    year: String,
    month: String,
    day: String,
    hour: String,
    minute: String,
    second: String,
    unix: String,
    unix_tsms: String,
    stem: &'a str,
    ext: &'a str,
    fullname: &'a str,
    fname_hash: String,
    content_hash: Option<String>,
}

impl<'a> Fields<'a> {
    fn from_artifact(
        artifact: &'a Artifact,
        at: SystemTime,
        xxh32_hex: impl Fn(&[u8]) -> String,
        content_hash: Option<String>,
    ) -> Result<Self, KeyPolicyError> {
        let duration = at
            .duration_since(UNIX_EPOCH)
            .map_err(|_| KeyPolicyError::InvalidTime)?;
        let datetime = DateTime::from_timestamp(duration.as_secs() as i64, 0)
            .ok_or(KeyPolicyError::InvalidTime)?;
        let fname_hash = xxh32_hex(artifact.stem().as_bytes());
        Ok(Self {
            year: datetime.format("%Y").to_string(),
            month: datetime.format("%m").to_string(),
            day: datetime.format("%d").to_string(),
            hour: datetime.format("%H").to_string(),
            minute: datetime.format("%M").to_string(),
            second: datetime.format("%S").to_string(),
            unix: duration.as_secs().to_string(),
            unix_tsms: duration.as_millis().to_string(),
            stem: artifact.stem(),
            ext: artifact.ext(),
            fullname: artifact.file_name(),
            fname_hash,
            content_hash,
        })
    }

    fn interpolate(&self, template: &str, hmac: Option<&str>) -> String {
        let hash4 = &self.fname_hash[..self.fname_hash.len().min(4)];
        let hash8 = &self.fname_hash[..self.fname_hash.len().min(8)];
        let mut out = template
            .replace("{year}", &self.year)
            .replace("{month}", &self.month)
            .replace("{day}", &self.day)
            .replace("{hour}", &self.hour)
            .replace("{minute}", &self.minute)
            .replace("{second}", &self.second)
            .replace("{unix_tsms}", &self.unix_tsms)
            .replace("{unixtsms}", &self.unix_tsms)
            .replace("{unix_ts}", &self.unix)
            .replace("{unixts}", &self.unix)
            .replace("{unix}", &self.unix)
            .replace("{filenamehash}", &self.fname_hash)
            .replace("{filename}", self.stem)
            .replace("{fullname}", self.fullname)
            .replace("{fname_hash4}", hash4)
            .replace("{fnamehash4}", hash4)
            .replace("{fname_hash8}", hash8)
            .replace("{fnamehash8}", hash8)
            .replace("{fname_hash}", &self.fname_hash)
            .replace("{fnamehash}", &self.fname_hash)
            .replace("{stem}", self.stem)
            .replace("{fname}", self.stem)
            .replace("{ext}", self.ext);
        if let Some(content_hash) = &self.content_hash {
            let content4 = &content_hash[..content_hash.len().min(4)];
            let content8 = &content_hash[..content_hash.len().min(8)];
            out = out
                .replace("{content_hash4}", content4)
                .replace("{contenthash4}", content4)
                .replace("{content_hash8}", content8)
                .replace("{contenthash8}", content8)
                .replace("{content_hash}", content_hash)
                .replace("{contenthash}", content_hash);
        }
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
