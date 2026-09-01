use upgit_uploaders::s3::{S3Config, S3Uploader};

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
        host: String::new(),
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
