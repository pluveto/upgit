use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Serialize;
use upgit_core::{Artifact, Locator, ObjectKey, UploadError, Uploader};

use crate::util::{read_bytes, status_error};

#[derive(Serialize)]
struct PutBody<'a> {
    branch: &'a str,
    message: String,
    content: String,
}

#[derive(Debug, Clone)]
pub struct GithubConfig {
    pub pat: String,
    pub username: String,
    pub repo: String,
    pub branch: String,
}

#[derive(Debug, Clone)]
pub struct GithubUploader {
    config: GithubConfig,
}

impl GithubUploader {
    pub fn new(mut config: GithubConfig) -> Self {
        if config.branch.trim().is_empty() {
            config.branch = "master".to_string();
        }
        Self { config }
    }

    pub fn branch(&self) -> &str {
        &self.config.branch
    }

    pub fn contents_url(&self, key: &ObjectKey) -> String {
        format!(
            "https://api.github.com/repos/{}/{}/contents/{}",
            self.config.username,
            self.config.repo,
            key.as_str()
        )
    }

    pub fn locator_for(&self, key: &ObjectKey) -> Locator {
        Locator::new(format!(
            "https://raw.githubusercontent.com/{}/{}/{}/{}",
            self.config.username,
            self.config.repo,
            self.config.branch,
            key.as_str()
        ))
    }
}

impl Uploader for GithubUploader {
    fn upload(&self, artifact: &Artifact, key: &ObjectKey) -> Result<Locator, UploadError> {
        let data = read_bytes(artifact)?;
        let body = serde_json::to_string(&PutBody {
            branch: &self.config.branch,
            message: format!("upload {} via upgit", artifact.file_name()),
            content: STANDARD.encode(&data),
        })
        .map_err(|e| UploadError::message(e.to_string()))?;
        let url = self.contents_url(key);
        match ureq::put(&url)
            .set("Authorization", &format!("token {}", self.config.pat))
            .set("Accept", "application/vnd.github.v3+json")
            .set("Content-Type", "application/json")
            .set("User-Agent", "upgit")
            .send_string(&body)
        {
            Ok(_) => Ok(self.locator_for(key)),
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                if text.contains("sha wasn't supplied") {
                    return Ok(self.locator_for(key));
                }
                Err(status_error("github", code, &text))
            }
            Err(e) => Err(UploadError::message(e.to_string())),
        }
    }
}
