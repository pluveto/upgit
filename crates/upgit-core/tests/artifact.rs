//! Artifact is a value: name + size. Zero-byte and over-limit files are not artifacts.

use upgit_core::Artifact;

#[test]
fn rejects_zero_size() {
    let err =
        Artifact::from_name_and_size("empty.png", 0, Some(5 * 1024 * 1024)).expect_err("zero size");
    let msg = err.to_string();
    assert!(msg.contains("zero") || msg.contains("size"), "got {msg}");
}

#[test]
fn rejects_over_size_limit() {
    let err = Artifact::from_name_and_size("big.png", 6, Some(5)).expect_err("over limit");
    let msg = err.to_string();
    assert!(
        msg.contains("limit") || msg.contains("size") || msg.contains("larger"),
        "got {msg}"
    );
}

#[test]
fn unlimited_when_limit_is_none() {
    let a = Artifact::from_name_and_size("big.png", 10_000_000, None).expect("ok");
    assert_eq!(a.file_name(), "big.png");
    assert_eq!(a.stem(), "big");
    assert_eq!(a.ext(), ".png");
    assert_eq!(a.size(), 10_000_000);
}
