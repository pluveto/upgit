use upgit_core::ObjectKey;
use upgit_uploaders::s3::{S3Config, S3Uploader};

fn uploader(url_format: &str) -> S3Uploader {
    S3Uploader::new(S3Config {
        region: "us-west-2".into(),
        bucket_name: "my-bucket".into(),
        access_key: "AKID".into(),
        secret_key: "SECRET".into(),
        endpoint: "https://s3.us-west-2.amazonaws.com".into(),
        url_format: url_format.into(),
    })
}

#[test]
fn locator_uses_default_url_format() {
    let key = ObjectKey::parse("2022/01/a.png").expect("key");
    assert_eq!(
        uploader("").locator_for(&key).as_str(),
        "https://s3.us-west-2.amazonaws.com/my-bucket/2022/01/a.png"
    );
}

#[test]
fn locator_applies_custom_url_format() {
    let key = ObjectKey::parse("a.png").expect("key");
    let uploader = S3Uploader::new(S3Config {
        region: "us-west-2".into(),
        bucket_name: "my-bucket".into(),
        access_key: "AKID".into(),
        secret_key: "SECRET".into(),
        endpoint: "https://s3.us-west-2.amazonaws.com".into(),
        url_format: "https://cdn.example.com/{path}".into(),
    });
    assert_eq!(
        uploader.locator_for(&key).as_str(),
        "https://cdn.example.com/a.png"
    );
}

#[test]
fn locator_trims_duplicate_slashes() {
    let key = ObjectKey::parse("a.png").expect("key");
    let uploader = S3Uploader::new(S3Config {
        region: "us-west-2".into(),
        bucket_name: "my-bucket".into(),
        access_key: "AKID".into(),
        secret_key: "SECRET".into(),
        endpoint: "https://s3.us-west-2.amazonaws.com/".into(),
        url_format: "{endpoint}/{bucket}/{path}".into(),
    });
    assert_eq!(
        uploader.locator_for(&key).as_str(),
        "https://s3.us-west-2.amazonaws.com/my-bucket/a.png"
    );
}

/// AWS GET Object canonical request from the SigV4 header-auth docs.
/// Signature independently HMAC-SHA256'd (the published Signature=fe5f80f7… in that
/// page does not match their own hashed canonical request).
#[test]
fn sigv4_matches_independent_get_object_vector() {
    let uploader = S3Uploader::new(S3Config {
        region: "us-east-1".into(),
        bucket_name: "examplebucket".into(),
        access_key: "AKIAIOSFODNN7EXAMPLE".into(),
        secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".into(),
        endpoint: "https://examplebucket.s3.amazonaws.com".into(),
        url_format: String::new(),
    });
    let payload_hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let auth = uploader.sign_request(
        "GET",
        "/test.txt",
        &[
            ("host", "examplebucket.s3.amazonaws.com"),
            ("range", "bytes=0-9"),
            ("x-amz-content-sha256", payload_hash),
            ("x-amz-date", "20130524T000000Z"),
        ],
        payload_hash,
        "20130524T000000Z",
    );
    assert!(
        auth.contains("Signature=f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41"),
        "got {auth}"
    );
    assert!(
        auth.contains("Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request"),
        "got {auth}"
    );
    assert!(
        auth.contains("SignedHeaders=host;range;x-amz-content-sha256;x-amz-date"),
        "got {auth}"
    );
}

const S3_NO_SUCH_BUCKET: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Error><Code>NoSuchBucket</Code><Message>The specified bucket does not exist</Message><BucketName>my-bucket</BucketName><RequestId>rid</RequestId></Error>"#;

const S3_BAD_SIG: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Error><Code>SignatureDoesNotMatch</Code><Message>The request signature we calculated does not match the signature you provided.</Message></Error>"#;

#[test]
fn explain_404_does_not_dump_xml() {
    let err = uploader("").explain(404, S3_NO_SUCH_BUCKET);
    let s = err.to_string();
    assert!(s.contains("not found") || s.contains("bucket"), "got {s}");
    assert!(
        s.contains("bucket_name") || s.contains("my-bucket"),
        "got {s}"
    );
    assert!(!s.contains("<?xml"), "dumped XML: {s}");
    assert!(!s.contains("RequestId"), "dumped XML: {s}");
}

#[test]
fn explain_403_signature_points_at_keys() {
    let err = uploader("").explain(403, S3_BAD_SIG);
    let s = err.to_string();
    assert!(s.contains("signature") || s.contains("403"), "got {s}");
    assert!(
        s.contains("access_key") || s.contains("secret_key"),
        "got {s}"
    );
    assert!(!s.contains("<?xml"), "dumped XML: {s}");
}

#[test]
fn explain_401_points_at_credentials() {
    let err = uploader("").explain(401, "");
    let s = err.to_string();
    assert!(s.contains("401"), "got {s}");
    assert!(s.contains("access_key"), "got {s}");
}
