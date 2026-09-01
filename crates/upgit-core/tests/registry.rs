use upgit_core::{Artifact, Locator, ObjectKey, Registry, UploadError, Uploader};

struct Stub;

impl Uploader for Stub {
    fn upload(&self, _artifact: &Artifact, _key: &ObjectKey) -> Result<Locator, UploadError> {
        Ok(Locator::new("https://example.com/x.png"))
    }
}

#[test]
fn unknown_id_lists_known_uploaders() {
    let mut registry = Registry::new();
    registry.register("qiniu", Box::new(Stub));
    registry.register("smms", Box::new(Stub));
    let err = registry.get("nope").expect_err("unknown");
    let msg = err.to_string();
    assert_eq!(msg, "unknown uploader `nope` (configured: qiniu, smms)");
}
