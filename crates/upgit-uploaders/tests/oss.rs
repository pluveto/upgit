use upgit_core::ObjectKey;
use upgit_uploaders::oss::{OssConfig, OssUploader};

fn uploader() -> OssUploader {
    OssUploader::new(OssConfig {
        endpoint: "https://oss-cn-shanghai.aliyuncs.com".into(),
        access_key_id: "ak".into(),
        access_key_secret: "sk".into(),
        bucket_name: "my-bucket".into(),
        host: "https://cdn.example.com".into(),
    })
}

#[test]
fn locator_is_host_joined_with_key() {
    let key = ObjectKey::parse("2022/01/a.png").expect("key");
    assert_eq!(
        uploader().locator_for(&key).as_str(),
        "https://cdn.example.com/2022/01/a.png"
    );
}

#[test]
fn locator_does_not_double_slash_when_host_has_trailing_slash() {
    let uploader = OssUploader::new(OssConfig {
        endpoint: "https://oss-cn-shanghai.aliyuncs.com".into(),
        access_key_id: "ak".into(),
        access_key_secret: "sk".into(),
        bucket_name: "my-bucket".into(),
        host: "https://cdn.example.com/".into(),
    });
    let key = ObjectKey::parse("a.png").expect("key");
    assert_eq!(
        uploader.locator_for(&key).as_str(),
        "https://cdn.example.com/a.png"
    );
}

#[test]
fn oss_authorization_matches_independent_hmac_sha1_vector() {
    let key = ObjectKey::parse("2022/01/a.png").expect("key");
    let auth = uploader().authorization_for("image/png", "Mon, 31 Jan 2022 12:00:00 GMT", &key);
    assert_eq!(auth, "OSS ak:0VtS1SWowIIIIYhCa17MSgSADMU=");
}

#[test]
fn explain_404_does_not_dump_xml() {
    let body = r#"<?xml version="1.0"?><Error><Code>NoSuchBucket</Code><Message>The specified bucket does not exist.</Message><RequestId>rid</RequestId></Error>"#;
    let err = uploader().explain(404, body);
    let s = err.to_string();
    assert!(s.contains("not found") || s.contains("bucket"), "got {s}");
    assert!(!s.contains("<?xml"), "dumped XML: {s}");
    assert!(!s.contains("RequestId"), "dumped XML: {s}");
}

#[test]
fn explain_403_signature_points_at_keys() {
    let body = r#"<Error><Code>SignatureDoesNotMatch</Code><Message>The request signature we calculated does not match.</Message></Error>"#;
    let err = uploader().explain(403, body);
    let s = err.to_string();
    assert!(s.contains("signature") || s.contains("403"), "got {s}");
    assert!(
        s.contains("access_key_id") || s.contains("access_key_secret"),
        "got {s}"
    );
    assert!(
        !s.contains("<?xml") && !s.contains("<Error>"),
        "dumped XML: {s}"
    );
}
