//! Structural check of the Chinese development handbook.

const DEV: &str = include_str!("../../../docs/DEVELOPMENT.zh-CN.md");

#[test]
fn handbook_covers_environment_three_giants_and_release() {
    for needle in [
        "三巨头",
        "Eric Evans",
        "John Ousterhout",
        "Alan Kay",
        "rust-toolchain.toml",
        "install-git-hooks.sh",
        "cargo-release",
        "cargo release release",
        "RUSTFLAGS",
        "main",
        "v0.2-main",
    ] {
        assert!(
            DEV.contains(needle),
            "docs/DEVELOPMENT.zh-CN.md must contain {needle:?}"
        );
    }
}

#[test]
fn handbook_does_not_mention_go() {
    let lower = DEV.to_ascii_lowercase();
    assert!(
        !lower.contains("golang"),
        "handbook must not mention golang"
    );
    assert!(
        !DEV.contains("Go 语言") && !DEV.contains("Go语言"),
        "handbook must not mention Go"
    );
}
