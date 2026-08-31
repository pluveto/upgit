use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Serialize;
use upgit_core::{Artifact, Locator, ObjectKey, UploadError, Uploader};

use crate::util::{could_not_reach, json_string_field, looks_like_rate_limit, read_bytes};

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

    fn repo_slug(&self) -> String {
        format!("{}/{}", self.config.username, self.config.repo)
    }

    /// Map a GitHub Contents API status + body to a user-facing error. Never dumps JSON.
    pub fn explain(&self, status: u16, body: &str) -> UploadError {
        let slug = self.repo_slug();
        match status {
            401 => UploadError::new(
                "GitHub",
                "GitHub personal access token is invalid or expired (HTTP 401).",
                "Create a token with the \"repo\" scope at https://github.com/settings/tokens and put it in [uploaders.github] `pat`.",
                Some(status),
            ),
            429 => Self::rate_limit_error(status),
            403 if looks_like_rate_limit(body) => Self::rate_limit_error(status),
            403 => UploadError::new(
                "GitHub",
                format!("GitHub denied access to `{slug}` (HTTP 403)."),
                format!(
                    "The PAT in [uploaders.github] `pat` lacks access to `{slug}`. Grant the \"repo\" scope and access to this repository."
                ),
                Some(status),
            ),
            404 => UploadError::new(
                "GitHub",
                format!("GitHub repository `{slug}` was not found (HTTP 404)."),
                "Check [uploaders.github] username and repo. The repo must exist. A missing or private repo, or a PAT without the \"repo\" scope, also returns 404.",
                Some(status),
            ),
            409 | 422 => UploadError::new(
                "GitHub",
                format!("GitHub cannot create or update that path (HTTP {status})."),
                "Check [uploaders.github] branch and the file path. Updating an existing file on the Contents API requires its SHA.",
                Some(status),
            ),
            500..=599 => UploadError::new(
                "GitHub",
                format!("GitHub is failing (HTTP {status})."),
                "Retry later; this is a GitHub server error, not a config problem.",
                Some(status),
            ),
            _ => {
                let what = match json_string_field(body, "message") {
                    Some(msg) => format!("GitHub upload failed (HTTP {status}): {msg}"),
                    None => format!("GitHub upload failed (HTTP {status})."),
                };
                UploadError::new(
                    "GitHub",
                    what,
                    "Verify [uploaders.github] pat, username, repo, and branch.",
                    Some(status),
                )
            }
        }
    }

    fn rate_limit_error(status: u16) -> UploadError {
        UploadError::new(
            "GitHub",
            format!("GitHub rate limit exceeded (HTTP {status})."),
            "Wait and retry, or use a PAT with a higher rate limit in [uploaders.github] `pat`.",
            Some(status),
        )
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
                Err(self.explain(code, &text))
            }
            Err(e) => Err(could_not_reach("GitHub", "api.github.com", e)),
        }
    }
}
