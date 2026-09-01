use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, UNIX_EPOCH};

use upgit_core::{Artifact, KeyPolicy, KeyPolicyError};

fn at_2022_01_31_noon() -> std::time::SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_643_630_400)
}

fn png(name: &str) -> Artifact {
    Artifact::from_name_and_size(name, 1024, Some(5 * 1024 * 1024)).expect("valid artifact")
}

struct TempFile {
    artifact: Artifact,
    dir: std::path::PathBuf,
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn artifact_with_bytes(name: &str, bytes: &[u8]) -> TempFile {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "upgit-naming-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(name);
    std::fs::write(&path, bytes).expect("write");
    let artifact = Artifact::from_path(&path, None).expect("artifact");
    TempFile { artifact, dir }
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
fn content_hash_differs_for_same_stem_different_bytes() {
    let a = artifact_with_bytes("logo.png", b"alpha");
    let b = artifact_with_bytes("logo.png", b"bravo");
    let policy = KeyPolicy::template("{content_hash}{ext}");
    let ka = policy
        .apply(&a.artifact, at_2022_01_31_noon())
        .expect("key a");
    let kb = policy
        .apply(&b.artifact, at_2022_01_31_noon())
        .expect("key b");
    assert_ne!(ka.as_str(), kb.as_str());
}

#[test]
fn fname_hash_is_name_only() {
    let a = artifact_with_bytes("logo.png", b"alpha");
    let b = artifact_with_bytes("logo.png", b"bravo");
    let policy = KeyPolicy::template("{fname_hash}{ext}");
    let ka = policy
        .apply(&a.artifact, at_2022_01_31_noon())
        .expect("key a");
    let kb = policy
        .apply(&b.artifact, at_2022_01_31_noon())
        .expect("key b");
    assert_eq!(ka.as_str(), kb.as_str());
}

#[test]
fn content_hash_errors_when_artifact_has_no_bytes() {
    let err = KeyPolicy::template("{content_hash}{ext}")
        .apply(&png("logo.png"), at_2022_01_31_noon())
        .expect_err("missing content");
    assert_eq!(err, KeyPolicyError::MissingContent);
}

#[test]
fn content_hash_is_md5_hex_of_file_bytes() {
    // md5("hello") = 5d41402abc4b2a76b9719d911017c592
    let file = artifact_with_bytes("logo.png", b"hello");
    let key = KeyPolicy::template("{content_hash4}/{content_hash8}/{contenthash}{ext}")
        .apply(&file.artifact, at_2022_01_31_noon())
        .expect("key");
    assert_eq!(
        key.as_str(),
        "5d41/5d41402a/5d41402abc4b2a76b9719d911017c592.png"
    );
}
