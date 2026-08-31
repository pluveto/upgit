use upgit_core::ObjectKey;
use upgit_uploaders::upyun::{UpyunConfig, UpyunUploader};

fn uploader() -> UpyunUploader {
    UpyunUploader::new(UpyunConfig {
        host: "cdn.example.com".into(),
        bucket_name: "mybucket".into(),
        user_name: "operator".into(),
        pass_word: "secret".into(),
    })
}

#[test]
fn locator_is_https_host_and_key() {
    let key = ObjectKey::parse("2022/01/a.png").expect("key");
    assert_eq!(
        uploader().locator_for(&key).as_str(),
        "https://cdn.example.com/2022/01/a.png"
    );
}

#[test]
fn authorization_matches_md5_sign_vector() {
    let auth = uploader().authorization_for(
        "PUT",
        "/mybucket/2022/01/a.png",
        "Mon, 31 Jan 2022 12:00:00 GMT",
        5,
    );
    assert_eq!(auth, "UpYun operator:27fd70f6f2779937aa9cf204048bc610");
}
