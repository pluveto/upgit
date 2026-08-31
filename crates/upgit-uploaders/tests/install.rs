use upgit_core::Registry;
use upgit_uploaders::AppConfig;

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
    let err = cfg.install_into(&mut registry).expect_err("unknown kind");
    let msg = err.to_string();
    assert!(msg.contains("sftp"), "got {msg}");
    assert!(msg.contains("mystery"), "got {msg}");
}

#[test]
fn zhihu_shaped_qiniu_config_needs_no_jsonc() {
    let cfg = AppConfig::from_toml(
        r#"
default_uploader = "qiniu"

[uploaders.qiniu]
bucket = "moqian-public"
access_key = "ak"
secret_key = "sk"
prefix = "http://file.moqian.cn/"
"#,
    )
    .expect("parse");
    assert_eq!(cfg.default_uploader(), Some("qiniu"));
    let mut registry = Registry::new();
    cfg.install_into(&mut registry).expect("install");
    registry.get("qiniu").expect("qiniu registered");
}

#[test]
fn sample_config_installs_qiniu_without_extensions_dir() {
    let cfg = AppConfig::from_toml(include_str!("../../../config.sample.toml")).expect("sample");
    let mut registry = Registry::new();
    cfg.install_into(&mut registry).expect("install sample");
    registry.get("qiniu").expect("qiniu");
}

#[test]
fn qiniu_static_token_is_rejected() {
    let cfg = AppConfig::from_toml(
        r#"
[uploaders.qiniu]
token = "expired-web-token"
prefix = "https://cdn.example.com/"
"#,
    )
    .expect("parse");
    let mut registry = Registry::new();
    let err = cfg.install_into(&mut registry).expect_err("token");
    let msg = err.to_string();
    assert!(
        msg.contains("expire") || msg.contains("access_key"),
        "got {msg}"
    );
    assert!(
        !msg.to_lowercase().contains("no such file"),
        "must not look like a missing local file: {msg}"
    );
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
    let err = cfg.install_into(&mut registry).expect_err("missing field");
    let msg = err.to_string();
    assert!(msg.contains("access_key"), "got {msg}");
}
