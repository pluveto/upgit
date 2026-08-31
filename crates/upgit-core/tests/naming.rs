//! KeyPolicy turns an Artifact's name + a clock into an ObjectKey.
//! Rename does not happen inside an Uploader (Evans: naming is its own policy;
//! Ousterhout: one place to change the template).

use std::time::{Duration, UNIX_EPOCH};

use upgit_core::{Artifact, KeyPolicy};

fn at_2022_01_31_noon() -> std::time::SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_643_630_400)
}

fn png(name: &str) -> Artifact {
    Artifact::from_name_and_size(name, 1024, Some(5 * 1024 * 1024)).expect("valid artifact")
}

#[test]
fn template_builds_remote_object_key() {
    let policy = KeyPolicy::template("{year}/{month}/{stem}_{unix}{ext}");
    let key = policy
        .apply(&png("logo.png"), at_2022_01_31_noon())
        .expect("key");
    assert_eq!(key.as_str(), "2022/01/logo_1643630400.png");
}

#[test]
fn keep_original_name_under_target_dir() {
    let policy = KeyPolicy::keep_original_in("my_images/demo");
    let key = policy
        .apply(&png("logo.png"), at_2022_01_31_noon())
        .expect("key");
    assert_eq!(key.as_str(), "my_images/demo/logo.png");
}

#[test]
fn target_dir_leading_slash_is_stripped() {
    let policy = KeyPolicy::keep_original_in("/my_images/demo/");
    let key = policy
        .apply(&png("logo.png"), at_2022_01_31_noon())
        .expect("key");
    assert_eq!(key.as_str(), "my_images/demo/logo.png");
}

#[test]
fn hmac_placeholder_uses_sha256_of_interpolated_format() {
    // Independent vector: HMAC-SHA256("2022_01_31_1643630400.png", key) hex[:31]
    let policy = KeyPolicy::template("{year}/{month}/upgit_{hmac}{ext}").with_hmac(
        "74d11935-b2ad-5a3f-8184-5ecdf4f4906b",
        "{year}_{month}_{day}_{unix}{ext}",
        Some(31),
    );
    let key = policy
        .apply(&png("logo.png"), at_2022_01_31_noon())
        .expect("key");
    assert_eq!(
        key.as_str(),
        "2022/01/upgit_26f8a9ff5ef845c3a60a24de37634eb.png"
    );
}

#[test]
fn empty_template_is_rejected() {
    let err = KeyPolicy::template("   ")
        .apply(&png("logo.png"), at_2022_01_31_noon())
        .expect_err("empty template");
    let msg = err.to_string();
    assert!(
        msg.contains("template") || msg.contains("empty"),
        "error should mention the empty template, got {msg}"
    );
}
