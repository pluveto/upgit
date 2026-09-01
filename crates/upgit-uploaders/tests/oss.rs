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
fn oss_authorization_matches_independent_hmac_sha1_vector() {
    let key = ObjectKey::parse("2022/01/a.png").expect("key");
    let auth = uploader().authorization_for("image/png", "Mon, 31 Jan 2022 12:00:00 GMT", &key);
    assert_eq!(auth, "OSS ak:0VtS1SWowIIIIYhCa17MSgSADMU=");
}
