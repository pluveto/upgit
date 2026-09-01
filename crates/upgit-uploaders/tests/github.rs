use upgit_core::ObjectKey;
use upgit_uploaders::github::{GithubConfig, GithubUploader};

fn uploader(branch: &str) -> GithubUploader {
    GithubUploader::new(GithubConfig {
        pat: "ghp_test".into(),
        username: "alice".into(),
        repo: "pics".into(),
        branch: branch.into(),
    })
}

#[test]
fn locator_is_raw_githubusercontent_url() {
    let key = ObjectKey::parse("2022/01/a.png").expect("key");
    assert_eq!(
        uploader("main").locator_for(&key).as_str(),
        "https://raw.githubusercontent.com/alice/pics/main/2022/01/a.png"
    );
}

#[test]
fn empty_branch_defaults_to_main() {
    let key = ObjectKey::parse("logo.png").expect("key");
    let uploader = uploader("");
    assert_eq!(uploader.branch(), "main");
    assert_eq!(
        uploader.locator_for(&key).as_str(),
        "https://raw.githubusercontent.com/alice/pics/main/logo.png"
    );
}

#[test]
fn contents_url_puts_key_on_the_github_api_path() {
    let key = ObjectKey::parse("2022/01/a.png").expect("key");
    assert_eq!(
        uploader("master").contents_url(&key),
        "https://api.github.com/repos/alice/pics/contents/2022/01/a.png"
    );
}

const NOT_FOUND_BODY: &str = r#"{"message":"Not Found","documentation_url":"https://docs.github.com/rest/repos/contents#create-or-update-file-contents","status":"404"}"#;

const BAD_CREDENTIALS_BODY: &str = r#"{"message":"Bad credentials","documentation_url":"https://docs.github.com/rest","status":"401"}"#;

const FORBIDDEN_BODY: &str = r#"{"message":"Resource not accessible by personal access token","documentation_url":"https://docs.github.com/rest/repos/contents#create-or-update-file-contents","status":"403"}"#;

const RATE_LIMIT_BODY: &str = r#"{"message":"API rate limit exceeded for user ID 1.","documentation_url":"https://docs.github.com/rest/overview/resources-in-the-rest-api#rate-limiting","status":"403"}"#;

fn assert_no_raw_github_json(s: &str) {
    assert!(!s.contains("documentation_url"), "dumped JSON: {s}");
    assert!(
        !s.contains("create-or-update-file-contents"),
        "dumped JSON: {s}"
    );
    assert!(!s.contains("resources-in-the-rest-api"), "dumped JSON: {s}");
}

#[test]
fn explain_404_does_not_dump_github_json() {
    let err = uploader("master").explain(404, NOT_FOUND_BODY);
    let s = err.to_string();
    assert!(
        s.contains("not found") || s.contains("Not found"),
        "got {s}"
    );
    assert!(s.contains("username") || s.contains("repo"), "got {s}");
    assert!(s.contains("alice/pics"), "got {s}");
    assert_no_raw_github_json(&s);
}

#[test]
fn explain_401_mentions_pat_not_json() {
    let err = uploader("master").explain(401, BAD_CREDENTIALS_BODY);
    let s = err.to_string();
    assert!(s.contains("401"), "got {s}");
    let lower = s.to_ascii_lowercase();
    assert!(lower.contains("pat") || lower.contains("token"), "got {s}");
    assert!(s.contains("hint:"), "got {s}");
    assert_no_raw_github_json(&s);
}

#[test]
fn explain_403_mentions_access_not_json() {
    let err = uploader("master").explain(403, FORBIDDEN_BODY);
    let s = err.to_string();
    assert!(s.contains("403"), "got {s}");
    let lower = s.to_ascii_lowercase();
    assert!(
        lower.contains("access") || lower.contains("permission") || lower.contains("pat"),
        "got {s}"
    );
    assert!(
        s.contains("username") || s.contains("repo") || s.contains("alice/pics"),
        "got {s}"
    );
    assert_no_raw_github_json(&s);
}

#[test]
fn explain_403_rate_limit_says_so() {
    let err = uploader("master").explain(403, RATE_LIMIT_BODY);
    let s = err.to_string();
    let lower = s.to_ascii_lowercase();
    assert!(lower.contains("rate limit"), "got {s}");
    assert_no_raw_github_json(&s);
}

#[test]
fn explain_other_uses_truncated_message_not_full_json() {
    let err = uploader("master").explain(
        418,
        r#"{"message":"I'm a teapot","documentation_url":"https://docs.github.com/rest/repos/contents#create-or-update-file-contents","status":"418"}"#,
    );
    let s = err.to_string();
    assert!(s.contains("418"), "got {s}");
    assert!(s.contains("teapot"), "got {s}");
    assert!(s.contains("hint:"), "got {s}");
    assert!(s.contains("uploaders.github"), "got {s}");
    assert_no_raw_github_json(&s);
}
