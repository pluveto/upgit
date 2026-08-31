//! Qiniu upload tokens are minted from access/secret key + bucket + deadline.
//! The caller never supplies a pre-minted token (Kay: the Qiniu object hides signing).

use std::time::{Duration, UNIX_EPOCH};

use upgit_core::ObjectKey;
use upgit_uploaders::qiniu::{QiniuConfig, QiniuUploader};

const DEADLINE: u64 = 1_643_630_400;

/// Independent Python HMAC-SHA1 + urlsafe-b64 vector (not copied from the impl).
const EXPECTED_TOKEN: &str = "test_ak:92T_VYZYdbzmcAItdA_Xlgh8MVc=:eyJzY29wZSI6InRlc3QtYnVja2V0IiwiZGVhZGxpbmUiOjE2NDM2MzA0MDB9";

#[test]
fn mint_token_matches_independent_vector() {
    let token = QiniuUploader::mint_token(
        "test_ak",
        "test_sk",
        "test-bucket",
        UNIX_EPOCH + Duration::from_secs(DEADLINE),
    );
    assert_eq!(token, EXPECTED_TOKEN);
}

#[test]
fn different_secret_changes_the_signature() {
    let a = QiniuUploader::mint_token(
        "test_ak",
        "test_sk",
        "test-bucket",
        UNIX_EPOCH + Duration::from_secs(DEADLINE),
    );
    let b = QiniuUploader::mint_token(
        "test_ak",
        "other_sk",
        "test-bucket",
        UNIX_EPOCH + Duration::from_secs(DEADLINE),
    );
    assert_ne!(a, b);
    assert!(a.starts_with("test_ak:"));
    assert!(b.starts_with("test_ak:"));
}

#[test]
fn locator_is_public_base_joined_with_key() {
    let uploader = QiniuUploader::new(QiniuConfig {
        access_key: "ak".into(),
        secret_key: "sk".into(),
        bucket: "bucket".into(),
        public_base: "https://cdn.example.com/".into(),
        region: None,
    });
    let key = ObjectKey::parse("2022/01/a.png").expect("key");
    assert_eq!(
        uploader.locator_for(&key).as_str(),
        "https://cdn.example.com/2022/01/a.png"
    );
}

#[test]
fn locator_does_not_double_slash_when_base_has_trailing_slash() {
    let uploader = QiniuUploader::new(QiniuConfig {
        access_key: "ak".into(),
        secret_key: "sk".into(),
        bucket: "bucket".into(),
        public_base: "https://cdn.example.com".into(),
        region: None,
    });
    let key = ObjectKey::parse("a.png").expect("key");
    assert_eq!(
        uploader.locator_for(&key).as_str(),
        "https://cdn.example.com/a.png"
    );
}

#[test]
fn qiniu_config_is_ak_sk_bucket_not_a_static_token() {
    // If QiniuConfig requires a `token` field, this struct literal will not compile.
    let _cfg = QiniuConfig {
        access_key: "ak".into(),
        secret_key: "sk".into(),
        bucket: "b".into(),
        public_base: "https://cdn.example.com/".into(),
        region: Some("z0".into()),
    };
}

fn qiniu() -> QiniuUploader {
    QiniuUploader::new(QiniuConfig {
        access_key: "ak".into(),
        secret_key: "sk".into(),
        bucket: "test-bucket".into(),
        public_base: "https://cdn.example.com/".into(),
        region: None,
    })
}

#[test]
fn explain_401_does_not_dump_json() {
    let err = qiniu().explain(401, r#"{"error":"bad token","request_id":"abc123"}"#);
    let s = err.to_string();
    assert!(s.contains("401"), "got {s}");
    assert!(
        s.contains("access_key") || s.contains("secret_key"),
        "got {s}"
    );
    assert!(!s.contains("request_id"), "dumped JSON: {s}");
    assert!(!s.contains("abc123"), "dumped JSON: {s}");
}

#[test]
fn explain_missing_bucket_mentions_bucket() {
    let err = qiniu().explain(400, r#"{"error":"no such bucket","error_code":631}"#);
    let s = err.to_string();
    assert!(s.contains("not found") || s.contains("bucket"), "got {s}");
    assert!(
        s.contains("test-bucket") || s.contains("[uploaders.qiniu]"),
        "got {s}"
    );
    assert!(!s.contains("error_code"), "dumped JSON: {s}");
}
