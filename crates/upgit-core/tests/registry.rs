//! Uploaders are looked up by name at runtime. The binary must not `match` on id.

use upgit_core::{Artifact, Locator, ObjectKey, Registry, UploadError, Uploader};

struct Stub;

impl Uploader for Stub {
    fn upload(&self, _artifact: &Artifact, _key: &ObjectKey) -> Result<Locator, UploadError> {
        Ok(Locator::new("https://example.com/x.png"))
    }
}

#[test]
fn get_returns_a_registered_uploader() {
    let mut registry = Registry::new();
    registry.register("stub", Box::new(Stub));
    let uploader = registry.get("stub").expect("registered");
    let artifact = Artifact::from_name_and_size("x.png", 1, Some(1024)).expect("artifact");
    let key = ObjectKey::parse("x.png").expect("key");
    let locator = uploader.upload(&artifact, &key).expect("upload");
    assert_eq!(locator.as_str(), "https://example.com/x.png");
}

#[test]
fn unknown_id_lists_known_uploaders() {
    let mut registry = Registry::new();
    registry.register("qiniu", Box::new(Stub));
    registry.register("smms", Box::new(Stub));
    let err = registry.get("nope").expect_err("unknown");
    let msg = err.to_string();
    assert!(msg.contains("nope") || msg.contains("unknown"), "got {msg}");
    assert!(msg.contains("qiniu"), "got {msg}");
    assert!(msg.contains("smms"), "got {msg}");
}

#[test]
fn empty_registry_does_not_look_like_a_missing_upload_file() {
    let registry = Registry::new();
    let err = registry.get("qiniu").expect_err("unknown");
    let msg = err.to_string();
    assert!(msg.contains("qiniu"), "got {msg}");
    assert!(
        msg.contains("config") || msg.contains("uploaders") || msg.contains("init"),
        "got {msg}"
    );
    assert!(!msg.to_lowercase().contains("no such file"), "got {msg}");
}
