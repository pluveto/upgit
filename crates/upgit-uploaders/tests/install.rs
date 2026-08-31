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
fn smms_table_name_is_enough_without_type_or_recipe_field() {
    let cfg = AppConfig::from_toml(
        r#"
default = "smms"

[uploaders.smms]
token = "tok"
"#,
    )
    .expect("parse");
    let mut registry = Registry::new();
    cfg.install_into(&mut registry).expect("install smms");
    registry.get("smms").expect("smms");
}

#[test]
fn catalog_bundles_http_recipes() {
    use upgit_uploaders::RecipeCatalog;
    assert!(RecipeCatalog::contains("smms"));
    assert!(RecipeCatalog::contains("lskypro2"));
    assert!(RecipeCatalog::contains("gitee"));
    assert!(RecipeCatalog::load("smms").is_ok());
    RecipeCatalog::load("gitee").expect("load gitee");
    for id in [
        "dalexni",
        "imgtg",
        "juejin",
        "moetu",
        "netease",
        "sougou",
        "upload_cc",
    ] {
        assert!(RecipeCatalog::contains(id), "missing recipe {id}");
        RecipeCatalog::load(id).unwrap_or_else(|e| panic!("load {id}: {e}"));
    }
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

fn every_assignment_has_a_comment(text: &str) {
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('[') {
            continue;
        }
        if trimmed.contains('=') {
            assert!(
                i > 0 && lines[i - 1].trim_start().starts_with('#'),
                "field `{trimmed}` needs a comment immediately above it"
            );
        }
    }
}

#[test]
fn sample_config_defaults_to_github_and_lists_uploaders() {
    let text = include_str!("../../../config.sample.toml");
    let cfg = AppConfig::from_toml(text).expect("sample");
    assert_eq!(cfg.default_uploader(), Some("github"));
    every_assignment_has_a_comment(text);
    for table in [
        "[uploaders.github]",
        "[uploaders.s3]",
        "[uploaders.aliyunoss]",
        "[uploaders.qcloudcos]",
        "[uploaders.upyun]",
        "[uploaders.qiniu]",
        "[uploaders.gitee]",
        "[uploaders.smms]",
    ] {
        assert!(text.contains(table), "sample missing {table}");
    }
    let mut registry = Registry::new();
    cfg.install_into(&mut registry)
        .expect("sample config should install");
    for id in [
        "github",
        "s3",
        "aliyunoss",
        "qcloudcos",
        "upyun",
        "qiniu",
        "smms",
    ] {
        registry.get(id).unwrap_or_else(|_| panic!("{id}"));
    }
}

#[test]
fn github_init_template_installs_github_only() {
    let text = include_str!("../../../config.github.toml");
    let cfg = AppConfig::from_toml(text).expect("github template");
    assert_eq!(cfg.default_uploader(), Some("github"));
    assert!(cfg.uploaders.contains_key("github"));
    assert!(!cfg.uploaders.contains_key("qiniu"));
    every_assignment_has_a_comment(text);
    let mut registry = Registry::new();
    cfg.install_into(&mut registry)
        .expect("github template should install");
    registry.get("github").expect("github registered");
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
fn install_github_from_toml() {
    let cfg = AppConfig::from_toml(
        r#"
[uploaders.github]
pat = "ghp_test"
username = "alice"
repo = "pics"
branch = "main"
"#,
    )
    .expect("parse");
    let mut registry = Registry::new();
    cfg.install_into(&mut registry).expect("install github");
    registry.get("github").expect("github registered");
}

#[test]
fn github_table_name_is_enough_without_type_field() {
    let cfg = AppConfig::from_toml(
        r#"
default = "github"

[uploaders.github]
pat = "ghp_test"
username = "alice"
repo = "pics"
branch = "master"
"#,
    )
    .expect("parse");
    let mut registry = Registry::new();
    cfg.install_into(&mut registry)
        .expect("install github without type");
    registry.get("github").expect("github");
}

#[test]
fn github_profile_requires_pat() {
    let cfg = AppConfig::from_toml(
        r#"
[uploaders.github]
username = "alice"
repo = "pics"
"#,
    )
    .expect("parse");
    let mut registry = Registry::new();
    let err = cfg.install_into(&mut registry).expect_err("missing field");
    let msg = err.to_string();
    assert!(msg.contains("pat"), "got {msg}");
}

#[test]
fn install_s3_from_sample_like_toml() {
    let cfg = AppConfig::from_toml(
        r#"
[uploaders.s3]
region = "us-west-2"
bucket_name = "my-bucket"
access_key = "your-access-key"
secret_key = "your-secret-key"
endpoint = "https://s3.us-west-2.amazonaws.com"
url_format = "{endpoint}/{bucket}/{path}"
"#,
    )
    .expect("parse");
    let mut registry = Registry::new();
    cfg.install_into(&mut registry).expect("install s3");
    registry.get("s3").expect("s3 registered");
}

#[test]
fn install_aliyunoss_from_toml() {
    let cfg = AppConfig::from_toml(
        r#"
[uploaders.aliyunoss]
endpoint = "https://oss-cn-shanghai.aliyuncs.com"
access_key_id = "your-access-key-id"
access_key_secret = "your-access-key-secret"
bucket_name = "your-bucket-name"
host = "https://cdn.example.com"
"#,
    )
    .expect("parse");
    let mut registry = Registry::new();
    cfg.install_into(&mut registry).expect("install oss");
    registry.get("aliyunoss").expect("aliyunoss registered");
}

#[test]
fn install_qcloudcos_from_toml() {
    let cfg = AppConfig::from_toml(
        r#"
[uploaders.qcloudcos]
host = "xxx.cos.ap-chengdu.myqcloud.com"
secret_id = "sid"
secret_key = "skey"
"#,
    )
    .expect("parse");
    let mut registry = Registry::new();
    cfg.install_into(&mut registry).expect("install cos");
    registry.get("qcloudcos").expect("qcloudcos registered");
}

#[test]
fn install_upyun_from_toml() {
    let cfg = AppConfig::from_toml(
        r#"
[uploaders.upyun]
host = "cdn.example.com"
bucket_name = "my-bucket"
user_name = "operator"
pass_word = "secret"
"#,
    )
    .expect("parse");
    let mut registry = Registry::new();
    cfg.install_into(&mut registry).expect("install upyun");
    registry.get("upyun").expect("upyun registered");
}

#[test]
fn qiniu_is_not_inferred_as_s3() {
    let cfg = AppConfig::from_toml(
        r#"
[uploaders.cdn]
access_key = "ak"
secret_key = "sk"
bucket = "bucket"
public_base = "https://cdn.example.com/"
"#,
    )
    .expect("parse");
    let mut registry = Registry::new();
    cfg.install_into(&mut registry)
        .expect("qiniu keys without endpoint must stay qiniu");
    registry.get("cdn").expect("registered");
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
