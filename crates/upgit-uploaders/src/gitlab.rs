use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Serialize;
use upgit_core::{Artifact, Locator, ObjectKey, UploadError, Uploader};

use crate::util::{
    could_not_reach, host_of, http_origin, join_host_path, json_string_field, percent_encode,
    read_bytes, remote_http_error,
};

#[derive(Serialize)]
struct FileBody<'a> {
    branch: &'a str,
    content: String,
    encoding: &'a str,
    commit_message: String,
}

#[derive(Debug, Clone)]
pub struct GitlabConfig {
    pub url: String,
    pub project: String,
    pub token: String,
    pub branch: String,
    pub public_base: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GitlabUploader {
    config: GitlabConfig,
}

impl GitlabUploader {
    pub fn new(mut config: GitlabConfig) -> Self {
        if config.branch.trim().is_empty() {
            config.branch = "main".to_string();
        }
        config.url = http_origin(&config.url);
        config.project = config.project.trim().trim_matches('/').to_string();
        config.public_base = config
            .public_base
            .map(|base| http_origin(&base))
            .filter(|s| !s.is_empty());
        Self { config }
    }

    pub fn locator_for(&self, key: &ObjectKey) -> Locator {
        if let Some(base) = &self.config.public_base {
            Locator::new(join_host_path(base, key.as_str()))
        } else {
            Locator::new(format!(
                "{}/{}/-/raw/{}/{}",
                self.config.url,
                self.config.project,
                self.config.branch,
                key.as_str()
            ))
        }
    }

    /// Status, project/branch, and GitLab's `message` if present. Does not guess a cause.
    pub fn explain(&self, status: u16, body: &str) -> UploadError {
        let mut target = format!("`{}`", self.config.project);
        if !self.config.branch.is_empty() {
            target.push_str(" branch `");
            target.push_str(&self.config.branch);
            target.push('`');
        }
        remote_http_error(
            "GitLab",
            status,
            &target,
            json_string_field(body, "message").as_deref(),
            "Check [uploaders.gitlab] url, project, token, and branch.",
        )
    }

    fn files_url(&self, key: &ObjectKey) -> String {
        format!(
            "{}/api/v4/projects/{}/repository/files/{}",
            self.config.url,
            percent_encode(&self.config.project),
            percent_encode(key.as_str()),
        )
    }

    fn encode_file_body(&self, file_name: &str, data: &[u8]) -> Result<String, UploadError> {
        serde_json::to_string(&FileBody {
            branch: &self.config.branch,
            content: STANDARD.encode(data),
            encoding: "base64",
            commit_message: format!("upload {file_name} via upgit"),
        })
        .map_err(|e| UploadError::message(e.to_string()))
    }

    fn file_already_exists(status: u16, body: &str) -> bool {
        status == 400 && body.to_ascii_lowercase().contains("already exists")
    }

    fn authorized(&self, req: ureq::Request) -> ureq::Request {
        req.set("PRIVATE-TOKEN", &self.config.token)
            .set("User-Agent", "upgit")
    }

    fn create_or_update(
        &self,
        key: &ObjectKey,
        artifact: &Artifact,
        data: &[u8],
    ) -> Result<Locator, UploadError> {
        let body = self.encode_file_body(artifact.file_name(), data)?;
        let url = self.files_url(key);
        match self
            .authorized(ureq::post(&url))
            .set("Content-Type", "application/json")
            .send_string(&body)
        {
            Ok(_) => Ok(self.locator_for(key)),
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                if Self::file_already_exists(code, &text) {
                    return self.update_file(&url, &body, key);
                }
                Err(self.explain(code, &text))
            }
            Err(e) => Err(could_not_reach("GitLab", host_of(&self.config.url), e)),
        }
    }

    fn update_file(&self, url: &str, body: &str, key: &ObjectKey) -> Result<Locator, UploadError> {
        match self
            .authorized(ureq::put(url))
            .set("Content-Type", "application/json")
            .send_string(body)
        {
            Ok(_) => Ok(self.locator_for(key)),
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                Err(self.explain(code, &text))
            }
            Err(e) => Err(could_not_reach("GitLab", host_of(&self.config.url), e)),
        }
    }
}

impl Uploader for GitlabUploader {
    fn upload(&self, artifact: &Artifact, key: &ObjectKey) -> Result<Locator, UploadError> {
        let data = read_bytes(artifact)?;
        self.create_or_update(key, artifact, &data)
    }
}
