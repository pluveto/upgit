//! Qiniu upload tokens are minted from access/secret key + bucket + deadline.
//! The caller never supplies a pre-minted token (Kay: the Qiniu object hides signing).

use std::time::{Duration, UNIX_EPOCH};

use upgit_core::ObjectKey;
use upgit_uploaders::qiniu::{mint_upload_token, QiniuConfig, QiniuUploader};

const DEADLINE: u64 = 1_643_630_400;

/// Independent Python HMAC-SHA1 + urlsafe-b64 vector (not copied from the impl).
const EXPECTED_TOKEN: &str = "test_ak:92T_VYZYdbzmcAItdA_Xlgh8MVc=:eyJzY29wZSI6InRlc3QtYnVja2V0IiwiZGVhZGxpbmUiOjE2NDM2MzA0MDB9";

#[test]
fn mint_token_matches_independent_vector() {
    let token = mint_upload_token(
        "test_ak",
        "test_sk",
        "test-bucket",
        UNIX_EPOCH + Duration::from_secs(DEADLINE),
    );
    assert_eq!(token, EXPECTED_TOKEN);
}

#[test]
fn different_secret_changes_the_signature() {
    let a = mint_upload_token(
        "test_ak",
        "test_sk",
        "test-bucket",
        UNIX_EPOCH + Duration::from_secs(DEADLINE),
    );
    let b = mint_upload_token(
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
