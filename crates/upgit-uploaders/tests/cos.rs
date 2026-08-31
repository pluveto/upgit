use upgit_core::ObjectKey;
use upgit_uploaders::cos::{CosConfig, CosUploader};

fn uploader() -> CosUploader {
    CosUploader::new(CosConfig {
        host: "example-1250000000.cos.ap-guangzhou.myqcloud.com".into(),
        secret_id: "AKIDtest".into(),
        secret_key: "sktest".into(),
    })
}

#[test]
fn locator_is_https_host_and_key() {
    let key = ObjectKey::parse("2022/01/a.png").expect("key");
    assert_eq!(
        uploader().locator_for(&key).as_str(),
        "https://example-1250000000.cos.ap-guangzhou.myqcloud.com/2022/01/a.png"
    );
}

#[test]
fn authorization_matches_frozen_put_vector() {
    let auth = uploader().authorization_for(
        "PUT",
        "/2022/01/a.png",
        &[
            ("Host", "example-1250000000.cos.ap-guangzhou.myqcloud.com"),
            ("Content-MD5", "XUFAKrxLKna5cZ2REBfFkg=="),
            ("Content-Type", "image/png"),
        ],
        1_643_630_400,
        1_643_634_000,
    );
    assert_eq!(
        auth,
        "q-sign-algorithm=sha1&q-ak=AKIDtest&q-sign-time=1643630400;1643634000&q-key-time=1643630400;1643634000&q-header-list=content-md5;content-type;host&q-url-param-list=&q-signature=525b96b2e2c2a64cab1f3421d324794efb90079e"
    );
}

#[test]
fn explain_403_signature_does_not_dump_xml() {
    let body = r#"<?xml version="1.0"?><Error><Code>SignatureDoesNotMatch</Code><Message>The request signature we calculated does not match.</Message><RequestId>rid</RequestId></Error>"#;
    let err = uploader().explain(403, body);
    let s = err.to_string();
    assert!(s.contains("signature") || s.contains("403"), "got {s}");
    assert!(
        s.contains("secret_id") || s.contains("secret_key"),
        "got {s}"
    );
    assert!(!s.contains("<?xml"), "dumped XML: {s}");
    assert!(!s.contains("RequestId"), "dumped XML: {s}");
}

#[test]
fn explain_404_mentions_host() {
    let err = uploader().explain(404, "");
    let s = err.to_string();
    assert!(s.contains("404"), "got {s}");
    assert!(s.contains("host") || s.contains("not found"), "got {s}");
}
