use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha1::Sha1;
use upgit_core::{Artifact, Locator, ObjectKey, UploadError, Uploader};

use crate::form::{self, Part};

type HmacSha1 = Hmac<Sha1>;

/// Compact policy JSON must keep this field order (scope, then deadline).
#[derive(Serialize)]
struct UploadPolicy<'a> {
    scope: &'a str,
    deadline: u64,
}

pub fn mint_upload_token(
    access_key: &str,
    secret_key: &str,
    bucket: &str,
    deadline: SystemTime,
) -> String {
    let unix = deadline
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let json = serde_json::to_string(&UploadPolicy {
        scope: bucket,
        deadline: unix,
    })
    .expect("policy json");
    let encoded_policy = URL_SAFE.encode(json.as_bytes());
    let mut mac = HmacSha1::new_from_slice(secret_key.as_bytes()).expect("hmac-sha1 key");
    mac.update(encoded_policy.as_bytes());
    let encoded_sign = URL_SAFE.encode(mac.finalize().into_bytes());
    format!("{access_key}:{encoded_sign}:{encoded_policy}")
}

#[derive(Debug, Clone)]
pub struct QiniuConfig {
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub public_base: String,
    pub region: Option<String>,
}

#[derive(Debug, Clone)]
pub struct QiniuUploader {
    config: QiniuConfig,
}

impl QiniuUploader {
    pub fn new(config: QiniuConfig) -> Self {
        Self { config }
    }

    pub fn locator_for(&self, key: &ObjectKey) -> Locator {
        let base = self.config.public_base.trim_end_matches('/');
        Locator::new(format!("{base}/{}", key.as_str()))
    }

    fn upload_url(&self) -> String {
        match self.config.region.as_deref().map(str::trim) {
            None | Some("") | Some("z0") => "https://upload.qiniup.com".to_string(),
            Some(region) => format!("https://upload-{region}.qiniup.com"),
        }
    }
}

impl Uploader for QiniuUploader {
    fn upload(&self, artifact: &Artifact, key: &ObjectKey) -> Result<Locator, UploadError> {
        let path = artifact.path().ok_or_else(|| {
            UploadError::message("artifact has no local path; cannot upload bytes")
        })?;
        let data = std::fs::read(path).map_err(|e| UploadError::message(e.to_string()))?;
        let deadline = SystemTime::now() + Duration::from_secs(3600);
        let token = mint_upload_token(
            &self.config.access_key,
            &self.config.secret_key,
            &self.config.bucket,
            deadline,
        );
        let (content_type, body) = form::encode(&[
            Part::Text {
                name: "token",
                value: &token,
            },
            Part::Text {
                name: "key",
                value: key.as_str(),
            },
            Part::File {
                name: "file",
                filename: artifact.file_name(),
                data: &data,
            },
        ]);
        match ureq::post(&self.upload_url())
            .set("Content-Type", &content_type)
            .send_bytes(&body)
        {
            Ok(_) => Ok(self.locator_for(key)),
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                Err(UploadError::message(format!(
                    "qiniu upload HTTP {code}: {text}"
                )))
            }
            Err(e) => Err(UploadError::message(e.to_string())),
        }
    }
}
