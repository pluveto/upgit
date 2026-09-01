use upgit_core::Artifact;

#[test]
fn rejects_over_size_limit() {
    let err = Artifact::from_name_and_size("big.png", 6, Some(5)).expect_err("over limit");
    let msg = err.to_string();
    assert!(
        msg.contains("limit") || msg.contains("size") || msg.contains("larger"),
        "got {msg}"
    );
}
