use md5::{Digest, Md5};
use upgit_core::{Artifact, Locator, ObjectKey, UploadError, Uploader};

use crate::util::{hex_lower, hostname, http_date_gmt, join_host_path, read_bytes, status_error};

#[derive(Debug, Clone)]
pub struct UpyunConfig {
    pub host: String,
    pub bucket_name: String,
    pub user_name: String,
    pub pass_word: String,
}

#[derive(Debug, Clone)]
pub struct UpyunUploader {
    config: UpyunConfig,
}

impl UpyunUploader {
    pub fn new(config: UpyunConfig) -> Self {
        Self { config }
    }

    pub fn locator_for(&self, key: &ObjectKey) -> Locator {
        let host = hostname(&self.config.host);
        Locator::new(format!("https://{}", join_host_path(host, key.as_str())))
    }

    fn uri_for(&self, key: &ObjectKey) -> String {
        format!("/{}/{}", self.config.bucket_name, key.as_str())
    }

    pub fn authorization_for(&self, method: &str, uri: &str, date: &str, length: i64) -> String {
        let password_md5 = md5_hex(self.config.pass_word.as_bytes());
        let sign_src = format!("{method}&{uri}&{date}&{length}&{password_md5}");
        let sig = md5_hex(sign_src.as_bytes());
        format!("UpYun {}:{}", self.config.user_name, sig)
    }
}

impl Uploader for UpyunUploader {
    fn upload(&self, artifact: &Artifact, key: &ObjectKey) -> Result<Locator, UploadError> {
        let data = read_bytes(artifact)?;
        let uri = self.uri_for(key);
        let url = format!("http://v0.api.upyun.com{uri}");
        let date = http_date_gmt(std::time::SystemTime::now());
        let length = data.len() as i64;
        // WriteFile converts PUT to POST then signs with POST.
        let authorization = self.authorization_for("POST", &uri, &date, length);
        match ureq::post(&url)
            .set("Date", &date)
            .set("Authorization", &authorization)
            .set("Mkdir", "true")
            .send_bytes(&data)
        {
            Ok(_) => Ok(self.locator_for(key)),
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                Err(status_error("upyun", code, &text))
            }
            Err(e) => Err(UploadError::message(e.to_string())),
        }
    }
}

fn md5_hex(data: &[u8]) -> String {
    hex_lower(&Md5::digest(data))
}
