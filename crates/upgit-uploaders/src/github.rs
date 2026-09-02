use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Serialize;
use upgit_core::{Artifact, Locator, ObjectKey, UploadError, Uploader};

use crate::util::{could_not_reach, json_string_field, looks_like_rate_limit, read_bytes};

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

#[derive(Debug, Clone, PartialEq, Eq)]
enum RepoProbe {
    Missing,
    Found { default_branch: String },
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BranchProbe {
    Missing,
    Found,
    Unknown,
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
            404 => self.explain_not_found(self.probe_repo(), self.probe_branch()),
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

    fn repo_url(&self) -> String {
        format!(
            "https://api.github.com/repos/{}/{}",
            self.config.username, self.config.repo
        )
    }

    fn probe_repo(&self) -> RepoProbe {
        match self.authorized(ureq::get(&self.repo_url())).call() {
            Ok(resp) => {
                let text = resp.into_string().unwrap_or_default();
                match json_string_field(&text, "default_branch") {
                    Some(branch) => RepoProbe::Found {
                        default_branch: branch,
                    },
                    None => RepoProbe::Found {
                        default_branch: String::new(),
                    },
                }
            }
            Err(ureq::Error::Status(404 | 401, _)) => RepoProbe::Missing,
            Err(_) => RepoProbe::Unknown,
        }
    }

    fn probe_branch(&self) -> BranchProbe {
        if self.config.branch.is_empty() {
            return BranchProbe::Unknown;
        }
        let url = format!("{}/branches/{}", self.repo_url(), self.config.branch);
        match self.authorized(ureq::get(&url)).call() {
            Ok(_) => BranchProbe::Found,
            Err(ureq::Error::Status(404, _)) => BranchProbe::Missing,
            Err(_) => BranchProbe::Unknown,
        }
    }

    fn explain_not_found(&self, repo: RepoProbe, branch: BranchProbe) -> UploadError {
        let slug = self.repo_slug();
        let configured = self.config.branch.as_str();
        match (repo, branch, configured) {
            (RepoProbe::Missing, _, _) => UploadError::new(
                "GitHub",
                format!("GitHub repository `{slug}` was not found (HTTP 404)."),
                "Check [uploaders.github] username and repo. The repo must exist. A missing or private repo, or a PAT without the \"repo\" scope, also returns 404.",
                Some(404),
            ),
            (RepoProbe::Found { default_branch }, BranchProbe::Missing, branch)
                if !branch.is_empty() =>
            {
                let hint = if !default_branch.is_empty() && default_branch != branch {
                    format!(
                        "The repository exists; its default branch is `{default_branch}`. Set [uploaders.github] branch to `{default_branch}`, or omit branch to use the default."
                    )
                } else {
                    format!(
                        "The repository `{slug}` exists, but branch `{branch}` does not. Set [uploaders.github] branch to a branch that exists, or omit it to use the default."
                    )
                };
                UploadError::new(
                    "GitHub",
                    format!("GitHub branch `{branch}` was not found in `{slug}` (HTTP 404)."),
                    hint,
                    Some(404),
                )
            }
            (RepoProbe::Found { default_branch }, _, branch) if !branch.is_empty() => {
                let hint = if !default_branch.is_empty() && default_branch != branch {
                    format!(
                        "The repository exists; default branch is `{default_branch}`. Confirm [uploaders.github] branch (`{branch}`) and that the PAT can write Contents."
                    )
                } else {
                    format!(
                        "The repository `{slug}` exists. Confirm [uploaders.github] branch (`{branch}`) and that the PAT can write Contents."
                    )
                };
                UploadError::new(
                    "GitHub",
                    format!(
                        "GitHub could not write that path on `{slug}` branch `{branch}` (HTTP 404)."
                    ),
                    hint,
                    Some(404),
                )
            }
            (RepoProbe::Found { .. }, _, _) => UploadError::new(
                "GitHub",
                format!("GitHub could not write that path in `{slug}` (HTTP 404)."),
                "The repository exists. Confirm the PAT can write Contents on this repo.",
                Some(404),
            ),
            (RepoProbe::Unknown, _, branch) if !branch.is_empty() => UploadError::new(
                "GitHub",
                format!(
                    "GitHub returned 404 for `{slug}` on branch `{branch}`."
                ),
                "The repo may be missing or private, the branch may not exist, or the PAT lacks the \"repo\" scope. GitHub returns 404 for all of these.",
                Some(404),
            ),
            (RepoProbe::Unknown, _, _) => UploadError::new(
                "GitHub",
                format!("GitHub repository `{slug}` was not found (HTTP 404)."),
                "Check [uploaders.github] username and repo. A missing or private repo, or a PAT without the \"repo\" scope, also returns 404.",
                Some(404),
            ),
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
                Err(self.explain(code, &text))
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
    fn contents_404_wrong_branch_does_not_claim_missing_repo() {
        let err = gh("main").explain_not_found(
            RepoProbe::Found {
                default_branch: "master".into(),
            },
            BranchProbe::Missing,
        );
        let text = err.to_string();
        assert!(
            text.contains("branch `main` was not found in `pluveto/0images`"),
            "{text}"
        );
        assert!(text.contains("default branch is `master`"), "{text}");
        assert!(
            !text.contains("repository `pluveto/0images` was not found"),
            "{text}"
        );
        assert!(!text.contains("documentation_url"), "{text}");
    }

    #[test]
    fn contents_404_missing_repo_says_repo() {
        let err = gh("main").explain_not_found(RepoProbe::Missing, BranchProbe::Unknown);
        assert!(
            err.to_string()
                .contains("repository `pluveto/0images` was not found"),
            "{err}"
        );
    }

    #[test]
    fn locator_uses_head_when_branch_omitted() {
        let key = ObjectKey::parse("a.png").unwrap();
        assert!(gh("").locator_for(&key).as_str().contains("/HEAD/a.png"));
    }
}
