use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use upgit_core::{Artifact, Locator, ObjectKey, UploadError, Uploader};

use crate::util::{
    content_type_for, could_not_reach, host_of, hostname, http_date_gmt, join_host_path,
    looks_like_missing_bucket, looks_like_signature_error, read_bytes, xml_error_summary,
};

type HmacSha1 = Hmac<Sha1>;

#[derive(Debug, Clone)]
pub struct OssConfig {
    pub endpoint: String,
    pub access_key_id: String,
    pub access_key_secret: String,
    pub bucket_name: String,
    pub host: String,
}

#[derive(Debug, Clone)]
pub struct OssUploader {
    config: OssConfig,
}

impl OssUploader {
    pub fn new(config: OssConfig) -> Self {
        Self { config }
    }

    pub fn locator_for(&self, key: &ObjectKey) -> Locator {
        Locator::new(join_host_path(&self.config.host, key.as_str()))
    }

    fn put_url(&self, key: &ObjectKey) -> String {
        let host = hostname(&self.config.endpoint);
        let scheme = if self.config.endpoint.trim().starts_with("http://") {
            "http"
        } else {
            "https"
        };
        format!(
            "{scheme}://{}.{}/{}",
            self.config.bucket_name,
            host,
            key.as_str()
        )
    }

    pub fn authorization_for(&self, content_type: &str, date: &str, key: &ObjectKey) -> String {
        let string_to_sign = format!(
            "PUT\n\n{content_type}\n{date}\n/{}/{}",
            self.config.bucket_name,
            key.as_str()
        );
        let mut mac =
            HmacSha1::new_from_slice(self.config.access_key_secret.as_bytes()).expect("hmac-sha1");
        mac.update(string_to_sign.as_bytes());
        let sig = STANDARD.encode(mac.finalize().into_bytes());
        format!("OSS {}:{}", self.config.access_key_id, sig)
    }

    /// Map an OSS HTTP status + body to a user-facing error. Never dumps XML.
    pub fn explain(&self, status: u16, body: &str) -> UploadError {
        let bucket = self.config.bucket_name.as_str();
        match status {
            401 => UploadError::new(
                "OSS",
                "OSS rejected credentials (HTTP 401).",
                "Check [uploaders.aliyunoss] access_key_id and access_key_secret.",
                Some(status),
            ),
            403 if looks_like_signature_error(body) => UploadError::new(
                "OSS",
                "OSS signature did not match (HTTP 403).",
                "Check [uploaders.aliyunoss] access_key_id, access_key_secret, and endpoint.",
                Some(status),
            ),
            403 => UploadError::new(
                "OSS",
                format!("OSS denied access to bucket `{bucket}` (HTTP 403)."),
                "Check [uploaders.aliyunoss] access_key_id, access_key_secret, and bucket_name.",
                Some(status),
            ),
            404 => UploadError::new(
                "OSS",
                format!("OSS bucket `{bucket}` was not found (HTTP 404)."),
                "Check [uploaders.aliyunoss] bucket_name and endpoint. The bucket must exist.",
                Some(status),
            ),
            500..=599 => UploadError::new(
                "OSS",
                format!("OSS is failing (HTTP {status})."),
                "Retry later; this is an OSS server error, not a config problem.",
                Some(status),
            ),
            _ if looks_like_signature_error(body) => UploadError::new(
                "OSS",
                format!("OSS signature did not match (HTTP {status})."),
                "Check [uploaders.aliyunoss] access_key_id, access_key_secret, and endpoint.",
                Some(status),
            ),
            _ if looks_like_missing_bucket(body) => UploadError::new(
                "OSS",
                format!("OSS bucket `{bucket}` was not found (HTTP {status})."),
                "Check [uploaders.aliyunoss] bucket_name and endpoint. The bucket must exist.",
                Some(status),
            ),
            _ => UploadError::new(
                "OSS",
                xml_error_summary("OSS", status, body),
                "Verify [uploaders.aliyunoss] endpoint, access_key_id, access_key_secret, bucket_name, and host.",
                Some(status),
            ),
        }
    }
}

impl Uploader for OssUploader {
    fn upload(&self, artifact: &Artifact, key: &ObjectKey) -> Result<Locator, UploadError> {
        let data = read_bytes(artifact)?;
        let content_type = content_type_for(artifact.file_name());
        let date = http_date_gmt(std::time::SystemTime::now());
        let authorization = self.authorization_for(content_type, &date, key);
        let url = self.put_url(key);
        match ureq::put(&url)
            .set("Authorization", &authorization)
            .set("Content-Type", content_type)
            .set("Date", &date)
            .send_bytes(&data)
        {
            Ok(_) => Ok(self.locator_for(key)),
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                Err(self.explain(code, &text))
            }
            Err(e) => {
                let host = format!(
                    "{}.{}",
                    self.config.bucket_name,
                    host_of(&self.config.endpoint)
                );
                Err(could_not_reach("OSS", &host, e))
            }
        }
    }
}
