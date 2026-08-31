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
fn empty_branch_defaults_to_master() {
    let key = ObjectKey::parse("logo.png").expect("key");
    let uploader = uploader("");
    assert_eq!(uploader.branch(), "master");
    assert_eq!(
        uploader.locator_for(&key).as_str(),
        "https://raw.githubusercontent.com/alice/pics/master/logo.png"
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
