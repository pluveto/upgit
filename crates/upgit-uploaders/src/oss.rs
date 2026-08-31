use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use upgit_core::{Artifact, Locator, ObjectKey, UploadError, Uploader};

use crate::util::{
    content_type_for, hostname, http_date_gmt, join_host_path, read_bytes, status_error,
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
                Err(status_error("aliyunoss", code, &text))
            }
            Err(e) => Err(UploadError::message(e.to_string())),
        }
    }
}
