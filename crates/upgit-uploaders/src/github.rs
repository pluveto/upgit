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
    #[serde(skip_serializing_if = "Option::is_none")]
    sha: Option<&'a str>,
}

fn encode_put_body(
    branch: &str,
    file_name: &str,
    data: &[u8],
    sha: Option<&str>,
) -> Result<String, UploadError> {
    serde_json::to_string(&PutBody {
        branch,
        message: format!("upload {file_name} via upgit"),
        content: STANDARD.encode(data),
        sha,
    })
    .map_err(|e| UploadError::message(e.to_string()))
}

fn existing_file_needs_sha(status: u16, body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    if lower.contains("wasn't supplied") || lower.contains("wasnt supplied") {
        return true;
    }
    matches!(status, 409 | 422) && lower.contains("sha")
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
            config.branch = "main".to_string();
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

    fn authorized(&self, req: ureq::Request) -> ureq::Request {
        req.set("Authorization", &format!("token {}", self.config.pat))
            .set("Accept", "application/vnd.github.v3+json")
            .set("User-Agent", "upgit")
    }

    fn put_contents(
        &self,
        key: &ObjectKey,
        artifact: &Artifact,
        data: &[u8],
        sha: Option<&str>,
    ) -> Result<Locator, UploadError> {
        let body = encode_put_body(&self.config.branch, artifact.file_name(), data, sha)?;
        let url = self.contents_url(key);
        match self
            .authorized(ureq::put(&url))
            .set("Content-Type", "application/json")
            .send_string(&body)
        {
            Ok(_) => Ok(self.locator_for(key)),
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                if sha.is_none() && existing_file_needs_sha(code, &text) {
                    let existing = self.fetch_sha(key)?;
                    return self.put_contents(key, artifact, data, Some(&existing));
                }
                Err(self.explain(code, &text))
            }
            Err(e) => Err(could_not_reach("GitHub", "api.github.com", e)),
        }
    }

    fn fetch_sha(&self, key: &ObjectKey) -> Result<String, UploadError> {
        let url = self.contents_url(key);
        match self
            .authorized(ureq::get(&url))
            .query("ref", &self.config.branch)
            .call()
        {
            Ok(resp) => {
                let text = resp.into_string().unwrap_or_default();
                json_string_field(&text, "sha")
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| self.explain(200, &text))
            }
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                Err(self.explain(code, &text))
            }
            Err(e) => Err(could_not_reach("GitHub", "api.github.com", e)),
        }
    }
}

impl Uploader for GithubUploader {
    fn upload(&self, artifact: &Artifact, key: &ObjectKey) -> Result<Locator, UploadError> {
        let data = read_bytes(artifact)?;
        self.put_contents(key, artifact, &data, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_body_with_sha_serializes_sha() {
        let json = encode_put_body("main", "logo.png", b"abc", Some("deadbeef")).expect("json");
        let v: serde_json::Value = serde_json::from_str(&json).expect("parse");
        assert_eq!(v["sha"], "deadbeef");
        assert_eq!(v["branch"], "main");
        assert_eq!(v["message"], "upload logo.png via upgit");
        assert!(v["content"].as_str().is_some());
    }

    #[test]
    fn put_body_without_sha_omits_field() {
        let json = encode_put_body("main", "logo.png", b"abc", None).expect("json");
        let v: serde_json::Value = serde_json::from_str(&json).expect("parse");
        assert!(v.get("sha").is_none());
        assert_eq!(v["branch"], "main");
    }
}
