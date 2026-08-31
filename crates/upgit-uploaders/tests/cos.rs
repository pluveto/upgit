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
