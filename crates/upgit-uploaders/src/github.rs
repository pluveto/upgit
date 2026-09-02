use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Serialize;
use upgit_core::{Artifact, Locator, ObjectKey, UploadError, Uploader};

use crate::util::{could_not_reach, json_string_field, read_bytes, remote_http_error};

#[derive(Serialize)]
struct PutBody<'a> {
    #[serde(skip_serializing_if = "str::is_empty")]
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
        config.branch = config.branch.trim().to_string();
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
        let branch = if self.config.branch.is_empty() {
            "HEAD"
        } else {
            self.config.branch.as_str()
        };
        Locator::new(format!(
            "https://raw.githubusercontent.com/{}/{}/{}/{}",
            self.config.username,
            self.config.repo,
            branch,
            key.as_str()
        ))
    }

    fn repo_slug(&self) -> String {
        format!("{}/{}", self.config.username, self.config.repo)
    }

    /// Status, request target, and GitHub's `message` if present. Does not guess a cause.
    pub fn explain(&self, status: u16, body: &str) -> UploadError {
        self.explain_for(status, body, None)
    }

    fn explain_for(&self, status: u16, body: &str, key: Option<&ObjectKey>) -> UploadError {
        remote_http_error(
            "GitHub",
            status,
            &self.request_target(key),
            json_string_field(body, "message").as_deref(),
            "Check [uploaders.github] pat, username, repo, and branch.",
        )
    }

    fn request_target(&self, key: Option<&ObjectKey>) -> String {
        let mut target = format!("`{}`", self.repo_slug());
        if !self.config.branch.is_empty() {
            target.push_str(" branch `");
            target.push_str(&self.config.branch);
            target.push('`');
        }
        if let Some(key) = key {
            target.push_str(" path `");
            target.push_str(key.as_str());
            target.push('`');
        }
        target
    }

    fn locator_from_put(&self, body: &str, key: &ObjectKey) -> Locator {
        let parsed = serde_json::from_str::<serde_json::Value>(body.trim()).ok();
        let url = parsed
            .as_ref()
            .and_then(|v| v.get("content"))
            .and_then(|c| c.get("download_url"))
            .and_then(|u| u.as_str())
            .filter(|s| !s.is_empty());
        match url {
            Some(url) => Locator::new(url),
            None => self.locator_for(key),
        }
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
            Ok(resp) => {
                let text = resp.into_string().unwrap_or_default();
                Ok(self.locator_from_put(&text, key))
            }
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                if sha.is_none() && existing_file_needs_sha(code, &text) {
                    let existing = self.fetch_sha(key)?;
                    return self.put_contents(key, artifact, data, Some(&existing));
                }
                Err(self.explain_for(code, &text, Some(key)))
            }
            Err(e) => Err(could_not_reach("GitHub", "api.github.com", e)),
        }
    }

    fn fetch_sha(&self, key: &ObjectKey) -> Result<String, UploadError> {
        let url = self.contents_url(key);
        let mut req = self.authorized(ureq::get(&url));
        if !self.config.branch.is_empty() {
            req = req.query("ref", &self.config.branch);
        }
        match req.call() {
            Ok(resp) => {
                let text = resp.into_string().unwrap_or_default();
                json_string_field(&text, "sha")
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| self.explain_for(200, &text, Some(key)))
            }
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                Err(self.explain_for(code, &text, Some(key)))
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
    use upgit_core::ObjectKey;

    fn gh(branch: &str) -> GithubUploader {
        GithubUploader::new(GithubConfig {
            pat: "x".into(),
            username: "pluveto".into(),
            repo: "0images".into(),
            branch: branch.into(),
        })
    }

    #[test]
    fn empty_branch_is_not_rewritten_to_main() {
        assert_eq!(gh("").branch(), "");
        assert_eq!(gh("  master  ").branch(), "master");
    }

    #[test]
    fn put_body_omits_empty_branch() {
        let body = encode_put_body("", "a.png", b"hi", None).unwrap();
        assert!(!body.contains("branch"), "{body}");
        let body = encode_put_body("master", "a.png", b"hi", None).unwrap();
        assert!(body.contains("\"branch\":\"master\""), "{body}");
    }

    #[test]
    fn contents_404_quotes_status_target_and_github_message() {
        let err = gh("main").explain(
            404,
            r#"{"message":"Not Found","documentation_url":"https://docs.github.com/rest"}"#,
        );
        let text = err.to_string();
        assert!(text.contains("HTTP 404"), "{text}");
        assert!(text.contains("`pluveto/0images`"), "{text}");
        assert!(text.contains("branch `main`"), "{text}");
        assert!(text.contains("Not Found"), "{text}");
        assert!(
            !text.contains("repository `pluveto/0images` was not found"),
            "{text}"
        );
        assert!(!text.contains("documentation_url"), "{text}");
    }

    #[test]
    fn locator_uses_head_when_branch_omitted() {
        let key = ObjectKey::parse("a.png").unwrap();
        assert!(gh("").locator_for(&key).as_str().contains("/HEAD/a.png"));
    }
}
