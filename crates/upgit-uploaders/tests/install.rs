use upgit_core::Registry;
use upgit_uploaders::{AppConfig, HostCatalog, RecipeCatalog};

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
    assert!(
        !text.contains("blob/next/"),
        "sample must not pin blob/next links"
    );
    assert!(
        !text.to_ascii_lowercase().contains("0.2 parity"),
        "sample must not mention 0.2 parity"
    );
    assert!(
        !text.to_ascii_lowercase().contains("jsonc"),
        "sample must not mention JSONC"
    );
    assert!(
        !text.to_ascii_lowercase().contains("first-class"),
        "sample must not mention first-class"
    );
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
    assert!(cfg.uploaders.contains_key("github"));
    assert!(cfg.uploaders.contains_key("smms"));
}

#[test]
fn github_init_template_is_github_only() {
    let text = include_str!("../../../config.github.toml");
    let cfg = AppConfig::from_toml(text).expect("github template");
    assert_eq!(cfg.default_uploader(), Some("github"));
    assert!(cfg.uploaders.contains_key("github"));
    assert!(!cfg.uploaders.contains_key("qiniu"));
    every_assignment_has_a_comment(text);
    assert!(
        !text.contains("ghp_"),
        "packed pat must not look like a real PAT"
    );
    assert!(text.contains("PASTE_YOUR_TOKEN"), "got {text}");
    assert!(!text.contains("blob/next/"), "must not pin blob/next links");
    let mut registry = Registry::new();
    let err = cfg
        .install_into(&mut registry)
        .expect_err("placeholder pat must not install");
    let msg = err.to_string();
    assert!(msg.contains("pat"), "got {msg}");
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

#[test]
fn empty_smms_table_requires_token() {
    let cfg = AppConfig::from_toml("[uploaders.smms]\n").expect("parse");
    let mut registry = Registry::new();
    let err = cfg.install_into(&mut registry).expect_err("missing token");
    assert_eq!(
        err.to_string(),
        "uploader `smms` is missing required field `token`"
    );
}

#[test]
fn empty_catbox_table_installs_anonymously() {
    let cfg = AppConfig::from_toml("[uploaders.catbox]\n").expect("parse");
    let mut registry = Registry::new();
    cfg.install_into(&mut registry)
        .expect("anonymous catbox must install without userhash");
    registry.get("catbox").expect("catbox registered");
}

#[test]
fn catbox_with_userhash_installs() {
    let cfg = AppConfig::from_toml(
        r#"
[uploaders.catbox]
userhash = "abc123"
"#,
    )
    .expect("parse");
    let mut registry = Registry::new();
    cfg.install_into(&mut registry)
        .expect("catbox with userhash");
    registry.get("catbox").expect("catbox registered");
}

#[test]
fn github_placeholder_pat_is_missing_field() {
    let cfg = AppConfig::from_toml(
        r#"
[uploaders.github]
pat = "ghp_..."
username = "alice"
repo = "pics"
"#,
    )
    .expect("parse");
    let mut registry = Registry::new();
    let err = cfg
        .install_into(&mut registry)
        .expect_err("placeholder pat");
    let msg = err.to_string();
    assert!(msg.contains("pat"), "got {msg}");
    assert!(msg.contains("missing required field"), "got {msg}");
}

#[test]
fn hmac_naming_without_hmac_key_errors() {
    let cfg = AppConfig::from_toml(
        r#"
naming = "{year}/{hmac}{ext}"

[uploaders.github]
pat = "real-token"
username = "alice"
repo = "pics"
"#,
    )
    .expect("parse");
    let err = cfg.namer().expect_err("hmac");
    let msg = err.to_string();
    assert!(msg.contains("hmac_key"), "got {msg}");
    assert!(msg.contains("{hmac}"), "got {msg}");
}

#[test]
fn unknown_kind_and_expired_qiniu_copy_has_no_jsonc() {
    let kind_err = AppConfig::from_toml(
        r#"
[uploaders.mystery]
type = "sftp"
host = "example"
"#,
    )
    .expect("parse")
    .install_into(&mut Registry::new())
    .expect_err("kind");
    let kind_msg = kind_err.to_string();
    assert!(kind_msg.contains("sftp"), "got {kind_msg}");
    let kind_lower = kind_msg.to_ascii_lowercase();
    assert!(!kind_lower.contains("jsonc"), "got {kind_msg}");
    assert!(!kind_lower.contains("extensions"), "got {kind_msg}");
    assert!(!kind_lower.contains("first-class"), "got {kind_msg}");
    assert!(!kind_msg.contains("upgit init"), "got {kind_msg}");

    let token_err = AppConfig::from_toml(
        r#"
[uploaders.qiniu]
token = "expired-web-token"
prefix = "https://cdn.example.com/"
"#,
    )
    .expect("parse")
    .install_into(&mut Registry::new())
    .expect_err("token");
    let token_msg = token_err.to_string();
    let token_lower = token_msg.to_ascii_lowercase();
    assert!(!token_lower.contains("jsonc"), "got {token_msg}");
    assert!(!token_lower.contains("extensions"), "got {token_msg}");
    assert!(!token_msg.contains("upgit init"), "got {token_msg}");
}

#[test]
fn host_catalog_contains_github_and_smms() {
    let ids: Vec<_> = HostCatalog::ids().collect();
    assert_eq!(ids[0], "github");
    assert!(ids.contains(&"github"));
    assert!(ids.contains(&"smms"));
    assert!(ids.contains(&"s3"));
    let builtins = &ids[..6];
    assert_eq!(
        builtins,
        ["github", "s3", "aliyunoss", "qcloudcos", "upyun", "qiniu"]
    );
    let recipe_ids: Vec<_> = HostCatalog::ids().skip(6).collect();
    let catalog: Vec<_> = RecipeCatalog::ids().collect();
    assert_eq!(recipe_ids, catalog);
    let s3 = HostCatalog::all()
        .iter()
        .find(|h| h.id == "s3")
        .expect("s3");
    assert!(s3.title.contains("MinIO"), "got {}", s3.title);
    assert!(
        s3.title.contains("R2") || s3.title.contains("Cloudflare"),
        "got {}",
        s3.title
    );
    assert!(s3.title.contains("Ceph"), "got {}", s3.title);
    assert!(s3.title.contains("Flexify.IO"), "got {}", s3.title);
    assert!(
        s3.title.contains("IBM Cloud Object Storage"),
        "got {}",
        s3.title
    );
}

#[test]
fn overlay_from_iter_sets_default_and_nested_pat() {
    let mut cfg = AppConfig::from_toml(
        r#"
[uploaders.github]
pat = "old"
username = "alice"
repo = "pics"
"#,
    )
    .expect("parse");
    cfg.overlay_from_iter([
        ("UPGIT_DEFAULT", "smms"),
        ("UPGIT_NAMING", "{stem}{ext}"),
        ("UPGIT_SIZE_LIMIT", "0"),
        ("UPGIT_HMAC_KEY", "secret"),
        ("UPGIT_UPLOADERS__GITHUB__PAT", "xxx"),
    ]);
    assert_eq!(cfg.default_uploader(), Some("smms"));
    assert_eq!(cfg.naming.as_deref(), Some("{stem}{ext}"));
    assert_eq!(cfg.size_limit, Some(0));
    assert_eq!(cfg.hmac_key.as_deref(), Some("secret"));
    let pat = cfg.uploaders["github"]
        .fields
        .get("pat")
        .and_then(|v| v.as_str());
    assert_eq!(pat, Some("xxx"));
    assert_eq!(
        cfg.uploaders["github"]
            .fields
            .get("username")
            .and_then(|v| v.as_str()),
        Some("alice")
    );
}

#[test]
fn overlay_from_iter_upgit_token_sets_github_pat() {
    let mut cfg = AppConfig::default();
    cfg.overlay_from_iter([("UPGIT_TOKEN", "ghp_from_env")]);
    let github = cfg.uploaders.get("github").expect("github table");
    let pat = github.fields.get("pat").and_then(|v| v.as_str());
    assert_eq!(pat, Some("ghp_from_env"));
}

#[test]
fn sample_zh_has_http_tables_and_is_not_init_output() {
    let text = include_str!("../../../config.sample.zh-CN.toml");
    let cfg = AppConfig::from_toml(text).expect("zh sample");
    assert!(!text.contains("由 `upgit init` 生成"));
    every_assignment_has_a_comment(text);
    for table in [
        "[uploaders.imgtg]",
        "[uploaders.juejin]",
        "[uploaders.moetu]",
        "[uploaders.netease]",
        "[uploaders.sougou]",
        "[uploaders.upload_cc]",
    ] {
        assert!(text.contains(table), "zh sample missing {table}");
    }
    assert_eq!(cfg.default_uploader(), Some("github"));
    assert_eq!(
        cfg.uploaders["github"]
            .fields
            .get("branch")
            .and_then(|v| v.as_str()),
        Some("main")
    );
}
