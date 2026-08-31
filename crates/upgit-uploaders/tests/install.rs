use upgit_core::Registry;
use upgit_uploaders::{install, AppConfig};

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
    install(&mut registry, cfg.uploaders.clone()).expect("install");
    registry.get("qiniu").expect("qiniu registered");
    registry.get("smms").expect("smms registered");
    let err = registry.get("nope").expect_err("unknown");
    let msg = err.to_string();
    assert!(msg.contains("nope"), "got {msg}");
    assert!(msg.contains("qiniu"), "got {msg}");
    assert!(msg.contains("smms"), "got {msg}");
}

#[test]
fn install_rejects_unknown_kind() {
    let cfg = AppConfig::from_toml(
        r#"
[uploaders.mystery]
type = "sftp"
host = "example"
"#,
    )
    .expect("parse");
    let mut registry = Registry::new();
    let err = install(&mut registry, cfg.uploaders).expect_err("unknown kind");
    let msg = err.to_string();
    assert!(msg.contains("sftp"), "got {msg}");
    assert!(msg.contains("mystery"), "got {msg}");
}

#[test]
fn qiniu_profile_requires_access_key() {
    let cfg = AppConfig::from_toml(
        r#"
[uploaders.qiniu]
type = "qiniu"
secret_key = "sk"
bucket = "bucket"
public_base = "https://cdn.example.com/"
"#,
    )
    .expect("parse");
    let mut registry = Registry::new();
    let err = install(&mut registry, cfg.uploaders).expect_err("missing field");
    let msg = err.to_string();
    assert!(msg.contains("access_key"), "got {msg}");
}
