use upgit_core::Registry;
use upgit_uploaders::{AppConfig, InstallError};

#[test]
fn install_registers_qiniu_and_http_from_config() {
    let cfg = AppConfig::from_toml(
        r#"
default = "qiniu"
naming = "{year}/{month}/{stem}_{unix}{ext}"

[link]
"raw.githubusercontent.com" = "cdn.jsdelivr.net/gh"

[uploaders.qiniu]
type = "qiniu"
access_key = "ak"
secret_key = "sk"
bucket = "bucket"
public_base = "https://cdn.example.com/"
region = "z0"

[uploaders.smms]
type = "http"
recipe = "smms"
token = "tok"
"#,
    )
    .expect("parse");
    let mut registry = Registry::new();
    cfg.install_into(&mut registry).expect("install");
    registry.get("qiniu").expect("qiniu registered");
    registry.get("smms").expect("smms registered");
    let err = registry.get("nope").expect_err("unknown");
    let msg = err.to_string();
    assert!(msg.contains("nope"), "got {msg}");
    assert!(msg.contains("qiniu"), "got {msg}");
    assert!(msg.contains("smms"), "got {msg}");
}

#[test]
fn install_registers_gitlab_from_profile_without_type() {
    let cfg = AppConfig::from_toml(
        r#"
default = "gitlab"

[uploaders.gitlab]
url = "https://gitlab.example.com"
project = "group/name"
token = "tok"
branch = "main"
public_base = "https://cdn.example.com/"
"#,
    )
    .expect("parse");
    let mut registry = Registry::new();
    cfg.install_into(&mut registry).expect("install");
    registry.get("gitlab").expect("gitlab registered");
}

#[test]
fn install_infers_gitlab_from_token_project_host() {
    let cfg = AppConfig::from_toml(
        r#"
[uploaders.corp]
host = "https://gitlab.corp.example"
project = "group/name"
token = "tok"
"#,
    )
    .expect("parse");
    let mut registry = Registry::new();
    cfg.install_into(&mut registry).expect("install");
    registry.get("corp").expect("corp registered as gitlab");
}

#[test]
fn unknown_kind_lists_gitlab_among_built_ins() {
    let cfg = AppConfig::from_toml(
        r#"
[uploaders.webdav]
type = "webdav"
"#,
    )
    .expect("parse");
    let mut registry = Registry::new();
    let err = cfg.install_into(&mut registry).expect_err("unknown kind");
    let msg = err.to_string();
    match err {
        InstallError::UnknownKind { id, kind } => {
            assert_eq!(id, "webdav");
            assert_eq!(kind, "webdav");
        }
        other => panic!("expected UnknownKind, got {other}"),
    }
    assert!(msg.contains("gitlab"), "got {msg}");
    assert!(msg.contains("github"), "got {msg}");
}
