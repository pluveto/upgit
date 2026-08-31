use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha1::Sha1;
use upgit_core::{Artifact, Locator, ObjectKey, UploadError, Uploader};

use crate::form::{self, Part};
use crate::util::{could_not_reach, host_of, json_string_field, looks_like_signature_error};

type HmacSha1 = Hmac<Sha1>;

/// Compact policy JSON must keep this field order (scope, then deadline).
#[derive(Serialize)]
struct UploadPolicy<'a> {
    scope: &'a str,
    deadline: u64,
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

    /// Mint an upload token from AK/SK. The Qiniu object owns this; callers do not.
    pub fn mint_token(
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

    fn token_for_upload(&self) -> String {
        let deadline = SystemTime::now() + Duration::from_secs(3600);
        Self::mint_token(
            &self.config.access_key,
            &self.config.secret_key,
            &self.config.bucket,
            deadline,
        )
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

    /// Map a Qiniu HTTP status + body to a user-facing error. Never dumps JSON.
    pub fn explain(&self, status: u16, body: &str) -> UploadError {
        let bucket = self.config.bucket.as_str();
        let lower = body.to_ascii_lowercase();
        match status {
            401 => UploadError::new(
                "Qiniu",
                "Qiniu rejected credentials (HTTP 401).",
                "Check [uploaders.qiniu] access_key and secret_key.",
                Some(status),
            ),
            403 if looks_like_signature_error(body) => UploadError::new(
                "Qiniu",
                "Qiniu signature did not match (HTTP 403).",
                "Check [uploaders.qiniu] access_key and secret_key.",
                Some(status),
            ),
            403 => UploadError::new(
                "Qiniu",
                format!("Qiniu denied access to bucket `{bucket}` (HTTP 403)."),
                "Check [uploaders.qiniu] access_key, secret_key, and bucket.",
                Some(status),
            ),
            404 => UploadError::new(
                "Qiniu",
                "Qiniu upload endpoint was not found (HTTP 404).",
                "Check [uploaders.qiniu] region. The bucket must exist in that region.",
                Some(status),
            ),
            500..=599 => UploadError::new(
                "Qiniu",
                format!("Qiniu is failing (HTTP {status})."),
                "Retry later; this is a Qiniu server error, not a config problem.",
                Some(status),
            ),
            _ if lower.contains("incorrect region") || lower.contains("wrong region") => {
                UploadError::new(
                    "Qiniu",
                    format!("Qiniu upload used the wrong region (HTTP {status})."),
                    "Check [uploaders.qiniu] region (z0, z1, z2, na0, as0, …).",
                    Some(status),
                )
            }
            _ if lower.contains("no such bucket")
                || lower.contains("nosuchbucket")
                || lower.contains("\"error_code\":631")
                || lower.contains("error_code\": 631") =>
            {
                UploadError::new(
                    "Qiniu",
                    format!("Qiniu bucket `{bucket}` was not found (HTTP {status})."),
                    "Check [uploaders.qiniu] bucket. The bucket must exist.",
                    Some(status),
                )
            }
            _ if looks_like_signature_error(body)
                || lower.contains("bad token")
                || lower.contains("invalid token") =>
            {
                UploadError::new(
                    "Qiniu",
                    format!("Qiniu rejected the upload token (HTTP {status})."),
                    "Check [uploaders.qiniu] access_key and secret_key.",
                    Some(status),
                )
            }
            _ => {
                let what = match json_string_field(body, "error") {
                    Some(msg) => format!("Qiniu upload failed (HTTP {status}): {msg}"),
                    None => format!("Qiniu upload failed (HTTP {status})."),
                };
                UploadError::new(
                    "Qiniu",
                    what,
                    "Verify [uploaders.qiniu] access_key, secret_key, bucket, and region.",
                    Some(status),
                )
            }
        }
    }
}

impl Uploader for QiniuUploader {
    fn upload(&self, artifact: &Artifact, key: &ObjectKey) -> Result<Locator, UploadError> {
        let path = artifact.path().ok_or_else(|| {
            UploadError::message("artifact has no local path; cannot upload bytes")
        })?;
        let data = std::fs::read(path).map_err(|e| UploadError::message(e.to_string()))?;
        let token = self.token_for_upload();
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
                Err(self.explain(code, &text))
            }
            Err(e) => Err(could_not_reach("Qiniu", host_of(&self.upload_url()), e)),
        }
    }
}
