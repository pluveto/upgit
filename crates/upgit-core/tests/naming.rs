use std::time::{Duration, UNIX_EPOCH};

use upgit_core::{Artifact, KeyPolicy};

fn at_2022_01_31_noon() -> std::time::SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_643_630_400)
}

fn png(name: &str) -> Artifact {
    Artifact::from_name_and_size(name, 1024, Some(5 * 1024 * 1024)).expect("valid artifact")
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
