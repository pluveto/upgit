use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Serialize;
use upgit_core::{Artifact, Locator, ObjectKey, UploadError, Uploader};

use crate::util::{
    could_not_reach, host_of, http_origin, join_host_path, json_string_field,
    looks_like_rate_limit, percent_encode, read_bytes,
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

    /// Map a GitLab Repository Files API status + body to a user-facing error.
    /// Never dumps JSON.
    pub fn explain(&self, status: u16, body: &str) -> UploadError {
        let project = self.config.project.as_str();
        match status {
            401 => UploadError::new(
                "GitLab",
                "GitLab personal access token is invalid or expired (HTTP 401).",
                format!(
                    "Create a token with the \"api\" or \"write_repository\" scope at {}/-/user_settings/personal_access_tokens and put it in [uploaders.gitlab] `token`.",
                    self.config.url
                ),
                Some(status),
            ),
            429 => Self::rate_limit_error(status),
            403 if looks_like_rate_limit(body) => Self::rate_limit_error(status),
            403 => UploadError::new(
                "GitLab",
                format!("GitLab denied access to `{project}` (HTTP 403)."),
                format!(
                    "The token in [uploaders.gitlab] `token` lacks access to `{project}`. Grant the \"api\" or \"write_repository\" scope and a Developer role on this project."
                ),
                Some(status),
            ),
            404 => UploadError::new(
                "GitLab",
                format!("GitLab project `{project}` was not found (HTTP 404)."),
                "Check [uploaders.gitlab] url, project, and token. A missing project, or a token without access, also returns 404. Private projects make GitLab raw URLs 404; set public_base to a CDN, reverse proxy, or public raw prefix.",
                Some(status),
            ),
            400 | 409 | 422 => UploadError::new(
                "GitLab",
                format!("GitLab cannot create or update that path (HTTP {status})."),
                "Check [uploaders.gitlab] branch and the file path.",
                Some(status),
            ),
            500..=599 => UploadError::new(
                "GitLab",
                format!("GitLab is failing (HTTP {status})."),
                "Retry later; this is a GitLab server error, not a config problem.",
                Some(status),
            ),
            _ => {
                let what = match json_string_field(body, "message") {
                    Some(msg) => format!("GitLab upload failed (HTTP {status}): {msg}"),
                    None => format!("GitLab upload failed (HTTP {status})."),
                };
                UploadError::new(
                    "GitLab",
                    what,
                    "Verify [uploaders.gitlab] url, project, token, and branch. Private projects make GitLab raw URLs 404; set public_base.",
                    Some(status),
                )
            }
        }
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

    fn rate_limit_error(status: u16) -> UploadError {
        UploadError::new(
            "GitLab",
            format!("GitLab rate limit exceeded (HTTP {status})."),
            "Wait and retry, or use a token with a higher rate limit in [uploaders.gitlab] `token`.",
            Some(status),
        )
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
